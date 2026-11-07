//! Safe session ownership for the Wii OS Dolphin C ABI.
//!
//! Dolphin remains the native C++ emulator. This crate owns the dedicated host
//! thread, copies all launch strings into that thread, and exposes only a
//! process-sized window handle and lifecycle operations to Rust.

#![cfg_attr(
    not(feature = "linked-dolphin"),
    allow(dead_code, unused_imports, unused_variables)
)]

use std::{
    ffi::CString,
    fmt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
};

#[cfg(feature = "linked-dolphin")]
#[link(name = "wii_dolphin_ffi")]
unsafe extern "C" {
    fn wii_dolphin_ffi_abi_version() -> u32;
    fn wii_dolphin_ffi_run(
        content_path: *const std::ffi::c_char,
        user_dir: *const std::ffi::c_char,
        title_id: u64,
        parent_window: usize,
        video_backend: *const std::ffi::c_char,
    ) -> i32;
    fn wii_dolphin_ffi_request_stop() -> i32;
    fn wii_dolphin_ffi_render_window() -> usize;
    fn wii_dolphin_ffi_netplay_run(
        user_dir: *const std::ffi::c_char,
        parent_window: usize,
        video_backend: *const std::ffi::c_char,
    ) -> i32;
    fn wii_dolphin_ffi_netplay_host(
        game_path: *const std::ffi::c_char,
        port: u16,
        nickname: *const std::ffi::c_char,
    ) -> i32;
    fn wii_dolphin_ffi_netplay_join(
        game_path: *const std::ffi::c_char,
        address: *const std::ffi::c_char,
        port: u16,
        nickname: *const std::ffi::c_char,
    ) -> i32;
    fn wii_dolphin_ffi_netplay_start_game() -> i32;
    fn wii_dolphin_ffi_netplay_status() -> i32;
    fn wii_dolphin_ffi_netplay_poll_event(event: *mut FfiNetPlayEvent) -> i32;
    fn wii_dolphin_ffi_netplay_stop() -> i32;
}

