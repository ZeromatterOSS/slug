/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dice::InjectedKey;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryEnvironmentEntry {
    name: CompactString,
    value: Arc<str>,
}

impl RepositoryEnvironmentEntry {
    pub fn new(name: impl Into<CompactString>, value: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &Arc<str> {
        &self.value
    }
}

impl fmt::Debug for RepositoryEnvironmentEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryEnvironmentEntry")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryEnvironmentCanonicalError {
    OutOfOrder {
        previous: CompactString,
        current: CompactString,
    },
    Duplicate {
        name: CompactString,
    },
    EmptyNeed,
    ConflictingNeedWorkspace {
        left: NormalizedAbsolutePath,
        right: NormalizedAbsolutePath,
    },
}

impl fmt::Display for RepositoryEnvironmentCanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfOrder { previous, current } => write!(
                f,
                "repository environment names are not sorted: {current:?} follows {previous:?}"
            ),
            Self::Duplicate { name } => {
                write!(f, "duplicate repository environment name {name:?}")
            }
            Self::EmptyNeed => f.write_str("repository environment need is empty"),
            Self::ConflictingNeedWorkspace { left, right } => write!(
                f,
                "repository environment needs belong to different workspaces: {left} and {right}"
            ),
        }
    }
}

impl std::error::Error for RepositoryEnvironmentCanonicalError {}

fn validate_sorted_unique<'a>(
    mut names: impl Iterator<Item = &'a str>,
) -> Result<(), RepositoryEnvironmentCanonicalError> {
    let Some(mut previous) = names.next() else {
        return Ok(());
    };
    for current in names {
        match previous.cmp(current) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(RepositoryEnvironmentCanonicalError::Duplicate {
                    name: current.into(),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(RepositoryEnvironmentCanonicalError::OutOfOrder {
                    previous: previous.into(),
                    current: current.into(),
                });
            }
        }
        previous = current;
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryEnvironmentSnapshot {
    entries: Arc<[RepositoryEnvironmentEntry]>,
}

impl Default for RepositoryEnvironmentSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl RepositoryEnvironmentSnapshot {
    pub fn empty() -> Self {
        Self {
            entries: Arc::from([]),
        }
    }

    pub fn from_canonical(
        entries: impl Into<Arc<[RepositoryEnvironmentEntry]>>,
    ) -> Result<Self, RepositoryEnvironmentCanonicalError> {
        let entries = entries.into();
        validate_sorted_unique(entries.iter().map(RepositoryEnvironmentEntry::name))?;
        Ok(Self { entries })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RepositoryEnvironmentEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Arc<str>> {
        self.entries
            .binary_search_by(|entry| entry.name().cmp(name))
            .ok()
            .map(|index| self.entries[index].value())
    }

    pub fn present_name_frontier(&self) -> RepositoryEnvironmentNameFrontier {
        RepositoryEnvironmentNameFrontier::from_unsorted(
            self.entries.iter().map(|entry| entry.name.clone()),
        )
    }
}

impl fmt::Debug for RepositoryEnvironmentSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryEnvironmentSnapshot")
            .field("entries", &self.entries)
            .finish()
    }
}

/// Immutable repository Host inputs installed for one DICE transaction.
///
/// This type is shared below core so the loading-owned repository context can
/// consume the command snapshot in a later packet without reversing the crate
/// dependency. Core remains the sole production installer and injected-key
/// lifecycle owner.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryHostInputTransaction {
    snapshot: RepositoryEnvironmentSnapshot,
    frontier: RepositoryEnvironmentNameFrontier,
}

impl Default for RepositoryHostInputTransaction {
    fn default() -> Self {
        Self::new(
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::empty(),
        )
    }
}

impl RepositoryHostInputTransaction {
    pub fn new(
        snapshot: RepositoryEnvironmentSnapshot,
        frontier: RepositoryEnvironmentNameFrontier,
    ) -> Self {
        Self { snapshot, frontier }
    }

    pub fn snapshot(&self) -> &RepositoryEnvironmentSnapshot {
        &self.snapshot
    }

    pub fn frontier(&self) -> &RepositoryEnvironmentNameFrontier {
        &self.frontier
    }
}

impl fmt::Debug for RepositoryHostInputTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryHostInputTransaction")
            .field("snapshot", &self.snapshot)
            .field("frontier", &self.frontier)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositoryEnvironmentCell {
    Unauthorized,
    Observed(Option<Arc<str>>),
}

impl RepositoryEnvironmentCell {
    pub fn observed(value: Option<Arc<str>>) -> Self {
        Self::Observed(value)
    }

