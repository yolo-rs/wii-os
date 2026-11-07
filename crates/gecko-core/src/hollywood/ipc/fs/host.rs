use crate::hollywood::ipc::{DeviceContext, IPC_EINVAL, IosDevice};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

const SEEK_SET: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;

const IOCTL_GET_FILE_STATS: u32 = 0xB;

#[derive(Default)]
pub struct MemoryNandVfs {
    pub dirs: BTreeSet<String>,
    pub files: BTreeMap<String, Vec<u8>>,
}

static MEMORY_VFS: LazyLock<Mutex<MemoryNandVfs>> = LazyLock::new(|| {
    let mut vfs = MemoryNandVfs::default();
    vfs.dirs.insert("/".to_string());
    for d in [
        "/sys",
        "/ticket",
        "/title",
        "/title/00000001",
        "/title/00000001/00000002",
        "/title/00000001/00000002/data",
        "/title/00010000",
        "/title/00010000/52445350",
        "/title/00010000/52445350/content",
        "/title/00010000/52445350/data",
        "/shared1",
        "/shared2",
        "/shared2/sys",
        "/shared2/wc24",
        "/shared2/wc24/mbox",
        "/tmp",
        "/import",
        "/meta",
        "/wfs",
    ] {
        vfs.dirs.insert(d.to_string());
    }
    Mutex::new(vfs)
});

