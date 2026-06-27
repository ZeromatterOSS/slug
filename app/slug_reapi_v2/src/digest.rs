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
}

impl fmt::Display for ReapiDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.hash, self.size_bytes)
    }
}
