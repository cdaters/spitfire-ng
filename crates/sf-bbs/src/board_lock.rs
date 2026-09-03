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

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::ApplicationError;

/// Exclusive cross-process coordination for operations that require a cold
/// board. The lock lives beside the board root so it remains held while a
/// restore atomically exchanges that root directory.
pub(crate) struct BoardOperationLock {
    _file: File,
}

impl BoardOperationLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, ApplicationError> {
        let path = lock_path(root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| ApplicationError::BoardLockIo {
                path: path.clone(),
                source,
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(ApplicationError::BoardInUse(root.to_path_buf()))
            }
            Err(std::fs::TryLockError::Error(source)) => {
                Err(ApplicationError::BoardLockIo { path, source })
            }
        }
    }
}

fn lock_path(root: &Path) -> Result<PathBuf, ApplicationError> {
    let parent = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| ApplicationError::MissingBoardRoot(root.to_path_buf()))?;
    let name = root
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ApplicationError::MissingBoardRoot(root.to_path_buf()))?;
    let mut lock_name = OsString::from(".");
    lock_name.push(name);
    lock_name.push(".spitfire-ng.lock");
    Ok(parent.join(lock_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_operation_lock_excludes_a_second_owner() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        std::fs::create_dir(&root).unwrap();
        let first = BoardOperationLock::acquire(&root).unwrap();
        assert!(matches!(
            BoardOperationLock::acquire(&root),
            Err(ApplicationError::BoardInUse(path)) if path == root
        ));
        drop(first);
        BoardOperationLock::acquire(&root).unwrap();
    }
}
