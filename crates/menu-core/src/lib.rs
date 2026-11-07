//! Small, platform-neutral BIOS and Wii System Menu 4.3 state core shared by native and web hosts.
//! Rendering, storage, audio, NetPlay transport, and emulator boot remain host responsibilities.

pub const GRID_COLUMNS: usize = 4;
pub const GRID_ROWS: usize = 3;
pub const CHANNELS_PER_PAGE: usize = GRID_COLUMNS * GRID_ROWS;
pub const MIN_CHANNEL_PAGES: usize = 4;
pub const MAX_CHANNEL_PAGES: usize = 20;
pub const NAND_TOTAL_BLOCKS: u32 = 2174;
pub const SAFE_MODE_HOLD_THRESHOLD_MS: u32 = 3000;

use serde::{Deserialize, Serialize};

pub mod c_reverse_ipl;
pub use c_reverse_ipl::*;

pub mod chans_vm;
pub use chans_vm::*;

pub mod mii;
pub use mii::*;

pub mod network;
pub use network::*;

pub mod thp;
pub use thp::*;

pub mod bns;
pub use bns::*;

pub mod wad;
pub use wad::*;

/// C ABI action identifiers and opaque menu handle are implemented at the end of this module.

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub title: String,
    /// Host-resolved asset key; never a bundled proprietary title image.
    pub icon: Option<String>,
    #[serde(default)]
    pub content_kind: ContentKind,
    /// User-local path or URI. It must never point to bundled copyrighted content.
    #[serde(default)]
    pub content_path: Option<String>,
    #[serde(default)]
    pub title_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    /// Optional NAND block size for Data Management display.
    #[serde(default)]
    pub blocks: Option<u32>,
    /// Optional publisher or subtitle shown in ChannelTitle banner preview.
    #[serde(default)]
    pub publisher: Option<String>,
    /// Maximum supported players (1..=4).
    #[serde(default)]
    pub players: Option<u8>,
    /// Preferred page index (0-based) when user reorders the channel grid.
    #[serde(default)]
    pub preferred_page: Option<usize>,
    /// Preferred slot index (0..12) when user reorders the channel grid.
    #[serde(default)]
    pub preferred_slot: Option<usize>,
    /// Local files needed in addition to the bootable content (for example an IOS WAD).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_files: Vec<RequiredFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequiredFile {
    pub role: String,
    pub content_path: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentKind {
    #[default]
    Homebrew,
    GameCube,
    WiiDisc,
    WiiWad,
    /// A title already installed in the operator's Dolphin NAND/profile.
    WiiNandTitle,
    WiiU,
    Application,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataManagementTab {
    #[default]
    SaveDataWii,
    SaveDataGameCube,
    ChannelsWii,
    ChannelsSd,
}

/// Complete Wii System Menu 4.3E SYSCONF configuration shared across BIOS UI, server DB, and runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WiiSysconf {
    pub console_nickname: String,
    pub system_version: String,
    pub region: String,
    /// "16:9" or "4:3"
    pub aspect_ratio: String,
    /// "480p", "576i", or "480i"
    pub tv_resolution: String,
    pub eurgb60: bool,
    pub burn_in_reduction: bool,
    pub screen_offset_h: i8,
    pub screen_offset_v: i8,
    /// "surround", "stereo", or "mono"
    pub sound_mode: String,
    pub bgm_enabled: bool,
    /// "top" or "bottom"
    pub sensor_bar_position: String,
    /// 1..=5
    pub sensor_bar_sensitivity: u8,
    pub wiiconnect24: bool,
    pub standby_connection: bool,
    /// "bright", "dim", or "off"
    pub slot_illumination: String,
    /// "fr", "en", "de", "es", "it", "nl"
    pub language: String,
    pub country: String,
    pub parental_controls_enabled: bool,
    /// "d3d12", "vulkan", "opengl", or "webgpu"
    pub video_backend: String,
    /// 16-digit Wii Friend Code formatted as XXXX-XXXX-XXXX-XXXX
    pub friend_code: String,
    pub netplay_default_port: u16,
    pub netplay_pad_buffer: u16,
}

