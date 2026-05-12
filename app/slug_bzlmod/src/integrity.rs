/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Subresource Integrity (SRI) hash verification for bzlmod.
//!
//! This module implements hash verification following the SRI specification
//! used by Bazel's bzlmod system. The format is `algorithm-base64hash`.
//!
//! Example: `sha256-wLoLQVeHb/8a/so988MhVoaxM6HOYQ3MDYE7Z9pd1TI=`

use base64::Engine;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;

/// Errors that can occur during integrity verification.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum IntegrityError {
    #[error("Invalid integrity format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported hash algorithm: {0}")]
    UnsupportedAlgorithm(String),

    #[error("Invalid base64 encoding in integrity hash")]
    InvalidBase64,

    #[error(
        "Integrity mismatch: expected {expected}, computed {computed} (algorithm: {algorithm})"
    )]
    Mismatch {
        algorithm: String,
        expected: String,
        computed: String,
    },
}

/// Supported hash algorithms for SRI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    /// Parse a hash algorithm from a string.
    pub fn from_str(s: &str) -> Result<Self, IntegrityError> {
        match s.to_lowercase().as_str() {
            "sha256" => Ok(HashAlgorithm::Sha256),
            "sha384" => Ok(HashAlgorithm::Sha384),
            "sha512" => Ok(HashAlgorithm::Sha512),
            _ => Err(IntegrityError::UnsupportedAlgorithm(s.to_string())),
        }
    }

    /// Get the algorithm name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "sha256",
            HashAlgorithm::Sha384 => "sha384",
            HashAlgorithm::Sha512 => "sha512",
        }
    }
}

/// A parsed SRI integrity value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integrity {
    /// The hash algorithm used.
    pub algorithm: HashAlgorithm,
    /// The base64-encoded hash value.
    pub hash: String,
}

impl Integrity {
    /// Parse an SRI integrity string (e.g., "sha256-abc123...").
    pub fn parse(integrity: &str) -> Result<Self, IntegrityError> {
        let (algo, hash) = integrity
            .split_once('-')
            .ok_or_else(|| IntegrityError::InvalidFormat(integrity.to_string()))?;

        let algorithm = HashAlgorithm::from_str(algo)?;

        // Verify the base64 encoding is valid
        base64::engine::general_purpose::STANDARD
            .decode(hash)
            .map_err(|_| IntegrityError::InvalidBase64)?;

        Ok(Self {
            algorithm,
            hash: hash.to_string(),
        })
    }

    /// Compute the hash of data and return an Integrity value.
    pub fn compute(algorithm: HashAlgorithm, data: &[u8]) -> Self {
        let hash = match algorithm {
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
            }
            HashAlgorithm::Sha384 => {
                let mut hasher = Sha384::new();
                hasher.update(data);
                base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
            }
            HashAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
            }
        };

        Self { algorithm, hash }
    }

    /// Convert to SRI string format.
    pub fn to_sri_string(&self) -> String {
        format!("{}-{}", self.algorithm.as_str(), self.hash)
    }

    /// Verify that data matches this integrity value.
    pub fn verify(&self, data: &[u8]) -> Result<(), IntegrityError> {
        let computed = Self::compute(self.algorithm, data);

        if computed.hash == self.hash {
            Ok(())
        } else {
            Err(IntegrityError::Mismatch {
                algorithm: self.algorithm.as_str().to_string(),
                expected: self.hash.clone(),
                computed: computed.hash,
            })
        }
    }
}

/// Verify data against an SRI integrity string.
///
/// # Arguments
///
/// * `data` - The data to verify.
/// * `expected` - The expected SRI integrity string (e.g., "sha256-abc123...").
///
/// # Returns
///
/// `Ok(())` if the data matches, or an error if verification fails.
///
/// # Example
///
/// ```ignore
/// use slug_bzlmod::integrity::verify_integrity;
///
/// let data = b"Hello, World!";
/// let integrity = "sha256-3/1gIbsr1bCvZ2KQgJ7DpTGR3YHH9wpLKGiKNiGCmG8=";
///
/// verify_integrity(data, integrity)?;
/// ```
pub fn verify_integrity(data: &[u8], expected: &str) -> slug_error::Result<()> {
    let integrity = Integrity::parse(expected)?;
    integrity.verify(data)?;
    Ok(())
}

