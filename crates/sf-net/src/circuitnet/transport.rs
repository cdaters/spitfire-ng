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

//! CircuitNET framing and negotiation, independent of socket and native storage.
use super::{Batch, NetworkId, NodeId, Receipt, Role, MAX_ARTIFACT};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const PROTOCOL: &str = "CIRCUITNET-NG";
pub const ALPN: &[u8] = b"circuitnet-ng/1";
pub const MAX_FRAME: usize = MAX_ARTIFACT + 4096;
pub const CONTROL_FRAME: usize = 4096;
pub const CAPABILITIES: [&str; 7] = [
    "atomic-batch",
    "symmetric-poll",
    "directed-routing",
    "remote-dossier-control",
    "file-distribution",
    "file-hash-have",
    "catalog-sync",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    #[error("connect")]
    Connect,
    #[error("tls")]
    Tls,
    #[error("timeout")]
    Timeout,
    #[error("auth-failed")]
    AuthFailed,
    #[error("unknown-node")]
    UnknownNode,
    #[error("wrong-network")]
    WrongNetwork,
    #[error("topology-mismatch")]
    TopologyMismatch,
    #[error("unsupported-version")]
    UnsupportedVersion,
    #[error("malformed-frame")]
    MalformedFrame,
    #[error("oversized")]
    Oversized,
    #[error("conflicting-message")]
    ConflictingMessage,
    #[error("unauthorized-codename")]
    UnauthorizedCodename,
    #[error("custody")]
    Custody,
    #[error("held")]
    Held,
    #[error("busy")]
    Busy,
    #[error("interrupted")]
    Interrupted,
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => Self::Timeout,
            _ => Self::Interrupted,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Test,
    Poll,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub protocol: String,
    pub major: u16,
    pub minimum_minor: u16,
    pub maximum_minor: u16,
    pub capabilities: Vec<String>,
    pub network: NetworkId,
    pub node: NodeId,
    pub role: Role,
    pub mode: Mode,
}
impl Hello {
    pub fn new(network: NetworkId, node: NodeId, role: Role, mode: Mode) -> Self {
        Self {
            protocol: PROTOCOL.into(),
            major: 1,
            minimum_minor: 0,
            maximum_minor: 4,
            capabilities: CAPABILITIES.iter().map(|s| (*s).into()).collect(),
            network,
            node,
            role,
            mode,
        }
    }
    /// The two bounded C4 extensions; baseline negotiation remains independent.
    pub fn c4_capabilities(&self, remote: &Self) -> Result<(bool, bool), Error> {
        let minor = self.negotiate(remote)?;
        let has = |name: &str| {
            minor >= 2
                && self.capabilities.iter().any(|c| c == name)
                && remote.capabilities.iter().any(|c| c == name)
        };
        Ok((has("directed-routing"), has("remote-dossier-control")))
    }
    pub fn file_capabilities(&self, remote: &Self) -> Result<(bool, bool), Error> {
        let minor = self.negotiate(remote)?;
        let has = |name: &str| {
            minor >= 3
                && self.capabilities.iter().any(|c| c == name)
                && remote.capabilities.iter().any(|c| c == name)
        };
        let files = has("file-distribution");
        Ok((files, files && has("file-hash-have")))
    }
    pub fn catalog_capability(&self, remote: &Self) -> Result<bool, Error> {
        Ok(self.negotiate(remote)? >= 4
            && self.capabilities.iter().any(|s| s == "catalog-sync")
            && remote.capabilities.iter().any(|s| s == "catalog-sync"))
    }
    pub fn negotiate(&self, remote: &Self) -> Result<u16, Error> {
        if remote.protocol != PROTOCOL
            || remote.major != 1
            || remote.minimum_minor > remote.maximum_minor
            || self.minimum_minor.max(remote.minimum_minor)
                > self.maximum_minor.min(remote.maximum_minor)
            || remote.capabilities.len() > 8
            || remote.capabilities.iter().any(|s| s.len() > 32)
            || CAPABILITIES[..2]
                .iter()
                .any(|c| !remote.capabilities.iter().any(|r| r == c))
        {
            return Err(Error::UnsupportedVersion);
        }
        if self.network != remote.network {
            return Err(Error::WrongNetwork);
        }
        if self.mode != remote.mode {
            return Err(Error::MalformedFrame);
        }
        Ok(self.maximum_minor.min(remote.maximum_minor))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Frame {
    Hello {
        hello: Hello,
    },
    Offer {
        batch: Option<Batch>,
    },
    Ack {
        receipt: Receipt,
        imported: u32,
        duplicates: u32,
    },
    Error {
        code: Error,
    },
    Controls {
        requests: Vec<super::control::Request>,
    },
    ControlResults {
        results: Vec<super::control::SubscriptionResult>,
    },
    FileOffer {
        publication: Option<super::files::Publication>,
    },
    FileWant {
        want: super::files::Want,
    },
    FileReceipt {
        receipt: super::files::Receipt,
    },
    CatalogHead {
        revision: u64,
        hash: Option<String>,
    },
    CatalogRequest {
        revision: u64,
        hash: Option<String>,
        enabled: bool,
    },
    CatalogObject {
        catalog: Option<super::catalog::Signed>,
    },
    CatalogAck {
        revision: u64,
        hash: Option<String>,
    },
    Close {},
}
pub fn read(reader: &mut impl Read, limit: usize) -> Result<Frame, Error> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(Error::MalformedFrame);
    }
    if length > limit.min(MAX_FRAME) {
        return Err(Error::Oversized);
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    let frame: Frame = serde_json::from_slice(&bytes).map_err(|_| Error::MalformedFrame)?;
    if let Frame::Error { code } = frame {
        return Err(code);
    }
    Ok(frame)
}
pub fn write(writer: &mut impl Write, frame: &Frame) -> Result<usize, Error> {
    let bytes = serde_json::to_vec(frame).map_err(|_| Error::MalformedFrame)?;
    if bytes.len() > MAX_FRAME {
        return Err(Error::Oversized);
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(bytes.len() + 4)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn hello() -> Hello {
        Hello::new(
            NetworkId::new("test").unwrap(),
            NodeId::new("END1").unwrap(),
            Role::End,
            Mode::Poll,
        )
    }
    #[test]
    fn versions_negotiate_only_common_known_minors() {
        let h = hello();
        let mut r = h.clone();
        assert_eq!(h.negotiate(&r), Ok(4));
        r.maximum_minor = 0;
        assert_eq!(h.negotiate(&r), Ok(0));
        r.major = 2;
        assert_eq!(h.negotiate(&r), Err(Error::UnsupportedVersion));
        r = h.clone();
        r.minimum_minor = 5;
        r.maximum_minor = 5;
        assert!(h.negotiate(&r).is_err());
        r = h.clone();
        r.network = NetworkId::new("wrong").unwrap();
        assert_eq!(h.negotiate(&r), Err(Error::WrongNetwork));
        r = h.clone();
        r.capabilities.clear();
        assert!(h.negotiate(&r).is_err());
    }
    #[test]
    fn c4_capabilities_are_optional_independent_and_minor_gated() {
        let h = hello();
        let mut r = h.clone();
        assert_eq!(h.c4_capabilities(&r), Ok((true, true)));
        r.capabilities.retain(|c| c != "remote-dossier-control");
        assert_eq!(h.c4_capabilities(&r), Ok((true, false)));
        r.maximum_minor = 1;
        assert_eq!(h.c4_capabilities(&r), Ok((false, false)));
        r.maximum_minor = 2;
        r.capabilities.retain(|c| c != "directed-routing");
        assert_eq!(h.negotiate(&r), Ok(2));
        assert_eq!(h.c4_capabilities(&r), Ok((false, false)));
        r.capabilities.push("future-feature".into());
        assert_eq!(h.negotiate(&r), Ok(2));
    }
    #[test]
    fn c6_file_capabilities_do_not_change_older_peers_or_claim_resume() {
        let h = hello();
        let mut r = h.clone();
        assert_eq!(h.file_capabilities(&r), Ok((true, true)));
        assert!(!h.capabilities.iter().any(|c| c == "file-resume"));
        for minor in 0..=2 {
            r.maximum_minor = minor;
            assert_eq!(h.file_capabilities(&r), Ok((false, false)));
            assert_eq!(h.negotiate(&r), Ok(minor));
        }
        r = h.clone();
        r.capabilities.retain(|c| c != "file-hash-have");
        assert_eq!(h.file_capabilities(&r), Ok((true, false)));
        r.capabilities.retain(|c| c != "file-distribution");
        assert_eq!(h.file_capabilities(&r), Ok((false, false)));
    }
    #[test]
    fn framing_roundtrip_and_hostile_bounds() {
        let mut bytes = Vec::new();
        write(&mut bytes, &Frame::Hello { hello: hello() }).unwrap();
        assert!(matches!(
            read(&mut bytes.as_slice(), CONTROL_FRAME),
            Ok(Frame::Hello { .. })
        ));
        for length in [0, MAX_FRAME as u32 + 1, u32::MAX] {
            assert!(read(&mut length.to_be_bytes().as_slice(), MAX_FRAME).is_err());
        }
        assert!(read(&mut [0, 0, 0, 2, b'{', b'!'].as_slice(), MAX_FRAME).is_err());
        assert!(read(&mut [0, 0, 0, 2, 255, 255].as_slice(), MAX_FRAME).is_err());
        assert!(read(&mut &bytes[..bytes.len() - 1], MAX_FRAME).is_err());
        let hostile = br#"{"type":"close","secret":"x"}"#;
        let mut input = (hostile.len() as u32).to_be_bytes().to_vec();
        input.extend(hostile);
        assert!(read(&mut input.as_slice(), MAX_FRAME).is_err());
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn catalog_capability_requires_both_peers_and_minor_four() {
        let h = Hello::new(
            NetworkId::new("synthetic").unwrap(),
            NodeId::new("END1").unwrap(),
            Role::End,
            Mode::Poll,
        );
        let mut remote = h.clone();
        assert!(h.catalog_capability(&remote).unwrap());
        for minor in 0..4 {
            remote.maximum_minor = minor;
            assert!(!h.catalog_capability(&remote).unwrap());
            assert_eq!(h.negotiate(&remote).unwrap(), minor);
        }
        remote = h.clone();
        remote.capabilities.retain(|c| c != "catalog-sync");
        assert!(!h.catalog_capability(&remote).unwrap());
        assert_eq!(h.file_capabilities(&remote).unwrap(), (true, true));
        assert_eq!(h.c4_capabilities(&remote).unwrap(), (true, true));
    }
}
