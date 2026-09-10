// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::model::{MAX_DOCUMENT, MAX_ENTRIES};
use super::{ensure, Error, Result};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(value: &str, length: usize) -> Result<Vec<u8>> {
    ensure(
        value.len() == length * 2
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid lowercase hexadecimal integrity value",
    )?;
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let s = std::str::from_utf8(pair)
                .map_err(|_| Error::Rejected("invalid hexadecimal value".into()))?;
            u8::from_str_radix(s, 16)
                .map_err(|_| Error::Rejected("invalid hexadecimal value".into()))
        })
        .collect()
}
pub fn id() -> String {
    let random: [u8; 16] = rand::random();
    hex(&random)
}
pub fn safe_id(value: &str) -> Result<()> {
    unhex(value, 16).map(|_| ())
}
pub fn relative(value: &str) -> Result<()> {
    ensure(
        !value.is_empty()
            && value.len() <= 4096
            && !value.contains(['\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
            && !Path::new(value).is_absolute(),
        "unsafe deployment inventory path",
    )?;
    ensure(
        value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part.chars().any(char::is_control)
                && {
                    let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
                    !matches!(
                        base.as_str(),
                        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                    ) && !(base.len() == 4
                        && (base.starts_with("COM") || base.starts_with("LPT"))
                        && matches!(base.as_bytes()[3], b'1'..=b'9'))
                }
        }),
        "unsafe deployment inventory component",
    )
}
pub fn real(path: &Path, directory: bool) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        ensure(
            !matches!(component, Component::ParentDir),
            "parent traversal is not permitted",
        )?;
        current.push(component);
        // /tmp and user-selected ancestors may be OS aliases; reject symlinks
        // at the actual object boundary, then work only with canonical roots.
    }
    let meta = fs::symlink_metadata(&current)?;
    ensure(
        !meta.file_type().is_symlink()
            && if directory {
                meta.is_dir()
            } else {
                meta.is_file()
            },
        "deployment path must be a real regular file or directory",
    )?;
    Ok(current.canonicalize()?)
}
pub fn private_dir(path: &Path) -> Result<()> {
    if path.exists() {
        real(path, true)?;
        return Ok(());
    }
    #[allow(unused_mut)] // Unix mode configuration requires a mutable builder.
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    sync_dir(
        path.parent()
            .ok_or_else(|| Error::Rejected("missing parent directory".into()))?,
    )
}
pub fn parents(root: &Path, relative_path: &str) -> Result<PathBuf> {
    relative(relative_path)?;
    real(root, true)?;
    let mut path = root.to_path_buf();
    let parts: Vec<_> = relative_path.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        path.push(part);
        private_dir(&path)?;
    }
    path.push(parts[parts.len() - 1]);
    Ok(path)
}
pub fn read(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    real(path, false)?;
    let file = File::open(path)?;
    ensure(
        file.metadata()?.len() <= maximum,
        "deployment document exceeds size limit",
    )?;
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    ensure(
        bytes.len() as u64 <= maximum,
        "deployment document exceeds size limit",
    )?;
    Ok(bytes)
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_dir(
        path.parent()
            .ok_or_else(|| Error::Rejected("missing file parent".into()))?,
    )
}
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<String> {
    let bytes = serde_json::to_vec_pretty(value)?;
    ensure(
        bytes.len() as u64 <= MAX_DOCUMENT,
        "deployment manifest exceeds size limit",
    )?;
    write_new(path, &bytes)?;
    Ok(digest(&bytes))
}
pub fn hash(path: &Path) -> Result<(u64, String)> {
    real(path, false)?;
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = add(size, n as u64)?;
        digest.update(&buffer[..n]);
    }
    Ok((size, format!("{digest:x}", digest = digest.finalize())))
}
pub fn copy(source: &Path, target: &Path) -> Result<(u64, String)> {
    real(source, false)?;
    let mut input = File::open(source)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(target)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    sync_dir(
        target
            .parent()
            .ok_or_else(|| Error::Rejected("missing copy parent".into()))?,
    )?;
    hash(target)
}
pub fn replace(source: &Path, destination: &Path) -> Result<()> {
    real(source, false)?;
    let parent = destination
        .parent()
        .ok_or_else(|| Error::Rejected("missing replacement parent".into()))?;
    real(parent, true)?;
    if destination.exists() {
        real(destination, false)?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut File::open(source)?, temp.as_file_mut())?;
    temp.as_file_mut().sync_all()?;
    temp.persist(destination).map_err(|e| Error::Io(e.error))?;
    sync_dir(parent)
}
pub fn executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        real(path, false)?;
    }
    Ok(())
}
pub fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    // Windows directory FlushFileBuffers is not a portable permission-free
    // operation. Files and the SQLite journal are flushed; real reboot/power
    // loss acceptance remains explicitly deferred on Windows.
    #[cfg(not(unix))]
    {
        real(path, true)?;
    }
    Ok(())
}
pub fn inventory(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    real(root, true)?;
    let mut result = vec![];
    let mut pending = vec![root.to_path_buf()];
    let mut names = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for item in fs::read_dir(directory)? {
            let item = item?;
            let path = item.path();
            let meta = fs::symlink_metadata(&path)?;
            ensure(
                !meta.file_type().is_symlink(),
                "symlink in deployment inventory",
            )?;
            let name = path
                .strip_prefix(root)
                .map_err(|_| Error::Rejected("inventory escaped root".into()))?
                .to_str()
                .ok_or_else(|| Error::Rejected("inventory path is not portable UTF-8".into()))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            relative(&name)?;
            ensure(
                names.insert(name.to_ascii_lowercase()) && names.len() <= MAX_ENTRIES,
                "duplicate, case-conflicting or excessive deployment inventory",
            )?;
            if meta.is_dir() {
                pending.push(path);
            } else {
                ensure(meta.is_file(), "special file in deployment inventory")?;
                result.push((name, path));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
pub fn tree_bytes(root: &Path) -> Result<u64> {
    inventory(root)?
        .iter()
        .try_fold(0, |sum, (_, path)| add(sum, fs::metadata(path)?.len()))
}
pub fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| Error::Rejected("storage accounting overflow".into()))
}
pub fn multiply(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| Error::Rejected("storage accounting overflow".into()))
}

pub trait Capacity {
    fn available(&self, path: &Path) -> Result<u64>;
}
pub struct HostCapacity;
impl Capacity for HostCapacity {
    fn available(&self, path: &Path) -> Result<u64> {
        available(path)
    }
}
#[cfg(unix)]
fn available(path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Error::Rejected("invalid capacity path".into()))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: NUL-terminated path and valid writable statvfs storage. A failed
    // call never reads the uninitialized output.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let stats = unsafe { stats.assume_init() };
    // libc uses different integer widths across Unix targets.
    #[allow(clippy::unnecessary_cast)]
    multiply(stats.f_bavail as u64, stats.f_frsize as u64)
}
#[cfg(windows)]
fn available(path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    // SAFETY: Valid NUL-terminated UTF-16 path and output pointer; optional
    // totals are null as permitted by GetDiskFreeSpaceExW.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(available)
}
#[cfg(not(any(unix, windows)))]
fn available(_: &Path) -> Result<u64> {
    Err(Error::Rejected(
        "free-space checks unsupported on this host".into(),
    ))
}