/// Compute the SRI hash of data.
///
/// # Arguments
///
/// * `data` - The data to hash.
/// * `algorithm` - The hash algorithm to use (default: SHA-256).
///
/// # Returns
///
/// The SRI integrity string.
pub fn compute_integrity(data: &[u8], algorithm: HashAlgorithm) -> String {
    Integrity::compute(algorithm, data).to_sri_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known test vectors
    const TEST_DATA: &[u8] = b"Hello, World!";
    // SHA-256 of "Hello, World!" is 3/1gIbsr1bCvZ2KQgJ7DpTGR3YHH9wpLKGiKNiGCmG8=
    const TEST_SHA256: &str = "sha256-3/1gIbsr1bCvZ2KQgJ7DpTGR3YHH9wpLKGiKNiGCmG8=";

    #[test]
    fn test_parse_sha256() {
        let integrity = Integrity::parse(TEST_SHA256).unwrap();
        assert_eq!(integrity.algorithm, HashAlgorithm::Sha256);
        assert_eq!(
            integrity.hash,
            "3/1gIbsr1bCvZ2KQgJ7DpTGR3YHH9wpLKGiKNiGCmG8="
        );
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(Integrity::parse("not-valid-format").is_err());
        assert!(Integrity::parse("sha256").is_err());
        assert!(Integrity::parse("").is_err());
    }

    #[test]
    fn test_parse_unsupported_algorithm() {
        let result = Integrity::parse("md5-abc123");
        assert!(matches!(
            result,
            Err(IntegrityError::UnsupportedAlgorithm(_))
        ));
    }

    #[test]
    fn test_parse_invalid_base64() {
        let result = Integrity::parse("sha256-not!valid!base64!!");
        assert!(matches!(result, Err(IntegrityError::InvalidBase64)));
    }

    #[test]
    fn test_compute_sha256() {
        let integrity = Integrity::compute(HashAlgorithm::Sha256, TEST_DATA);
        assert_eq!(integrity.algorithm, HashAlgorithm::Sha256);
        assert_eq!(
            integrity.hash,
            "3/1gIbsr1bCvZ2KQgJ7DpTGR3YHH9wpLKGiKNiGCmG8="
        );
    }

    #[test]
    fn test_verify_success() {
        verify_integrity(TEST_DATA, TEST_SHA256).unwrap();
    }

    #[test]
    fn test_verify_failure() {
        let wrong_data = b"Wrong data";
        let result = verify_integrity(wrong_data, TEST_SHA256);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_sri_string() {
        let integrity = Integrity::compute(HashAlgorithm::Sha256, TEST_DATA);
        assert_eq!(integrity.to_sri_string(), TEST_SHA256);
    }

    #[test]
    fn test_sha384() {
        let data = b"test";
        let integrity = Integrity::compute(HashAlgorithm::Sha384, data);
        assert_eq!(integrity.algorithm, HashAlgorithm::Sha384);

        // Verify it parses back correctly
        let parsed = Integrity::parse(&integrity.to_sri_string()).unwrap();
        assert_eq!(parsed.algorithm, HashAlgorithm::Sha384);
        parsed.verify(data).unwrap();
    }

    #[test]
    fn test_sha512() {
        let data = b"test";
        let integrity = Integrity::compute(HashAlgorithm::Sha512, data);
        assert_eq!(integrity.algorithm, HashAlgorithm::Sha512);

        // Verify it parses back correctly
        let parsed = Integrity::parse(&integrity.to_sri_string()).unwrap();
        assert_eq!(parsed.algorithm, HashAlgorithm::Sha512);
        parsed.verify(data).unwrap();
    }
}
