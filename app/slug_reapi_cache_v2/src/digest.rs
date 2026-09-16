/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use sha2::Digest;
use sha2::Sha256;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReapiDigest {
    hash: String,
    size_bytes: u64,
}

impl ReapiDigest {
    pub fn new(hash: impl Into<String>, size_bytes: u64) -> Result<Self, String> {
        let hash = hash.into();
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
        {
            return Err("REAPI SHA-256 digest must be 64 lowercase hex characters".to_owned());
        }
        if size_bytes > i64::MAX as u64 {
            return Err("REAPI digest size exceeds protobuf int64 range".to_owned());
        }
        Ok(Self { hash, size_bytes })
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let Some((hash, size)) = value.split_once('/') else {
            return Err(format!("REAPI digest must be hash/size: {value}"));
        };
        let size_bytes = size
            .parse()
            .map_err(|_| format!("REAPI digest size must be an integer: {value}"))?;
        Self::new(hash, size_bytes)
    }

    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = format!("{:x}", hasher.finalize());
        Self {
            hash,
            size_bytes: bytes.len() as u64,
        }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), Self> {
        let actual = Self::of_bytes(bytes);
        if actual == *self { Ok(()) } else { Err(actual) }
    }
}

#[cfg(test)]
mod tests {
    use super::ReapiDigest;

    #[test]
    fn public_digest_values_are_canonical_and_wire_representable() {
        let hash = ReapiDigest::of_bytes(b"x").hash().to_owned();
        assert!(ReapiDigest::new(&hash, i64::MAX as u64).is_ok());
        assert!(ReapiDigest::new(&hash, i64::MAX as u64 + 1).is_err());
        assert!(ReapiDigest::new("a", 0).is_err());
        assert!(ReapiDigest::new(hash.to_uppercase(), 0).is_err());
    }
}

impl fmt::Display for ReapiDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.hash, self.size_bytes)
    }
}
