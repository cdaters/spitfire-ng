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

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::io;
use super::model::*;
use super::{ensure, Error, Result};

pub struct InstallLock {
    _file: File,
}
impl InstallLock {
    pub fn acquire(root: &Path) -> Result<Self> {
        let path = root.join("maintenance.lock");
        io::real(&path, false)?;
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        file.try_lock()
            .map_err(|_| Error::Rejected("another deployment operation is active".into()))?;
        Ok(Self { _file: file })
    }
}
pub fn load(root: &Path) -> Result<(Installation, Control)> {
    io::real(root, true)?;
    let policy = io::read(&root.join("installation.toml"), MAX_DOCUMENT)?;
    let policy = std::str::from_utf8(&policy)
        .map_err(|_| Error::Rejected("installation policy is not UTF-8".into()))?;
    let installation: Installation = toml::from_str(policy)?;
    ensure(
        installation.format == FORMAT,
        "unsupported installation policy format",
    )?;
    io::safe_id(&installation.id)?;
    installation.storage.validate()?;
    io::unhex(&installation.release_public_key, 32)?;
    let dbpath = root.join("deployment.sqlite3");
    io::real(&dbpath, false)?;
    let db = Connection::open_with_flags(dbpath, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let checked: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    ensure(checked == "ok", "deployment journal failed integrity check")?;
    let value: String = db.query_row(
        "SELECT document FROM deployment WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    ensure(
        value.len() as u64 <= MAX_DOCUMENT,
        "deployment state exceeds size limit",
    )?;
    let state: Control = serde_json::from_str(&value)?;
    validate(&installation, &state)?;
    Ok((installation, state))
}
fn validate(installation: &Installation, state: &Control) -> Result<()> {
    ensure(
        state.format == FORMAT && state.installation_id == installation.id,
        "installation/journal identity mismatch",
    )?;
    ensure(
        state.releases.contains_key(&state.active)
            && state.releases.len() <= 128
            && state.previous.len() <= 128
            && state.backups.len() <= 10000,
        "invalid runtime/backup history",
    )?;
    for (version, release) in &state.releases {
        ensure(
            version == &release.manifest.runtime.version,
            "release registry version mismatch",
        )?;
        super::model::version(version)?;
        io::unhex(&release.manifest_sha256, 32)?;
    }
    for (id, hash) in &state.backups {
        io::safe_id(id)?;
        io::unhex(hash, 32)?;
    }
    for old in &state.previous {
        ensure(
            state.releases.contains_key(old),
            "rollback history references missing release",
        )?;
    }
    if let Some(tx) = &state.transaction {
        io::safe_id(&tx.id)?;
        ensure(
            state.releases.contains_key(&tx.old),
            "update journal has no old runtime",
        )?;
        super::model::version(&tx.target)?;
        if matches!(tx.phase, Phase::Commit | Phase::Cleanup) {
            ensure(
                state.active == tx.target,
                "committed update pointer is inconsistent",
            )?;
        } else {
            ensure(
                state.active == tx.old,
                "pre-commit active runtime is inconsistent",
            )?;
        }
        if let Some(id) = &tx.checkpoint {
            io::safe_id(id)?;
        }
        if let Some(hash) = &tx.checkpoint_sha256 {
            io::unhex(hash, 32)?;
        }
        if let Some(hash) = &tx.stage_sha256 {
            io::unhex(hash, 32)?;
        }
        if matches!(tx.phase, Phase::ApplyRuntime | Phase::ValidateActive) {
            ensure(
                tx.checkpoint.is_some()
                    && tx.checkpoint_sha256.is_some()
                    && tx.stage_sha256.is_some(),
                "activation journal is missing verified checkpoint/stage evidence",
            )?;
        }
    }
    Ok(())
}
pub fn initialize(root: &Path, state: &Control) -> Result<()> {
    let path = root.join("deployment.sqlite3");
    ensure(!path.exists(), "deployment journal already exists")?;
    io::write_new(&path, b"")?;
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE deployment(singleton INTEGER PRIMARY KEY CHECK(singleton=1),document TEXT NOT NULL);")?;
    db.execute(
        "INSERT INTO deployment VALUES(1,?1)",
        [serde_json::to_string(state)?],
    )?;
    drop(db);
    io::sync_dir(root)
}
pub fn save(root: &Path, state: &Control) -> Result<()> {
    let path = root.join("deployment.sqlite3");
    io::real(&path, false)?;
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.execute_batch("PRAGMA synchronous=FULL;")?;
    let encoded = serde_json::to_string(state)?;
    ensure(
        encoded.len() as u64 <= MAX_DOCUMENT,
        "deployment history exceeds size limit",
    )?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    ensure(
        tx.execute(
            "UPDATE deployment SET document=?1 WHERE singleton=1",
            [encoded],
        )? == 1,
        "deployment journal row missing",
    )?;
    tx.commit()?;
    drop(db);
    io::sync_dir(root)
}

pub fn locator(board: &Path) -> Result<Option<Locator>> {
    let path = board.join(LOCATOR);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = io::read(&path, 65536)?;
    let value: Locator = toml::from_str(
        std::str::from_utf8(&bytes)
            .map_err(|_| Error::Rejected("invalid installation locator".into()))?,
    )?;
    ensure(value.format == FORMAT, "unsupported installation locator")?;
    io::safe_id(&value.id)?;
    Ok(Some(value))
}
pub fn write_locator(board: &Path, root: &Path, id: &str) -> Result<()> {
    let locator = Locator {
        format: FORMAT,
        id: id.into(),
        installation: root.to_path_buf(),
    };
    io::write_new(
        &board.join(LOCATOR),
        toml::to_string_pretty(&locator)?.as_bytes(),
    )
}
pub fn check_locator(root: &Path, installation: &Installation) -> Result<()> {
    let board = installation
        .config
        .parent()
        .ok_or_else(|| Error::Rejected("invalid configured board".into()))?;
    let locator = locator(board)?.ok_or_else(|| {
        Error::Rejected(
            "managed board locator is missing; installation adoption/recovery is required".into(),
        )
    })?;
    ensure(
        locator.id == installation.id
            && io::real(&locator.installation, true)? == io::real(root, true)?,
        "board belongs to a different installation",
    )
}
pub fn release_root(root: &Path, version: &str) -> Result<PathBuf> {
    super::model::version(version)?;
    Ok(root.join("releases").join(version))
}

pub fn preserve_locator(target: &Path, stage: &Path) -> Result<()> {
    if target.join(LOCATOR).exists() {
        locator(target)?;
        io::copy(&target.join(LOCATOR), &stage.join(LOCATOR))?;
        io::sync_dir(stage)?;
    }
    Ok(())
}
