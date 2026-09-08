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

//! Implementation-neutral signed catalog snapshots and lifecycle validation.
use super::{digest, text_valid, Codename, NetworkId, NodeId};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BYTES: usize = 512 * 1024;
pub const MAX_ENTRIES: usize = 256;
const DOMAIN: &[u8] = b"CIRCUITNET-NG-CATALOG-1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    #[error("invalid-catalog")]
    Invalid,
    #[error("invalid-signature")]
    Signature,
    #[error("invalid-publisher")]
    Publisher,
    #[error("catalog-rollback")]
    Rollback,
    #[error("catalog-fork")]
    Fork,
    #[error("catalog-missing-revision")]
    Missing,
    #[error("catalog-lifecycle")]
    Lifecycle,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    Proposed,
    Active,
    Deprecated,
    Retired,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Intent {
    Ordinary,
    Reactivate,
    Reuse,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub codename: Codename,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub required: bool,
    pub status: Lifecycle,
    pub effective_revision: u64,
    pub retired_revision: Option<u64>,
    pub historical_reference: String,
    pub moderator_role: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub format: String,
    pub schema: u16,
    pub network: NetworkId,
    pub catalog_id: String,
    pub revision: u64,
    pub previous_revision: u64,
    pub previous_hash: Option<String>,
    pub published_at: i64,
    pub publisher: NodeId,
    pub governance_reference: String,
    pub rationale: String,
    pub intent: Intent,
    pub entries: Vec<Entry>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed {
    pub body: Body,
    pub hash: String,
    pub signature: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    pub network: NetworkId,
    pub catalog_id: String,
    pub publisher: NodeId,
    pub public_key: String,
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn is_hex(s: &str, bytes: usize) -> bool {
    s.len() == bytes * 2
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn unhex(s: &str, bytes: usize) -> Result<Vec<u8>, Error> {
    if !is_hex(s, bytes) {
        return Err(Error::Invalid);
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| Error::Invalid))
        .collect()
}
impl Authority {
    pub fn validate(&self) -> Result<(), Error> {
        if !is_hex(&self.catalog_id, 16) || !is_hex(&self.public_key, 32) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
impl Body {
    pub fn validate(&self) -> Result<(), Error> {
        if self.format != "circuitnet-ng-catalog"
            || self.schema != 1
            || !is_hex(&self.catalog_id, 16)
            || self.revision == 0
            || self.revision > i64::MAX as u64
            || self.previous_revision != self.revision - 1
            || (self.revision == 1) != self.previous_hash.is_none()
            || self.previous_hash.as_ref().is_some_and(|h| !is_hex(h, 32))
            || !(0..=253402300799).contains(&self.published_at)
            || self.governance_reference.is_empty()
            || !text_valid(&self.governance_reference, 160, false)
            || self.rationale.is_empty()
            || !text_valid(&self.rationale, 1024, false)
            || self.entries.len() > MAX_ENTRIES
        {
            return Err(Error::Invalid);
        }
        let mut ids = BTreeSet::new();
        let mut live_codes = BTreeSet::new();
        let mut previous = "";
        for e in &self.entries {
            if !is_hex(&e.id, 16)
                || e.id.as_str() <= previous
                || !ids.insert(&e.id)
                || e.display_name.is_empty()
                || !text_valid(&e.display_name, 80, false)
                || e.description.is_empty()
                || !text_valid(&e.description, 1024, false)
                || e.category.is_empty()
                || !text_valid(&e.category, 60, false)
                || !text_valid(&e.historical_reference, 160, false)
                || e.moderator_role.is_empty()
                || !text_valid(&e.moderator_role, 80, false)
                || e.effective_revision == 0
                || e.effective_revision > self.revision
                || e.retired_revision
                    .is_some_and(|r| r == 0 || r > self.revision)
                || (e.status == Lifecycle::Retired) != e.retired_revision.is_some()
                || (e.status != Lifecycle::Retired && !live_codes.insert(&e.codename))
            {
                return Err(Error::Invalid);
            }
            previous = &e.id;
        }
        Ok(())
    }
    /// Fixed field order, compact UTF-8 JSON; entries ordered by immutable ID.
    pub fn canonical(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_BYTES - 512 {
            return Err(Error::Invalid);
        }
        Ok(bytes)
    }
}
impl Signed {
    pub fn sign(body: Body, pkcs8: &[u8]) -> Result<Self, Error> {
        let key = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| Error::Signature)?;
        let bytes = body.canonical()?;
        let hash = digest(&bytes);
        let signature = hex(key.sign(&[DOMAIN, bytes.as_slice()].concat()).as_ref());
        Ok(Self {
            body,
            hash,
            signature,
        })
    }
    pub fn public_key(pkcs8: &[u8]) -> Result<String, Error> {
        Ok(hex(Ed25519KeyPair::from_pkcs8(pkcs8)
            .map_err(|_| Error::Signature)?
            .public_key()
            .as_ref()))
    }
    pub fn generate_key() -> Result<Vec<u8>, Error> {
        Ok(
            Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
                .map_err(|_| Error::Signature)?
                .as_ref()
                .to_vec(),
        )
    }
    pub fn verify(&self, authority: &Authority) -> Result<(), Error> {
        authority.validate()?;
        if self.body.network != authority.network
            || self.body.catalog_id != authority.catalog_id
            || self.body.publisher != authority.publisher
        {
            return Err(Error::Publisher);
        }
        let bytes = self.body.canonical()?;
        if self.hash != digest(&bytes) {
            return Err(Error::Signature);
        }
        UnparsedPublicKey::new(&ED25519, unhex(&authority.public_key, 32)?)
            .verify(
                &[DOMAIN, bytes.as_slice()].concat(),
                &unhex(&self.signature, 64)?,
            )
            .map_err(|_| Error::Signature)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.body.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::Invalid);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::Invalid);
        }
        let s: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        s.body.validate()?;
        Ok(s)
    }
    /// Returns false only for an exact current replay. Verification is separate.
    pub fn follows(&self, old: Option<&Self>) -> Result<bool, Error> {
        let Some(old) = old else {
            if self.body.revision != 1 {
                return Err(Error::Missing);
            }
            let mut codes = BTreeSet::new();
            if self.body.intent != Intent::Ordinary
                || self
                    .body
                    .entries
                    .iter()
                    .any(|e| !codes.insert(&e.codename) || e.effective_revision != 1)
            {
                return Err(Error::Lifecycle);
            }
            return Ok(true);
        };
        if self.body.revision < old.body.revision {
            return Err(Error::Rollback);
        }
        if self.body.revision == old.body.revision {
            return if self == old {
                Ok(false)
            } else {
                Err(Error::Fork)
            };
        }
        if self.body.revision != old.body.revision + 1 {
            return Err(Error::Missing);
        }
        if self.body.previous_hash.as_ref() != Some(&old.hash) {
            return Err(Error::Fork);
        }
        if self.body.published_at < old.body.published_at {
            return Err(Error::Invalid);
        }
        let new: BTreeMap<_, _> = self.body.entries.iter().map(|e| (&e.id, e)).collect();
        for before in &old.body.entries {
            let after = new.get(&before.id).ok_or(Error::Lifecycle)?;
            if before.codename != after.codename {
                return Err(Error::Lifecycle);
            }
            if before != *after && after.effective_revision != self.body.revision {
                return Err(Error::Lifecycle);
            }
            let valid = match (before.status, after.status) {
                (a, b) if a == b => true,
                (Lifecycle::Proposed, Lifecycle::Active | Lifecycle::Retired)
                | (Lifecycle::Active, Lifecycle::Deprecated | Lifecycle::Retired)
                | (Lifecycle::Deprecated, Lifecycle::Retired) => true,
                (Lifecycle::Retired | Lifecycle::Deprecated, Lifecycle::Active) => {
                    self.body.intent == Intent::Reactivate
                }
                _ => false,
            };
            if before.status == Lifecycle::Retired
                && after.status == Lifecycle::Retired
                && before.retired_revision != after.retired_revision
            {
                return Err(Error::Lifecycle);
            }
            if !valid
                || (before.status != Lifecycle::Retired
                    && after.status == Lifecycle::Retired
                    && after.retired_revision != Some(self.body.revision))
            {
                return Err(Error::Lifecycle);
            }
        }
        for after in &self.body.entries {
            if old.body.entries.iter().any(|e| e.id == after.id) {
                continue;
            }
            if after.effective_revision != self.body.revision {
                return Err(Error::Lifecycle);
            }
            if old
                .body
                .entries
                .iter()
                .any(|e| e.codename == after.codename)
                && (self.body.intent != Intent::Reuse
                    || self.body.entries.iter().any(|e| {
                        e.id != after.id
                            && e.codename == after.codename
                            && e.status != Lifecycle::Retired
                    }))
            {
                return Err(Error::Lifecycle);
            }
        }
        Ok(true)
    }
    pub fn changes(&self, old: Option<&Self>) -> String {
        let mut out = format!(
            "# CircuitNET NG Conference Changes\n\nRevision {}\n\nGovernance: {}\n\n{}\n\n",
            self.body.revision, self.body.governance_reference, self.body.rationale
        );
        for e in &self.body.entries {
            let before = old.and_then(|s| s.body.entries.iter().find(|b| b.id == e.id));
            if before == Some(e) {
                continue;
            }
            let action = match (before, e.status) {
                (None, _) => "ADDED",
                (_, Lifecycle::Retired) => "RETIRED",
                (_, Lifecycle::Deprecated) => "DEPRECATED",
                (Some(b), Lifecycle::Active) if b.status == Lifecycle::Retired => "REACTIVATED",
                _ => "UPDATED",
            };
            out.push_str(&format!(
                "- {action}: {} — {} (identity {})\n",
                e.codename, e.display_name, e.id
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<u8>, Authority, Body) {
        let key = Signed::generate_key().unwrap();
        let network = NetworkId::new("synthetic").unwrap();
        let publisher = NodeId::new("ROOT1").unwrap();
        let catalog_id = "a".repeat(32);
        let a = Authority {
            network: network.clone(),
            publisher: publisher.clone(),
            catalog_id: catalog_id.clone(),
            public_key: Signed::public_key(&key).unwrap(),
        };
        let b = Body {
            format: "circuitnet-ng-catalog".into(),
            schema: 1,
            network,
            catalog_id,
            revision: 1,
            previous_revision: 0,
            previous_hash: None,
            published_at: 1,
            publisher,
            governance_reference: "bootstrap-1".into(),
            rationale: "Initial synthetic catalog".into(),
            intent: Intent::Ordinary,
            entries: vec![Entry {
                id: "1".repeat(32),
                codename: Codename::new("RETROCOM").unwrap(),
                display_name: "Retro Computing".into(),
                description: "Independent retro computing discussion".into(),
                category: "computing".into(),
                required: false,
                status: Lifecycle::Active,
                effective_revision: 1,
                retired_revision: None,
                historical_reference: "new synthetic test area".into(),
                moderator_role: "moderator".into(),
            }],
        };
        (key, a, b)
    }
    fn next(s: &Signed) -> Body {
        let mut b = s.body.clone();
        b.revision += 1;
        b.previous_revision = s.body.revision;
        b.previous_hash = Some(s.hash.clone());
        b.published_at += 1;
        b
    }
    #[test]
    fn signature_canonical_bytes_publisher_and_tamper_fail_closed() {
        let (key, a, b) = fixture();
        let s = Signed::sign(b.clone(), &key).unwrap();
        s.verify(&a).unwrap();
        assert_eq!(s, Signed::sign(b, &key).unwrap());
        assert_eq!(Signed::decode(&s.encode().unwrap()).unwrap(), s);
        assert_eq!(s.follows(None), Ok(true));
        assert_eq!(s.follows(Some(&s)), Ok(false));
        let mut bad = s.clone();
        bad.body.entries[0].description.push('!');
        assert_eq!(bad.verify(&a), Err(Error::Signature));
        let mut wrong = a.clone();
        wrong.publisher = NodeId::new("OTHER").unwrap();
        assert_eq!(s.verify(&wrong), Err(Error::Publisher));
        wrong = a.clone();
        wrong.public_key = Signed::public_key(&Signed::generate_key().unwrap()).unwrap();
        assert_eq!(s.verify(&wrong), Err(Error::Signature));
        let json = String::from_utf8(s.encode().unwrap()).unwrap();
        assert!(!json.contains("conference_number"));
        assert!(!json.contains("conference_id"));
        assert!(!json.contains("private_key"));
    }
    #[test]
    fn revision_chain_replay_rollback_fork_and_missing_revision() {
        let (key, a, b) = fixture();
        let first = Signed::sign(b, &key).unwrap();
        let second = Signed::sign(next(&first), &key).unwrap();
        second.verify(&a).unwrap();
        assert_eq!(second.follows(Some(&first)), Ok(true));
        assert_eq!(first.follows(Some(&second)), Err(Error::Rollback));
        let mut b = second.body.clone();
        b.rationale = "Alternate decision".into();
        let fork = Signed::sign(b, &key).unwrap();
        assert_eq!(fork.follows(Some(&second)), Err(Error::Fork));
        let mut b = next(&second);
        b.previous_hash = Some("0".repeat(64));
        let broken = Signed::sign(b, &key).unwrap();
        assert_eq!(broken.follows(Some(&second)), Err(Error::Fork));
        assert_eq!(broken.follows(Some(&first)), Err(Error::Missing));
    }
    #[test]
    fn retirement_reactivation_and_privileged_reuse_preserve_generations() {
        let (key, _, b) = fixture();
        let first = Signed::sign(b, &key).unwrap();
        let mut b = next(&first);
        b.entries[0].status = Lifecycle::Deprecated;
        b.entries[0].effective_revision = 2;
        let deprecated = Signed::sign(b, &key).unwrap();
        assert_eq!(deprecated.follows(Some(&first)), Ok(true));
        let mut b = next(&deprecated);
        b.entries[0].status = Lifecycle::Retired;
        b.entries[0].effective_revision = 3;
        b.entries[0].retired_revision = Some(3);
        let retired = Signed::sign(b, &key).unwrap();
        assert_eq!(retired.follows(Some(&deprecated)), Ok(true));
        assert!(retired
            .changes(Some(&deprecated))
            .contains("RETIRED: RETROCOM"));
        let mut b = next(&retired);
        b.entries[0].status = Lifecycle::Active;
        b.entries[0].effective_revision = 4;
        b.entries[0].retired_revision = None;
        assert_eq!(
            Signed::sign(b.clone(), &key)
                .unwrap()
                .follows(Some(&retired)),
            Err(Error::Lifecycle)
        );
        b.intent = Intent::Reactivate;
        let revived = Signed::sign(b, &key).unwrap();
        assert_eq!(revived.follows(Some(&retired)), Ok(true));
        assert_eq!(revived.body.entries[0].id, first.body.entries[0].id);
        let mut b = next(&retired);
        let mut fresh = first.body.entries[0].clone();
        fresh.id = "2".repeat(32);
        fresh.effective_revision = 4;
        fresh.description = "A deliberately different concept".into();
        b.entries.push(fresh);
        assert_eq!(
            Signed::sign(b.clone(), &key)
                .unwrap()
                .follows(Some(&retired)),
            Err(Error::Lifecycle)
        );
        b.intent = Intent::Reuse;
        let reused = Signed::sign(b, &key).unwrap();
        assert_eq!(reused.follows(Some(&retired)), Ok(true));
        assert_eq!(reused.body.entries.len(), 2);
        let mut b = next(&reused);
        b.entries.remove(0);
        assert_eq!(
            Signed::sign(b, &key).unwrap().follows(Some(&reused)),
            Err(Error::Lifecycle)
        );
    }
    #[test]
    fn catalog_bounds_unsafe_text_and_unknown_fields() {
        let (key, _, mut b) = fixture();
        b.entries[0].description = "terminal\u{1b}[2J".into();
        assert_eq!(Signed::sign(b, &key), Err(Error::Invalid));
        assert_eq!(
            Signed::decode(&vec![b' '; MAX_BYTES + 1]),
            Err(Error::Invalid)
        );
        let (_, _, mut b) = fixture();
        b.entries.push(b.entries[0].clone());
        assert_eq!(b.validate(), Err(Error::Invalid));
        let (_, _, b) = fixture();
        let mut json = serde_json::to_value(&b).unwrap();
        json["local_conference_number"] = serde_json::json!(42);
        assert!(serde_json::from_value::<Body>(json).is_err());
    }
}

#[cfg(test)]
mod kit_tests {
    use super::*;
    #[test]
    fn official_seed_and_review_have_one_explicit_disposition_per_area() {
        let signed = Signed::decode(include_bytes!(
            "../../../../docs/circuitnet-ng/config/catalog.json"
        ))
        .unwrap();
        let authority: Authority = serde_json::from_slice(include_bytes!(
            "../../../../docs/circuitnet-ng/config/catalog-authority.json"
        ))
        .unwrap();
        signed.verify(&authority).unwrap();
        assert_eq!(signed.follows(None), Ok(true));
        let review: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../docs/circuitnet-ng/config/catalog-review.json"
        ))
        .unwrap();
        let rows = review["conferences"].as_array().unwrap();
        assert_eq!(rows.len(), signed.body.entries.len());
        for entry in &signed.body.entries {
            let matching: Vec<_> = rows
                .iter()
                .filter(|r| r["codename"] == entry.codename.as_str())
                .collect();
            assert_eq!(matching.len(), 1);
            assert_eq!(matching[0]["purpose"], entry.description);
            assert_eq!(matching[0]["initial_distribution"], true);
        }
        let charter = include_str!("../../../../docs/circuitnet-ng/CHARTER.md");
        assert!(charter.contains("Version 1.0"));
        assert!(charter.contains("seven"));
        assert!(charter.contains("not end-to-end encrypted"));
        let application = include_str!("../../../../docs/circuitnet-ng/NODE-APPLICATION.txt");
        assert!(application.contains("DO NOT INCLUDE passwords"));
        assert!(application.contains("static IP"));
    }
}
