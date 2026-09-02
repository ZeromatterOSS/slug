use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;

use super::*;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum HostSelectedExtensionOwnerKind {
    ModuleExtension,
    InnateRepositoryRule,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionOwner {
    id: HostSelectedExtensionId,
    unique_name: CanonicalRepoName,
}

impl HostSelectedExtensionOwner {
    pub fn kind(&self) -> HostSelectedExtensionOwnerKind {
        self.id.kind
    }

    #[doc(hidden)]
    pub fn unique_name(&self) -> &CanonicalRepoName {
        &self.unique_name
    }

    #[cfg(test)]
    pub(crate) fn testing(unique_name: &str) -> Self {
        Self::testing_with_kind(unique_name, HostSelectedExtensionOwnerKind::ModuleExtension)
    }

    #[cfg(test)]
    pub(crate) fn testing_with_kind(
        unique_name: &str,
        kind: HostSelectedExtensionOwnerKind,
    ) -> Self {
        Self {
            id: HostSelectedExtensionId {
                bzl_file: CanonicalLabel::parse("@@//:extension.bzl").unwrap(),
                extension_name: "extension".into(),
                isolation: None,
                kind,
            },
            unique_name: CanonicalRepoName::new(unique_name).unwrap(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDemand {
    requested: CanonicalRepoName,
    owner: Arc<HostSelectedExtensionOwner>,
}

impl HostSelectedExtensionDemand {
    pub fn requested(&self) -> &CanonicalRepoName {
        &self.requested
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum HostSelectedExtensionDemandErrorDisposition {
    Missing,
    Other,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionOwnerInputs {
    owner: Arc<HostSelectedExtensionOwner>,
    request: HostSelectedExtensionDefinitionLoadRequest,
    modules: Arc<[HostSelectedExtensionOwnerModuleInput]>,
    root_proxies: Option<Arc<[crate::NonrootExtensionProxy]>>,
}

impl HostSelectedExtensionOwnerInputs {
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn request(&self) -> &HostSelectedExtensionDefinitionLoadRequest {
        &self.request
    }
    pub fn modules(&self) -> &[HostSelectedExtensionOwnerModuleInput] {
        &self.modules
    }
    pub fn root_proxies(&self) -> Option<&[crate::NonrootExtensionProxy]> {
        self.root_proxies.as_deref()
    }
}

/// Graph-owned inputs for Bazel's synthetic `use_repo_rule` extension.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedInnateRepositoryOwnerInputs {
    owner: Arc<HostSelectedExtensionOwner>,
    unique_name: CanonicalRepoName,
    bzl_file: CanonicalLabel,
    rule_name: CompactString,
    label_conversion_base: CanonicalLabel,
    base_mapping: HostSelectedRepositoryMapping,
    mapping: HostSelectedRepositoryMapping,
    tags: Arc<[crate::NonrootExtensionTag]>,
    imports: Arc<[HostSelectedExtensionDefinitionImport]>,
    overrides: Arc<[HostSelectedExtensionDefinitionOverride]>,
}

impl HostSelectedInnateRepositoryOwnerInputs {
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }

    pub fn definition_parts(
        &self,
    ) -> (
        &CanonicalLabel,
        &str,
        &CanonicalLabel,
        &SmallMap<ApparentRepoName, CanonicalRepoName>,
    ) {
        (
            &self.bzl_file,
            &self.rule_name,
            &self.label_conversion_base,
            &self.mapping.entries,
        )
    }

    pub fn namespace_parts(
        &self,
    ) -> (
        &CanonicalRepoName,
        &CanonicalRepoName,
        &SmallMap<ApparentRepoName, CanonicalRepoName>,
        &[HostSelectedExtensionDefinitionOverride],
    ) {
        (
            &self.unique_name,
            &self.base_mapping.context_repo,
            &self.base_mapping.entries,
            &self.overrides,
        )
    }

    pub fn validation_parts(
        &self,
    ) -> (
        &[HostSelectedExtensionDefinitionImport],
        &[HostSelectedExtensionDefinitionOverride],
    ) {
        (&self.imports, &self.overrides)
    }

    pub fn tags(&self) -> &[crate::NonrootExtensionTag] {
        &self.tags
    }
}

/// One selected-owner use, kept separate from the legacy root-only aggregate input.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionOwnerModuleInput {
    canonical_repo: CanonicalRepoName,
    name: CompactString,
    version: CompactString,
    is_root: bool,
    mapping: HostSelectedRepositoryMapping,
    tags: Arc<[crate::NonrootExtensionTag]>,
}

impl HostSelectedExtensionOwnerModuleInput {
    pub fn parts(
        &self,
    ) -> (
        &CanonicalRepoName,
        &str,
        &str,
        bool,
        &SmallMap<ApparentRepoName, CanonicalRepoName>,
        &[crate::NonrootExtensionTag],
    ) {
        (
            &self.canonical_repo,
            &self.name,
            &self.version,
            self.is_root,
            &self.mapping.entries,
            &self.tags,
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionDemandError(DemandError);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DemandError {
    Mappings(HostSelectedExtensionMappingsError),
    Missing {
        requested: CanonicalRepoName,
    },
    Ambiguous {
        requested: CanonicalRepoName,
        first: Arc<HostSelectedExtensionOwner>,
        conflicting: Arc<HostSelectedExtensionOwner>,
    },
    Inconsistent {
        requested: CanonicalRepoName,
        owner: Arc<HostSelectedExtensionOwner>,
    },
}

impl fmt::Display for HostSelectedExtensionDemandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl HostSelectedExtensionDemandError {
    pub fn disposition(&self) -> HostSelectedExtensionDemandErrorDisposition {
        match self.0 {
            DemandError::Missing { .. } => HostSelectedExtensionDemandErrorDisposition::Missing,
            _ => HostSelectedExtensionDemandErrorDisposition::Other,
        }
    }
}
impl std::error::Error for HostSelectedExtensionDemandError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedExtensionOwnerInputsError(OwnerInputsError);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum OwnerInputsError {
    Mappings(HostSelectedExtensionMappingsError),
    Missing {
        owner: Arc<HostSelectedExtensionOwner>,
    },
    Inconsistent {
        owner: Arc<HostSelectedExtensionOwner>,
    },
    Unsupported {
        owner: Arc<HostSelectedExtensionOwner>,
    },
    Invalid {
        owner: Arc<HostSelectedExtensionOwner>,
        message: CompactString,
    },
}

impl fmt::Display for HostSelectedExtensionOwnerInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl std::error::Error for HostSelectedExtensionOwnerInputsError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedInnateRepositoryOwnerInputsError(OwnerInputsError);

impl fmt::Display for HostSelectedInnateRepositoryOwnerInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for HostSelectedInnateRepositoryOwnerInputsError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionDemandKey {
    workspace: NormalizedAbsolutePath,
    requested: CanonicalRepoName,
}

impl HostSelectedExtensionDemandKey {
    pub fn new(workspace: NormalizedAbsolutePath, requested: CanonicalRepoName) -> Self {
        Self {
            workspace,
            requested,
        }
    }
}
impl fmt::Display for HostSelectedExtensionDemandKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-extension-demand:{}:{}",
            self.workspace, self.requested
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionDemandObservationKey(HostSelectedExtensionDemandKey);
impl HostSelectedExtensionDemandObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, requested: CanonicalRepoName) -> Self {
        Self(HostSelectedExtensionDemandKey::new(workspace, requested))
    }
}
impl fmt::Display for HostSelectedExtensionDemandObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedExtensionDemand {
    result: Arc<Result<HostSelectedExtensionDemand, HostSelectedExtensionDemandError>>,
    observations: PathObservationEpoch,
}
impl ObservedHostSelectedExtensionDemand {
    pub fn result(
        &self,
    ) -> &Arc<Result<HostSelectedExtensionDemand, HostSelectedExtensionDemandError>> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum DemandObservationError {
    Mappings(ExtensionMappingsObservationError),
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostSelectedExtensionDemandObservationError(DemandObservationError);

impl HostSelectedExtensionDemandObservationError {
    #[doc(hidden)]
    pub fn selected_frontier(&self) -> crate::HostSelectedObservationFrontier {
        match &self.0 {
            DemandObservationError::Mappings(error) => {
                super::extension_mappings_observation_frontier(error)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionOwnerInputsKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
}
impl HostSelectedExtensionOwnerInputsKey {
    pub fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self {
        Self { workspace, owner }
    }
}
impl fmt::Display for HostSelectedExtensionOwnerInputsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-extension-owner-inputs:{}:{}",
            self.workspace, self.owner.unique_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionOwnerInputsObservationKey(HostSelectedExtensionOwnerInputsKey);
impl HostSelectedExtensionOwnerInputsObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self {
        Self(HostSelectedExtensionOwnerInputsKey::new(workspace, owner))
    }
}

#[doc(hidden)]
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedInnateRepositoryOwnerInputsKey { workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner> }
#[rustfmt::skip]
impl HostSelectedInnateRepositoryOwnerInputsKey { pub fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self { workspace, owner } } }
#[rustfmt::skip]
impl fmt::Display for HostSelectedInnateRepositoryOwnerInputsKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "host-selected-innate-repository-owner-inputs:{}:{}", self.workspace, self.owner.unique_name) } }

#[doc(hidden)]
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedInnateRepositoryOwnerInputsObservationKey(HostSelectedInnateRepositoryOwnerInputsKey);
#[rustfmt::skip]
impl HostSelectedInnateRepositoryOwnerInputsObservationKey { pub fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self(HostSelectedInnateRepositoryOwnerInputsKey::new(workspace, owner)) } }
#[rustfmt::skip]
impl fmt::Display for HostSelectedInnateRepositoryOwnerInputsObservationKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) } }
impl fmt::Display for HostSelectedExtensionOwnerInputsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedExtensionOwnerInputs {
    result: Arc<Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError>>,
    observations: PathObservationEpoch,
}

#[doc(hidden)]
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedInnateRepositoryOwnerInputs { result: Arc<Result<HostSelectedInnateRepositoryOwnerInputs, HostSelectedInnateRepositoryOwnerInputsError>>, observations: PathObservationEpoch }
#[rustfmt::skip]
impl ObservedHostSelectedInnateRepositoryOwnerInputs { pub fn result(&self) -> &Arc<Result<HostSelectedInnateRepositoryOwnerInputs, HostSelectedInnateRepositoryOwnerInputsError>> { &self.result } pub fn observations(&self) -> &PathObservationEpoch { &self.observations } }
impl ObservedHostSelectedExtensionOwnerInputs {
    pub fn result(
        &self,
    ) -> &Arc<Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError>> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum OwnerInputsObservationError {
    Mappings(ExtensionMappingsObservationError),
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostSelectedExtensionOwnerInputsObservationError(OwnerInputsObservationError);

impl HostSelectedExtensionOwnerInputsObservationError {
    #[doc(hidden)]
    pub fn selected_frontier(&self) -> crate::HostSelectedObservationFrontier {
        match &self.0 {
            OwnerInputsObservationError::Mappings(error) => {
                super::extension_mappings_observation_frontier(error)
            }
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostSelectedInnateRepositoryOwnerInputsObservationError(OwnerInputsObservationError);

impl HostSelectedInnateRepositoryOwnerInputsObservationError {
    #[doc(hidden)]
    pub fn selected_frontier(&self) -> crate::HostSelectedObservationFrontier {
        match &self.0 {
            OwnerInputsObservationError::Mappings(error) => {
                super::extension_mappings_observation_frontier(error)
            }
        }
    }
}

type MappingChild = SourcePreparationOutcome<
    Result<(ExtensionMappingsResult, PathObservationEpoch), ExtensionMappingsObservationError>,
>;

async fn mappings_child(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    observed: bool,
) -> MappingChild {
    if !observed {
        return match ctx
            .compute(&HostSelectedExtensionMappingsKey::new(workspace.dupe()))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => {
                SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
            }
            Err(error) => SourcePreparationOutcome::Complete(Ok((
                Arc::new(Err(HostSelectedExtensionMappingsError::RoutesCompute(
                    error.to_string().into(),
                ))),
                PathObservationEpoch::empty(),
            ))),
        };
    }
    match ctx
        .compute(&HostSelectedExtensionMappingsObservationKey::new(
            workspace.dupe(),
        ))
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(Err(error))) => {
            SourcePreparationOutcome::Complete(Err(error))
        }
        Ok(SourcePreparationOutcome::Complete(Ok(value))) => SourcePreparationOutcome::Complete(
            Ok((value.result().dupe(), value.observations().dupe())),
        ),
        Err(error) => SourcePreparationOutcome::Complete(Ok((
            Arc::new(Err(HostSelectedExtensionMappingsError::RoutesCompute(
                error.to_string().into(),
            ))),
            PathObservationEpoch::empty(),
        ))),
    }
}

fn demand_from_mappings(
    requested: &CanonicalRepoName,
    mappings: &HostSelectedExtensionMappings,
) -> Result<HostSelectedExtensionDemand, HostSelectedExtensionDemandError> {
    let mut found = None;
    for usage in mappings.usages.iter().filter(|usage| {
        let suffix = requested
            .as_str()
            .strip_prefix(usage.unique_name.as_str())
            .and_then(|suffix| suffix.strip_prefix('+'));
        suffix.is_some_and(|internal_name| !internal_name.is_empty())
            || usage.imports.values().any(|name| name == requested)
    }) {
        let owner = Arc::new(HostSelectedExtensionOwner {
            id: usage.id.clone(),
            unique_name: usage.unique_name.clone(),
        });
        match &found {
            None => found = Some(owner),
            Some(first) if first.id == owner.id && first.unique_name == owner.unique_name => {}
            Some(first) if first.id == owner.id => {
                return Err(HostSelectedExtensionDemandError(
                    DemandError::Inconsistent {
                        requested: requested.clone(),
                        owner: first.dupe(),
                    },
                ));
            }
            Some(first) => {
                return Err(HostSelectedExtensionDemandError(DemandError::Ambiguous {
                    requested: requested.clone(),
                    first: first.dupe(),
                    conflicting: owner,
                }));
            }
        }
    }
    found
        .map(|owner| HostSelectedExtensionDemand {
            requested: requested.clone(),
            owner,
        })
        .ok_or_else(|| {
            HostSelectedExtensionDemandError(DemandError::Missing {
                requested: requested.clone(),
            })
        })
}

fn owner_inputs(
    owner: Arc<HostSelectedExtensionOwner>,
    mappings: &HostSelectedExtensionMappings,
) -> Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError> {
    let uses = mappings
        .usages
        .iter()
        .filter(|usage| usage.id == owner.id)
        .collect::<Vec<_>>();
    if uses.is_empty()
        || uses
            .iter()
            .any(|usage| usage.unique_name != owner.unique_name)
    {
        return Err(HostSelectedExtensionOwnerInputsError(
            OwnerInputsError::Inconsistent { owner },
        ));
    }
    if owner.kind() != HostSelectedExtensionOwnerKind::ModuleExtension
        || owner.id.isolation.is_some()
        || owner.id.extension_name.split_ascii_whitespace().count() != 1
    {
        return Err(HostSelectedExtensionOwnerInputsError(
            OwnerInputsError::Unsupported { owner },
        ));
    }
    let definition_repo = owner.id.bzl_file.package().repo();
    let host_ordinal =
        match find_canonical_route_ordinal(definition_repo, mappings.routes.entries.iter()) {
            CanonicalRouteMatch::Unique(ordinal) => ordinal,
            CanonicalRouteMatch::Missing => {
                return Err(HostSelectedExtensionOwnerInputsError(
                    OwnerInputsError::Missing { owner },
                ));
            }
            CanonicalRouteMatch::Duplicate { .. } => {
                return Err(HostSelectedExtensionOwnerInputsError(
                    OwnerInputsError::Inconsistent { owner },
                ));
            }
        };
    let mapping = mappings.mappings.get(host_ordinal).ok_or_else(|| {
        HostSelectedExtensionOwnerInputsError(OwnerInputsError::Missing {
            owner: owner.dupe(),
        })
    })?;
    if &mapping.context_repo != definition_repo {
        return Err(HostSelectedExtensionOwnerInputsError(
            OwnerInputsError::Inconsistent { owner },
        ));
    }
    let imports = uses
        .iter()
        .flat_map(|usage| usage.validation_imports.iter().cloned())
        .collect();
    let overrides = mappings
        .overrides
        .iter()
        .filter(|value| value.id == owner.id)
        .map(|value| HostSelectedExtensionDefinitionOverride {
            generated_name: value.generated_name.clone(),
            replacement: value.replacement.clone(),
            must_exist: value.must_exist,
            location: value.location.clone(),
        })
        .collect();
    let request = HostSelectedExtensionDefinitionLoadRequest {
        bzl_file: owner.id.bzl_file.clone(),
        extension_name: owner.id.extension_name.clone(),
        unique_name: owner.unique_name.clone(),
        base_mapping: mapping.clone(),
        mapping: mapping.clone(),
        source: selected_extension_definition_source(&mappings.routes, mapping, &owner.id.bzl_file)
            .ok_or_else(|| {
                HostSelectedExtensionOwnerInputsError(OwnerInputsError::Unsupported {
                    owner: owner.dupe(),
                })
            })?,
        imports,
        overrides,
    };
    let mut modules = Vec::new();
    let mut root_proxies = None;
    for (route_index, route) in mappings.routes.entries.iter().enumerate() {
        let matching = uses.iter().any(|usage| usage.owner == route.entry.key);
        if !matching {
            continue;
        }
        match &route.entry.source {
            HostGraphModuleSource::Root(module) => {
                let header = module.header.as_ref().ok_or_else(|| {
                    HostSelectedExtensionOwnerInputsError(OwnerInputsError::Missing {
                        owner: owner.dupe(),
                    })
                })?;
                let matching_usages = mappings
                    .root_usages
                    .iter()
                    .filter(|usage| {
                        usage.isolation.is_none()
                            && usage.extension_name == owner.id.extension_name
                            && resolve_extension_label(
                                &HostGraphModuleKey::Root,
                                usage.bzl_label.as_str(),
                                &route.mapping,
                            )
                            .is_ok_and(|label| label == owner.id.bzl_file)
                    })
                    .collect::<Vec<_>>();
                let tags = matching_usages
                    .iter()
                    .flat_map(|usage| usage.tags.iter().cloned())
                    .collect();
                root_proxies = Some(
                    matching_usages
                        .iter()
                        .flat_map(|usage| usage.proxies.iter().cloned())
                        .collect(),
                );
                let mapping = mappings.mappings.get(route_index).ok_or_else(|| {
                    HostSelectedExtensionOwnerInputsError(OwnerInputsError::Missing {
                        owner: owner.dupe(),
                    })
                })?;
                let version = crate::module_version::BazelModuleVersion::parse(
                    header.version.as_deref().unwrap_or_default(),
                )
                .map_err(|error| {
                    HostSelectedExtensionOwnerInputsError(OwnerInputsError::Invalid {
                        owner: owner.dupe(),
                        message: error.to_string().into(),
                    })
                })?;
                modules.push(HostSelectedExtensionOwnerModuleInput {
                    canonical_repo: route.canonical_repo.clone(),
                    name: header.name.clone(),
                    version: version.normalized().into(),
                    is_root: true,
                    mapping: mapping.clone(),
                    tags,
                });
            }
            HostGraphModuleSource::Discovered(module) => {
                let tags = module
                    .module
                    .extension_usages
                    .iter()
                    .filter(|usage| {
                        usage.isolation.is_none()
                            && usage.extension_name == owner.id.extension_name
                            && resolve_extension_label(
                                &route.entry.key,
                                usage.bzl_label.as_str(),
                                &route.mapping,
                            )
                            .is_ok_and(|label| label == owner.id.bzl_file)
                    })
                    .flat_map(|usage| usage.tags.iter().cloned())
                    .collect();
                let mapping = mappings.mappings.get(route_index).ok_or_else(|| {
                    HostSelectedExtensionOwnerInputsError(OwnerInputsError::Missing {
                        owner: owner.dupe(),
                    })
                })?;
                let HostGraphModuleKey::Module { version, .. } = &route.entry.key else {
                    return Err(HostSelectedExtensionOwnerInputsError(
                        OwnerInputsError::Invalid {
                            owner: owner.dupe(),
                            message: "nonroot route has a root key".into(),
                        },
                    ));
                };
                modules.push(HostSelectedExtensionOwnerModuleInput {
                    canonical_repo: route.canonical_repo.clone(),
                    name: module.module.base.declared_name.clone(),
                    version: version.normalized().into(),
                    is_root: false,
                    mapping: mapping.clone(),
                    tags,
                });
            }
        }
    }
    Ok(HostSelectedExtensionOwnerInputs {
        owner,
        request,
        modules: modules.into(),
        root_proxies,
    })
}

fn innate_owner_inputs(
    owner: Arc<HostSelectedExtensionOwner>,
    mappings: &HostSelectedExtensionMappings,
) -> Result<HostSelectedInnateRepositoryOwnerInputs, HostSelectedInnateRepositoryOwnerInputsError> {
    let fail = |kind| HostSelectedInnateRepositoryOwnerInputsError(kind);
    let uses = mappings
        .usages
        .iter()
        .filter(|usage| usage.id == owner.id)
        .collect::<Vec<_>>();
    let owner_key = uses.first().map(|usage| &usage.owner);
    if owner.kind() != HostSelectedExtensionOwnerKind::InnateRepositoryRule
        || owner.id.isolation.is_some()
        || owner_key.is_none()
        || uses
            .iter()
            .any(|usage| Some(&usage.owner) != owner_key || usage.unique_name != owner.unique_name)
    {
        return Err(fail(OwnerInputsError::Unsupported { owner }));
    }
    let owner_key = owner_key.expect("nonempty uses checked");
    let route_index = mappings
        .routes
        .entries
        .iter()
        .position(|route| &route.entry.key == owner_key)
        .ok_or_else(|| {
            fail(OwnerInputsError::Missing {
                owner: owner.dupe(),
            })
        })?;
    let base_mapping = mappings.base_mappings.get(route_index).ok_or_else(|| {
        fail(OwnerInputsError::Missing {
            owner: owner.dupe(),
        })
    })?;
    let mapping = mappings.mappings.get(route_index).ok_or_else(|| {
        fail(OwnerInputsError::Missing {
            owner: owner.dupe(),
        })
    })?;
    let (raw_label, rule_name) = owner
        .id
        .extension_name
        .rsplit_once(' ')
        .filter(|(label, name)| !label.is_empty() && !name.is_empty())
        .ok_or_else(|| {
            fail(OwnerInputsError::Invalid {
                owner: owner.dupe(),
                message: "innate repository rule identity lacks label or name".into(),
            })
        })?;
    if rule_name.starts_with('_') {
        return Err(fail(OwnerInputsError::Invalid {
            owner,
            message: "innate repository rule name is private".into(),
        }));
    }
    let bzl_file = resolve_extension_label(owner_key, raw_label, mapping).map_err(|error| {
        fail(OwnerInputsError::Invalid {
            owner: owner.dupe(),
            message: error.to_string().into(),
        })
    })?;
    let rule_name = CompactString::from(rule_name);
    let route = &mappings.routes.entries[route_index];
    let tags: Arc<[crate::NonrootExtensionTag]> = match &route.entry.source {
        HostGraphModuleSource::Root(_) => mappings
            .root_usages
            .iter()
            .filter(|candidate| {
                candidate.isolation.is_none()
                    && candidate.extension_name == owner.id.extension_name
                    && resolve_extension_label(
                        &HostGraphModuleKey::Root,
                        candidate.bzl_label.as_str(),
                        &route.mapping,
                    )
                    .is_ok_and(|label| label == owner.id.bzl_file)
            })
            .flat_map(|candidate| candidate.tags.iter().cloned())
            .collect(),
        HostGraphModuleSource::Discovered(module) => module
            .module
            .extension_usages
            .iter()
            .filter(|candidate| {
                candidate.isolation.is_none()
                    && candidate.extension_name == owner.id.extension_name
                    && resolve_extension_label(
                        &route.entry.key,
                        candidate.bzl_label.as_str(),
                        &route.mapping,
                    )
                    .is_ok_and(|label| label == owner.id.bzl_file)
            })
            .flat_map(|candidate| candidate.tags.iter().cloned())
            .collect(),
    };
    if tags.is_empty() {
        return Err(fail(OwnerInputsError::Invalid {
            owner,
            message: "innate repository rule owner has no retained repo call".into(),
        }));
    }
    let overrides = mappings
        .overrides
        .iter()
        .filter(|value| value.id == owner.id)
        .map(|value| HostSelectedExtensionDefinitionOverride {
            generated_name: value.generated_name.clone(),
            replacement: value.replacement.clone(),
            must_exist: value.must_exist,
            location: value.location.clone(),
        })
        .collect();
    Ok(HostSelectedInnateRepositoryOwnerInputs {
        unique_name: owner.unique_name.clone(),
        label_conversion_base: owner.id.bzl_file.clone(),
        owner,
        bzl_file,
        rule_name,
        base_mapping: base_mapping.clone(),
        mapping: mapping.clone(),
        tags,
        imports: uses
            .iter()
            .flat_map(|usage| usage.validation_imports.iter().cloned())
            .collect(),
        overrides,
    })
}

async fn demand_compute(
    key: &HostSelectedExtensionDemandKey,
    ctx: &mut DiceComputations<'_>,
    observed: bool,
) -> SourcePreparationOutcome<
    Result<
        (
            Arc<Result<HostSelectedExtensionDemand, HostSelectedExtensionDemandError>>,
            PathObservationEpoch,
        ),
        DemandObservationError,
    >,
> {
    match mappings_child(ctx, &key.workspace, observed).await {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(DemandObservationError::Mappings(error)))
        }
        SourcePreparationOutcome::Complete(Ok((result, epoch))) => {
            SourcePreparationOutcome::Complete(Ok((
                Arc::new(match result.as_ref() {
                    Ok(mappings) => demand_from_mappings(&key.requested, mappings),
                    Err(error) => Err(HostSelectedExtensionDemandError(DemandError::Mappings(
                        error.clone(),
                    ))),
                }),
                epoch,
            )))
        }
    }
}
async fn inputs_compute(
    key: &HostSelectedExtensionOwnerInputsKey,
    ctx: &mut DiceComputations<'_>,
    observed: bool,
) -> SourcePreparationOutcome<
    Result<
        (
            Arc<Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError>>,
            PathObservationEpoch,
        ),
        OwnerInputsObservationError,
    >,
> {
    finish_inputs_mappings_child(
        key.owner.dupe(),
        mappings_child(ctx, &key.workspace, observed).await,
    )
}

fn finish_inputs_mappings_child(
    owner: Arc<HostSelectedExtensionOwner>,
    child: MappingChild,
) -> SourcePreparationOutcome<
    Result<
        (
            Arc<Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError>>,
            PathObservationEpoch,
        ),
        OwnerInputsObservationError,
    >,
> {
    match child {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(OwnerInputsObservationError::Mappings(error)))
        }
        SourcePreparationOutcome::Complete(Ok((result, epoch))) => {
            SourcePreparationOutcome::Complete(Ok((
                Arc::new(match result.as_ref() {
                    Ok(mappings) => owner_inputs(owner, mappings),
                    Err(error) => Err(HostSelectedExtensionOwnerInputsError(
                        OwnerInputsError::Mappings(error.clone()),
                    )),
                }),
                epoch,
            )))
        }
    }
}

