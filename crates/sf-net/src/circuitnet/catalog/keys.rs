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

//! Explicit operator-enrolled signing-key transitions, separate from TLS.
use super::*;
const KEY_DOMAIN: &[u8] = b"CIRCUITNET-NG-CATALOG-KEY-1\n";
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Planned,
    Emergency,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub format: String,
    pub previous: Authority,
    pub replacement: Authority,
    pub revision: u64,
    pub catalog_hash: String,
    pub mode: Mode,
    pub published_at: i64,
    pub reference: String,
    pub rationale: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub change: Change,
    pub old_signature: Option<String>,
    pub new_signature: String,
}
impl Change {
    fn bytes(&self) -> Result<Vec<u8>, Error> {
        self.previous.validate()?;
        self.replacement.validate()?;
        if self.format != "circuitnet-ng-catalog-key"
            || self.previous.network != self.replacement.network
            || self.previous.catalog_id != self.replacement.catalog_id
            || self.previous.publisher != self.replacement.publisher
            || self.previous.public_key == self.replacement.public_key
            || self.revision == 0
            || self.revision >= i64::MAX as u64
            || !is_hex(&self.catalog_hash, 32)
            || !(0..=253402300799).contains(&self.published_at)
            || self.reference.is_empty()
            || !text_valid(&self.reference, 160, false)
            || self.rationale.is_empty()
            || !text_valid(&self.rationale, 1024, false)
        {
            return Err(Error::Invalid);
        }
        Ok([
            KEY_DOMAIN,
            serde_json::to_vec(self)
                .map_err(|_| Error::Invalid)?
                .as_slice(),
        ]
        .concat())
    }
}
impl Authority {
    pub fn fingerprint(&self) -> Result<String, Error> {
        self.validate()?;
        Ok(digest(&unhex(&self.public_key, 32)?))
    }
}
impl Transition {
    pub fn sign(change: Change, old: Option<&[u8]>, new: &[u8]) -> Result<Self, Error> {
        let bytes = change.bytes()?;
        let sign = |key: &[u8]| -> Result<String, Error> {
            Ok(hex(Ed25519KeyPair::from_pkcs8(key)
                .map_err(|_| Error::Signature)?
                .sign(&bytes)
                .as_ref()))
        };
        let value = Self {
            change,
            old_signature: old.map(sign).transpose()?,
            new_signature: sign(new)?,
        };
        value.verify()?;
        Ok(value)
    }
    pub fn verify(&self) -> Result<(), Error> {
        let bytes = self.change.bytes()?;
        let verify = |key: &str, sig: &str| -> Result<(), Error> {
            UnparsedPublicKey::new(&ED25519, unhex(key, 32)?)
                .verify(&bytes, &unhex(sig, 64)?)
                .map_err(|_| Error::Signature)
        };
        verify(&self.change.replacement.public_key, &self.new_signature)?;
        match (self.change.mode, self.old_signature.as_ref()) {
            (Mode::Planned, Some(s)) => verify(&self.change.previous.public_key, s),
            (Mode::Emergency, None) => Ok(()),
            _ => Err(Error::Signature),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transition_requires_both_planned_signers_and_binds_every_authority_field() {
        let old = Signed::generate_key().unwrap();
        let new = Signed::generate_key().unwrap();
        let wrong = Signed::generate_key().unwrap();
        let previous = Authority {
            network: NetworkId::new("synthetic").unwrap(),
            catalog_id: "a".repeat(32),
            publisher: NodeId::new("ROOT").unwrap(),
            public_key: Signed::public_key(&old).unwrap(),
        };
        let mut replacement = previous.clone();
        replacement.public_key = Signed::public_key(&new).unwrap();
        let change = Change {
            format: "circuitnet-ng-catalog-key".into(),
            previous,
            replacement,
            revision: 1,
            catalog_hash: "b".repeat(64),
            mode: Mode::Planned,
            published_at: 100,
            reference: "synthetic-decision".into(),
            rationale: "Rotate synthetic key".into(),
        };
        assert!(Transition::sign(change.clone(), Some(&wrong), &new).is_err());
        assert!(Transition::sign(change.clone(), Some(&old), &wrong).is_err());
        assert!(Transition::sign(change.clone(), None, &new).is_err());
        let signed = Transition::sign(change.clone(), Some(&old), &new).unwrap();
        assert_eq!(signed.verify(), Ok(()));
        let mut forged = signed.clone();
        forged.change.catalog_hash = "c".repeat(64);
        assert!(forged.verify().is_err());
        let mut forged = signed.clone();
        forged.change.replacement.publisher = NodeId::new("OTHER").unwrap();
        assert!(forged.verify().is_err());
        let mut forged = signed.clone();
        forged.change.mode = Mode::Emergency;
        assert!(forged.verify().is_err());
        let mut emergency = change;
        emergency.mode = Mode::Emergency;
        assert!(Transition::sign(emergency.clone(), Some(&old), &new).is_err());
        assert!(Transition::sign(emergency, None, &new)
            .unwrap()
            .verify()
            .is_ok());
    }
}
