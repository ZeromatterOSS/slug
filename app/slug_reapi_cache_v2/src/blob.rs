/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use crate::ReapiDigest;

/// Byte-bearing CAS object borrowed from a request-local action projection.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiBlob {
    digest: ReapiDigest,
    data: Arc<[u8]>,
}

impl ReapiBlob {
    pub fn from_bytes(data: Vec<u8>) -> Self {
        let digest = ReapiDigest::of_bytes(&data);
        Self {
            digest,
            data: Arc::from(data),
        }
    }

    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn shared_data(&self) -> Arc<[u8]> {
        self.data.clone()
    }
}