async fn innate_inputs_compute(
    key: &HostSelectedInnateRepositoryOwnerInputsKey,
    ctx: &mut DiceComputations<'_>,
    observed: bool,
) -> SourcePreparationOutcome<
    Result<
        (
            Arc<
                Result<
                    HostSelectedInnateRepositoryOwnerInputs,
                    HostSelectedInnateRepositoryOwnerInputsError,
                >,
            >,
            PathObservationEpoch,
        ),
        OwnerInputsObservationError,
    >,
> {
    match mappings_child(ctx, &key.workspace, observed).await {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(OwnerInputsObservationError::Mappings(error)))
        }
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            SourcePreparationOutcome::Complete(Ok((
                Arc::new(match result.as_ref() {
                    Ok(mappings) => innate_owner_inputs(key.owner.dupe(), mappings),
                    Err(error) => Err(HostSelectedInnateRepositoryOwnerInputsError(
                        OwnerInputsError::Mappings(error.clone()),
                    )),
                }),
                observations,
            )))
        }
    }
}

#[async_trait]
impl Key for HostSelectedExtensionDemandKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<HostSelectedExtensionDemand, HostSelectedExtensionDemandError>>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match demand_compute(self, ctx, false).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => unreachable!(),
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[async_trait]
impl Key for HostSelectedExtensionDemandObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostSelectedExtensionDemand, HostSelectedExtensionDemandObservationError>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match demand_compute(&self.0, ctx, true).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostSelectedExtensionDemandObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedExtensionDemand {
                    result,
                    observations,
                }))
            }
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[async_trait]
impl Key for HostSelectedExtensionOwnerInputsKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<HostSelectedExtensionOwnerInputs, HostSelectedExtensionOwnerInputsError>>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match inputs_compute(self, ctx, false).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => unreachable!(),
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[async_trait]
impl Key for HostSelectedExtensionOwnerInputsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedExtensionOwnerInputs,
            HostSelectedExtensionOwnerInputsObservationError,
        >,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match inputs_compute(&self.0, ctx, true).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostSelectedExtensionOwnerInputsObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedExtensionOwnerInputs {
                    result,
                    observations,
                }))
            }
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedInnateRepositoryOwnerInputsKey {
    type Value = SourcePreparationOutcome<
        Arc<
            Result<
                HostSelectedInnateRepositoryOwnerInputs,
                HostSelectedInnateRepositoryOwnerInputsError,
            >,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match innate_inputs_compute(self, ctx, false).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => unreachable!(),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedInnateRepositoryOwnerInputsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedInnateRepositoryOwnerInputs,
            HostSelectedInnateRepositoryOwnerInputsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match innate_inputs_compute(&self.0, ctx, true).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedInnateRepositoryOwnerInputsObservationError(error),
                ))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostSelectedInnateRepositoryOwnerInputs {
                        result,
                        observations,
                    },
                ))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interim_module::NonrootModuleBuilder;
    use crate::interim_module::NonrootModuleKey;
    use crate::module_eval::EvaluatedRootModule;
    use crate::module_eval::RootModuleHeader;
    use crate::module_eval::RootModuleRegistrations;
    use crate::source_preparation::HostDiscoveredModule;
    use crate::source_preparation::HostDiscoveredModuleProvenance;

    fn owner_id(name: &str) -> HostSelectedExtensionId {
        HostSelectedExtensionId {
            bzl_file: CanonicalLabel::parse("@@//:extension.bzl").unwrap(),
            extension_name: name.into(),
            isolation: None,
            kind: HostSelectedExtensionOwnerKind::ModuleExtension,
        }
    }

    fn mappings(
        usages: impl IntoIterator<Item = HostSelectedExtensionUsage>,
    ) -> HostSelectedExtensionMappings {
        HostSelectedExtensionMappings {
            routes: Arc::new(HostSelectedModuleRoutes {
                entries: Arc::from([]),
                extension_projection: testing_extension_mapping_projection(),
            }),
            root_usages: Arc::from([]),
            usages: usages.into_iter().collect(),
            overrides: Arc::from([]),
            base_mappings: Arc::from([]),
            mappings: Arc::from([]),
        }
    }

    fn span() -> crate::LogicalSpan {
        crate::LogicalSpan {
            file: crate::LogicalModuleFileId::new("/MODULE.bazel"),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
        }
    }

    fn tag(name: &str) -> crate::NonrootExtensionTag {
        crate::NonrootExtensionTag {
            tag_class: name.into(),
            attributes: Arc::new(SmallMap::new()),
            dev_dependency: false,
            location: span(),
        }
    }

    fn repo_tag(name: &str) -> crate::NonrootExtensionTag {
        let mut value = tag("repo");
        value.attributes = Arc::new(SmallMap::from_iter([(
            CompactString::from("name"),
            crate::NonrootAttributeValue::String(name.into()),
        )]));
        value
    }

    fn owner_inputs_mappings() -> (
        Arc<HostSelectedExtensionOwner>,
        HostSelectedExtensionMappings,
    ) {
        let id = owner_id("extension");
        let owner = Arc::new(HostSelectedExtensionOwner {
            id: id.clone(),
            unique_name: CanonicalRepoName::new("+extension").unwrap(),
        });
        let root_key = HostGraphModuleKey::Root;
        let dep_key = HostGraphModuleKey::Module {
            name: "dep".into(),
            version: crate::module_version::BazelModuleVersion::parse("1.0+build").unwrap(),
        };
        let root_final = HostSelectedRepositoryMapping {
            context_repo: CanonicalRepoName::root(),
            entries: Arc::new(SmallMap::from_iter([(
                ApparentRepoName::new("generated").unwrap(),
                CanonicalRepoName::new("+final").unwrap(),
            )])),
            order: Arc::from([ApparentRepoName::new("generated").unwrap()]),
        };
        let root_base = HostSelectedRepositoryMapping {
            context_repo: CanonicalRepoName::root(),
            entries: Arc::new(SmallMap::from_iter([(
                ApparentRepoName::new("generated").unwrap(),
                CanonicalRepoName::new("+base").unwrap(),
            )])),
            order: Arc::from([ApparentRepoName::new("generated").unwrap()]),
        };
        let dep_final = HostSelectedRepositoryMapping {
            context_repo: CanonicalRepoName::new("dep+1.0").unwrap(),
            entries: Arc::new(SmallMap::from_iter([(
                ApparentRepoName::new("root").unwrap(),
                CanonicalRepoName::root(),
            )])),
            order: Arc::from([ApparentRepoName::new("root").unwrap()]),
        };
        let root_entry = HostSelectedModuleEntry {
            key: root_key.clone(),
            source: HostGraphModuleSource::Root(Arc::new(EvaluatedRootModule {
                header: Some(RootModuleHeader {
                    name: "root".into(),
                    version: Some("1.0+build".into()),
                    repo_name: None,
                }),
                dependencies: Arc::from([]),
                registrations: RootModuleRegistrations::default(),
            })),
            dependencies: Arc::from([]),
            original_dependencies: Arc::from([]),
            nodep_dependencies: Arc::from([]),
        };
        let mut nonroot = NonrootModuleBuilder::new(
            NonrootModuleKey::new("dep", "1.0+build"),
            "dep",
            "1.0+build",
            "dep",
        )
        .build()
        .unwrap();
        nonroot.extension_usages = Arc::from([crate::NonrootExtensionUsage {
            bzl_label: "@root//:extension.bzl".into(),
            extension_name: "extension".into(),
            proxies: Arc::from([]),
            tags: Arc::from([tag("dep")]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: None,
        }]);
        let dep_entry = HostSelectedModuleEntry {
            key: dep_key.clone(),
            source: HostGraphModuleSource::Discovered(Arc::new(HostDiscoveredModule {
                module: nonroot,
                provenance: HostDiscoveredModuleProvenance::Registry {
                    selected_registry: crate::RegistryBaseUrl::new("https://registry.invalid"),
                    module_file_attempts: Arc::from([]),
                },
            })),
            dependencies: Arc::from([]),
            original_dependencies: Arc::from([]),
            nodep_dependencies: Arc::from([]),
        };
        let root_usage = crate::module_eval::RootExtensionUsage {
            bzl_label: "//:extension.bzl".into(),
            extension_name: "extension".into(),
            proxies: Arc::from([]),
            tags: Arc::from([tag("root-one"), tag("root-two")]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: None,
        };
        let request = "+extension+generated";
        (
            owner,
            HostSelectedExtensionMappings {
                routes: Arc::new(HostSelectedModuleRoutes {
                    entries: Arc::from([
                        HostSelectedModuleRoute {
                            entry: root_entry,
                            canonical_repo: CanonicalRepoName::root(),
                            mapping: root_final.clone(),
                            registry_repo_spec: None,
                        },
                        HostSelectedModuleRoute {
                            entry: dep_entry,
                            canonical_repo: CanonicalRepoName::new("dep+1.0").unwrap(),
                            mapping: dep_final.clone(),
                            registry_repo_spec: None,
                        },
                    ]),
                    extension_projection: testing_extension_mapping_projection(),
                }),
                root_usages: Arc::from([root_usage]),
                usages: Arc::from([
                    usage(root_key, id.clone(), "+extension", request),
                    usage(dep_key, id, "+extension", request),
                ]),
                overrides: Arc::from([]),
                base_mappings: Arc::from([root_base, dep_final.clone()]),
                mappings: Arc::from([root_final, dep_final]),
            },
        )
    }

    fn usage(
        owner: HostGraphModuleKey,
        id: HostSelectedExtensionId,
        unique_name: &str,
        imported: &str,
    ) -> HostSelectedExtensionUsage {
        HostSelectedExtensionUsage {
            owner,
            id,
            unique_name: CanonicalRepoName::new(unique_name).unwrap(),
            imports: Arc::new(SmallMap::from_iter([(
                ApparentRepoName::new("local").unwrap(),
                CanonicalRepoName::new(imported).unwrap(),
            )])),
            validation_imports: Arc::from([]),
        }
    }

    #[test]
    fn demand_authenticates_owner_namespaces_and_has_typed_dispositions() {
        let requested = CanonicalRepoName::new("+extension+generated").unwrap();
        let owner = owner_id("extension");
        let result = demand_from_mappings(
            &requested,
            &mappings([
                usage(
                    HostGraphModuleKey::Root,
                    owner.clone(),
                    "+extension",
                    "+extension+generated",
                ),
                usage(
                    HostGraphModuleKey::Root,
                    owner,
                    "+extension",
                    "+extension+generated",
                ),
            ]),
        )
        .unwrap();
        assert_eq!(result.requested(), &requested);
        assert_eq!(
            result.owner().unique_name,
            CanonicalRepoName::new("+extension").unwrap()
        );

        let sibling = CanonicalRepoName::new("+extension+sibling").unwrap();
        let sibling_result = demand_from_mappings(
            &sibling,
            &mappings([usage(
                HostGraphModuleKey::Root,
                owner_id("extension"),
                "+extension",
                "+extension+generated",
            )]),
        )
        .unwrap();
        assert_eq!(sibling_result.requested(), &sibling);
        assert_eq!(sibling_result.owner().unique_name().as_str(), "+extension");

        let colliding = demand_from_mappings(
            &CanonicalRepoName::new("+extension+sibling").unwrap(),
            &mappings([usage(
                HostGraphModuleKey::Root,
                owner_id("ext"),
                "+ext",
                "+ext+generated",
            )]),
        )
        .unwrap_err();
        assert_eq!(
            colliding.disposition(),
            HostSelectedExtensionDemandErrorDisposition::Missing
        );

        let empty_internal = demand_from_mappings(
            &CanonicalRepoName::new("+extension+").unwrap(),
            &mappings([usage(
                HostGraphModuleKey::Root,
                owner_id("extension"),
                "+extension",
                "+extension+generated",
            )]),
        )
        .unwrap_err();
        assert_eq!(
            empty_internal.disposition(),
            HostSelectedExtensionDemandErrorDisposition::Missing
        );

        let id = owner_id("extension");
        let error = demand_from_mappings(
            &requested,
            &mappings([
                usage(
                    HostGraphModuleKey::Root,
                    id.clone(),
                    "+one",
                    "+extension+generated",
                ),
                usage(HostGraphModuleKey::Root, id, "+two", "+extension+generated"),
            ]),
        )
        .unwrap_err();
        assert_eq!(
            error.disposition(),
            HostSelectedExtensionDemandErrorDisposition::Other
        );
        let missing =
            demand_from_mappings(&CanonicalRepoName::new("+missing").unwrap(), &mappings([]))
                .unwrap_err();
        assert_eq!(
            missing.disposition(),
            HostSelectedExtensionDemandErrorDisposition::Missing
        );
    }

    #[test]
    fn owner_inputs_uses_host_final_namespace_and_projects_usage_rows_in_graph_order() {
        let (owner, mappings) = owner_inputs_mappings();
        assert_eq!(
            mappings.base_mappings[0]
                .entries
                .get("generated")
                .unwrap()
                .as_str(),
            "+base"
        );
        let inputs = owner_inputs(owner, &mappings).unwrap();
        let (_, _, base, _) = inputs.request().namespace_parts();
        assert_eq!(base.get("generated").unwrap().as_str(), "+final");
        assert_eq!(
            inputs
                .request()
                .parts()
                .3
                .get("generated")
                .unwrap()
                .as_str(),
            "+final"
        );
        let rows = inputs.modules();
        assert!(inputs.root_proxies().unwrap().is_empty());
        assert_eq!(rows.len(), 2);
        let first = rows[0].parts();
        let second = rows[1].parts();
        assert_eq!(
            (first.0.as_str(), first.1, first.2, first.3),
            ("", "root", "1.0", true)
        );
        assert_eq!(
            first
                .5
                .iter()
                .map(|tag| tag.tag_class.as_str())
                .collect::<Vec<_>>(),
            ["root-one", "root-two"]
        );
        assert_eq!(first.4.get("generated").unwrap().as_str(), "+final");
        assert_eq!(
            (second.0.as_str(), second.1, second.2, second.3),
            ("dep+1.0", "dep", "1.0", false)
        );
        assert_eq!(
            second
                .5
                .iter()
                .map(|tag| tag.tag_class.as_str())
                .collect::<Vec<_>>(),
            ["dep"]
        );
        assert_eq!(second.4.get("root").unwrap(), &CanonicalRepoName::root());
        let mut no_root = mappings.clone();
        no_root.usages = no_root.usages[1..].into();
        let nonroot_only = owner_inputs(inputs.owner().dupe(), &no_root).unwrap();
        assert!(matches!(
            nonroot_only.request().source(),
            HostSelectedExtensionDefinitionSource::Root
        ));
        assert_eq!(nonroot_only.modules().len(), 1);
        assert!(!nonroot_only.modules()[0].parts().3);
        assert!(nonroot_only.root_proxies().is_none());

        let mut missing_host = mappings.clone();
        missing_host.routes = Arc::new(HostSelectedModuleRoutes {
            entries: missing_host.routes.entries[1..].into(),
            extension_projection: testing_extension_mapping_projection(),
        });
        assert!(matches!(
            owner_inputs(inputs.owner().dupe(), &missing_host),
            Err(HostSelectedExtensionOwnerInputsError(
                OwnerInputsError::Missing { .. }
            ))
        ));

        let mut duplicate_host = mappings.clone();
        let mut duplicate_routes = duplicate_host.routes.entries.to_vec();
        duplicate_routes.insert(1, duplicate_routes[0].clone());
        duplicate_host.routes = Arc::new(HostSelectedModuleRoutes {
            entries: duplicate_routes.into(),
            extension_projection: testing_extension_mapping_projection(),
        });
        assert!(matches!(
            owner_inputs(inputs.owner().dupe(), &duplicate_host),
            Err(HostSelectedExtensionOwnerInputsError(
                OwnerInputsError::Inconsistent { .. }
            ))
        ));

        let mut missing_mapping = mappings.clone();
        missing_mapping.mappings = Arc::from([]);
        assert!(matches!(
            owner_inputs(inputs.owner().dupe(), &missing_mapping),
            Err(HostSelectedExtensionOwnerInputsError(
                OwnerInputsError::Missing { .. }
            ))
        ));

        let mut mismatched_context = mappings.clone();
        let mut final_mappings = mismatched_context.mappings.to_vec();
        final_mappings[0].context_repo = CanonicalRepoName::new("dep+1.0").unwrap();
        mismatched_context.mappings = final_mappings.into();
        assert!(matches!(
            owner_inputs(inputs.owner().dupe(), &mismatched_context),
            Err(HostSelectedExtensionOwnerInputsError(
                OwnerInputsError::Inconsistent { .. }
            ))
        ));
        let mut invalid = mappings.clone();
        let mut routes = invalid.routes.entries.to_vec();
        let HostGraphModuleSource::Root(module) = &routes[0].entry.source else {
            unreachable!()
        };
        let mut root = (**module).clone();
        root.header.as_mut().unwrap().version = Some("1+2+3".into());
        routes[0].entry.source = HostGraphModuleSource::Root(Arc::new(root));
        invalid.routes = Arc::new(HostSelectedModuleRoutes {
            entries: routes.into(),
            extension_projection: testing_extension_mapping_projection(),
        });
        assert!(matches!(
            owner_inputs(inputs.owner().dupe(), &invalid),
            Err(HostSelectedExtensionOwnerInputsError(
                OwnerInputsError::Invalid { .. }
            ))
        ));
    }

    #[test]
    fn innate_owner_inputs_keep_actual_and_synthetic_labels_and_require_one_owner() {
        let (_, mut mappings) = owner_inputs_mappings();
        let mut root_mapping = mappings.mappings[0].clone();
        let mut entries = (*root_mapping.entries).clone();
        entries.insert(
            ApparentRepoName::new("definition").unwrap(),
            CanonicalRepoName::new("dep+1.0").unwrap(),
        );
        root_mapping.entries = Arc::new(entries);
        let id = HostSelectedExtensionId {
            bzl_file: CanonicalLabel::parse("@@//:MODULE.bazel").unwrap(),
            extension_name: "@definition//defs:repo.bzl repo".into(),
            isolation: None,
            kind: HostSelectedExtensionOwnerKind::InnateRepositoryRule,
        };
        let owner = Arc::new(HostSelectedExtensionOwner {
            id: id.clone(),
            unique_name: CanonicalRepoName::new("+repo").unwrap(),
        });
        mappings.root_usages = Arc::from([crate::module_eval::RootExtensionUsage {
            bzl_label: "//:MODULE.bazel".into(),
            extension_name: id.extension_name.clone(),
            proxies: Arc::from([]),
            tags: Arc::from([repo_tag("first"), repo_tag("second")]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: None,
        }]);
        mappings.usages = Arc::from([usage(
            HostGraphModuleKey::Root,
            id.clone(),
            "+repo",
            "+repo+first",
        )]);
        mappings.mappings = Arc::from([root_mapping]);
        mappings.base_mappings = mappings.base_mappings[..1].into();
        mappings.routes = Arc::new(HostSelectedModuleRoutes {
            entries: mappings.routes.entries[..1].into(),
            extension_projection: testing_extension_mapping_projection(),
        });

        let inputs = innate_owner_inputs(owner.dupe(), &mappings).unwrap();
        let (actual, rule, synthetic, _) = inputs.definition_parts();
        assert_eq!(actual.to_string(), "@@dep+1.0//defs:repo.bzl");
        assert_eq!(rule, "repo");
        assert_eq!(synthetic.to_string(), "@@//:MODULE.bazel");
        assert_eq!(
            inputs
                .tags()
                .iter()
                .map(|tag| match tag.attributes.get("name").unwrap() {
                    crate::NonrootAttributeValue::String(value) => value.as_str(),
                    value => panic!("unexpected name value: {value:?}"),
                })
                .collect::<Vec<_>>(),
            ["first", "second"]
        );

        let mut changed_mapping = mappings.clone();
        let mut changed = changed_mapping.mappings[0].clone();
        let mut entries = (*changed.entries).clone();
        entries.insert(
            ApparentRepoName::new("definition").unwrap(),
            CanonicalRepoName::root(),
        );
        changed.entries = Arc::new(entries);
        changed_mapping.mappings = Arc::from([changed]);
        let b = innate_owner_inputs(owner.dupe(), &changed_mapping).unwrap();
        let restored = innate_owner_inputs(owner.dupe(), &mappings).unwrap();
        assert_ne!(inputs, b);
        assert_eq!(inputs, restored);

        let mut validated = mappings.clone();
        let mut use_row = validated.usages[0].clone();
        use_row.validation_imports = Arc::from([HostSelectedExtensionDefinitionImport {
            local_name: "alias".into(),
            generated_name: "first".into(),
            location: span(),
        }]);
        validated.usages = Arc::from([use_row]);
        validated.overrides = Arc::from([HostSelectedExtensionOverride {
            id: id.clone(),
            generated_name: "second".into(),
            replacement: CanonicalRepoName::root(),
            must_exist: true,
            location: span(),
        }]);
        let validation = innate_owner_inputs(owner.dupe(), &validated).unwrap();
        let (imports, overrides) = validation.validation_parts();
        assert_eq!(imports[0].parts().0, "alias");
        assert_eq!(
            overrides[0].parts(),
            ("second", &CanonicalRepoName::root(), true)
        );

        let mut multiple = mappings.clone();
        multiple.usages = Arc::from([mappings.usages[0].clone(), mappings.usages[0].clone()]);
        assert!(innate_owner_inputs(owner.dupe(), &multiple).is_ok());
        let mut inconsistent = multiple.clone();
        let mut second = inconsistent.usages[1].clone();
        second.unique_name = CanonicalRepoName::new("+other").unwrap();
        inconsistent.usages = Arc::from([inconsistent.usages[0].clone(), second]);
        assert!(matches!(
            innate_owner_inputs(owner.dupe(), &inconsistent),
            Err(HostSelectedInnateRepositoryOwnerInputsError(
                OwnerInputsError::Unsupported { .. }
            ))
        ));
        let mut two_owners = multiple;
        let mut second = two_owners.usages[1].clone();
        second.owner = HostGraphModuleKey::Module {
            name: "dep".into(),
            version: crate::module_version::BazelModuleVersion::parse("1.0").unwrap(),
        };
        two_owners.usages = Arc::from([two_owners.usages[0].clone(), second]);
        assert!(matches!(
            innate_owner_inputs(owner.dupe(), &two_owners),
            Err(HostSelectedInnateRepositoryOwnerInputsError(
                OwnerInputsError::Unsupported { .. }
            ))
        ));

        let mut empty = mappings.clone();
        let mut root_usage = empty.root_usages[0].clone();
        root_usage.tags = Arc::from([]);
        empty.root_usages = Arc::from([root_usage]);
        assert!(matches!(
            innate_owner_inputs(owner.dupe(), &empty),
            Err(HostSelectedInnateRepositoryOwnerInputsError(
                OwnerInputsError::Invalid { .. }
            ))
        ));

        for (extension_name, isolation, expected_invalid) in [
            ("repo", None, true),
            (
                "//defs:repo.bzl repo",
                Some(HostSelectedExtensionIsolation::Root {
                    proxy: "proxy".into(),
                }),
                false,
            ),
        ] {
            let mut id = owner.id.clone();
            id.extension_name = extension_name.into();
            id.isolation = isolation;
            let candidate = Arc::new(HostSelectedExtensionOwner {
                id: id.clone(),
                unique_name: owner.unique_name.clone(),
            });
            let mut invalid = mappings.clone();
            invalid.usages =
                Arc::from([usage(HostGraphModuleKey::Root, id, "+repo", "+repo+first")]);
            let result = innate_owner_inputs(candidate, &invalid);
            assert!(matches!(
                (expected_invalid, result),
                (
                    true,
                    Err(HostSelectedInnateRepositoryOwnerInputsError(
                        OwnerInputsError::Invalid { .. }
                    ))
                ) | (
                    false,
                    Err(HostSelectedInnateRepositoryOwnerInputsError(
                        OwnerInputsError::Unsupported { .. }
                    ))
                )
            ));
        }
    }

    #[test]
    fn innate_owner_inputs_accept_nonroot_owner_mapping_and_reject_private_export() {
        let (_, mut mappings) = owner_inputs_mappings();
        let dep_key = mappings.routes.entries[1].entry.key.clone();
        let id = HostSelectedExtensionId {
            bzl_file: CanonicalLabel::parse("@@dep+1.0//:MODULE.bazel").unwrap(),
            extension_name: "@root//defs:repo.bzl repo".into(),
            isolation: None,
            kind: HostSelectedExtensionOwnerKind::InnateRepositoryRule,
        };
        let owner = Arc::new(HostSelectedExtensionOwner {
            id: id.clone(),
            unique_name: CanonicalRepoName::new("dep+1+repo").unwrap(),
        });
        let mut routes = mappings.routes.entries.to_vec();
        let HostGraphModuleSource::Discovered(module) = &routes[1].entry.source else {
            unreachable!()
        };
        let mut module = (**module).clone();
        module.module.extension_usages = Arc::from([crate::NonrootExtensionUsage {
            bzl_label: "//:MODULE.bazel".into(),
            extension_name: id.extension_name.clone(),
            proxies: Arc::from([]),
            tags: Arc::from([repo_tag("out")]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: None,
        }]);
        routes[1].entry.source = HostGraphModuleSource::Discovered(Arc::new(module));
        mappings.routes = Arc::new(HostSelectedModuleRoutes {
            entries: routes.into(),
            extension_projection: testing_extension_mapping_projection(),
        });
        mappings.usages = Arc::from([usage(dep_key, id.clone(), "dep+1+repo", "dep+1+repo+out")]);
        let inputs = innate_owner_inputs(owner, &mappings).unwrap();
        let (actual, rule, synthetic, _) = inputs.definition_parts();
        assert_eq!(actual.to_string(), "@@//defs:repo.bzl");
        assert_eq!(rule, "repo");
        assert_eq!(synthetic.to_string(), "@@dep+1.0//:MODULE.bazel");

        let mut private_id = id;
        private_id.extension_name = "@root//defs:repo.bzl _repo".into();
        let private = Arc::new(HostSelectedExtensionOwner {
            id: private_id.clone(),
            unique_name: CanonicalRepoName::new("dep+1+_repo").unwrap(),
        });
        let mut private_mappings = mappings;
        private_mappings.usages = Arc::from([usage(
            private_mappings.routes.entries[1].entry.key.clone(),
            private_id,
            "dep+1+_repo",
            "dep+1+_repo+out",
        )]);
        assert!(matches!(
            innate_owner_inputs(private, &private_mappings),
            Err(HostSelectedInnateRepositoryOwnerInputsError(
                OwnerInputsError::Invalid { .. }
            ))
        ));
    }

    #[test]
    fn owner_input_request_retains_selected_definition_source() {
        let (_, mut mappings) = owner_inputs_mappings();
        let id = HostSelectedExtensionId {
            bzl_file: CanonicalLabel::parse("@@dep+1.0//:extension.bzl").unwrap(),
            extension_name: "extension".into(),
            isolation: None,
            kind: HostSelectedExtensionOwnerKind::ModuleExtension,
        };
        let owner = Arc::new(HostSelectedExtensionOwner {
            id: id.clone(),
            unique_name: CanonicalRepoName::new("+extension").unwrap(),
        });
        mappings.usages = mappings
            .usages
            .iter()
            .cloned()
            .map(|mut usage| {
                usage.id = id.clone();
                usage
            })
            .collect();
        let mut root_usages = mappings.root_usages.to_vec();
        root_usages[0].bzl_label = "@dep//:extension.bzl".into();
        mappings.root_usages = root_usages.into();
        let mut final_mappings = mappings.mappings.to_vec();
        let dep = ApparentRepoName::new("dep").unwrap();
        let canonical = CanonicalRepoName::new("dep+1.0").unwrap();
        let mut entries = (*final_mappings[1].entries).clone();
        entries.insert(dep.clone(), canonical.clone());
        final_mappings[1].entries = Arc::new(entries);
        final_mappings[1].order = final_mappings[1]
            .order
            .iter()
            .cloned()
            .chain([dep])
            .collect();
        mappings.mappings = final_mappings.into();
        let mut routes = mappings.routes.entries.to_vec();
        routes[1].registry_repo_spec =
            Some(selected_registry_proof_spec(routes[1].entry.key.clone()));
        mappings.routes = Arc::new(HostSelectedModuleRoutes {
            entries: routes.into(),
            extension_projection: testing_extension_mapping_projection(),
        });

        let expected = selected_extension_definition_source(
            &mappings.routes,
            &mappings.mappings[1],
            &id.bzl_file,
        )
        .unwrap();
        let inputs = owner_inputs(owner, &mappings).unwrap();
        assert_eq!(inputs.request().source(), &expected);
        let HostSelectedExtensionDefinitionSource::Selected {
            definition,
            apparent_repo,
        } = inputs.request().source()
        else {
            panic!("owner input must retain selected source")
        };
        assert_eq!(definition.view().canonical_repo(), &canonical);
        assert_eq!(apparent_repo.as_str(), "dep");
        assert!(definition.view().repo_spec().is_some());

        let mut nonroot_only = mappings;
        nonroot_only.usages = nonroot_only.usages[1..].into();
        let inputs = owner_inputs(inputs.owner().dupe(), &nonroot_only).unwrap();
        assert_eq!(inputs.modules().len(), 1);
        assert!(!inputs.modules()[0].parts().3);
        assert!(matches!(
            inputs.request().source(),
            HostSelectedExtensionDefinitionSource::Selected { .. }
        ));
    }

    #[test]
    fn owner_inputs_forwards_observed_epochs_and_keeps_need_and_outer_carrierless() {
        let (owner, mappings) = owner_inputs_mappings();
        let demand = slug_workspace_v2::PathObservationDemand::new(
            slug_workspace_v2::PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/selected-extension-demand/epoch").unwrap(),
            slug_workspace_v2::PathObservationOperation::Lstat,
        );
        let epoch = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(slug_workspace_v2::PathObservationResult::Lstat(
                slug_workspace_v2::PathOperationResult::Missing,
            )),
        )])
        .unwrap();
        let complete = finish_inputs_mappings_child(
            owner.dupe(),
            SourcePreparationOutcome::Complete(Ok((Arc::new(Ok(mappings)), epoch.dupe()))),
        );
        assert!(
            matches!(complete, SourcePreparationOutcome::Complete(Ok((result, observations))) if result.is_ok() && observations == epoch)
        );
        let need = SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(demand.dupe()),
        );
        assert!(matches!(
            finish_inputs_mappings_child(owner.dupe(), SourcePreparationOutcome::Need(need)),
            SourcePreparationOutcome::Need(_)
        ));
        let outer = ExtensionMappingsObservationError::RootFiles(
            slug_workspace_v2::ObservedPathFrontierError::from(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                    demand,
                    result_operation: slug_workspace_v2::PathObservationOperation::FileBytes,
                },
            ),
        );
        assert!(matches!(
            finish_inputs_mappings_child(owner, SourcePreparationOutcome::Complete(Err(outer))),
            SourcePreparationOutcome::Complete(Err(OwnerInputsObservationError::Mappings(
                ExtensionMappingsObservationError::RootFiles(_)
            )))
        ));
    }

    #[test]
    fn demand_and_owner_input_keys_are_complete_only() {
        let workspace = NormalizedAbsolutePath::new("/selected-extension-demand-test").unwrap();
        let requested = CanonicalRepoName::new("+extension+generated").unwrap();
        let legacy = HostSelectedExtensionDemandKey::new(workspace.dupe(), requested.clone());
        let observed = HostSelectedExtensionDemandObservationKey::new(workspace.dupe(), requested);
        assert_eq!(
            legacy.to_string(),
            "host-selected-extension-demand:\"/selected-extension-demand-test\":@@+extension+generated"
        );
        assert_eq!(observed.to_string(), format!("observed-{legacy}"));
        let demand = slug_workspace_v2::PathObservationDemand::new(
            slug_workspace_v2::PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/selected-extension-demand-test/need").unwrap(),
            slug_workspace_v2::PathObservationOperation::Lstat,
        );
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(demand),
        ));
        assert!(!HostSelectedExtensionDemandKey::validity(&need));
        let complete = SourcePreparationOutcome::Complete(Arc::new(Err(
            HostSelectedExtensionDemandError(DemandError::Missing {
                requested: CanonicalRepoName::new("+missing").unwrap(),
            }),
        )));
        assert!(HostSelectedExtensionDemandKey::validity(&complete));
        assert!(HostSelectedExtensionDemandKey::equality(
            &complete, &complete
        ));
    }
}
