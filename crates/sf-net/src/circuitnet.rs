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

//! Development/offline CircuitNET NG envelope; no legacy codec or I/O authority.
pub mod transport;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FORMAT: &str = "circuitnet-ng-offline";
pub const MAX_ARTIFACT: usize = 4 * 1024 * 1024;
pub const MAX_MESSAGES: usize = 32;
pub const MAX_NODES: usize = 128;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid CircuitNET identity, topology or envelope")]
    Invalid,
    #[error("CircuitNET exchange exceeds its admission bound")]
    Bounds,
    #[error("invalid CircuitNET JSON encoding")]
    Json(#[from] serde_json::Error),
}

macro_rules! token {
    ($name:ident, $valid:expr, $normalize:expr) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl $name {
            pub fn new(value: &str) -> Result<Self, Error> {
                if !($valid)(value) {
                    return Err(Error::Invalid);
                }
                Ok(Self(($normalize)(value)))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(value: String) -> Result<Self, Error> {
                Self::new(&value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
fn bounded_token(s: &str, max: usize, hyphen: bool) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || (hyphen && b == b'-'))
}
token!(NodeId, |s: &str| bounded_token(s, 8, false), |s: &str| s
    .to_ascii_uppercase());
token!(Codename, |s: &str| bounded_token(s, 8, true), |s: &str| s
    .to_ascii_uppercase());
token!(
    NetworkId,
    |s: &str| bounded_token(s, 32, true),
    |s: &str| s.to_ascii_lowercase()
);
token!(
    MessageId,
    |s: &str| {
        s.split_once(':').is_some_and(|(node, id)| {
            NodeId::new(node).is_ok() && id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
        })
    },
    |s: &str| {
        let (node, id) = s.split_once(':').expect("validated ID");
        format!("{}:{}", node.to_ascii_uppercase(), id.to_ascii_lowercase())
    }
);
impl MessageId {
    pub fn origin(&self) -> NodeId {
        NodeId::new(self.0.split_once(':').expect("validated ID").0).expect("validated origin")
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Role {
    End,
    Host,
    Root,
}
impl Role {
    pub fn can_transit(self) -> bool {
        self != Self::End
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: NodeId,
    pub role: Role,
    pub parent: Option<NodeId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topology {
    pub nodes: Vec<Node>,
}
impl Topology {
    pub fn validate(&self) -> Result<(), Error> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_NODES {
            return Err(Error::Bounds);
        }
        let ids: BTreeSet<_> = self.nodes.iter().map(|n| &n.id).collect();
        if ids.len() != self.nodes.len()
            || self.nodes.iter().filter(|n| n.parent.is_none()).count() != 1
        {
            return Err(Error::Invalid);
        }
        for n in &self.nodes {
            if (n.role == Role::Root && n.parent.is_some())
                || (n.role == Role::End && n.parent.is_none())
            {
                return Err(Error::Invalid);
            }
            let mut seen = BTreeSet::new();
            let mut current = n;
            loop {
                if !seen.insert(&current.id) {
                    return Err(Error::Invalid);
                }
                let Some(parent) = &current.parent else {
                    break;
                };
                current = self.node(parent)?;
                if !current.role.can_transit() {
                    return Err(Error::Invalid);
                }
            }
        }
        Ok(())
    }
    pub fn node(&self, id: &NodeId) -> Result<&Node, Error> {
        self.nodes
            .iter()
            .find(|n| &n.id == id)
            .ok_or(Error::Invalid)
    }
    pub fn neighbors(&self, id: &NodeId) -> Result<Vec<NodeId>, Error> {
        let node = self.node(id)?;
        Ok(self
            .nodes
            .iter()
            .filter(|n| n.parent.as_ref() == Some(id) || node.parent.as_ref() == Some(&n.id))
            .map(|n| n.id.clone())
            .collect())
    }
    pub fn path(&self, from: &NodeId, to: &NodeId) -> Result<Vec<NodeId>, Error> {
        self.validate()?;
        let mut pending = vec![(from.clone(), vec![from.clone()])];
        while let Some((id, path)) = pending.pop() {
            if &id == to {
                return Ok(path);
            }
            for next in self.neighbors(&id)? {
                if !path.contains(&next) {
                    let mut p = path.clone();
                    p.push(next.clone());
                    pending.push((next, p));
                }
            }
        }
        Err(Error::Invalid)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: MessageId,
    pub origin: NodeId,
    pub codename: Codename,
    pub author: String,
    pub subject: String,
    pub body: String,
    pub timestamp: i64,
    pub reply: Option<MessageId>,
    pub path: Vec<NodeId>,
}
fn text_valid(s: &str, max: usize, body: bool) -> bool {
    s.len() <= max
        && !s.chars().any(|c| {
            (c.is_control() && !(body && matches!(c, '\r' | '\n' | '\t')))
                || matches!(c,'\u{2028}'|'\u{2029}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
}
impl Message {
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.origin() != self.origin
            || self.reply.as_ref() == Some(&self.id)
            || self.author.is_empty()
            || self.author.chars().count() > 60
            || !text_valid(&self.author, 120, false)
            || !text_valid(&self.subject, 72, false)
            || !text_valid(&self.body, 65536, true)
            || !(0..=253402300799).contains(&self.timestamp)
            || self.path.is_empty()
            || self.path.len() > MAX_NODES
            || self.path.first() != Some(&self.origin)
            || self.path.iter().collect::<BTreeSet<_>>().len() != self.path.len()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<String, Error> {
        let mut value = self.clone();
        value.path.clear();
        Ok(digest(&serde_json::to_vec(&value)?))
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub format: String,
    pub version: u16,
    pub network: NetworkId,
    pub sender: NodeId,
    pub neighbor: NodeId,
    pub messages: Vec<Message>,
}
impl Batch {
    pub fn validate(&self) -> Result<(), Error> {
        if self.format != FORMAT || self.version != 1 || self.sender == self.neighbor {
            return Err(Error::Invalid);
        }
        if self.messages.is_empty() || self.messages.len() > MAX_MESSAGES {
            return Err(Error::Bounds);
        }
        let mut ids = BTreeSet::new();
        for m in &self.messages {
            m.validate()?;
            if !ids.insert(&m.id)
                || m.path.last() != Some(&self.sender)
                || m.path.contains(&self.neighbor)
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        bound(bytes)?;
        let b: Self = serde_json::from_slice(bytes)?;
        b.validate()?;
        Ok(b)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let b = serde_json::to_vec(self)?;
        bound(&b)?;
        Ok(b)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub format: String,
    pub version: u16,
    pub network: NetworkId,
    pub sender: NodeId,
    pub neighbor: NodeId,
    pub artifact: String,
    pub accepted: Vec<MessageId>,
}
impl Receipt {
    pub fn validate(&self) -> Result<(), Error> {
        if self.format != "circuitnet-ng-offline-receipt"
            || self.version != 1
            || self.sender == self.neighbor
            || self.artifact.len() != 64
            || !self
                .artifact
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.accepted.is_empty()
            || self.accepted.len() > MAX_MESSAGES
            || self.accepted.iter().collect::<BTreeSet<_>>().len() != self.accepted.len()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        bound(bytes)?;
        let r: Self = serde_json::from_slice(bytes)?;
        r.validate()?;
        Ok(r)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }
}
fn bound(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_ARTIFACT {
        Err(Error::Bounds)
    } else {
        Ok(())
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> Topology {
        Topology {
            nodes: vec![
                Node {
                    id: NodeId::new("ROOT").unwrap(),
                    role: Role::Root,
                    parent: None,
                },
                Node {
                    id: NodeId::new("HOST").unwrap(),
                    role: Role::Host,
                    parent: Some(NodeId::new("ROOT").unwrap()),
                },
                Node {
                    id: NodeId::new("END1").unwrap(),
                    role: Role::End,
                    parent: Some(NodeId::new("HOST").unwrap()),
                },
                Node {
                    id: NodeId::new("END2").unwrap(),
                    role: Role::End,
                    parent: Some(NodeId::new("HOST").unwrap()),
                },
            ],
        }
    }
    #[test]
    fn node_codename_network_normalization_and_bounds() {
        assert_eq!(NodeId::new("end00001").unwrap().as_str(), "END00001");
        assert_eq!(Codename::new("sci-fi").unwrap().as_str(), "SCI-FI");
        assert_eq!(
            NetworkId::new("CircuitNet-Test").unwrap().as_str(),
            "circuitnet-test"
        );
        for value in [
            "",
            "123456789",
            "a b",
            "../a",
            "a*",
            "a?",
            "é",
            "a/b",
            "a\\b",
            "a.",
        ] {
            assert!(NodeId::new(value).is_err(), "{value}");
        }
        for value in ["", "123456789", "a b", "*", "-A", "A-", "é"] {
            assert!(Codename::new(value).is_err());
        }
        assert!(NetworkId::new(&"a".repeat(33)).is_err());
    }
    #[test]
    fn roles_tree_path_and_invalid_relationships() {
        let t = tree();
        t.validate().unwrap();
        assert_eq!(
            t.path(&NodeId::new("END1").unwrap(), &NodeId::new("END2").unwrap())
                .unwrap()
                .iter()
                .map(NodeId::as_str)
                .collect::<Vec<_>>(),
            vec!["END1", "HOST", "END2"]
        );
        let mut bad = t.clone();
        bad.nodes[0].parent = Some(bad.nodes[1].id.clone());
        assert!(bad.validate().is_err());
        let mut bad = t.clone();
        bad.nodes[1].parent = Some(bad.nodes[1].id.clone());
        assert!(bad.validate().is_err());
        let mut bad = t.clone();
        bad.nodes[1].parent = Some(bad.nodes[2].id.clone());
        assert!(bad.validate().is_err());
        let mut bad = t.clone();
        bad.nodes[3].parent = Some(bad.nodes[2].id.clone());
        assert!(bad.validate().is_err());
        let mut bad = t.clone();
        bad.nodes[3].parent = None;
        assert!(bad.validate().is_err());
        let mut bad = t.clone();
        bad.nodes.push(bad.nodes[2].clone());
        assert!(bad.validate().is_err());
        let mut bad = t;
        bad.nodes[1].parent = Some(NodeId::new("UNKNOWN").unwrap());
        assert!(bad.validate().is_err());
    }
    fn batch() -> Batch {
        Batch {
            format: FORMAT.into(),
            version: 1,
            network: NetworkId::new("circuitnet-test").unwrap(),
            sender: NodeId::new("END1").unwrap(),
            neighbor: NodeId::new("HOST").unwrap(),
            messages: vec![Message {
                id: MessageId::new("END1:00000000000000000000000000000001").unwrap(),
                origin: NodeId::new("END1").unwrap(),
                codename: Codename::new("CNTEST").unwrap(),
                author: "Synthetic".into(),
                subject: "Subject".into(),
                body: "Body\r\n".into(),
                timestamp: 1,
                reply: None,
                path: vec![NodeId::new("END1").unwrap()],
            }],
        }
    }
    #[test]
    fn deterministic_json_and_metadata_round_trip() {
        let b = batch();
        let bytes = b.encode().unwrap();
        assert_eq!(Batch::decode(&bytes).unwrap(), b);
        assert_eq!(bytes, b.encode().unwrap());
        assert!(!String::from_utf8(bytes).unwrap().contains("conference_id"));
    }
    #[test]
    fn hostile_envelope_rejects_version_fields_duplicates_encoding_and_size() {
        let b = batch();
        let mut bad = b.clone();
        bad.version = 2;
        assert!(bad.encode().is_err());
        let mut bad = b.clone();
        bad.messages.push(bad.messages[0].clone());
        assert!(bad.encode().is_err());
        let mut bad = b.clone();
        bad.messages[0].body = "a".repeat(65537);
        assert!(bad.encode().is_err());
        let mut bad = b.clone();
        bad.messages[0].origin = NodeId::new("END2").unwrap();
        assert!(bad.encode().is_err());
        let mut bad = b.clone();
        bad.messages[0].path.push(b.neighbor.clone());
        assert!(bad.encode().is_err());
        let mut value = serde_json::to_value(&b).unwrap();
        value["unknown"] = true.into();
        assert!(Batch::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(Batch::decode(&[255]).is_err());
        assert!(Batch::decode(&vec![b' '; MAX_ARTIFACT + 1]).is_err());
        let mut bad = b;
        bad.messages[0].author = "A\u{1b}[31m".into();
        assert!(bad.encode().is_err());
    }
}
