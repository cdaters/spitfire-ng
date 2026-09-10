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

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rusqlite::{types::ValueRef, Connection, OpenFlags};
use sf_core::{LogicalPath, LogicalPaths, RuntimeConfig, RuntimeDatabase, TerminalInfo};
use sha2::{Digest, Sha256};

use super::io;
use super::model::*;
use super::{ensure, Error, Result};

pub trait Runner {
    fn describe(&self, executable: &Path) -> Result<RuntimeDescriptor>;
    fn worker(&self, executable: &Path, request: &WorkerRequest) -> Result<BoardState>;
}
pub struct ProcessRunner;
impl Runner for ProcessRunner {
    fn describe(&self, executable: &Path) -> Result<RuntimeDescriptor> {
        Ok(serde_json::from_slice(&invoke(
            executable,
            "deployment-describe",
            None,
        )?)?)
    }
    fn worker(&self, executable: &Path, request: &WorkerRequest) -> Result<BoardState> {
        Ok(serde_json::from_slice(&invoke(
            executable,
            "deployment-worker",
            Some(&serde_json::to_vec(request)?),
        )?)?)
    }
}
fn invoke(executable: &Path, operation: &str, request: Option<&[u8]>) -> Result<Vec<u8>> {
    io::real(executable, false)?;
    let stdout = tempfile::NamedTempFile::new()?;
    let stderr = tempfile::NamedTempFile::new()?;
    let mut child = Command::new(executable)
        .arg(operation)
        .stdin(Stdio::piped())
        .stdout(stdout.reopen()?)
        .stderr(stderr.reopen()?)
        .spawn()?;
    if let Some(bytes) = request {
        let written = child
            .stdin
            .take()
            .ok_or_else(|| Error::Rejected("worker input unavailable".into()))?
            .write_all(bytes);
        if let Err(error) = written {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    } else {
        drop(child.stdin.take());
    }
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if let Some(status) = child.try_wait()? {
            ensure(
                status.success(),
                &format!(
                    "target runtime {operation} failed (exit {:?}); update cannot commit",
                    status.code()
                ),
            )?;
            return io::read(stdout.path(), MAX_DOCUMENT);
        }
        if Instant::now() >= deadline
            || stdout.as_file().metadata()?.len() > MAX_DOCUMENT
            || stderr.as_file().metadata()?.len() > MAX_DOCUMENT
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Rejected(
                "runtime worker exceeded time/output limits".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn from_stdin() -> Result<String> {
    let mut bytes = vec![];
    std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    ensure(bytes.len() <= 65536, "worker request too large")?;
    let request: WorkerRequest = serde_json::from_slice(&bytes)?;
    Ok(serde_json::to_string(&execute(&request)?)?)
}
pub fn execute(request: &WorkerRequest) -> Result<BoardState> {
    ensure(
        request.format == FORMAT && matches!(request.operation.as_str(), "inspect" | "migrate"),
        "unsupported offline worker request",
    )?;
    let config_path = io::real(&request.config, false)?;
    let root = config_path
        .parent()
        .ok_or_else(|| Error::Rejected("missing board root".into()))?;
    if request.operation == "migrate" {
        // This worker can mutate only an unadopted staged board. A real board
        // locator can never authorize an offline migration worker to bypass its
        // operation lock. The manager never hands live paths to this operation.
        ensure(
            !root.join(LOCATOR).exists(),
            "migration worker refuses a live managed board",
        )?;
        let _lock =
            crate::board_lock::BoardOperationLock::acquire(root).map_err(Error::application)?;
        migrate_config(&config_path)?;
        let config = RuntimeConfig::load(&config_path)?;
        let paths = LogicalPaths::resolve(root, &config.validate()?)?;
        let mut db = RuntimeDatabase::open(paths.database())?;
        db.migrate()?;
    }
    inspect(&config_path, &request.payload_root, true)
}

fn migrate_config(path: &Path) -> Result<()> {
    let bytes = io::read(path, MAX_DOCUMENT)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::Rejected("configuration is not UTF-8 TOML".into()))?;
    let mut document: toml::Value = toml::from_str(text)?;
    let version = document
        .get("format_version")
        .and_then(toml::Value::as_integer);
    if version == Some(1) {
        let table = document
            .as_table_mut()
            .ok_or_else(|| Error::Rejected("invalid configuration document".into()))?;
        table.insert(
            "format_version".into(),
            toml::Value::Integer(sf_core::CONFIG_FORMAT_VERSION.into()),
        );
        let output = toml::to_string_pretty(&document)?;
        let candidate: RuntimeConfig = toml::from_str(&output)?;
        candidate.validate()?;
        let parent = path
            .parent()
            .ok_or_else(|| Error::Rejected("missing configuration parent".into()))?;
        let temporary = tempfile::NamedTempFile::new_in(parent)?;
        std::fs::write(temporary.path(), output)?;
        io::replace(temporary.path(), path)?;
    } else {
        ensure(
            version == Some(sf_core::CONFIG_FORMAT_VERSION.into()),
            "unsupported configuration migration",
        )?;
    }
    Ok(())
}

pub fn paths(config: &Path) -> Result<(RuntimeConfig, LogicalPaths)> {
    let config_path = io::real(config, false)?;
    let cfg = RuntimeConfig::load(&config_path)?;
    let root = config_path
        .parent()
        .ok_or_else(|| Error::Rejected("missing board root".into()))?;
    let paths = LogicalPaths::resolve(root, &cfg.validate()?)?;
    crate::backup::validate_native_layout(&cfg.validate()?, &paths)?;
    crate::backup::validate_real_logical_directories(&paths)?;
    Ok((cfg, paths))
}

pub fn inspect(
    config_path: &Path,
    payload_root: &Path,
    require_current: bool,
) -> Result<BoardState> {
    let (config, paths) = paths(config_path)?;
    clean_journals(paths.database())?;
    let db = RuntimeDatabase::open_read_only(paths.database())?;
    let schema = db.schema_version()?;
    if require_current {
        ensure(
            schema == sf_core::SCHEMA_VERSION,
            "offline validation did not reach the runtime target schema",
        )?;
    }
    let identity = db.validate_snapshot_at_version(schema)?;
    ensure(
        identity == config.validate()?.identity,
        "board configuration/database identity mismatch",
    )?;
    ensure(
        db.file_operations_ready_for_cold_backup()?,
        "nonterminal file operation prevents deployment",
    )?;
    ensure(
        db.transfer_operations_ready_for_cold_backup()?,
        "active file transfer prevents deployment",
    )?;
    crate::DiskArtifactStore::validate_custody(paths.get(LogicalPath::System), &db)
        .map_err(|_| Error::Rejected("network artifact custody is inconsistent".into()))?;
    // Network settings are parsed by the same public domain readers as normal
    // operation, but no network session, recovery write or Event executor runs.
    for network in db
        .circuitnet_profiles()
        .map_err(|_| Error::Rejected("CircuitNET configuration cannot be read".into()))?
    {
        db.circuitnet_status(&network)
            .map_err(|_| Error::Rejected("CircuitNET durable state cannot be read".into()))?;
    }
    db.qwk_network_status()
        .map_err(|_| Error::Rejected("QWK state cannot be read".into()))?;
    db.ftn_status()
        .map_err(|_| Error::Rejected("FTN state cannot be read".into()))?;
    db.events()
        .map_err(|_| Error::Rejected("Events state cannot be read".into()))?;
    db.ftn_recovery_evidence()
        .map_err(|_| Error::Rejected("FTN custody state cannot be read".into()))?;
    let presentation = crate::PresentationResolver::load(&paths, &config.presentation);
    crate::resources::load_stock_resources(&paths, &TerminalInfo::in_memory(), &presentation)
        .map_err(|_| Error::Rejected("board startup resources cannot be loaded".into()))?;
    // Content readers resolve against the stopped source library during staged
    // validation. No payload is rewritten or reconstructed by the worker.
    let payload_paths = LogicalPaths::resolve(&io::real(payload_root, true)?, &config.validate()?)?;
    let storage = sf_core::FileStorage::open_existing(&payload_paths)?;
    for (area, file) in db.managed_cataloged_files()? {
        if schema >= 33
            && db
                .file_admission(file.id)
                .map_err(|_| Error::Rejected("invalid Files admission".into()))?
                .is_some()
        {
            continue;
        }
        storage.open_download(&area, &file)?;
    }
    if schema >= 33 {
        for (hash, size) in db
            .content_catalog()
            .map_err(|_| Error::Rejected("invalid content catalog".into()))?
        {
            storage.open_content(&hash, size)?;
        }
    }
    let (counts, identities) = database_fingerprints(paths.database())?;
    Ok(BoardState {
        schema,
        config_format: config.format_version,
        board_name: identity.name().into(),
        sysop_name: identity.sysop_name().into(),
        required_features: durable_features(),
        counts,
        identities,
    })
}

pub fn clean_journals(database: &Path) -> Result<()> {
    io::real(database, false)?;
    for suffix in ["-wal", "-journal"] {
        let mut name = database.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        ensure(
            !path.exists()
                || (io::real(&path, false).is_ok() && std::fs::metadata(path)?.len() == 0),
            "database has a pending WAL/journal; finish clean shutdown before deployment",
        )?;
    }
    Ok(())
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
pub fn database_fingerprints(
    path: &Path,
) -> Result<(BTreeMap<String, u64>, BTreeMap<String, String>)> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let tables: Vec<String> = connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get(0))?.collect::<std::result::Result<_,_>>()?;
    ensure(tables.len() <= 1024, "excessive database schema inventory")?;
    let mut counts = BTreeMap::new();
    let mut identities = BTreeMap::new();
    for table in tables {
        let count: i64 = connection.query_row(
            &format!("SELECT COUNT(*) FROM {}", quote(&table)),
            [],
            |r| r.get(0),
        )?;
        counts.insert(
            table.clone(),
            u64::try_from(count).map_err(|_| Error::Rejected("invalid row count".into()))?,
        );
        if table == "schema_migrations" {
            continue;
        }
        let mut stmt = connection.prepare(&format!("SELECT * FROM {}", quote(&table)))?;
        let cols: Vec<String> = stmt.column_names().iter().map(|name| quote(name)).collect();
        let mut primary: Vec<(i64, String)> = connection
            .prepare(&format!("PRAGMA table_info({})", quote(&table)))?
            .query_map([], |r| Ok((r.get(5)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        primary.retain(|(position, _)| *position > 0);
        primary.sort_by_key(|(position, _)| *position);
        let order = if primary.is_empty() {
            cols.join(",")
        } else {
            primary
                .iter()
                .map(|(_, name)| quote(name))
                .collect::<Vec<_>>()
                .join(",")
        };
        let sql = format!("SELECT * FROM {} ORDER BY {}", quote(&table), order);
        stmt = connection.prepare(&sql)?;
        let n = stmt.column_count();
        let mut rows = stmt.query([])?;
        let mut hasher = Sha256::new();
        while let Some(row) = rows.next()? {
            hasher.update(b"row\0");
            for index in 0..n {
                match row.get_ref(index)? {
                    ValueRef::Null => hasher.update(b"null\0"),
                    ValueRef::Integer(v) => {
                        hasher.update(b"int\0");
                        hasher.update(v.to_le_bytes());
                    }
                    ValueRef::Real(v) => {
                        hasher.update(b"real\0");
                        hasher.update(v.to_bits().to_le_bytes());
                    }
                    ValueRef::Text(v) | ValueRef::Blob(v) => {
                        hasher.update(if matches!(row.get_ref(index)?, ValueRef::Text(_)) {
                            b"text\0"
                        } else {
                            b"blob\0"
                        });
                        hasher.update((v.len() as u64).to_le_bytes());
                        hasher.update(v);
                    }
                }
            }
        }
        identities.insert(table, format!("{:x}", hasher.finalize()));
    }
    Ok((counts, identities))
}

pub fn preserved(before: &BoardState, after: &BoardState) -> Result<()> {
    ensure(
        before.board_name == after.board_name && before.sysop_name == after.sysop_name,
        "upgrade changed board identity",
    )?;
    for (table, hash) in &before.identities {
        ensure(
            after.identities.get(table) == Some(hash),
            &format!("upgrade did not preserve durable state in {table}"),
        )?;
    }
    Ok(())
}