impl Default for WiiSysconf {
    fn default() -> Self {
        Self {
            console_nickname: "Wii OS PAL".to_string(),
            system_version: "4.3E".to_string(),
            region: "EUR".to_string(),
            aspect_ratio: "16:9".to_string(),
            tv_resolution: "480p".to_string(),
            eurgb60: true,
            burn_in_reduction: false,
            screen_offset_h: 0,
            screen_offset_v: 0,
            sound_mode: "stereo".to_string(),
            bgm_enabled: true,
            sensor_bar_position: "bottom".to_string(),
            sensor_bar_sensitivity: 3,
            wiiconnect24: true,
            standby_connection: true,
            slot_illumination: "bright".to_string(),
            language: "fr".to_string(),
            country: "France".to_string(),
            parental_controls_enabled: false,
            video_backend: "d3d12".to_string(),
            friend_code: "4821-9034-7712-6508".to_string(),
            netplay_default_port: 2626,
            netplay_pad_buffer: 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuCatalog {
    pub schema_version: u32,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

impl MenuCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported menu catalog schemaVersion {}",
                self.schema_version
            ));
        }
        if self.channels.len() > CHANNELS_PER_PAGE * MAX_CHANNEL_PAGES {
            return Err("menu catalog exceeds 240 channel entries".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut app_ids = std::collections::HashSet::new();
        for channel in &self.channels {
            if channel.id.trim().is_empty() || channel.id.len() > 128 {
                return Err("each channel id must contain 1-128 characters".into());
            }
            if channel.title.trim().is_empty() || channel.title.len() > 256 {
                return Err(format!(
                    "channel {} must have a 1-256 character title",
                    channel.id
                ));
            }
            if !ids.insert(&channel.id) {
                return Err(format!("duplicate channel id {}", channel.id));
            }
            let mut required_roles = std::collections::HashSet::new();
            for required in &channel.required_files {
                if required.role.trim().is_empty() || required.role.len() > 64 {
                    return Err(format!(
                        "channel {} has a required file with an invalid role",
                        channel.id
                    ));
                }
                if !required_roles.insert(&required.role) {
                    return Err(format!(
                        "channel {} has duplicate required file role {}",
                        channel.id, required.role
                    ));
                }
                if !valid_workspace_path(&required.content_path) {
                    return Err(format!(
                        "channel {} required file {} must use a safe relative workspace path",
                        channel.id, required.role
                    ));
                }
            }
            if channel.content_kind == ContentKind::WiiNandTitle
                && !channel.title_id.as_deref().is_some_and(valid_wii_title_id)
            {
                return Err(format!(
                    "NAND title {} requires a 16-character hexadecimal titleId",
                    channel.id
                ));
            }
            if let Some(app_id) = channel.app_id.as_deref() {
                if app_id.trim().is_empty() || app_id.len() > 128 {
                    return Err(format!(
                        "channel {} appId must contain 1-128 characters",
                        channel.id
                    ));
                }
                if !app_ids.insert(app_id) {
                    return Err(format!("duplicate appId {app_id}"));
                }
            }
        }
        Ok(())
    }

    pub fn into_model(self) -> Result<MenuModel, String> {
        self.validate()?;
        Ok(MenuModel::new(self.channels))
    }

    pub fn into_bios_model(self) -> Result<MenuModel, String> {
        self.validate()?;
        Ok(MenuModel::new_bios_boot(self.channels))
    }
}

fn valid_workspace_path(path: &str) -> bool {
    !path.trim().is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && path
            .split(['/', '\\'])
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn valid_wii_title_id(title_id: &str) -> bool {
    title_id.len() == 16 && title_id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "screen", rename_all = "kebab-case")]
pub enum Screen {
    HealthAndSafety,
    Home,
    Preview {
        channel_id: String,
    },
    SdCardMenu,
    SettingSelect,
    DataManagement {
        #[serde(default)]
        tab: DataManagementTab,
    },
    Settings,
    About,
    Mail,
    LetterWriter,
    AddressBook,
    HomeButtonMenu,
    NetPlay,
    GamePackager {
        #[serde(default)]
        channel_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoundCue {
    Hover,
    Select,
    Back,
    Error,
    Startup,
    PageTurn,
    ChannelGrab,
    ChannelDrop,
    HomeMenuOpen,
    HomeMenuClose,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum MenuAction {
    ShowHealthAndSafety,
    HoldSafeMode {
        elapsed_ms: u32,
    },
    AcknowledgeHealth {
        #[serde(default)]
        safe_mode: bool,
    },
    MoveSelection {
        dx: i8,
        dy: i8,
    },
    FocusSlot {
        slot: usize,
    },
    Select,
    Start,
    Back,
    OpenSettingSelect,
    OpenDataManagement {
        #[serde(default)]
        tab: DataManagementTab,
    },
    OpenSettings,
    ChangeSettingsPage {
        page: u8,
    },
    OpenAbout,
    OpenMail,
    OpenLetterWriter,
    OpenAddressBook,
    OpenSdCardMenu,
    ToggleHomeButtonMenu,
    OpenNetPlay,
    OpenGamePackager {
        #[serde(default)]
        channel_id: Option<String>,
    },
    ChangePage {
        delta: i8,
    },
    MoveChannel {
        from_page: usize,
        from_slot: usize,
        to_page: usize,
        to_slot: usize,
    },
    EjectOrInsertDisc {
        inserted: bool,
    },
    LaunchFinished {
        succeeded: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "kebab-case")]
pub enum MenuEffect {
    Play(SoundCue),
    LaunchChannel {
        channel_id: String,
    },
    LaunchResult {
        succeeded: bool,
    },
    ChannelMoved {
        from_page: usize,
        from_slot: usize,
        to_page: usize,
        to_slot: usize,
    },
    EnteredSafeMode,
}

/// Stable BIOS and Wii System Menu 4.3 state independent of wgpu, WinUI, GTK, or browser APIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuModel {
    pages: Vec<Vec<Option<Channel>>>,
    selected: usize,
    page: usize,
    pub screen: Screen,
    previous_screen: Option<Box<Screen>>,
    pub settings_page: u8,
    pub safe_mode: bool,
    pub safe_mode_hold_ms: u32,
    pub health_acknowledged: bool,
    pub disc_inserted: bool,
    pub sd_inserted: bool,
    pub last_launch_succeeded: Option<bool>,
    pub sysconf: WiiSysconf,
}

impl MenuModel {
    pub fn new(channels: Vec<Channel>) -> Self {
        let page_count = ((channels.len().saturating_add(CHANNELS_PER_PAGE - 1))
            / CHANNELS_PER_PAGE)
            .max(MIN_CHANNEL_PAGES)
            .min(MAX_CHANNEL_PAGES);
        let mut pages = vec![vec![None; CHANNELS_PER_PAGE]; page_count];

        let mut unplaced = Vec::new();
        for channel in channels {
            if let (Some(p), Some(s)) = (channel.preferred_page, channel.preferred_slot)
                && p < pages.len()
                && s < CHANNELS_PER_PAGE
                && pages[p][s].is_none()
            {
                pages[p][s] = Some(channel);
            } else {
                unplaced.push(channel);
            }
        }

        let mut next_index = 0usize;
        for channel in unplaced {
            while next_index < pages.len() * CHANNELS_PER_PAGE {
                let p = next_index / CHANNELS_PER_PAGE;
                let s = next_index % CHANNELS_PER_PAGE;
                next_index += 1;
                if pages[p][s].is_none() {
                    pages[p][s] = Some(channel);
                    break;
                }
            }
        }

        Self {
            pages,
            selected: 0,
            page: 0,
            screen: Screen::Home,
            previous_screen: None,
            settings_page: 1,
            safe_mode: false,
            safe_mode_hold_ms: 0,
            health_acknowledged: true,
            disc_inserted: true,
            sd_inserted: true,
            last_launch_succeeded: None,
            sysconf: WiiSysconf::default(),
        }
    }

    pub fn new_bios_boot(channels: Vec<Channel>) -> Self {
        let mut model = Self::new(channels);
        model.screen = Screen::HealthAndSafety;
        model.health_acknowledged = false;
        model
    }

    pub fn selected_slot(&self) -> usize {
        self.selected
    }

    pub fn current_page(&self) -> usize {
        self.page
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn selected_channel(&self) -> Option<&Channel> {
        self.channel_at_selection()
    }

    pub fn channel_by_id(&self, id: &str) -> Option<&Channel> {
        self.pages
            .iter()
            .flat_map(|page| page.iter())
            .flatten()
            .find(|channel| channel.id == id)
    }

    pub fn page(&self, index: usize) -> Option<&[Option<Channel>]> {
        self.pages.get(index).map(Vec::as_slice)
    }

    pub fn all_pages(&self) -> &[Vec<Option<Channel>>] {
        &self.pages
    }

    pub fn move_channel(
        &mut self,
        from_page: usize,
        from_slot: usize,
        to_page: usize,
        to_slot: usize,
    ) -> bool {
        if from_page >= self.pages.len()
            || to_page >= self.pages.len()
            || from_slot >= CHANNELS_PER_PAGE
            || to_slot >= CHANNELS_PER_PAGE
        {
            return false;
        }
        if self.pages[from_page][from_slot].is_none() {
            return false;
        }
        if from_page == to_page && from_slot == to_slot {
            return true;
        }
        let source = self.pages[from_page][from_slot].take();
        let target = self.pages[to_page][to_slot].take();
        self.pages[to_page][to_slot] = source;
        self.pages[from_page][from_slot] = target;
        self.page = to_page;
        self.selected = to_slot;
        true
    }

    pub fn apply(&mut self, action: MenuAction) -> Option<MenuEffect> {
        match action {
            MenuAction::ShowHealthAndSafety => {
                self.screen = Screen::HealthAndSafety;
                self.health_acknowledged = false;
                self.safe_mode_hold_ms = 0;
                Some(MenuEffect::Play(SoundCue::Startup))
            }
            MenuAction::HoldSafeMode { elapsed_ms }
                if self.screen == Screen::HealthAndSafety =>
            {
                self.safe_mode_hold_ms = elapsed_ms;
                if elapsed_ms >= SAFE_MODE_HOLD_THRESHOLD_MS {
                    self.safe_mode = true;
                    self.health_acknowledged = true;
                    self.screen = Screen::Home;
                    Some(MenuEffect::EnteredSafeMode)
                } else {
                    None
                }
            }
            MenuAction::AcknowledgeHealth { safe_mode } => {
                self.safe_mode = safe_mode;
                self.health_acknowledged = true;
                self.screen = Screen::Home;
                if safe_mode {
                    Some(MenuEffect::EnteredSafeMode)
                } else {
                    Some(MenuEffect::Play(SoundCue::Select))
                }
            }
            MenuAction::FocusSlot { slot }
                if self.screen == Screen::Home && slot < CHANNELS_PER_PAGE =>
            {
                self.selected = slot;
                Some(MenuEffect::Play(SoundCue::Hover))
            }
            MenuAction::MoveSelection { dx, dy } if self.screen == Screen::Home => {
                let x = (self.selected % GRID_COLUMNS) as i16;
                let y = (self.selected / GRID_COLUMNS) as i16;
                let next_x = (x + i16::from(dx)).rem_euclid(GRID_COLUMNS as i16) as usize;
                let next_y = (y + i16::from(dy)).rem_euclid(GRID_ROWS as i16) as usize;
                self.selected = next_y * GRID_COLUMNS + next_x;
                Some(MenuEffect::Play(SoundCue::Hover))
            }
            MenuAction::Select if self.screen == Screen::HealthAndSafety => {
                self.health_acknowledged = true;
                self.screen = Screen::Home;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::Select if self.screen == Screen::Home => {
                let channel = self.channel_at_selection()?.clone();
                self.screen = Screen::Preview {
                    channel_id: channel.id,
                };
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::Start => match &self.screen {
                Screen::HealthAndSafety => {
                    self.health_acknowledged = true;
                    self.screen = Screen::Home;
                    Some(MenuEffect::Play(SoundCue::Select))
                }
                Screen::Preview { channel_id } => Some(MenuEffect::LaunchChannel {
                    channel_id: channel_id.clone(),
                }),
                Screen::Home => {
                    let channel = self.channel_at_selection()?.clone();
                    Some(MenuEffect::LaunchChannel {
                        channel_id: channel.id,
                    })
                }
                _ => None,
            },
            MenuAction::Back => {
                match &self.screen {
                    Screen::Home | Screen::HealthAndSafety => return None,
                    Screen::HomeButtonMenu => {
                        self.screen = self
                            .previous_screen
                            .take()
                            .map(|s| *s)
                            .unwrap_or(Screen::Home);
                        return Some(MenuEffect::Play(SoundCue::HomeMenuClose));
                    }
                    Screen::Settings | Screen::DataManagement { .. } => {
                        self.screen = Screen::SettingSelect;
                    }
                    Screen::LetterWriter | Screen::AddressBook => {
                        self.screen = Screen::Mail;
                    }
                    _ => {
                        self.screen = Screen::Home;
                    }
                }
                self.last_launch_succeeded = None;
                Some(MenuEffect::Play(SoundCue::Back))
            }
            MenuAction::OpenSettingSelect => {
                self.screen = Screen::SettingSelect;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenDataManagement { tab } => {
                self.screen = Screen::DataManagement { tab };
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenSettings => {
                self.screen = Screen::Settings;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::ChangeSettingsPage { page } => {
                self.settings_page = page.clamp(1, 3);
                self.screen = Screen::Settings;
                Some(MenuEffect::Play(SoundCue::PageTurn))
            }
            MenuAction::OpenAbout => {
                self.screen = Screen::About;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenMail => {
                self.screen = Screen::Mail;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenLetterWriter => {
                self.screen = Screen::LetterWriter;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenAddressBook => {
                self.screen = Screen::AddressBook;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenSdCardMenu => {
                self.screen = Screen::SdCardMenu;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::ToggleHomeButtonMenu => {
                if self.screen == Screen::HomeButtonMenu {
                    self.screen = self
                        .previous_screen
                        .take()
                        .map(|s| *s)
                        .unwrap_or(Screen::Home);
                    Some(MenuEffect::Play(SoundCue::HomeMenuClose))
                } else {
                    self.previous_screen = Some(Box::new(self.screen.clone()));
                    self.screen = Screen::HomeButtonMenu;
                    Some(MenuEffect::Play(SoundCue::HomeMenuOpen))
                }
            }
            MenuAction::OpenNetPlay => {
                self.screen = Screen::NetPlay;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::OpenGamePackager { channel_id } => {
                let target_id = channel_id.or_else(|| match &self.screen {
                    Screen::Preview { channel_id } => Some(channel_id.clone()),
                    _ => self.selected_channel().map(|c| c.id.clone()),
                });
                self.screen = Screen::GamePackager {
                    channel_id: target_id,
                };
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::ChangePage { delta } if self.screen == Screen::Home => {
                if !self.pages.is_empty() {
                    self.page = (self.page as i16 + i16::from(delta))
                        .rem_euclid(self.pages.len() as i16)
                        as usize;
                    self.selected = 0;
                    Some(MenuEffect::Play(SoundCue::PageTurn))
                } else {
                    None
                }
            }
            MenuAction::MoveChannel {
                from_page,
                from_slot,
                to_page,
                to_slot,
            } => {
                if self.move_channel(from_page, from_slot, to_page, to_slot) {
                    Some(MenuEffect::ChannelMoved {
                        from_page,
                        from_slot,
                        to_page,
                        to_slot,
                    })
                } else {
                    Some(MenuEffect::Play(SoundCue::Error))
                }
            }
            MenuAction::EjectOrInsertDisc { inserted } => {
                self.disc_inserted = inserted;
                Some(MenuEffect::Play(SoundCue::Select))
            }
            MenuAction::LaunchFinished { succeeded } => {
                self.last_launch_succeeded = Some(succeeded);
                Some(MenuEffect::LaunchResult { succeeded })
            }
            _ => None,
        }
    }

    fn channel_at_selection(&self) -> Option<&Channel> {
        self.pages.get(self.page)?.get(self.selected)?.as_ref()
    }
}

/// C-compatible API for native C frontends. The host owns rendering, input and launch effects;
/// the same validated channel/state model is shared with the Rust and WebAssembly frontends.
pub mod c_api {
    use super::*;
    use std::{
        ffi::c_char,
        panic::{AssertUnwindSafe, catch_unwind},
        ptr, slice, str,
    };

    pub const OK: i32 = 0;
    pub const INVALID_ARGUMENT: i32 = 1;
    pub const INVALID_CATALOG: i32 = 2;
    pub const PANIC: i32 = 3;
    pub const ACTION_MOVE_LEFT: u32 = 1;
    pub const ACTION_MOVE_RIGHT: u32 = 2;
    pub const ACTION_MOVE_UP: u32 = 3;
    pub const ACTION_MOVE_DOWN: u32 = 4;
    pub const ACTION_SELECT: u32 = 5;
    pub const ACTION_START: u32 = 6;
    pub const ACTION_BACK: u32 = 7;
    pub const ACTION_SETTINGS: u32 = 8;
    pub const ACTION_ABOUT: u32 = 9;
    pub const ACTION_MAIL: u32 = 10;
    pub const ACTION_PAGE_PREVIOUS: u32 = 11;
    pub const ACTION_PAGE_NEXT: u32 = 12;
    pub const ACTION_HOME_MENU: u32 = 13;
    pub const ACTION_SD_CARD_MENU: u32 = 14;
    pub const ACTION_DATA_MANAGEMENT: u32 = 15;
    pub const ACTION_NETPLAY: u32 = 16;
    pub const ACTION_GAME_PACKAGER: u32 = 17;

    #[repr(C)]
    pub struct Model {
        inner: MenuModel,
    }

    fn status<F: FnOnce() -> i32 + std::panic::UnwindSafe>(f: F) -> i32 {
        catch_unwind(f).unwrap_or(PANIC)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn wii_menu_model_create_json(
        data: *const u8,
        length: usize,
        out: *mut *mut Model,
    ) -> i32 {
        status(AssertUnwindSafe(|| {
            if data.is_null() || out.is_null() || length == 0 {
                return INVALID_ARGUMENT;
            }
            unsafe {
                *out = ptr::null_mut();
            }
            let bytes = unsafe { slice::from_raw_parts(data, length) };
            let Ok(text) = str::from_utf8(bytes) else {
                return INVALID_CATALOG;
            };
            let Ok(catalog) = serde_json::from_str::<MenuCatalog>(text) else {
                return INVALID_CATALOG;
            };
            let Ok(inner) = catalog.into_model() else {
                return INVALID_CATALOG;
            };
            unsafe {
                *out = Box::into_raw(Box::new(Model { inner }));
            }
            OK
        }))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn wii_menu_model_destroy(model: *mut Model) {
        if !model.is_null() {
            drop(unsafe { Box::from_raw(model) });
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn wii_menu_model_apply(
        model: *mut Model,
        action: u32,
        out_effect: *mut u32,
    ) -> i32 {
        status(AssertUnwindSafe(|| {
            if model.is_null() || out_effect.is_null() {
                return INVALID_ARGUMENT;
            }
            let model = unsafe { &mut *model };
            let action = match action {
                ACTION_MOVE_LEFT => MenuAction::MoveSelection { dx: -1, dy: 0 },
                ACTION_MOVE_RIGHT => MenuAction::MoveSelection { dx: 1, dy: 0 },
                ACTION_MOVE_UP => MenuAction::MoveSelection { dx: 0, dy: -1 },
                ACTION_MOVE_DOWN => MenuAction::MoveSelection { dx: 0, dy: 1 },
                ACTION_SELECT => MenuAction::Select,
                ACTION_START => MenuAction::Start,
                ACTION_BACK => MenuAction::Back,
                ACTION_SETTINGS => MenuAction::OpenSettings,
                ACTION_ABOUT => MenuAction::OpenAbout,
                ACTION_MAIL => MenuAction::OpenMail,
                ACTION_PAGE_PREVIOUS => MenuAction::ChangePage { delta: -1 },
                ACTION_PAGE_NEXT => MenuAction::ChangePage { delta: 1 },
                ACTION_HOME_MENU => MenuAction::ToggleHomeButtonMenu,
                ACTION_SD_CARD_MENU => MenuAction::OpenSdCardMenu,
                ACTION_DATA_MANAGEMENT => MenuAction::OpenDataManagement {
                    tab: DataManagementTab::SaveDataWii,
                },
                ACTION_NETPLAY => MenuAction::OpenNetPlay,
                ACTION_GAME_PACKAGER => MenuAction::OpenGamePackager { channel_id: None },
                _ => return INVALID_ARGUMENT,
            };
            let effect = model.inner.apply(action);
            unsafe {
                *out_effect = match effect {
                    None => 0,
                    Some(MenuEffect::Play(SoundCue::Hover | SoundCue::PageTurn)) => 1,
                    Some(MenuEffect::Play(
                        SoundCue::Select
                        | SoundCue::Startup
                        | SoundCue::ChannelGrab
                        | SoundCue::ChannelDrop
                        | SoundCue::HomeMenuOpen,
                    )) => 2,
                    Some(MenuEffect::Play(SoundCue::Back | SoundCue::HomeMenuClose)) => 3,
                    Some(MenuEffect::Play(SoundCue::Error)) => 4,
                    Some(MenuEffect::LaunchChannel { .. }) => 5,
                    Some(MenuEffect::LaunchResult { succeeded: true }) => 6,
                    Some(MenuEffect::LaunchResult { succeeded: false }) => 7,
                    Some(MenuEffect::ChannelMoved { .. }) => 8,
                    Some(MenuEffect::EnteredSafeMode) => 9,
                };
            }
            OK
        }))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn wii_menu_model_selection(
        model: *const Model,
        out_slot: *mut usize,
        out_page: *mut usize,
        out_page_count: *mut usize,
    ) -> i32 {
        if model.is_null() || out_slot.is_null() || out_page.is_null() || out_page_count.is_null() {
            return INVALID_ARGUMENT;
        }
        let model = unsafe { &(*model).inner };
        unsafe {
            *out_slot = model.selected_slot();
            *out_page = model.current_page();
            *out_page_count = model.page_count();
        }
        OK
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn wii_menu_model_selected_channel(
        model: *const Model,
        output: *mut c_char,
        capacity: usize,
        required: *mut usize,
    ) -> i32 {
        if model.is_null() || required.is_null() {
            return INVALID_ARGUMENT;
        }
        let Some(channel) = unsafe { &(*model).inner }.selected_channel() else {
            unsafe {
                *required = 0;
            }
            return OK;
        };
        let bytes = channel.id.as_bytes();
        unsafe {
            *required = bytes.len() + 1;
        }
        if output.is_null() || capacity < bytes.len() + 1 {
            return INVALID_ARGUMENT;
        }
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast(), bytes.len());
            *output.add(bytes.len()) = 0;
        }
        OK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_catalog() -> MenuCatalog {
        MenuCatalog {
            schema_version: 1,
            channels: vec![
                Channel {
                    id: "disc-channel".into(),
                    title: "Chaîne Disque".into(),
                    icon: None,
                    content_kind: ContentKind::WiiDisc,
                    content_path: Some("wii-os/userland/data/wii/roms/game.rvz".into()),
                    title_id: Some("RDSPAF".into()),
                    app_id: Some("disc".into()),
                    blocks: Some(0),
                    publisher: Some("Nintendo / Bandai Namco".into()),
                    players: Some(4),
                    preferred_page: None,
                    preferred_slot: None,
                    required_files: vec![],
                },
                Channel {
                    id: "mii-channel".into(),
                    title: "Chaîne Mii".into(),
                    icon: None,
                    content_kind: ContentKind::WiiWad,
                    content_path: None,
                    title_id: Some("0001000248414341".into()),
                    app_id: Some("mii".into()),
                    blocks: Some(42),
                    publisher: Some("Nintendo".into()),
                    players: Some(1),
                    preferred_page: None,
                    preferred_slot: None,
                    required_files: vec![],
                },
            ],
        }
    }

    #[test]
    fn bios_boot_and_safe_mode_work() {
        let mut model = sample_catalog().into_bios_model().unwrap();
        assert_eq!(model.screen, Screen::HealthAndSafety);
        assert_eq!(model.page_count(), MIN_CHANNEL_PAGES);

        let effect = model.apply(MenuAction::HoldSafeMode { elapsed_ms: 3100 });
        assert_eq!(effect, Some(MenuEffect::EnteredSafeMode));
        assert!(model.safe_mode);
        assert_eq!(model.screen, Screen::Home);
    }

    #[test]
    fn channel_drag_and_reorder_across_pages() {
        let mut model = sample_catalog().into_model().unwrap();
        let effect = model.apply(MenuAction::MoveChannel {
            from_page: 0,
            from_slot: 0,
            to_page: 2,
            to_slot: 5,
        });
        assert_eq!(
            effect,
            Some(MenuEffect::ChannelMoved {
                from_page: 0,
                from_slot: 0,
                to_page: 2,
                to_slot: 5,
            })
        );
        assert_eq!(model.current_page(), 2);
        assert_eq!(model.selected_slot(), 5);
        assert_eq!(
            model.selected_channel().map(|c| c.id.as_str()),
            Some("disc-channel")
        );
    }

    #[test]
    fn home_button_menu_preserves_previous_screen() {
        let mut model = sample_catalog().into_model().unwrap();
        model.apply(MenuAction::OpenMail);
        assert_eq!(model.screen, Screen::Mail);
        model.apply(MenuAction::ToggleHomeButtonMenu);
        assert_eq!(model.screen, Screen::HomeButtonMenu);
        model.apply(MenuAction::ToggleHomeButtonMenu);
        assert_eq!(model.screen, Screen::Mail);
    }

    #[test]
    fn c_reverse_ipl_math_matches_spec() {
        let grid = IplGridMath::default();
        let (x43, y43) = grid.slot_position(0, IplAspectRatio::Ratio4x3);
        assert!((x43 - 325.0).abs() < 1.0);
        assert!((y43 - 210.0).abs() < 1.0);

        let (left0, right0) = grid.arrow_visibility(0);
        assert!(!left0);
        assert!(right0);

        let (left3, right3) = grid.arrow_visibility(3);
        assert!(left3);
        assert!(!right3);

        let blocks = IplNandMath::bytes_to_blocks(262_144);
        assert_eq!(blocks, 2);
        assert_eq!(IplNandMath::free_blocks(284), 1890);
    }
}
