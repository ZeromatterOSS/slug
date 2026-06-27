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

use crate::repo::ApparentRepoName;
use crate::repo::CanonicalRepoName;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryMappingId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryMapping {
    id: RepositoryMappingId,
    entries: BTreeMap<ApparentRepoName, CanonicalRepoName>,
}

impl RepositoryMappingId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() {
            return Err("repository mapping id must not be empty".to_owned());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RepositoryMapping {
    pub fn new(id: RepositoryMappingId) -> Self {
        Self {
            id,
            entries: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> &RepositoryMappingId {
        &self.id
    }

    pub fn insert(&mut self, apparent: ApparentRepoName, canonical: CanonicalRepoName) {
        self.entries.insert(apparent, canonical);
    }

    pub fn resolve(&self, apparent: &ApparentRepoName) -> CanonicalRepoName {
        if apparent.is_root() {
            return CanonicalRepoName::root();
        }
        self.entries.get(apparent).cloned().unwrap_or_else(|| {
            CanonicalRepoName::new(apparent.as_str()).expect("validated apparent repo name")
        })
    }
}