// NetPlay is additive; keep ABI v1 so existing solo-run consumers continue to load.
const ABI_VERSION: u32 = 1;
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub content_path: Option<String>,
    pub user_dir: String,
    pub title_id: Option<u64>,
    pub parent_window: usize,
    pub video_backend: Option<String>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FfiNetPlayEvent {
    event_type: u32,
    code: i32,
    message: [std::ffi::c_char; 256],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum NetPlayStatus {
    Idle = 0,
    Starting = 1,
    Lobby = 2,
    Running = 3,
    Failed = 4,
    Stopping = 5,
    Stopped = 6,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetPlayEvent {
    pub event_type: u32,
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct NetPlayLaunchOptions {
    pub user_dir: String,
    pub parent_window: usize,
    pub video_backend: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DolphinError {
    AlreadyRunning,
    MissingLaunchTarget,
    InvalidPath(String),
    UnsupportedPlatform,
    AbiMismatch(u32),
    BootFailed,
    NetPlayNotReady,
    NativeFailure(i32),
    WorkerPanicked,
}

impl fmt::Display for DolphinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => f.write_str("une session Dolphin est déjà active"),
            Self::MissingLaunchTarget => f.write_str("un chemin de jeu ou un title ID NAND est requis"),
            Self::InvalidPath(path) => write!(f, "chemin non UTF-8 ou contenant NUL: {path}"),
            Self::UnsupportedPlatform => f.write_str("le Dolphin FFI intégré est disponible uniquement sous Windows"),
            Self::AbiMismatch(version) => write!(f, "version ABI Dolphin incompatible: {version}"),
            Self::BootFailed => f.write_str("Dolphin n’a pas pu démarrer le titre"),
            Self::NetPlayNotReady => f.write_str("la session NetPlay n’est pas prête"),
            Self::NativeFailure(code) => write!(f, "erreur du pont Dolphin: {code}"),
            Self::WorkerPanicked => f.write_str("le thread hôte Dolphin a paniqué"),
        }
    }
}

impl std::error::Error for DolphinError {}

pub struct DolphinSession {
    worker: Option<JoinHandle<Result<(), DolphinError>>>,
    completed: bool,
}

impl DolphinSession {
    pub fn launch(options: LaunchOptions) -> Result<Self, DolphinError> {
        if options.content_path.is_none() && options.title_id.is_none() {
            return Err(DolphinError::MissingLaunchTarget);
        }
        if options.content_path.is_some() && options.title_id.is_some() {
            return Err(DolphinError::MissingLaunchTarget);
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        {
            let _ = options;
            return Err(DolphinError::UnsupportedPlatform);
        }
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let path = options
                .content_path
                .as_deref()
                .map(|value| c_string(value, value))
                .transpose()?;
            let user = c_string(&options.user_dir, &options.user_dir)?;
            let backend = options
                .video_backend
                .as_deref()
                .map(|value| c_string(value, value))
                .transpose()?;
            let title_id = options.title_id.unwrap_or(0);
            let parent_window = options.parent_window;

            if SESSION_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(DolphinError::AlreadyRunning);
            }

            let worker = thread::Builder::new()
                .name("wii-os-dolphin-host".to_owned())
                .spawn(move || {
                    let _active_guard = ActiveSessionGuard;
                    // CStrings are moved into this closure; their pointers remain valid for
                    // the entire synchronous native run call.
                    let version = unsafe { wii_dolphin_ffi_abi_version() };
                    if version != ABI_VERSION {
                        return Err(DolphinError::AbiMismatch(version));
                    }
                    let status = unsafe {
                        wii_dolphin_ffi_run(
                            path.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
                            user.as_ptr(),
                            title_id,
                            parent_window,
                            backend.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
                        )
                    };
                    match status {
                        0 => Ok(()),
                        1 => Err(DolphinError::BootFailed),
                        code => Err(DolphinError::NativeFailure(code)),
                    }
                });
            let worker = match worker {
                Ok(worker) => worker,
                Err(_) => {
                    SESSION_ACTIVE.store(false, Ordering::Release);
                    return Err(DolphinError::WorkerPanicked);
                }
            };

            Ok(Self {
                worker: Some(worker),
                completed: false,
            })
        }
    }

    pub fn render_window(&self) -> Option<usize> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let window = unsafe { wii_dolphin_ffi_render_window() };
            return (window != 0).then_some(window);
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        None
    }

    pub fn try_wait(&mut self) -> Option<Result<(), DolphinError>> {
        if self.completed {
            return None;
        }
        let worker = self.worker.as_ref()?;
        if !worker.is_finished() {
            return None;
        }
        let worker = self.worker.take().expect("worker checked above");
        self.completed = true;
        Some(worker.join().unwrap_or(Err(DolphinError::WorkerPanicked)))
    }

    pub fn request_stop(&self) -> Result<(), DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            match unsafe { wii_dolphin_ffi_request_stop() } {
                0 => Ok(()),
                code => Err(DolphinError::NativeFailure(code)),
            }
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }
}

