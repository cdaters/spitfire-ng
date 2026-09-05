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

//! Daemon-owned restricted immutable packet evidence and cold-recovery checks.
use sf_core::network::{NetworkArtifactStore, NetworkError};
use sf_net::qwk;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
/// Restricted immutable artifacts live under SYSTEM and participate in the
/// existing cold snapshot inventory. Remote names never reach this store.
#[derive(Clone, Debug)]
pub struct DiskArtifactStore {
    root: PathBuf,
    admission: std::sync::Arc<sf_core::network::ImportCapacity>,
}
impl DiskArtifactStore {
    pub fn new(system: &Path) -> Result<Self, NetworkError> {
        if fs::symlink_metadata(system)?.file_type().is_symlink() {
            return Err(NetworkError::Unavailable);
        }
        let root = system.join("network-artifacts");
        if !root.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root)?;
        }
        let meta = fs::symlink_metadata(&root)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(NetworkError::Unavailable);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(NetworkError::Unavailable);
            }
        }
        Ok(Self {
            root,
            admission: Default::default(),
        })
    }
    pub fn validate_custody(
        system: &Path,
        database: &sf_core::RuntimeDatabase,
    ) -> Result<(), NetworkError> {
        let root = system.join("network-artifacts");
        if !root.exists() && database.network_artifact_inventory()?.is_empty() {
            return Ok(());
        }
        Self {
            root,
            admission: Default::default(),
        }
        .validate(database)
    }
    pub fn validate(&self, database: &sf_core::RuntimeDatabase) -> Result<(), NetworkError> {
        self.usage()?;
        for (id, size, complete) in database.network_artifact_inventory()? {
            if !complete {
                return Err(NetworkError::Unavailable);
            }
            let path = self.root.join(&id);
            let meta = fs::symlink_metadata(&path)?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != size {
                return Err(NetworkError::Unavailable);
            }
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(qwk::MAX_ARCHIVE as u64 + 1)
                .read_to_end(&mut bytes)?;
            if qwk::digest(&bytes) != id {
                return Err(NetworkError::Unavailable);
            }
        }
        Ok(())
    }
    pub fn recover(&self, database: &mut sf_core::RuntimeDatabase) -> Result<(), NetworkError> {
        let inventory = database.network_artifact_inventory()?;
        for (id, _, complete) in inventory {
            if complete {
                continue;
            }
            let path = self.root.join(&id);
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                    fs::remove_file(&path)?
                }
                Ok(_) => return Err(NetworkError::Unavailable),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            database.forget_incomplete_network_artifact(&id)?;
        }
        self.validate(database)?;
        database.recover_offline_requests(chrono::Utc::now().timestamp())?;
        Ok(())
    }
    pub fn preserve(&self, bytes: &[u8]) -> Result<String, NetworkError> {
        if bytes.len() > qwk::MAX_ARCHIVE {
            return Err(NetworkError::Capacity);
        }
        let id = qwk::digest(bytes);
        let target = self.root.join(&id);
        if target.exists() {
            let meta = fs::symlink_metadata(&target)?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != bytes.len() as u64
            {
                return Err(NetworkError::Unavailable);
            }
            let mut saved = Vec::new();
            fs::File::open(&target)?
                .take(qwk::MAX_ARCHIVE as u64 + 1)
                .read_to_end(&mut saved)?;
            if saved != bytes {
                return Err(NetworkError::Unavailable);
            }
            return Ok(id);
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&target)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(id)
    }
}

impl NetworkArtifactStore for DiskArtifactStore {
    fn admit_import(&self) -> Result<sf_core::network::ImportPermit<'_>, NetworkError> {
        self.admission.acquire()
    }
    fn preserve(&self, bytes: &[u8]) -> Result<String, NetworkError> {
        DiskArtifactStore::preserve(self, bytes)
    }
    fn usage(&self) -> Result<(u64, usize), NetworkError> {
        let mut total = 0u64;
        let mut count = 0usize;
        for item in fs::read_dir(&self.root)? {
            count += 1;
            if count > 10000 {
                return Err(NetworkError::Capacity);
            }
            let item = item?;
            let metadata = item.path().symlink_metadata()?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(NetworkError::Unavailable);
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(NetworkError::Capacity)?;
        }
        Ok((total, count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_removes_only_journaled_incomplete_writes_and_keeps_custody() {
        let temp = tempfile::tempdir().unwrap();
        let mut db = sf_core::RuntimeDatabase::open(&temp.path().join("board.sqlite3")).unwrap();
        db.migrate().unwrap();
        let store = DiskArtifactStore::new(temp.path()).unwrap();
        let pending = qwk::digest(b"interrupted artifact");
        let complete = store.preserve(b"complete evidence").unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("board.sqlite3")).unwrap();
        conn.execute(
            "INSERT INTO network_artifacts VALUES(?1,20,1,'pending')",
            [&pending],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO network_artifacts VALUES(?1,17,1,'complete')",
            [&complete],
        )
        .unwrap();
        fs::write(store.root.join(&pending), b"partial").unwrap();
        fs::write(store.root.join("operator-owned-note"), b"keep this").unwrap();
        assert!(store.validate(&db).is_err());
        store.recover(&mut db).unwrap();
        assert!(!store.root.join(pending).exists());
        assert_eq!(
            fs::read(store.root.join("operator-owned-note")).unwrap(),
            b"keep this"
        );
        assert_eq!(
            db.network_artifact_inventory().unwrap(),
            vec![(complete.clone(), 17, true)]
        );
        store.recover(&mut db).unwrap();
        fs::write(store.root.join(complete), b"corrupted custody").unwrap();
        assert!(store.validate(&db).is_err());
    }
    #[test]
    fn board_import_admission_is_bounded_and_released_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let store = DiskArtifactStore::new(temp.path()).unwrap();
        let clone = store.clone();
        let first = store.admit_import().unwrap();
        let second = clone.admit_import().unwrap();
        assert!(store.admit_import().is_err());
        drop(first);
        let third = store.admit_import().unwrap();
        drop((second, third));
        store.admit_import().unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn private_modes_and_symlink_refusal() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let store = DiskArtifactStore::new(temp.path()).unwrap();
        let id = store.preserve(b"synthetic private packet").unwrap();
        assert_eq!(
            fs::metadata(&store.root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(store.root.join(id))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let other = qwk::digest(b"other");
        symlink(temp.path().join("outside"), store.root.join(other)).unwrap();
        assert!(store.preserve(b"other").is_err());
        assert!(store.usage().is_err());
        assert!(!temp.path().join("outside").exists());
    }
}
