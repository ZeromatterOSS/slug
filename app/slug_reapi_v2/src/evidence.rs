/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use crate::digest::ReapiDigest;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExecutionEvidence {
    pub executor_boundary: &'static str,
    pub backend: String,
    pub reapi_actions: u64,
    pub direct_local_actions: u64,
    pub ac_hits: u64,
    pub ac_misses: u64,
    pub uploaded_digests: Vec<ReapiDigest>,
    pub materialized_outputs: Vec<ReapiDigest>,
}

impl ExecutionEvidence {
    pub fn reapi(backend: impl Into<String>) -> Self {
        Self {
            executor_boundary: "reapi",
            backend: backend.into(),
            reapi_actions: 0,
            direct_local_actions: 0,
            ac_hits: 0,
            ac_misses: 0,
            uploaded_digests: Vec::new(),
            materialized_outputs: Vec::new(),
        }
    }

    pub fn record_action(mut self) -> Self {
        self.reapi_actions += 1;
        self
    }

    pub fn record_ac_hit(mut self) -> Self {
        self.ac_hits += 1;
        self
    }

    pub fn record_ac_miss(mut self) -> Self {
        self.ac_misses += 1;
        self
    }

    pub fn record_upload(mut self, digest: ReapiDigest) -> Self {
        self.uploaded_digests.push(digest);
        self
    }

    pub fn record_materialized_output(mut self, digest: ReapiDigest) -> Self {
        self.materialized_outputs.push(digest);
        self
    }
}
