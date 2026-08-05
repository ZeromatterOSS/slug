/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Borrow;
use std::collections::BTreeMap;

use allocative::Allocative;

use crate::repo::ApparentRepoName;
use crate::repo::CanonicalRepoName;

impl Borrow<str> for ApparentRepoName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct RepositoryMappingId(String);

#[derive(Debug, Clone, Allocative)]
pub struct RepositoryMapping {
    id: RepositoryMappingId,
    entries: BTreeMap<ApparentRepoName, CanonicalRepoName>,
    candidate_keys: Vec<ApparentRepoName>,
}

pub(crate) enum OptionMappingLookup {
    Visible(CanonicalRepoName),
    NonVisible { did_you_mean_suffix: String },
}

impl PartialEq for RepositoryMapping {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.entries == other.entries
    }
}

impl Eq for RepositoryMapping {}

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
            candidate_keys: Vec::new(),
        }
    }

    pub fn id(&self) -> &RepositoryMappingId {
        &self.id
    }

    pub fn insert(&mut self, apparent: ApparentRepoName, canonical: CanonicalRepoName) {
        if !self.entries.contains_key(&apparent) {
            self.candidate_keys.push(apparent.clone());
        }
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

    pub(crate) fn option_lookup(&self, apparent: &str) -> OptionMappingLookup {
        match self.entries.get(apparent) {
            Some(canonical) => OptionMappingLookup::Visible(canonical.clone()),
            None => OptionMappingLookup::NonVisible {
                did_you_mean_suffix: spellcheck(apparent, &self.candidate_keys),
            },
        }
    }
}

// Repository names are ASCII under Bazel's grammar, so ASCII lowercasing is
// Java-compatible for the source SpellChecker path.
fn spellcheck(input: &str, candidates: &[ApparentRepoName]) -> String {
    let input = input.to_ascii_lowercase();
    let mut best_distance = std::cmp::min(5, (input.encode_utf16().count() + 1) / 2);
    let mut best = None;
    for candidate in candidates {
        if let Some(distance) = bounded_utf16_distance(
            &input,
            &candidate.as_str().to_ascii_lowercase(),
            best_distance,
        )
        .filter(|distance| *distance < best_distance)
        {
            best_distance = distance;
            best = Some(candidate);
        }
    }
    best.map(|candidate| format!(" (did you mean '{}'?)", candidate.as_str()))
        .unwrap_or_default()
}

fn bounded_utf16_distance(left: &str, right: &str, maximum: usize) -> Option<usize> {
    let left: Vec<_> = left.encode_utf16().collect();
    let right: Vec<_> = right.encode_utf16().collect();
    if left.len().abs_diff(right.len()) > maximum {
        return None;
    }
    let mut row: Vec<_> = (0..=right.len()).collect();
    for (index, left_unit) in left.iter().enumerate() {
        let mut previous = index;
        row[0] = index + 1;
        let mut best_in_row = row[0];
        for (column, right_unit) in right.iter().enumerate() {
            let old = row[column + 1];
            row[column + 1] = std::cmp::min(
                previous + usize::from(left_unit != right_unit),
                1 + std::cmp::min(row[column], row[column + 1]),
            );
            previous = old;
            best_in_row = best_in_row.min(row[column + 1]);
        }
        if best_in_row > maximum {
            return None;
        }
    }
    (row[right.len()] <= maximum).then_some(row[right.len()])
}