    pub fn value(&self) -> Option<&Option<Arc<str>>> {
        match self {
            Self::Unauthorized => None,
            Self::Observed(value) => Some(value),
        }
    }
}

impl fmt::Debug for RepositoryEnvironmentCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("Unauthorized"),
            Self::Observed(None) => f.write_str("Observed(None)"),
            Self::Observed(Some(_)) => f.write_str("Observed(Some(<redacted>))"),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryEnvironmentCellKey {
    workspace: NormalizedAbsolutePath,
    name: CompactString,
}

impl RepositoryEnvironmentCellKey {
    pub fn new(workspace: NormalizedAbsolutePath, name: impl Into<CompactString>) -> Self {
        Self {
            workspace,
            name: name.into(),
        }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for RepositoryEnvironmentCellKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-environment-cell:{}:{:?}",
            self.workspace, self.name
        )
    }
}

impl InjectedKey for RepositoryEnvironmentCellKey {
    type Value = RepositoryEnvironmentCell;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryEnvironmentNameFrontier {
    names: Arc<[CompactString]>,
}

impl Default for RepositoryEnvironmentNameFrontier {
    fn default() -> Self {
        Self::empty()
    }
}

impl RepositoryEnvironmentNameFrontier {
    pub fn empty() -> Self {
        Self {
            names: Arc::from([]),
        }
    }

    pub fn from_canonical(
        names: impl Into<Arc<[CompactString]>>,
    ) -> Result<Self, RepositoryEnvironmentCanonicalError> {
        let names = names.into();
        validate_sorted_unique(names.iter().map(CompactString::as_str))?;
        Ok(Self { names })
    }