impl Drop for DolphinSession {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.request_stop();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

/// One native Dolphin NetPlay host/client session. NetPlay uses Dolphin's own
/// ENet protocol and runs alongside the existing single-game session API.
pub struct DolphinNetPlaySession {
    worker: Option<JoinHandle<Result<(), DolphinError>>>,
    completed: bool,
}

impl DolphinNetPlaySession {
    pub fn launch(options: NetPlayLaunchOptions) -> Result<Self, DolphinError> {
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        {
            let _ = options;
            return Err(DolphinError::UnsupportedPlatform);
        }
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let user = c_string(&options.user_dir, &options.user_dir)?;
            let backend = options
                .video_backend
                .as_deref()
                .map(|value| c_string(value, value))
                .transpose()?;
            if SESSION_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(DolphinError::AlreadyRunning);
            }
            let worker = thread::Builder::new()
                .name("wii-os-dolphin-netplay".to_owned())
                .spawn(move || {
                    let _active_guard = ActiveSessionGuard;
                    let version = unsafe { wii_dolphin_ffi_abi_version() };
                    if version != ABI_VERSION {
                        return Err(DolphinError::AbiMismatch(version));
                    }
                    let status = unsafe {
                        wii_dolphin_ffi_netplay_run(
                            user.as_ptr(),
                            options.parent_window,
                            backend.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
                        )
                    };
                    if status == 0 {
                        Ok(())
                    } else {
                        Err(DolphinError::NativeFailure(status))
                    }
                });
            let worker = match worker {
                Ok(worker) => worker,
                Err(_) => {
                    SESSION_ACTIVE.store(false, Ordering::Release);
                    return Err(DolphinError::WorkerPanicked);
                }
            };
            let session = Self { worker: Some(worker), completed: false };
            // The FFI publishes the adapter after platform and controller startup.
            // Wait briefly so callers can issue their first host/join command.
            for _ in 0..600 {
                if session.status() != NetPlayStatus::Idle {
                    return Ok(session);
                }
                if session.worker.as_ref().is_some_and(JoinHandle::is_finished) {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(50));
            }
            drop(session);
            Err(DolphinError::NetPlayNotReady)
        }
    }

    pub fn host(&self, game_path: &str, port: u16, nickname: &str) -> Result<(), DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let game = c_string(game_path, game_path)?;
            let nickname = c_string(nickname, nickname)?;
            native_result(unsafe {
                wii_dolphin_ffi_netplay_host(game.as_ptr(), port, nickname.as_ptr())
            })
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }

    pub fn join(
        &self,
        game_path: &str,
        address: &str,
        port: u16,
        nickname: &str,
    ) -> Result<(), DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let game = c_string(game_path, game_path)?;
            let address = c_string(address, address)?;
            let nickname = c_string(nickname, nickname)?;
            native_result(unsafe {
                wii_dolphin_ffi_netplay_join(
                    game.as_ptr(), address.as_ptr(), port, nickname.as_ptr(),
                )
            })
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }

    pub fn start_game(&self) -> Result<(), DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            native_result(unsafe { wii_dolphin_ffi_netplay_start_game() })
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }

    pub fn status(&self) -> NetPlayStatus {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            match unsafe { wii_dolphin_ffi_netplay_status() } {
                1 => NetPlayStatus::Starting,
                2 => NetPlayStatus::Lobby,
                3 => NetPlayStatus::Running,
                4 => NetPlayStatus::Failed,
                5 => NetPlayStatus::Stopping,
                6 => NetPlayStatus::Stopped,
                _ => NetPlayStatus::Idle,
            }
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        NetPlayStatus::Idle
    }

    pub fn poll_event(&self) -> Result<Option<NetPlayEvent>, DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            let mut event = FfiNetPlayEvent { event_type: 0, code: 0, message: [0; 256] };
            match unsafe { wii_dolphin_ffi_netplay_poll_event(&mut event) } {
                0 => Ok(None),
                1 => {
                    let bytes: Vec<u8> = event.message.iter().take_while(|&&b| b != 0)
                        .map(|&b| b as u8).collect();
                    Ok(Some(NetPlayEvent {
                        event_type: event.event_type,
                        code: event.code,
                        message: String::from_utf8_lossy(&bytes).into_owned(),
                    }))
                }
                code => Err(DolphinError::NativeFailure(code)),
            }
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }

    pub fn request_stop(&self) -> Result<(), DolphinError> {
        #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
        {
            native_result(unsafe { wii_dolphin_ffi_netplay_stop() })
        }
        #[cfg(not(all(target_os = "windows", feature = "linked-dolphin")))]
        Err(DolphinError::UnsupportedPlatform)
    }

    pub fn try_wait(&mut self) -> Option<Result<(), DolphinError>> {
        if self.completed || !self.worker.as_ref()?.is_finished() {
            return None;
        }
        self.completed = true;
        Some(self.worker.take()?.join().unwrap_or(Err(DolphinError::WorkerPanicked)))
    }
}

impl Drop for DolphinNetPlaySession {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            #[cfg(all(target_os = "windows", feature = "linked-dolphin"))]
            unsafe {
                let _ = wii_dolphin_ffi_request_stop();
            }
            let _ = worker.join();
        }
    }
}

fn native_result(code: i32) -> Result<(), DolphinError> {
    if code == 0 { Ok(()) } else { Err(DolphinError::NativeFailure(code)) }
}

struct ActiveSessionGuard;

impl Drop for ActiveSessionGuard {
    fn drop(&mut self) {
        SESSION_ACTIVE.store(false, Ordering::Release);
    }
}

fn c_string(value: &str, display: &str) -> Result<CString, DolphinError> {
    CString::new(value).map_err(|_| DolphinError::InvalidPath(display.to_owned()))
}

/// Converts a validated filesystem path to the UTF-8 representation required by the ABI.
pub fn path_to_abi(path: &Path) -> Result<String, DolphinError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| DolphinError::InvalidPath(path.display().to_string()))
}
