/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeSet;

use crate::digest::ReapiDigest;
use crate::input_tree::ReapiInputTree;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CasUploadPlan {
    missing_blobs: Vec<ReapiDigest>,
    uploaded_bytes: u64,
}

impl CasUploadPlan {
    pub fn from_missing(input_tree: &ReapiInputTree, missing: &[ReapiDigest]) -> Self {
        let requested = required_blobs(input_tree);
        let missing = missing.iter().cloned().collect::<BTreeSet<_>>();
        let missing_blobs = requested
            .into_iter()
            .filter(|digest| missing.contains(digest))
            .collect::<Vec<_>>();
        let uploaded_bytes = missing_blobs.iter().map(ReapiDigest::size_bytes).sum();
        Self {
            missing_blobs,
            uploaded_bytes,
        }
    }

    pub fn missing_blobs(&self) -> &[ReapiDigest] {
        &self.missing_blobs
    }

    pub fn uploaded_bytes(&self) -> u64 {
        self.uploaded_bytes
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GeneratedOutput {
    path: String,
    digest: ReapiDigest,
}

impl GeneratedOutput {
    pub fn new(path: impl Into<String>, digest: ReapiDigest) -> Self {
        Self {
            path: path.into(),
            digest,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedOutputReuploadPlan {
    missing_outputs: Vec<GeneratedOutput>,
}

impl GeneratedOutputReuploadPlan {
    pub fn from_missing(outputs: &[GeneratedOutput], missing: &[ReapiDigest]) -> Self {
        let missing = missing.iter().cloned().collect::<BTreeSet<_>>();
        let missing_outputs = outputs
            .iter()
            .filter(|output| missing.contains(output.digest()))
            .cloned()
            .collect();
        Self { missing_outputs }
    }

    pub fn missing_outputs(&self) -> &[GeneratedOutput] {
        &self.missing_outputs
    }
}

fn required_blobs(input_tree: &ReapiInputTree) -> BTreeSet<ReapiDigest> {
    let mut blobs = BTreeSet::new();
    blobs.extend(
        input_tree
            .directory_blobs()
            .iter()
            .map(|blob| blob.digest().clone()),
    );
    blobs.extend(
        input_tree
            .inline_blobs()
            .iter()
            .map(|blob| blob.digest().clone()),
    );
    for entry in input_tree.entries() {
        blobs.insert(entry.digest().clone());
    }
    blobs
}