pub fn normalize_nand_path(path: impl AsRef<Path>) -> String {
    let raw = path.as_ref().to_string_lossy().replace('\\', "/");
    // Strip any leading host root prefix before known NAND root segments if present
    let s = if let Some(idx) = raw.find("/nand/") {
        &raw[idx + 5..]
    } else if raw.ends_with("/nand") {
        "/"
    } else {
        raw.as_str()
    };
    let trimmed = s.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub fn vfs_ensure_dir_all(nand_path: impl AsRef<Path>) {
    let norm = normalize_nand_path(nand_path);
    let mut vfs = MEMORY_VFS.lock().unwrap();
    vfs.dirs.insert("/".to_string());
    let mut cur = String::new();
    for seg in norm.split('/').filter(|s| !s.is_empty()) {
        cur.push('/');
        cur.push_str(seg);
        vfs.dirs.insert(cur.clone());
    }
}

pub fn vfs_create_dir(nand_path: impl AsRef<Path>) -> Result<(), bool> {
    let norm = normalize_nand_path(nand_path);
    let mut vfs = MEMORY_VFS.lock().unwrap();
    if vfs.dirs.contains(&norm) || vfs.files.contains_key(&norm) {
        return Err(true);
    }
    let mut cur = String::new();
    for seg in norm.split('/').filter(|s| !s.is_empty()) {
        cur.push('/');
        cur.push_str(seg);
        vfs.dirs.insert(cur.clone());
    }
    Ok(())
}

pub fn vfs_create_file(nand_path: impl AsRef<Path>) -> Result<(), bool> {
    let norm = normalize_nand_path(nand_path);
    if let Some((parent, _)) = norm.rsplit_once('/') {
        vfs_ensure_dir_all(if parent.is_empty() { "/" } else { parent });
    }
    let mut vfs = MEMORY_VFS.lock().unwrap();
    if vfs.files.contains_key(&norm) || vfs.dirs.contains(&norm) {
        return Err(true);
    }
    vfs.files.insert(norm, Vec::new());
    Ok(())
}

pub fn vfs_write_file(nand_path: impl AsRef<Path>, data: Vec<u8>) {
    let norm = normalize_nand_path(nand_path);
    if let Some((parent, _)) = norm.rsplit_once('/') {
        vfs_ensure_dir_all(if parent.is_empty() { "/" } else { parent });
    }
    MEMORY_VFS.lock().unwrap().files.insert(norm, data);
}

pub fn vfs_read_file(nand_path: impl AsRef<Path>) -> Option<Vec<u8>> {
    let norm = normalize_nand_path(nand_path);
    MEMORY_VFS.lock().unwrap().files.get(&norm).cloned()
}

pub fn vfs_exists(nand_path: impl AsRef<Path>) -> bool {
    let norm = normalize_nand_path(nand_path);
    let vfs = MEMORY_VFS.lock().unwrap();
    vfs.dirs.contains(&norm) || vfs.files.contains_key(&norm)
}

pub fn vfs_is_dir(nand_path: impl AsRef<Path>) -> bool {
    let norm = normalize_nand_path(nand_path);
    MEMORY_VFS.lock().unwrap().dirs.contains(&norm)
}

pub fn vfs_is_file(nand_path: impl AsRef<Path>) -> bool {
    let norm = normalize_nand_path(nand_path);
    MEMORY_VFS.lock().unwrap().files.contains_key(&norm)
}

pub fn vfs_read_dir(nand_path: impl AsRef<Path>) -> Option<Vec<String>> {
    let norm = normalize_nand_path(nand_path);
    let vfs = MEMORY_VFS.lock().unwrap();
    if !vfs.dirs.contains(&norm) {
        return None;
    }
    let prefix = if norm == "/" { "/".to_string() } else { format!("{norm}/") };
    let mut children = BTreeSet::new();
    for d in &vfs.dirs {
        if let Some(rest) = d.strip_prefix(&prefix) {
            if !rest.is_empty() && !rest.contains('/') {
                children.insert(rest.to_string());
            }
        }
    }
    for f in vfs.files.keys() {
        if let Some(rest) = f.strip_prefix(&prefix) {
            if !rest.is_empty() && !rest.contains('/') {
                children.insert(rest.to_string());
            }
        }
    }
    Some(children.into_iter().collect())
}

pub fn vfs_delete(nand_path: impl AsRef<Path>) -> bool {
    let norm = normalize_nand_path(nand_path);
    let mut vfs = MEMORY_VFS.lock().unwrap();
    if vfs.files.remove(&norm).is_some() {
        return true;
    }
    vfs.dirs.remove(&norm)
}

pub fn vfs_rename(src_path: impl AsRef<Path>, dst_path: impl AsRef<Path>) -> bool {
    let src = normalize_nand_path(src_path);
    let dst = normalize_nand_path(dst_path);
    let mut vfs = MEMORY_VFS.lock().unwrap();
    if let Some(bytes) = vfs.files.remove(&src) {
        vfs.files.insert(dst, bytes);
        return true;
    }
    if vfs.dirs.remove(&src) {
        vfs.dirs.insert(dst);
        return true;
    }
    false
}

pub fn vfs_export_files() -> Vec<(String, Vec<u8>)> {
    let vfs = MEMORY_VFS.lock().unwrap();
    vfs.files.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

enum BackingStore {
    Host(File),
    Memory { nand_path: String, cursor: u64 },
}

pub struct HostBackedFile {
    path: PathBuf,
    store: BackingStore,
    mode: u32,
}

impl HostBackedFile {
    pub fn try_open(host_root: &Path, nand_path: &str, mode: u32) -> Option<Self> {
        let host_path = self::nand_to_host(host_root, nand_path);
        if host_path.is_file() {
            let mut opts = OpenOptions::new();
            match mode {
                0 | 1 => {
                    opts.read(true);
                }
                2 => {
                    opts.write(true);
                }
                _ => {
                    opts.read(true).write(true);
                }
            };

            if let Ok(file) = opts.open(&host_path) {
                return Some(Self {
                    path: host_path,
                    store: BackingStore::Host(file),
                    mode,
                });
            }
        }

        if vfs_is_file(nand_path) {
            return Some(Self {
                path: host_path,
                store: BackingStore::Memory {
                    nand_path: normalize_nand_path(nand_path),
                    cursor: 0,
                },
                mode,
            });
        }

        None
    }

    pub fn reopen(host_root: &Path, nand_path: &str, mode: u32, offset: u64) -> Option<Self> {
        let mut this = Self::try_open(host_root, nand_path, mode)?;
        match &mut this.store {
            BackingStore::Host(file) => {
                if let Err(err) = file.seek(SeekFrom::Start(offset)) {
                    tracing::warn!(path = format!("{}", this.path.display()), %err, "HostBackedFile: reopen seek failed");
                }
            }
            BackingStore::Memory { cursor, .. } => {
                *cursor = offset;
            }
        }
        Some(this)
    }
}

impl IosDevice for HostBackedFile {
    fn read(&mut self, ctx: &mut DeviceContext<'_>, out_ptr: u32, out_len: u32) -> i32 {
        match &mut self.store {
            BackingStore::Host(file) => {
                let mut buf = vec![0u8; out_len as usize];
                let n = match file.read(&mut buf) {
                    Ok(n) => n,
                    Err(err) => {
                        tracing::warn!(path = format!("{}", self.path.display()), %err, "HostBackedFile: read failed");
                        return IPC_EINVAL;
                    }
                };
                ctx.mmio.phys_slice_mut(out_ptr, n).copy_from_slice(&buf[..n]);
                n as i32
            }
            BackingStore::Memory { nand_path, cursor } => {
                let Some(bytes) = vfs_read_file(nand_path) else {
                    return IPC_EINVAL;
                };
                let start = (*cursor as usize).min(bytes.len());
                let n = (out_len as usize).min(bytes.len().saturating_sub(start));
                if n > 0 {
                    ctx.mmio
                        .phys_slice_mut(out_ptr, n)
                        .copy_from_slice(&bytes[start..start + n]);
                    *cursor += n as u64;
                }
                n as i32
            }
        }
    }

    fn write(&mut self, ctx: &mut DeviceContext<'_>, in_ptr: u32, in_len: u32) -> i32 {
        let buf = ctx.mmio.phys_slice(in_ptr, in_len as usize).to_vec();
        match &mut self.store {
            BackingStore::Host(file) => match file.write_all(&buf) {
                Ok(()) => in_len as i32,
                Err(err) => {
                    tracing::warn!(path = format!("{}", self.path.display()), %err, "HostBackedFile: write failed");
                    IPC_EINVAL
                }
            },
            BackingStore::Memory { nand_path, cursor } => {
                let mut vfs = MEMORY_VFS.lock().unwrap();
                let entry = vfs.files.entry(nand_path.clone()).or_default();
                let end = (*cursor as usize).saturating_add(buf.len());
                if entry.len() < end {
                    entry.resize(end, 0);
                }
                let start = *cursor as usize;
                entry[start..end].copy_from_slice(&buf);
                *cursor = end as u64;
                in_len as i32
            }
        }
    }

    fn seek(&mut self, _ctx: &mut DeviceContext<'_>, offset: i32, whence: i32) -> i32 {
        match &mut self.store {
            BackingStore::Host(file) => {
                let target = match whence {
                    SEEK_SET => SeekFrom::Start(offset as u64),
                    SEEK_CUR => SeekFrom::Current(offset as i64),
                    SEEK_END => SeekFrom::End(offset as i64),
                    _ => {
                        tracing::warn!(whence, "HostBackedFile: invalid whence");
                        return IPC_EINVAL;
                    }
                };

                match file.seek(target) {
                    Ok(pos) => pos as i32,
                    Err(err) => {
                        tracing::warn!(path = format!("{}", self.path.display()), %err, "HostBackedFile: seek failed");
                        IPC_EINVAL
                    }
                }
            }
            BackingStore::Memory { nand_path, cursor } => {
                let len = vfs_read_file(nand_path).map(|b| b.len() as i64).unwrap_or(0);
                let base = match whence {
                    SEEK_SET => 0,
                    SEEK_CUR => *cursor as i64,
                    SEEK_END => len,
                    _ => return IPC_EINVAL,
                };
                let next = base + offset as i64;
                if next < 0 {
                    return IPC_EINVAL;
                }
                *cursor = next as u64;
                next as i32
            }
        }
    }

    fn ioctl(
        &mut self,
        ctx: &mut DeviceContext<'_>,
        cmd: u32,
        _in_ptr: u32,
        _in_len: u32,
        out_ptr: u32,
        _out_len: u32,
    ) -> i32 {
        match cmd {
            IOCTL_GET_FILE_STATS => {
                let (size, pos) = match &mut self.store {
                    BackingStore::Host(file) => (
                        file.metadata().map(|m| m.len()).unwrap_or(0) as u32,
                        file.stream_position().unwrap_or(0) as u32,
                    ),
                    BackingStore::Memory { nand_path, cursor } => (
                        vfs_read_file(nand_path).map(|b| b.len() as u32).unwrap_or(0),
                        *cursor as u32,
                    ),
                };
                ctx.mmio.phys_write_u32(out_ptr, size);
                ctx.mmio.phys_write_u32(out_ptr + 4, pos);
                0
            }
            _ => {
                tracing::warn!(
                    device = &ctx.device_path,
                    path = format!("{}", self.path.display()),
                    cmd = format!("{cmd:#X}"),
                    "HostBackedFile: unhandled ioctl"
                );
                IPC_EINVAL
            }
        }
    }

    fn host_file_state(&mut self) -> Option<(u32, u64)> {
        let offset = match &mut self.store {
            BackingStore::Host(file) => file.stream_position().unwrap_or(0),
            BackingStore::Memory { cursor, .. } => *cursor,
        };
        Some((self.mode, offset))
    }
}

pub(super) fn nand_to_host(host_root: &Path, nand_path: &str) -> PathBuf {
    let trimmed = nand_path.trim_start_matches('/');
    host_root.join(trimmed)
}
