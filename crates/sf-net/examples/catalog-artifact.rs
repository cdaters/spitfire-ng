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

//! Offline artifact utility for kit reproducibility; no board or network operations.
use sf_net::circuitnet::catalog::{Authority, Body, Signed, MAX_BYTES};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
fn read(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_BYTES as u64
    {
        return Err("invalid artifact file".into());
    }
    let mut bytes = vec![];
    fs::File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err("oversized artifact".into());
    }
    Ok(bytes)
}
fn write(path: &str, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["key", output] => {
            let key = Signed::generate_key()?;
            write(output, &key)?;
            println!("{}", Signed::public_key(&key)?);
        }
        ["sign", body, key, output] => {
            let body: Body = serde_json::from_slice(&read(body)?)?;
            write(output, &Signed::sign(body, &read(key)?)?.encode()?)?;
        }
        ["validate", catalog, authority] => {
            let s = Signed::decode(&read(catalog)?)?;
            let a: Authority = serde_json::from_slice(&read(authority)?)?;
            s.verify(&a)?;
            println!("verified revision {} hash {}", s.body.revision, s.hash);
        }
        _ => {
            return Err(
                "catalog-artifact: key OUT | sign BODY KEY OUT | validate CATALOG AUTHORITY".into(),
            )
        }
    }
    Ok(())
}
