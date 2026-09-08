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

//! CircuitNET-native file publications. Metadata is separate from streamed bytes.
use super::{Codename, Error, MessageId, NetworkId, NodeId, MAX_NODES};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
pub const CHUNK: usize = 65536;
pub const MAX_FILE: u64 = 64 * 1024 * 1024;
pub const MAX_PUBLICATIONS: usize = 8;
pub const METADATA_FRAME: usize = 16384;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub network: NetworkId,
    /// Origin-scoped random publication identity, separate from content identity.
    pub id: MessageId,
    pub origin: NodeId,
    pub codename: Codename,
    pub sha256: String,
    pub size: u64,
    pub filename: String,
    pub description: String,
    pub timestamp: i64,
    pub path: Vec<NodeId>,
}
impl Publication {
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.origin() != self.origin
            || self.size > MAX_FILE
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.filename.is_empty()
            || self.filename.len() > 64
            || self.filename.starts_with('.')
            || !self
                .filename
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
            || self.description.len() > 4096
            || self.description.lines().count() > 20
            || self
                .description
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
            || self.path.is_empty()
            || self.path.len() > MAX_NODES
            || self.path.first() != Some(&self.origin)
            || self
                .path
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.path.len()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    /// Path changes per hop; publication metadata cannot change under one identity.
    pub fn fingerprint(&self) -> Result<String, Error> {
        self.validate()?;
        let mut stable = self.clone();
        stable.path = vec![self.origin.clone()];
        Ok(super::digest(&serde_json::to_vec(&stable)?))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Published,
    PendingApproval,
    Quarantined,
    Rejected,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub publication: MessageId,
    pub sha256: String,
    pub outcome: Outcome,
    /// Safe machine class, never local operator details or a filesystem path.
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Want {
    Send,
    Have,
    Complete { receipt: Receipt },
}

/// Payload phase only, after an authenticated negotiated Send decision.
/// A zero length ends the stream. No chunk is JSON or base64 encoded.
pub fn send(
    reader: &mut impl Read,
    writer: &mut impl Write,
    size: u64,
) -> Result<u64, super::transport::Error> {
    use super::transport::Error as E;
    if size > MAX_FILE {
        return Err(E::Oversized);
    }
    let mut buffer = [0; CHUNK];
    let mut sent = 0;
    while sent < size {
        let max = (size - sent).min(CHUNK as u64) as usize;
        let n = reader.read(&mut buffer[..max])?;
        if n == 0 {
            return Err(E::Custody);
        }
        writer.write_all(&(n as u32).to_be_bytes())?;
        writer.write_all(&buffer[..n])?;
        sent += n as u64;
    }
    writer.write_all(&0u32.to_be_bytes())?;
    writer.flush()?;
    Ok(sent)
}
pub fn receive(
    reader: &mut impl Read,
    writer: &mut impl Write,
    size: u64,
    hash: &str,
) -> Result<u64, super::transport::Error> {
    use super::transport::Error as E;
    use sha2::{Digest, Sha256};
    if size > MAX_FILE {
        return Err(E::Oversized);
    }
    let mut buffer = [0; CHUNK];
    let mut received = 0u64;
    let mut digest = Sha256::new();
    loop {
        let mut prefix = [0; 4];
        reader.read_exact(&mut prefix)?;
        let n = u32::from_be_bytes(prefix) as usize;
        if n == 0 {
            break;
        }
        if n > CHUNK || received + n as u64 > size {
            return Err(E::Oversized);
        }
        reader.read_exact(&mut buffer[..n])?;
        digest.update(&buffer[..n]);
        writer.write_all(&buffer[..n])?;
        received += n as u64;
    }
    if received != size || format!("{:x}", digest.finalize()) != hash {
        return Err(E::ConflictingMessage);
    }
    writer.flush()?;
    Ok(received)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn binary_chunks_are_bounded_and_final_integrity_is_required() {
        let bytes = vec![42u8; CHUNK * 2 + 5];
        let hash = super::super::digest(&bytes);
        let mut wire = Vec::new();
        assert_eq!(
            send(&mut Cursor::new(&bytes), &mut wire, bytes.len() as u64).unwrap(),
            bytes.len() as u64
        );
        let mut result = Vec::new();
        receive(
            &mut Cursor::new(&wire),
            &mut result,
            bytes.len() as u64,
            &hash,
        )
        .unwrap();
        assert_eq!(result, bytes);
        wire[4] ^= 1;
        assert!(receive(
            &mut Cursor::new(&wire),
            &mut Vec::new(),
            bytes.len() as u64,
            &hash
        )
        .is_err());
        assert!(receive(
            &mut Cursor::new(&(CHUNK as u32 + 1).to_be_bytes()),
            &mut Vec::new(),
            MAX_FILE,
            &hash
        )
        .is_err());
        assert!(receive(
            &mut Cursor::new(&wire[..10]),
            &mut Vec::new(),
            bytes.len() as u64,
            &hash
        )
        .is_err());
    }
}
