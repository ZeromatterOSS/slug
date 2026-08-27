/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::BuiltinBazelToolsSnapshot;
use crate::HostCanonicalSelectedModuleDefinition;
use crate::HostCanonicalSelectedModuleKind;
use crate::HostRepositoryLocalPathPolicy;
use crate::HostSelectedExtensionOwner;
use crate::OverrideAttributeValue;
use crate::RepoSpec;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum HostCanonicalRepositoryRouteKind {
    Root,
    Builtin,
    SelectedRegistry,
    SelectedNonregistry,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryRouteSource {
    Builtin(BuiltinBazelToolsRouteIdentity),
    Selected(HostCanonicalSelectedModuleDefinition),
    Generated(HostCanonicalGeneratedRepositoryRoute),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostCanonicalGeneratedRepositoryRoute {
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    internal_name: CompactString,
    repo_spec: RepoSpec,
    mapping_context: CanonicalRepoName,
    mapping: Arc<SmallMap<ApparentRepoName, CanonicalRepoName>>,
}

/// One source-complete repository route addressed only by canonical identity.
///
/// The apparent names inside a repository mapping are contextual lookup keys;
/// no root-apparent alias participates in this route's own identity.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryRoute {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
    source: HostCanonicalRepositoryRouteSource,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostCanonicalRepositoryRouteView<'a> {
    route: &'a HostCanonicalRepositoryRoute,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostGeneratedRepositoryEffectSeed<'a> {
    owner: &'a Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
}

impl<'a> HostGeneratedRepositoryEffectSeed<'a> {
    pub fn owner(self) -> &'a Arc<HostSelectedExtensionOwner> {
        self.owner
    }

    pub fn ordinal(self) -> usize {
        self.ordinal
    }
}

impl HostCanonicalRepositoryRoute {
    pub fn from_selected(
        workspace: NormalizedAbsolutePath,
        definition: HostCanonicalSelectedModuleDefinition,
    ) -> Self {
        let canonical_repo = definition.view().canonical_repo().clone();
        Self {
            workspace,
            canonical_repo,
            source: HostCanonicalRepositoryRouteSource::Selected(definition),
        }
    }

    pub fn builtin(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            canonical_repo: CanonicalRepoName::new("bazel_tools")
                .expect("the pinned built-in canonical repository name is valid"),
            source: HostCanonicalRepositoryRouteSource::Builtin(
                BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generated(
        workspace: NormalizedAbsolutePath,
        canonical_repo: CanonicalRepoName,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
        internal_name: impl Into<CompactString>,
        repo_spec: RepoSpec,
        mapping_context: CanonicalRepoName,
        mapping: SmallMap<ApparentRepoName, CanonicalRepoName>,
    ) -> Option<Self> {
        if canonical_repo.is_root()
            || canonical_repo.as_str() == "bazel_tools"
            || mapping_context != canonical_repo
        {
            return None;
        }
        Some(Self {
            workspace,
            canonical_repo,
            source: HostCanonicalRepositoryRouteSource::Generated(
                HostCanonicalGeneratedRepositoryRoute {
                    owner,
                    ordinal,
                    internal_name: internal_name.into(),
                    repo_spec,
                    mapping_context,
                    mapping: Arc::new(mapping),
                },
            ),
        })
    }

    pub fn view(&self) -> HostCanonicalRepositoryRouteView<'_> {
        HostCanonicalRepositoryRouteView { route: self }
    }

    pub fn mapping_target(&self, apparent_repo: &ApparentRepoName) -> Option<&CanonicalRepoName> {
        match &self.source {
            HostCanonicalRepositoryRouteSource::Builtin(_) => None,
            HostCanonicalRepositoryRouteSource::Selected(definition) => definition
                .view()
                .mapping()
                .find_map(|(apparent, canonical)| (apparent == apparent_repo).then_some(canonical)),
            HostCanonicalRepositoryRouteSource::Generated(generated) => {
                generated.mapping.get(apparent_repo)
            }
        }
    }
}

impl<'a> HostCanonicalRepositoryRouteView<'a> {
    pub fn kind(self) -> HostCanonicalRepositoryRouteKind {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Builtin(_) => {
                HostCanonicalRepositoryRouteKind::Builtin
            }
            HostCanonicalRepositoryRouteSource::Selected(definition) => {
                match definition.view().kind() {
                    HostCanonicalSelectedModuleKind::Root => HostCanonicalRepositoryRouteKind::Root,
                    HostCanonicalSelectedModuleKind::SelectedRegistry => {
                        HostCanonicalRepositoryRouteKind::SelectedRegistry
                    }
                    HostCanonicalSelectedModuleKind::SelectedNonregistry => {
                        HostCanonicalRepositoryRouteKind::SelectedNonregistry
                    }
                }
            }
            HostCanonicalRepositoryRouteSource::Generated(_) => {
                HostCanonicalRepositoryRouteKind::Generated
            }
        }
    }

    pub fn workspace(self) -> &'a NormalizedAbsolutePath {
        &self.route.workspace
    }

    pub fn canonical_repo(self) -> &'a CanonicalRepoName {
        &self.route.canonical_repo
    }

    pub fn mapping_context(self) -> &'a CanonicalRepoName {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Builtin(_) => &self.route.canonical_repo,
            HostCanonicalRepositoryRouteSource::Selected(definition) => {
                definition.view().mapping_context()
            }
            HostCanonicalRepositoryRouteSource::Generated(generated) => &generated.mapping_context,
        }
    }

    pub fn repo_spec(self) -> Option<&'a RepoSpec> {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Builtin(_) => None,
            HostCanonicalRepositoryRouteSource::Selected(definition) => {
                definition.view().repo_spec()
            }
            HostCanonicalRepositoryRouteSource::Generated(generated) => Some(&generated.repo_spec),
        }
    }

    pub fn local_path_policy(self) -> Option<HostRepositoryLocalPathPolicy> {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Builtin(_) => None,
            HostCanonicalRepositoryRouteSource::Selected(definition) => {
                definition.view().local_path_policy()
            }
            HostCanonicalRepositoryRouteSource::Generated(_) => {
                Some(HostRepositoryLocalPathPolicy::LocalUnsupported)
            }
        }
    }

    pub fn internal_name(self) -> Option<&'a str> {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Generated(generated) => {
                Some(&generated.internal_name)
            }
            _ => None,
        }
    }

    pub fn builtin_identity(self) -> Option<BuiltinBazelToolsRouteIdentity> {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Builtin(identity) => Some(*identity),
            _ => None,
        }
    }

    pub fn generated_effect_seed(self) -> Option<HostGeneratedRepositoryEffectSeed<'a>> {
        match &self.route.source {
            HostCanonicalRepositoryRouteSource::Generated(generated) => {
                Some(HostGeneratedRepositoryEffectSeed {
                    owner: &generated.owner,
                    ordinal: generated.ordinal,
                })
            }
            _ => None,
        }
    }
}