    pub fn from_unsorted(names: impl IntoIterator<Item = CompactString>) -> Self {
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort();
        names.dedup();
        Self {
            names: Arc::from(names),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CompactString> {
        self.names.iter()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names
            .binary_search_by(|candidate| candidate.as_str().cmp(name))
            .is_ok()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn union(&self, other: &Self) -> Self {
        Self::from_unsorted(self.iter().cloned().chain(other.iter().cloned()))
    }

    pub fn difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a CompactString> + 'a {
        self.iter().filter(|name| !other.contains(name))
    }
}

impl fmt::Debug for RepositoryEnvironmentNameFrontier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RepositoryEnvironmentNameFrontier")
            .field(&self.names)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct RepositoryPlatformData {
    os_name: CompactString,
    arch: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryPlatform(Arc<RepositoryPlatformData>);

impl RepositoryPlatform {
    pub fn new(os_name: impl Into<CompactString>, arch: impl Into<CompactString>) -> Self {
        Self(Arc::new(RepositoryPlatformData {
            os_name: os_name.into(),
            arch: arch.into(),
        }))
    }

    pub fn os_name(&self) -> &str {
        &self.0.os_name
    }

    pub fn arch(&self) -> &str {
        &self.0.arch
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RepositoryPlatformKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryPlatformKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}

impl fmt::Display for RepositoryPlatformKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-platform:{}", self.workspace)
    }
}

impl InjectedKey for RepositoryPlatformKey {
    type Value = RepositoryPlatform;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct NeedRepositoryEnvironmentNames {
    workspace: NormalizedAbsolutePath,
    names: RepositoryEnvironmentNameFrontier,
}

impl NeedRepositoryEnvironmentNames {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        names: RepositoryEnvironmentNameFrontier,
    ) -> Result<Self, RepositoryEnvironmentCanonicalError> {
        if names.is_empty() {
            return Err(RepositoryEnvironmentCanonicalError::EmptyNeed);
        }
        Ok(Self { workspace, names })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn names(&self) -> &RepositoryEnvironmentNameFrontier {
        &self.names
    }

    pub fn try_union(&self, other: &Self) -> Result<Self, RepositoryEnvironmentCanonicalError> {
        if self.workspace != other.workspace {
            return Err(
                RepositoryEnvironmentCanonicalError::ConflictingNeedWorkspace {
                    left: self.workspace.clone(),
                    right: other.workspace.clone(),
                },
            );
        }
        Ok(Self {
            workspace: self.workspace.clone(),
            names: self.names.union(&other.names),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(path: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(path).unwrap()
    }

    #[test]
    fn snapshot_requires_canonical_names_and_redacts_values() {
        let secret: Arc<str> = "sentinel-secret".into();
        let snapshot = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
            RepositoryEnvironmentEntry::new("A", secret.clone()),
            RepositoryEnvironmentEntry::new("B", ""),
        ]))
        .unwrap();
        assert!(Arc::ptr_eq(snapshot.get("A").unwrap(), &secret));
        assert_eq!(snapshot.get("B").unwrap().as_ref(), "");
        assert_eq!(snapshot.get("MISSING"), None);
        assert!(!format!("{snapshot:?}").contains("sentinel-secret"));
        assert!(matches!(
            RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("B", "1"),
                RepositoryEnvironmentEntry::new("A", "2"),
            ])),
            Err(RepositoryEnvironmentCanonicalError::OutOfOrder { .. })
        ));
        assert!(matches!(
            RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("A", "1"),
                RepositoryEnvironmentEntry::new("A", "2"),
            ])),
            Err(RepositoryEnvironmentCanonicalError::Duplicate { .. })
        ));
    }

    #[test]
    fn cell_states_are_distinct_and_redacted() {
        let secret: Arc<str> = "sentinel-secret".into();
        let unauthorized = RepositoryEnvironmentCell::Unauthorized;
        let absent = RepositoryEnvironmentCell::observed(None);
        let empty = RepositoryEnvironmentCell::observed(Some(Arc::from("")));
        let present = RepositoryEnvironmentCell::observed(Some(secret));
        assert_ne!(unauthorized, absent);
        assert_ne!(absent, empty);
        assert_ne!(empty, present);
        assert!(!format!("{present:?}").contains("sentinel-secret"));
    }

    #[test]
    fn frontier_and_needs_are_sorted_unique_and_workspace_scoped() {
        let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("B"),
            CompactString::new("A"),
            CompactString::new("B"),
        ]);
        assert_eq!(
            frontier
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
        let left = NeedRepositoryEnvironmentNames::new(workspace("/one"), frontier).unwrap();
        let right = NeedRepositoryEnvironmentNames::new(
            workspace("/one"),
            RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("C")]),
        )
        .unwrap();
        assert_eq!(left.try_union(&right).unwrap().names().len(), 3);
        let foreign = NeedRepositoryEnvironmentNames::new(
            workspace("/two"),
            RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("D")]),
        )
        .unwrap();
        assert!(matches!(
            left.try_union(&foreign),
            Err(RepositoryEnvironmentCanonicalError::ConflictingNeedWorkspace { .. })
        ));
    }

    #[test]
    fn key_identity_includes_workspace_name_and_complete_value() {
        let key_a = RepositoryEnvironmentCellKey::new(workspace("/one"), "A");
        let key_b = RepositoryEnvironmentCellKey::new(workspace("/one"), "B");
        let foreign = RepositoryEnvironmentCellKey::new(workspace("/two"), "A");
        assert_ne!(key_a, key_b);
        assert_ne!(key_a, foreign);
        let platform = RepositoryPlatformKey::new(workspace("/one"));
        assert_eq!(platform, RepositoryPlatformKey::new(workspace("/one")));
        assert_ne!(platform, RepositoryPlatformKey::new(workspace("/two")));
        assert_ne!(
            RepositoryEnvironmentCell::Unauthorized,
            RepositoryEnvironmentCell::Observed(None)
        );
        assert_ne!(
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryPlatform::new("linux", "aarch64")
        );
    }

    #[test]
    fn transaction_carrier_shares_snapshot_values_and_redacts_debug() {
        let secret: Arc<str> = "sentinel-secret".into();
        let snapshot = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
            RepositoryEnvironmentEntry::new("A", secret.clone()),
        ]))
        .unwrap();
        let carrier = RepositoryHostInputTransaction::new(
            snapshot,
            RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("A")]),
        );
        assert!(Arc::ptr_eq(carrier.snapshot().get("A").unwrap(), &secret));
        assert!(carrier.frontier().contains("A"));
        assert!(!format!("{carrier:?}").contains("sentinel-secret"));
    }

    #[test]
    fn retained_size_accounts_for_compact_names_and_deduplicates_shared_values() {
        fn retained_size(value: &dyn Allocative) -> usize {
            let mut graph = allocative::FlameGraphBuilder::default();
            graph.visit_root(value);
            graph.finish().flamegraph().total_size()
        }

        let text = "x".repeat(4096);
        let shared: Arc<str> = Arc::from(text.clone());
        let snapshot = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
            RepositoryEnvironmentEntry::new("LONG_REPOSITORY_ENVIRONMENT_NAME", shared.clone()),
        ]))
        .unwrap();
        let cell = RepositoryEnvironmentCell::observed(Some(shared));
        let distinct_cell = RepositoryEnvironmentCell::observed(Some(Arc::from(text)));
        let shared_size = retained_size(&(snapshot.clone(), cell));
        let distinct_size = retained_size(&(snapshot.clone(), distinct_cell));
        assert!(shared_size > 4096);
        assert!(distinct_size >= shared_size + 4096);
        assert!(retained_size(&snapshot) > retained_size(&RepositoryEnvironmentSnapshot::empty()));
    }
}
