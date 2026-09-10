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
use std::fs;
use std::path::Path;

use ring::signature::{UnparsedPublicKey, ED25519};

use super::io;
use super::model::*;
use super::worker::Runner;
use super::{ensure, Result};

pub fn binary_name() -> &'static str {
    if cfg!(windows) {
        "spitfire.exe"
    } else {
        "spitfire"
    }
}

pub fn verify(root: &Path, pin: &str) -> Result<ReleaseRecord> {
    io::real(root, true)?;
    let bytes = io::read(&root.join("release.json"), MAX_DOCUMENT)?;
    let signature = io::read(&root.join("release.sig"), 129)?;
    let signature = std::str::from_utf8(&signature)
        .map_err(|_| super::Error::Rejected("invalid release signature encoding".into()))?
        .trim();
    let signature = io::unhex(signature, 64)?;
    let pin = io::unhex(pin, 32)?;
    let mut signed = RELEASE_DOMAIN.to_vec();
    signed.extend_from_slice(&bytes);
    ensure(
        UnparsedPublicKey::new(&ED25519, pin)
            .verify(&signed, &signature)
            .is_ok(),
        "release signature does not match locally pinned SPITFIRE release authority",
    )?;
    let manifest: ReleaseManifest = serde_json::from_slice(&bytes)?;
    ensure(
        manifest.format == FORMAT,
        "unsupported release manifest format",
    )?;
    manifest.runtime.validate()?;
    version(&manifest.minimum_upgrade_version)?;
    ensure(
        manifest.executable == binary_name() && manifest.size_bytes > 0,
        "unsupported release runtime inventory",
    )?;
    io::unhex(&manifest.sha256, 32)?;
    let actual: BTreeSet<_> = io::inventory(root)?.into_iter().map(|(p, _)| p).collect();
    ensure(
        actual
            == BTreeSet::from([
                "release.json".into(),
                "release.sig".into(),
                binary_name().into(),
            ]),
        "release has missing or undeclared files",
    )?;
    ensure(
        io::hash(&root.join(binary_name()))? == (manifest.size_bytes, manifest.sha256.clone()),
        "release executable checksum/length mismatch",
    )?;
    Ok(ReleaseRecord {
        manifest,
        manifest_sha256: io::digest(&bytes),
    })
}

pub fn discover(
    source: &Path,
    pin: &str,
    current: &str,
) -> Result<Option<(std::path::PathBuf, ReleaseRecord)>> {
    io::real(source, true)?;
    let mut choices = Vec::new();
    let mut seen = BTreeSet::new();
    for item in fs::read_dir(source)? {
        ensure(choices.len() < 128, "release source exceeds package limit")?;
        let path = item?.path();
        let record = verify(&path, pin)?;
        let candidate = version(&record.manifest.runtime.version)?;
        ensure(
            seen.insert(candidate.clone()),
            "duplicate release version in source",
        )?;
        choices.push((candidate, path, record));
    }
    choices.sort_by(|a, b| a.0.cmp(&b.0));
    let current = version(current)?;
    Ok(choices
        .into_iter()
        .rev()
        .find(|(v, _, _)| v > &current)
        .map(|(_, p, r)| (p, r)))
}
pub fn compatible_upgrade(
    old: &RuntimeDescriptor,
    new: &ReleaseRecord,
    state: &BoardState,
) -> Result<()> {
    let candidate = &new.manifest.runtime;
    ensure(
        version(&candidate.version)? > version(&old.version)?,
        "release is not newer than the active runtime",
    )?;
    ensure(
        version(&old.version)? >= version(&new.manifest.minimum_upgrade_version)?,
        "source runtime is below release minimum upgrade version",
    )?;
    ensure(
        candidate.migration_source.contains(state.schema)
            && candidate.target_schema >= state.schema,
        "release does not support forward migration from current schema",
    )?;
    ensure(
        candidate.config_formats.contains(&state.config_format),
        "release does not support source configuration",
    )?;
    for (name, required) in &state.required_features {
        ensure(
            candidate
                .durable_features
                .get(name)
                .is_some_and(|v| v >= required),
            "release cannot preserve current durable feature generation",
        )?;
    }
    Ok(())
}
pub fn probe(root: &Path, record: &ReleaseRecord, runner: &dyn Runner) -> Result<()> {
    let actual = runner.describe(&root.join(binary_name()))?;
    ensure(
        actual == record.manifest.runtime,
        "runtime descriptor does not match authenticated release manifest",
    )
}
pub fn stage(source: &Path, destination: &Path, pin: &str, expected: &ReleaseRecord) -> Result<()> {
    if destination.exists() {
        ensure(
            verify(destination, pin)? == *expected,
            "existing versioned runtime differs from selected release",
        )?;
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| super::Error::Rejected("missing releases parent".into()))?;
    let temp = tempfile::Builder::new()
        .prefix(".runtime-")
        .tempdir_in(parent)?;
    for name in ["release.json", "release.sig", binary_name()] {
        io::copy(&source.join(name), &temp.path().join(name))?;
    }
    io::executable(&temp.path().join(binary_name()))?;
    ensure(
        verify(temp.path(), pin)? == *expected,
        "staged release differs from verified source",
    )?;
    io::sync_dir(temp.path())?;
    let staging = temp.keep();
    fs::rename(&staging, destination)?;
    io::sync_dir(parent)
}
