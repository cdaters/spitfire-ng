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

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

use crate::PasswordHashConfig;

pub const CREDENTIAL_SCHEME: &str = "argon2id-phc-v1";

#[derive(Clone, Debug)]
pub struct CredentialHasher {
    params: Params,
}

impl CredentialHasher {
    pub fn new(config: &PasswordHashConfig) -> Result<Self, CredentialError> {
        let params = Params::new(
            config.memory_kib,
            config.iterations,
            config.parallelism,
            None,
        )
        .map_err(CredentialError::InvalidParameters)?;
        Ok(Self { params })
    }

    pub fn hash(&self, password: &[u8]) -> Result<String, CredentialError> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone())
            .hash_password(password, &salt)
            .map(|hash| hash.to_string())
            .map_err(CredentialError::Hash)
    }

    pub fn verify(&self, password: &[u8], encoded: &str) -> Result<bool, CredentialError> {
        let parsed = PasswordHash::new(encoded).map_err(CredentialError::InvalidStoredHash)?;
        if parsed.algorithm.as_str() != "argon2id" {
            return Err(CredentialError::UnsupportedStoredAlgorithm);
        }
        Ok(Argon2::default().verify_password(password, &parsed).is_ok())
    }
}

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("invalid Argon2id cost parameters: {0}")]
    InvalidParameters(argon2::Error),
    #[error("password hashing failed")]
    Hash(argon2::password_hash::Error),
    #[error("stored password credential is malformed")]
    InvalidStoredHash(argon2::password_hash::Error),
    #[error("stored password credential does not use Argon2id")]
    UnsupportedStoredAlgorithm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_with_unique_salts_and_verifies_only_the_correct_password() {
        let hasher = CredentialHasher::new(&PasswordHashConfig::default()).unwrap();
        let one = hasher
            .hash(b"test-only correct horse battery staple")
            .unwrap();
        let two = hasher
            .hash(b"test-only correct horse battery staple")
            .unwrap();
        assert_ne!(one, two);
        assert!(one.starts_with("$argon2id$v=19$"));
        assert!(hasher
            .verify(b"test-only correct horse battery staple", &one)
            .unwrap());
        assert!(!hasher.verify(b"incorrect", &one).unwrap());
        assert!(!format!("{hasher:?}").contains("correct horse"));
    }
}
