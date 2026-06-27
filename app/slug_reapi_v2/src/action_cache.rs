/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::cas::GeneratedOutput;
use crate::digest::ReapiDigest;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ActionResult {
    output_files: Vec<GeneratedOutput>,
    stdout_digest: Option<ReapiDigest>,
    stderr_digest: Option<ReapiDigest>,
}

impl ActionResult {
    pub fn new(output_files: Vec<GeneratedOutput>) -> Self {
        Self {
            output_files,
            stdout_digest: None,
            stderr_digest: None,
        }
    }

    pub fn with_stdout_digest(mut self, digest: ReapiDigest) -> Self {
        self.stdout_digest = Some(digest);
        self
    }

    pub fn with_stderr_digest(mut self, digest: ReapiDigest) -> Self {
        self.stderr_digest = Some(digest);
        self
    }

    pub fn output_files(&self) -> &[GeneratedOutput] {
        &self.output_files
    }

    pub fn stdout_digest(&self) -> Option<&ReapiDigest> {
        self.stdout_digest.as_ref()
    }

    pub fn stderr_digest(&self) -> Option<&ReapiDigest> {
        self.stderr_digest.as_ref()
    }

    pub fn validate_local_outputs(&self, available: &[GeneratedOutput]) -> ActionCacheStatus {
        let available = available
            .iter()
            .map(|output| (output.path().to_owned(), output.digest().clone()))
            .collect::<BTreeMap<_, _>>();
        let missing_paths = self
            .output_files
            .iter()
            .filter(|expected| available.get(expected.path()) != Some(expected.digest()))
            .map(|expected| expected.path().to_owned())
            .collect::<Vec<_>>();
        if missing_paths.is_empty() {
            ActionCacheStatus::Hit
        } else {
            ActionCacheStatus::StaleLocal { missing_paths }
        }
    }

    pub fn validate_remote_cas(&self, available_digests: &[ReapiDigest]) -> ActionCacheStatus {
        let available_digests = available_digests.iter().cloned().collect::<BTreeSet<_>>();
        let missing_digests = self
            .output_files
            .iter()
            .map(GeneratedOutput::digest)
            .filter(|digest| !available_digests.contains(*digest))
            .cloned()
            .collect::<Vec<_>>();
        if missing_digests.is_empty() {
            ActionCacheStatus::Hit
        } else {
            ActionCacheStatus::OrphanedRemote { missing_digests }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActionCacheEntry {
    action_digest: ReapiDigest,
    result: ActionResult,
}

impl ActionCacheEntry {
    pub fn new(action_digest: ReapiDigest, result: ActionResult) -> Self {
        Self {
            action_digest,
            result,
        }
    }

    pub fn action_digest(&self) -> &ReapiDigest {
        &self.action_digest
    }

    pub fn result(&self) -> &ActionResult {
        &self.result
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActionCacheStatus {
    Hit,
    Miss,
    StaleLocal { missing_paths: Vec<String> },
    OrphanedRemote { missing_digests: Vec<ReapiDigest> },
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ActionCacheTable {
    entries: BTreeMap<ReapiDigest, ActionResult>,
}

impl ActionCacheTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, action_digest: ReapiDigest, result: ActionResult) {
        self.entries.insert(action_digest, result);
    }

    pub fn lookup(&self, action_digest: &ReapiDigest) -> Option<ActionCacheEntry> {
        self.entries
            .get(action_digest)
            .cloned()
            .map(|result| ActionCacheEntry::new(action_digest.clone(), result))
    }

    pub fn status_for(&self, action_digest: &ReapiDigest) -> ActionCacheStatus {
        if self.entries.contains_key(action_digest) {
            ActionCacheStatus::Hit
        } else {
            ActionCacheStatus::Miss
        }
    }
}