fn hash_attribute_value<H: Hasher>(value: &OverrideAttributeValue, state: &mut H) {
    std::mem::discriminant(value).hash(state);
    match value {
        OverrideAttributeValue::None => {}
        OverrideAttributeValue::Bool(value) => value.hash(state),
        OverrideAttributeValue::Int(value) => value.hash(state),
        OverrideAttributeValue::String(value) => value.hash(state),
        OverrideAttributeValue::Label(value) => value.hash(state),
        OverrideAttributeValue::Iterable(values) => {
            values.len().hash(state);
            for value in values.iter() {
                hash_attribute_value(value, state);
            }
        }
        OverrideAttributeValue::Map(values) => hash_attribute_map(values, state),
    }
}

fn hash_attribute_map<H: Hasher, K: Hash>(
    values: &SmallMap<K, OverrideAttributeValue>,
    state: &mut H,
) {
    let mut hashes = values
        .iter()
        .map(|(key, value)| {
            let mut entry = DefaultHasher::new();
            key.hash(&mut entry);
            hash_attribute_value(value, &mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes.hash(state);
}

fn hash_repo_spec<H: Hasher>(spec: &RepoSpec, state: &mut H) {
    spec.rule_id.bzl_file.hash(state);
    spec.rule_id.rule_name.hash(state);
    hash_attribute_map(spec.attributes.as_ref(), state);
}

fn hash_mapping<H: Hasher>(mapping: impl Iterator<Item = (impl Hash, impl Hash)>, state: &mut H) {
    let mut hashes = mapping
        .map(|(apparent, canonical)| {
            let mut entry = DefaultHasher::new();
            apparent.hash(&mut entry);
            canonical.hash(&mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes.hash(state);
}

impl Hash for HostCanonicalRepositoryRoute {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.canonical_repo.hash(state);
        self.view().kind().hash(state);
        match &self.source {
            HostCanonicalRepositoryRouteSource::Builtin(identity) => identity.hash(state),
            HostCanonicalRepositoryRouteSource::Selected(definition) => {
                let view = definition.view();
                view.local_path_policy().hash(state);
                if let Some(spec) = view.repo_spec() {
                    hash_repo_spec(spec, state);
                }
                hash_mapping(view.mapping(), state);
            }
            HostCanonicalRepositoryRouteSource::Generated(generated) => {
                generated.owner.hash(state);
                generated.ordinal.hash(state);
                generated.internal_name.hash(state);
                hash_repo_spec(&generated.repo_spec, state);
                generated.mapping_context.hash(state);
                hash_mapping(generated.mapping.iter(), state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::HostCanonicalRepositoryRoute;
    use super::HostCanonicalRepositoryRouteKind;

    #[test]
    fn builtin_route_is_canonical_and_structural() {
        let workspace = NormalizedAbsolutePath::new("/canonical-route").unwrap();
        let first = HostCanonicalRepositoryRoute::builtin(workspace.clone());
        let warm = HostCanonicalRepositoryRoute::builtin(workspace.clone());
        let other = HostCanonicalRepositoryRoute::builtin(
            NormalizedAbsolutePath::new("/other-canonical-route").unwrap(),
        );
        assert_eq!(
            first.view().kind(),
            HostCanonicalRepositoryRouteKind::Builtin
        );
        assert_eq!(first.view().canonical_repo().as_str(), "bazel_tools");
        assert_eq!(first.view().workspace(), &workspace);
        assert!(first.view().builtin_identity().is_some());
        assert!(first.view().generated_effect_seed().is_none());
        assert_eq!(first, warm);
        assert_ne!(first, other);
        let digest = |value: &HostCanonicalRepositoryRoute| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(digest(&first), digest(&warm));
        assert_ne!(digest(&first), digest(&other));
    }

    #[test]
    fn canonical_route_identity_has_no_apparent_spelling() {
        let source = include_str!("canonical_repository_route.rs");
        let start = source
            .find("pub struct HostCanonicalRepositoryRoute {")
            .unwrap();
        let end = source[start..]
            .find("pub struct HostCanonicalRepositoryRouteView")
            .map(|offset| start + offset)
            .unwrap();
        let carrier = &source[start..end];
        assert!(carrier.contains("workspace: NormalizedAbsolutePath"));
        assert!(carrier.contains("canonical_repo: CanonicalRepoName"));
        assert!(carrier.contains("source: HostCanonicalRepositoryRouteSource"));
        assert!(!carrier.contains("ApparentRepoName"));
        assert!(!carrier.contains("apparent"));
    }
}
