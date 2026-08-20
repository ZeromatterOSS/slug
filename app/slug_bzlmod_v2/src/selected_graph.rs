/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory of this source tree.
 */

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::interim_module::NonrootDependency;
use crate::interim_module::NonrootModuleKey;
use crate::module_eval::EvaluatedRootModule;
use crate::module_eval::HostEffectiveModuleOverride;
use crate::module_eval::HostEffectiveModuleOverrideError;
use crate::module_eval::HostEffectiveModuleOverrideKey;
use crate::module_eval::HostEffectiveModuleOverrideObservationKey;
use crate::module_eval::ObservedHostEffectiveModuleOverride;
use crate::module_eval::RegistryMultipleOverride;
use crate::module_eval::RootModuleCommandPolicyKey;
use crate::module_eval::RootModuleFilesKey;
use crate::module_eval::RootModuleFilesObservationKey;
use crate::module_eval::RootModuleOverride;
use crate::module_version::BazelModuleVersion;
use crate::source_preparation::HostDiscoveredModule;
use crate::source_preparation::HostDiscoveredModuleError;
use crate::source_preparation::HostDiscoveredModuleKey;
use crate::source_preparation::HostDiscoveredModuleObservationError;
use crate::source_preparation::HostDiscoveredModuleObservationKey;
use crate::source_preparation::SourcePreparationNeeds;
use crate::source_preparation::SourcePreparationOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative)]
pub(crate) enum HostGraphModuleKey {
    Root,
    Module {
        name: CompactString,
        version: BazelModuleVersion,
    },
}

impl HostGraphModuleKey {
    fn module(name: CompactString, version: BazelModuleVersion) -> Self {
        Self::Module { name, version }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Root => None,
            Self::Module { name, .. } => Some(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostGraphDependency {
    pub(crate) apparent_name: Option<CompactString>,
    pub(crate) key: HostGraphModuleKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostGraphModuleSource {
    Root(Arc<EvaluatedRootModule>),
    Discovered(Arc<HostDiscoveredModule>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostSelectedModuleEntry {
    pub(crate) key: HostGraphModuleKey,
    pub(crate) source: HostGraphModuleSource,
    pub(crate) dependencies: Arc<[HostGraphDependency]>,
    pub(crate) original_dependencies: Arc<[HostGraphDependency]>,
    pub(crate) nodep_dependencies: Arc<[HostGraphDependency]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostSelectedModuleGraph {
    pub(crate) resolved: Arc<[HostSelectedModuleEntry]>,
    pub(crate) unpruned: Arc<[HostSelectedModuleEntry]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostSelectedModuleGraphError {
    Input {
        owner: CompactString,
        message: CompactString,
    },
    DiscoveryCompute {
        module: HostGraphModuleKey,
        message: CompactString,
    },
    DiscoveryLeaf {
        module: HostGraphModuleKey,
        error: HostDiscoveredModuleError,
    },
    IncompatibleNeeds(CompactString),
    UnusedOverride {
        module_name: CompactString,
    },
    AllowedVersionMissing {
        module_name: CompactString,
        version: BazelModuleVersion,
    },
    MultipleVersionNoCeiling {
        module_name: CompactString,
        version: BazelModuleVersion,
    },
    MissingSelectedModule {
        module: HostGraphModuleKey,
    },
    DuplicateDependency {
        owner: HostGraphModuleKey,
        dependency: HostGraphModuleKey,
        first_apparent_name: CompactString,
        second_apparent_name: CompactString,
    },
}

impl fmt::Display for HostSelectedModuleGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostSelectedModuleGraphError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostSelectedModuleGraphKey {
    workspace: NormalizedAbsolutePath,
}

impl HostSelectedModuleGraphKey {
    #[allow(dead_code)]
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostSelectedModuleGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-selected-module-graph:{}", self.workspace)
    }
}

type GraphResult = Arc<Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>>;
type GraphOutcome = SourcePreparationOutcome<GraphResult>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct HostSelectedModuleGraphObservationKey(HostSelectedModuleGraphKey);

#[allow(dead_code)]
impl HostSelectedModuleGraphObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostSelectedModuleGraphKey::new(workspace))
    }
}

impl fmt::Display for HostSelectedModuleGraphObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
pub(crate) struct ObservedHostSelectedModuleGraph {
    result: GraphResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostSelectedModuleGraph {
    pub(crate) fn new(result: GraphResult, observations: PathObservationEpoch) -> Self {
        Self {
            result,
            observations,
        }
    }

    pub(crate) fn result(&self) -> &GraphResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostSelectedModuleGraphObservationError {
    Root(ObservedPathFrontierError),
    Effective(ObservedPathFrontierError),
    Discovery(HostDiscoveredModuleObservationError),
    Merge(ObservedPathFrontierError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum HostSelectedModuleGraphMode {
    Legacy,
    Observed,
}

type GraphDriverOutcome = SourcePreparationOutcome<
    Result<(GraphResult, PathObservationEpoch), HostSelectedModuleGraphObservationError>,
>;

fn graph_complete(
    result: Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn graph_error(
    error: HostSelectedModuleGraphError,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_complete(Err(error), observations)
}

fn root_compute_error(error: HostSelectedModuleGraphError) -> GraphDriverOutcome {
    graph_error(error, PathObservationEpoch::empty())
}

fn root_semantic_error(
    error: HostSelectedModuleGraphError,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_error(error, observations)
}

fn policy_compute_error(
    error: HostSelectedModuleGraphError,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_error(error, observations)
}

fn effective_stage_error(
    error: HostSelectedModuleGraphError,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_error(error, observations)
}

fn transform_stage_error(
    error: HostSelectedModuleGraphError,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_error(error, observations)
}

fn finish_select_stage(
    result: Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>,
    observations: PathObservationEpoch,
) -> GraphDriverOutcome {
    graph_complete(result, observations)
}

fn graph_frontier(error: HostSelectedModuleGraphObservationError) -> GraphDriverOutcome {
    SourcePreparationOutcome::Complete(Err(error))
}

fn merge_graph_prefix(
    prefix: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, HostSelectedModuleGraphObservationError> {
    PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                incoming
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(|error| HostSelectedModuleGraphObservationError::Merge(error.into()))
}
fn finish_observed_effective(
    outcome: SourcePreparationOutcome<
        Result<ObservedHostEffectiveModuleOverride, ObservedPathFrontierError>,
    >,
    prefix: &PathObservationEpoch,
) -> Result<
    (
        Arc<Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>>,
        PathObservationEpoch,
    ),
    GraphDriverOutcome,
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => Err(graph_frontier(
            HostSelectedModuleGraphObservationError::Effective(error),
        )),
        SourcePreparationOutcome::Complete(Ok(observed)) => Ok((
            observed.result().dupe(),
            merge_graph_prefix(prefix, observed.observations()).map_err(graph_frontier)?,
        )),
    }
}

#[derive(Debug, Clone)]
struct RawDependency {
    apparent_name: Option<CompactString>,
    requested: HostGraphModuleKey,
    transformed: HostGraphModuleKey,
}

#[derive(Debug, Clone)]
struct RawModule {
    key: HostGraphModuleKey,
    source: HostGraphModuleSource,
    dependencies: Vec<RawDependency>,
    original_dependencies: Vec<RawDependency>,
    nodep_dependencies: Vec<RawDependency>,
}

fn parse_version(
    module_name: &str,
    spelling: &str,
) -> Result<BazelModuleVersion, HostSelectedModuleGraphError> {
    BazelModuleVersion::parse(spelling).map_err(|error| HostSelectedModuleGraphError::Input {
        owner: format!("{module_name}@{spelling}").into(),
        message: error.to_string().into(),
    })
}

async fn effective_override(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
    module_name: &CompactString,
) -> Result<HostEffectiveModuleOverride, GraphDriverOutcome> {
    if let Some(value) = cache.get(module_name.as_str()) {
        return Ok(value.clone());
    }
    let key = HostEffectiveModuleOverrideKey::new(workspace.dupe(), module_name.clone());
    let (result, merged) = match mode {
        HostSelectedModuleGraphMode::Legacy => match ctx.compute(&key).await {
            Ok(result) => (result, observations.dupe()),
            Err(error) => {
                return Err(effective_stage_error(
                    HostSelectedModuleGraphError::Input {
                        owner: format!("effective override {module_name}").into(),
                        message: error.to_string().into(),
                    },
                    observations.dupe(),
                ));
            }
        },
        HostSelectedModuleGraphMode::Observed => {
            let outcome = ctx
                .compute(&HostEffectiveModuleOverrideObservationKey::new(
                    workspace.dupe(),
                    module_name.clone(),
                ))
                .await
                .map_err(|error| {
                    effective_stage_error(
                        HostSelectedModuleGraphError::Input {
                            owner: format!("effective override {module_name}").into(),
                            message: error.to_string().into(),
                        },
                        observations.dupe(),
                    )
                })?;
            finish_observed_effective(outcome, observations)?
        }
    };
    *observations = merged;
    let value = result.as_ref().clone().map_err(|error| {
        effective_stage_error(
            HostSelectedModuleGraphError::Input {
                owner: format!("effective override {module_name}").into(),
                message: error.to_string().into(),
            },
            observations.dupe(),
        )
    })?;
    cache.insert(module_name.clone(), value.clone());
    Ok(value)
}

async fn transform_request(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
    module_name: CompactString,
    version: BazelModuleVersion,
) -> Result<HostGraphModuleKey, GraphDriverOutcome> {
    if root_name == Some(module_name.as_str()) {
        return Ok(HostGraphModuleKey::Root);
    }
    let effective =
        effective_override(ctx, workspace, mode, cache, observations, &module_name).await?;
    let version = match effective.override_() {
        Some(RootModuleOverride::NonRegistry(_)) => BazelModuleVersion::empty(),
        Some(RootModuleOverride::RegistrySingle(single)) if !single.version.is_empty() => {
            parse_version(&module_name, &single.version)
                .map_err(|error| transform_stage_error(error, observations.dupe()))?
        }
        Some(RootModuleOverride::RegistrySingle(_))
        | Some(RootModuleOverride::RegistryMultiple(_)) => version,
        None if module_name == "bazel_tools" => BazelModuleVersion::empty(),
        None => version,
    };
    Ok(HostGraphModuleKey::module(module_name, version))
}

async fn raw_dependency(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
    apparent_name: Option<CompactString>,
    dependency: &NonrootDependency,
) -> Result<RawDependency, GraphDriverOutcome> {
    let version = parse_version(&dependency.name, &dependency.version)
        .map_err(|error| transform_stage_error(error, observations.dupe()))?;
    let requested = HostGraphModuleKey::module(dependency.name.clone(), version.clone());
    let transformed = transform_request(
        ctx,
        workspace,
        mode,
        root_name,
        cache,
        observations,
        dependency.name.clone(),
        version,
    )
    .await?;
    Ok(RawDependency {
        apparent_name,
        requested,
        transformed,
    })
}

async fn raw_dependencies(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
    dependencies: Vec<(Option<CompactString>, NonrootDependency)>,
) -> Result<Vec<RawDependency>, GraphDriverOutcome> {
    let mut result = Vec::new();
    for (apparent_name, dependency) in dependencies {
        result.push(
            raw_dependency(
                ctx,
                workspace,
                mode,
                root_name,
                cache,
                observations,
                apparent_name,
                &dependency,
            )
            .await?,
        );
    }
    Ok(result)
}

async fn raw_root(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    module: &EvaluatedRootModule,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
) -> Result<RawModule, GraphDriverOutcome> {
    let root_name = module.header.as_ref().map(|header| header.name.as_str());
    let mut dependencies = Vec::new();
    let mut nodep_dependencies = Vec::new();
    for dependency in module.dependencies.iter() {
        let version = parse_version(&dependency.name, &dependency.version)
            .map_err(|error| transform_stage_error(error, observations.dupe()))?;
        let requested = HostGraphModuleKey::module(dependency.name.clone(), version.clone());
        let transformed = transform_request(
            ctx,
            workspace,
            mode,
            root_name,
            cache,
            observations,
            dependency.name.clone(),
            version,
        )
        .await?;
        let value = RawDependency {
            apparent_name: Some(
                dependency
                    .repo_name
                    .clone()
                    .unwrap_or_else(|| dependency.name.clone()),
            ),
            requested,
            transformed,
        };
        if dependency.nodep {
            nodep_dependencies.push(value);
        } else {
            dependencies.push(value);
        }
    }
    if root_name != Some("bazel_tools")
        && !dependencies
            .iter()
            .any(|dependency| dependency.transformed.name() == Some("bazel_tools"))
    {
        let name = CompactString::new("bazel_tools");
        let requested = HostGraphModuleKey::module(name.clone(), BazelModuleVersion::empty());
        let transformed = transform_request(
            ctx,
            workspace,
            mode,
            root_name,
            cache,
            observations,
            name.clone(),
            BazelModuleVersion::empty(),
        )
        .await?;
        dependencies.push(RawDependency {
            apparent_name: Some(name),
            requested,
            transformed,
        });
    }
    Ok(RawModule {
        key: HostGraphModuleKey::Root,
        source: HostGraphModuleSource::Root(Arc::new(module.clone())),
        original_dependencies: dependencies.clone(),
        dependencies,
        nodep_dependencies,
    })
}

async fn raw_discovered(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
    key: HostGraphModuleKey,
    module: Arc<HostDiscoveredModule>,
) -> Result<RawModule, GraphDriverOutcome> {
    let base = &module.module.base;
    let dependencies = raw_dependencies(
        ctx,
        workspace,
        mode,
        root_name,
        cache,
        observations,
        base.dependencies
            .iter()
            .map(|(name, dependency)| (Some(name.clone()), dependency.clone()))
            .collect(),
    )
    .await?;
    let original_dependencies = raw_dependencies(
        ctx,
        workspace,
        mode,
        root_name,
        cache,
        observations,
        base.original_dependencies
            .iter()
            .map(|(name, dependency)| (Some(name.clone()), dependency.clone()))
            .collect(),
    )
    .await?;
    let nodep_dependencies = raw_dependencies(
        ctx,
        workspace,
        mode,
        root_name,
        cache,
        observations,
        base.nodep_dependencies
            .iter()
            .map(|dependency| (None, dependency.clone()))
            .collect(),
    )
    .await?;
    Ok(RawModule {
        key,
        source: HostGraphModuleSource::Discovered(module),
        dependencies,
        original_dependencies,
        nodep_dependencies,
    })
}

fn next_horizon(
    entries: &[RawModule],
    frontier: &[HostGraphModuleKey],
    prior_names: &SmallSet<CompactString>,
    seen: &SmallSet<HostGraphModuleKey>,
) -> Vec<HostGraphModuleKey> {
    let mut next = Vec::new();
    let mut queued = SmallSet::new();
    for owner in frontier {
        let Some(entry) = entries.iter().find(|entry| &entry.key == owner) else {
            continue;
        };
        for dependency in entry
            .dependencies
            .iter()
            .chain(entry.nodep_dependencies.iter().filter(|dependency| {
                dependency
                    .transformed
                    .name()
                    .is_some_and(|name| prior_names.contains(name))
            }))
        {
            if dependency.transformed != HostGraphModuleKey::Root
                && !seen.contains(&dependency.transformed)
                && queued.insert(dependency.transformed.clone())
            {
                next.push(dependency.transformed.clone());
            }
        }
    }
    next
}

type DiscoveryCarrier = (
    Arc<Result<HostDiscoveredModule, HostDiscoveredModuleError>>,
    PathObservationEpoch,
);
type LeafOutcome = Result<
    SourcePreparationOutcome<Result<DiscoveryCarrier, HostSelectedModuleGraphObservationError>>,
    CompactString,
>;

async fn discovered_leaf(
    ctx: &mut DiceComputations<'_>,
    mode: HostSelectedModuleGraphMode,
    workspace: NormalizedAbsolutePath,
    module: &HostGraphModuleKey,
) -> LeafOutcome {
    let HostGraphModuleKey::Module { name, version } = module else {
        unreachable!("root is never a leaf horizon");
    };
    let key = HostDiscoveredModuleKey::try_new(
        workspace.dupe(),
        NonrootModuleKey::new(name.clone(), version.normalized()),
    )
    .expect("typed graph versions construct checked Host keys");
    match mode {
        HostSelectedModuleGraphMode::Legacy => {
            let outcome = ctx
                .compute(&key)
                .await
                .map_err(|error| CompactString::new(error.to_string()))?;
            Ok(match outcome {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(result) => {
                    SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
                }
            })
        }
        HostSelectedModuleGraphMode::Observed => {
            match ctx
                .compute(
                    &HostDiscoveredModuleObservationKey::try_new(
                        workspace,
                        NonrootModuleKey::new(name.clone(), version.normalized()),
                    )
                    .expect("typed graph versions construct checked observed Host keys"),
                )
                .await
                .map_err(|error| CompactString::new(error.to_string()))?
            {
                SourcePreparationOutcome::Need(need) => Ok(SourcePreparationOutcome::Need(need)),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    Ok(SourcePreparationOutcome::Complete(Err(
                        HostSelectedModuleGraphObservationError::Discovery(error),
                    )))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    Ok(SourcePreparationOutcome::Complete(Ok((
                        observed.result().dupe(),
                        observed.observations().dupe(),
                    ))))
                }
            }
        }
    }
}

fn finish_horizon(
    next: &[HostGraphModuleKey],
    outcomes: &SmallMap<HostGraphModuleKey, LeafOutcome>,
    prefix: &PathObservationEpoch,
) -> Result<
    (
        Vec<(HostGraphModuleKey, Arc<HostDiscoveredModule>)>,
        PathObservationEpoch,
    ),
    GraphDriverOutcome,
> {
    let mut observations = prefix.dupe();
    let mut frontier = None;
    let mut needs: Option<SourcePreparationNeeds> = None;
    let mut need_error = None;
    let mut complete = Vec::with_capacity(next.len());
    let mut first_error = None;
    for module in next {
        match outcomes
            .get(module)
            .expect("every discovery horizon result is retained")
        {
            Err(message) if first_error.is_none() => {
                first_error = Some(HostSelectedModuleGraphError::DiscoveryCompute {
                    module: module.clone(),
                    message: message.clone(),
                });
            }
            Err(_) => {}
            Ok(SourcePreparationOutcome::Need(need)) => {
                needs = Some(match needs {
                    Some(current) => match current.try_union(need) {
                        Ok(union) => union,
                        Err(error) => {
                            need_error.get_or_insert_with(|| {
                                HostSelectedModuleGraphError::IncompatibleNeeds(
                                    format!("{error:?}").into(),
                                )
                            });
                            current
                        }
                    },
                    None => need.clone(),
                });
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                if frontier.is_none() {
                    frontier = Some(error.dupe());
                }
            }
            Ok(SourcePreparationOutcome::Complete(Ok((result, incoming)))) => {
                match merge_graph_prefix(&observations, incoming) {
                    Ok(merged) => observations = merged,
                    Err(error) => {
                        frontier.get_or_insert(error);
                    }
                }
                match result.as_ref() {
                    Ok(value) => complete.push((module.clone(), Arc::new(value.clone()))),
                    Err(error) if first_error.is_none() => {
                        first_error = Some(HostSelectedModuleGraphError::DiscoveryLeaf {
                            module: module.clone(),
                            error: error.clone(),
                        });
                    }
                    Err(_) => {}
                }
            }
        }
    }
    if let Some(error) = frontier {
        return Err(graph_frontier(error));
    }
    if let Some(error) = first_error.or(need_error) {
        return Err(graph_error(error, observations));
    }
    if let Some(need) = needs {
        return Err(SourcePreparationOutcome::Need(need));
    }
    Ok((complete, observations))
}

async fn discover_round(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root: RawModule,
    root_name: Option<&str>,
    prior_names: &SmallSet<CompactString>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
) -> Result<Vec<RawModule>, GraphDriverOutcome> {
    let mut entries = vec![root];
    let mut seen = SmallSet::from_iter([HostGraphModuleKey::Root]);
    let mut frontier = vec![HostGraphModuleKey::Root];
    loop {
        let next = next_horizon(&entries, &frontier, prior_names, &seen);
        if next.is_empty() {
            return Ok(entries);
        }
        let computed = ctx
            .compute_join(next.iter().cloned(), |ctx, module| {
                let workspace = workspace.dupe();
                Box::pin(async move {
                    let value = discovered_leaf(ctx, mode, workspace, &module).await;
                    (module, value)
                })
            })
            .await;
        let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
        let (complete, merged) = finish_horizon(&next, &outcomes, observations)?;
        *observations = merged;
        frontier.clear();
        for (key, module) in complete {
            seen.insert(key.clone());
            entries.push(
                raw_discovered(
                    ctx,
                    workspace,
                    mode,
                    root_name,
                    cache,
                    observations,
                    key.clone(),
                    module,
                )
                .await?,
            );
            frontier.push(key);
        }
    }
}

async fn discover_fixed_point(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: HostSelectedModuleGraphMode,
    root_module: &EvaluatedRootModule,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    observations: &mut PathObservationEpoch,
) -> Result<Vec<RawModule>, GraphDriverOutcome> {
    let mut prior_names = SmallSet::new();
    let mut previous_keys: Option<SmallSet<HostGraphModuleKey>> = None;
    loop {
        let root = raw_root(ctx, workspace, mode, root_module, cache, observations).await?;
        let entries = discover_round(
            ctx,
            workspace,
            mode,
            root,
            root_module
                .header
                .as_ref()
                .map(|header| header.name.as_str()),
            &prior_names,
            cache,
            observations,
        )
        .await?;
        let keys = entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<SmallSet<_>>();
        if previous_keys.as_ref() == Some(&keys) {
            return Ok(entries);
        }
        prior_names = entries
            .iter()
            .filter_map(|entry| entry.key.name().map(CompactString::new))
            .collect();
        previous_keys = Some(keys);
    }
}

fn multiple_versions(
    overrides: &SmallMap<CompactString, HostEffectiveModuleOverride>,
) -> Result<SmallMap<CompactString, Arc<[BazelModuleVersion]>>, HostSelectedModuleGraphError> {
    let mut result = SmallMap::new();
    for (name, override_) in overrides {
        let HostEffectiveModuleOverride::Root {
            override_:
                RootModuleOverride::RegistryMultiple(RegistryMultipleOverride { versions, .. }),
        } = override_
        else {
            continue;
        };
        let mut parsed = versions
            .iter()
            .map(|version| parse_version(name, version))
            .collect::<Result<Vec<_>, _>>()?;
        parsed.sort();
        result.insert(name.clone(), Arc::from(parsed));
    }
    Ok(result)
}

fn resolve_target(
    target: &HostGraphModuleKey,
    selected: &SmallMap<CompactString, BazelModuleVersion>,
    multiple: &SmallMap<CompactString, Arc<[BazelModuleVersion]>>,
) -> Result<HostGraphModuleKey, HostSelectedModuleGraphError> {
    let HostGraphModuleKey::Module { name, version } = target else {
        return Ok(HostGraphModuleKey::Root);
    };
    let version = if let Some(allowed) = multiple.get(name.as_str()) {
        allowed
            .iter()
            .find(|allowed| *allowed >= version)
            .cloned()
            .ok_or_else(|| HostSelectedModuleGraphError::MultipleVersionNoCeiling {
                module_name: name.clone(),
                version: version.clone(),
            })?
    } else {
        selected.get(name.as_str()).cloned().ok_or_else(|| {
            HostSelectedModuleGraphError::MissingSelectedModule {
                module: target.clone(),
            }
        })?
    };
    Ok(HostGraphModuleKey::module(name.clone(), version))
}

fn validate_and_reachable(
    entries: &[RawModule],
    selected: &SmallMap<CompactString, BazelModuleVersion>,
    multiple: &SmallMap<CompactString, Arc<[BazelModuleVersion]>>,
    include_nodep: bool,
) -> Result<Vec<HostGraphModuleKey>, HostSelectedModuleGraphError> {
    let mut result = Vec::new();
    let mut seen = SmallSet::new();
    let mut queue = VecDeque::from([HostGraphModuleKey::Root]);
    while let Some(key) = queue.pop_front() {
        if !seen.insert(key.clone()) {
            continue;
        }
        let entry = entries
            .iter()
            .find(|entry| entry.key == key)
            .ok_or_else(|| HostSelectedModuleGraphError::MissingSelectedModule {
                module: key.clone(),
            })?;
        result.push(key.clone());
        let mut dependency_names = SmallMap::<HostGraphModuleKey, CompactString>::new();
        for dependency in &entry.dependencies {
            let target = resolve_target(&dependency.transformed, selected, multiple)?;
            let apparent_name = dependency
                .apparent_name
                .clone()
                .expect("ordinary dependencies retain apparent names");
            if let Some(first) = dependency_names.get(&target)
                && first != &apparent_name
            {
                return Err(HostSelectedModuleGraphError::DuplicateDependency {
                    owner: key,
                    dependency: target,
                    first_apparent_name: first.clone(),
                    second_apparent_name: apparent_name,
                });
            }
            dependency_names.insert(target.clone(), apparent_name);
            queue.push_back(target);
        }
        if include_nodep {
            for dependency in &entry.nodep_dependencies {
                queue.push_back(resolve_target(&dependency.transformed, selected, multiple)?);
            }
        }
    }
    Ok(result)
}

fn rewrite_dependencies(
    dependencies: &[RawDependency],
    selected: &SmallMap<CompactString, BazelModuleVersion>,
    multiple: &SmallMap<CompactString, Arc<[BazelModuleVersion]>>,
    tolerate_unreachable_ceiling: bool,
) -> Result<Arc<[HostGraphDependency]>, HostSelectedModuleGraphError> {
    dependencies
        .iter()
        .map(|dependency| {
            let key = match resolve_target(&dependency.transformed, selected, multiple) {
                Ok(key) => key,
                Err(HostSelectedModuleGraphError::MultipleVersionNoCeiling { .. })
                    if tolerate_unreachable_ceiling =>
                {
                    dependency.transformed.clone()
                }
                Err(error) => return Err(error),
            };
            Ok(HostGraphDependency {
                apparent_name: dependency.apparent_name.clone(),
                key,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Arc::from)
}

fn rewrite_entry(
    entry: &RawModule,
    selected: &SmallMap<CompactString, BazelModuleVersion>,
    multiple: &SmallMap<CompactString, Arc<[BazelModuleVersion]>>,
    tolerate_unreachable_ceiling: bool,
) -> Result<HostSelectedModuleEntry, HostSelectedModuleGraphError> {
    Ok(HostSelectedModuleEntry {
        key: entry.key.clone(),
        source: entry.source.clone(),
        dependencies: rewrite_dependencies(
            &entry.dependencies,
            selected,
            multiple,
            tolerate_unreachable_ceiling,
        )?,
        original_dependencies: entry
            .original_dependencies
            .iter()
            .map(|dependency| HostGraphDependency {
                apparent_name: dependency.apparent_name.clone(),
                key: dependency.requested.clone(),
            })
            .collect::<Vec<_>>()
            .into(),
        nodep_dependencies: rewrite_dependencies(
            &entry.nodep_dependencies,
            selected,
            multiple,
            tolerate_unreachable_ceiling,
        )?,
    })
}

fn select_graph(
    entries: Vec<RawModule>,
    candidate_overrides: &[CompactString],
    overrides: &SmallMap<CompactString, HostEffectiveModuleOverride>,
) -> Result<HostSelectedModuleGraph, HostSelectedModuleGraphError> {
    let discovered_names = entries
        .iter()
        .filter_map(|entry| entry.key.name().map(CompactString::new))
        .collect::<SmallSet<_>>();
    for name in candidate_overrides {
        if !matches!(
            overrides.get(name.as_str()),
            Some(HostEffectiveModuleOverride::None)
        ) && !discovered_names.contains(name.as_str())
        {
            return Err(HostSelectedModuleGraphError::UnusedOverride {
                module_name: name.clone(),
            });
        }
    }
    let multiple = multiple_versions(overrides)?;
    for (name, versions) in &multiple {
        for version in versions.iter() {
            if !entries
                .iter()
                .any(|entry| entry.key == HostGraphModuleKey::module(name.clone(), version.clone()))
            {
                return Err(HostSelectedModuleGraphError::AllowedVersionMissing {
                    module_name: name.clone(),
                    version: version.clone(),
                });
            }
        }
    }
    let mut selected = SmallMap::<CompactString, BazelModuleVersion>::new();
    for entry in &entries {
        let HostGraphModuleKey::Module { name, version } = &entry.key else {
            continue;
        };
        if multiple.contains_key(name.as_str()) {
            continue;
        }
        if selected
            .get(name.as_str())
            .is_none_or(|current| version > current)
        {
            selected.insert(name.clone(), version.clone());
        }
    }
    validate_and_reachable(&entries, &selected, &multiple, true)?;
    let resolved_keys = validate_and_reachable(&entries, &selected, &multiple, false)?;
    let mut resolved = Vec::with_capacity(resolved_keys.len());
    for key in resolved_keys {
        let entry = entries
            .iter()
            .find(|entry| entry.key == key)
            .ok_or_else(|| HostSelectedModuleGraphError::MissingSelectedModule {
                module: key.clone(),
            })?;
        resolved.push(rewrite_entry(entry, &selected, &multiple, false)?);
    }
    let unpruned = entries
        .iter()
        .map(|entry| rewrite_entry(entry, &selected, &multiple, true))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HostSelectedModuleGraph {
        resolved: Arc::from(resolved),
        unpruned: Arc::from(unpruned),
    })
}

impl HostSelectedModuleGraphKey {
    async fn root_files(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostSelectedModuleGraphMode,
    ) -> Result<
        (
            Arc<Result<crate::module_eval::RootModuleFiles, CompactString>>,
            PathObservationEpoch,
        ),
        GraphDriverOutcome,
    > {
        match mode {
            HostSelectedModuleGraphMode::Legacy => {
                match ctx
                    .compute(&RootModuleFilesKey {
                        workspace: self.workspace.as_path().to_owned(),
                    })
                    .await
                {
                    Ok(result) => Ok((result, PathObservationEpoch::empty())),
                    Err(error) => Err(root_compute_error(HostSelectedModuleGraphError::Input {
                        owner: "root MODULE files".into(),
                        message: error.to_string().into(),
                    })),
                }
            }
            HostSelectedModuleGraphMode::Observed => {
                match ctx
                    .compute(&RootModuleFilesObservationKey::new(self.workspace.dupe()))
                    .await
                {
                    Err(error) => Err(root_compute_error(HostSelectedModuleGraphError::Input {
                        owner: "root MODULE files".into(),
                        message: error.to_string().into(),
                    })),
                    Ok(SourcePreparationOutcome::Need(need)) => {
                        Err(SourcePreparationOutcome::Need(need))
                    }
                    Ok(SourcePreparationOutcome::Complete(Err(error))) => Err(graph_frontier(
                        HostSelectedModuleGraphObservationError::Root(error),
                    )),
                    Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                        Ok((observed.result().dupe(), observed.observations().dupe()))
                    }
                }
            }
        }
    }

    async fn drive(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostSelectedModuleGraphMode,
    ) -> GraphDriverOutcome {
        let (files_result, mut observations) = match self.root_files(ctx, mode).await {
            Ok(complete) => complete,
            Err(outcome) => return outcome,
        };
        let files = match files_result.as_ref() {
            Ok(files) => files.clone(),
            Err(error) => {
                return root_semantic_error(
                    HostSelectedModuleGraphError::Input {
                        owner: "root MODULE files".into(),
                        message: error.clone(),
                    },
                    observations,
                );
            }
        };
        let policy = match ctx
            .compute(&RootModuleCommandPolicyKey {
                workspace: self.workspace.as_path().to_owned(),
            })
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return policy_compute_error(
                    HostSelectedModuleGraphError::Input {
                        owner: "command policy".into(),
                        message: error.to_string().into(),
                    },
                    observations,
                );
            }
        };
        let mut candidate_overrides = Vec::new();
        let mut candidate_seen = SmallSet::new();
        for (name, _) in files.overrides.iter() {
            candidate_seen.insert(name.clone());
            candidate_overrides.push(name.clone());
        }
        for (name, _) in policy.command_module_overrides() {
            if candidate_seen.insert(CompactString::new(name)) {
                candidate_overrides.push(name.into());
            }
        }
        let mut overrides = SmallMap::new();
        for name in &candidate_overrides {
            if let Err(outcome) = effective_override(
                ctx,
                &self.workspace,
                mode,
                &mut overrides,
                &mut observations,
                name,
            )
            .await
            {
                return outcome;
            }
        }
        let entries = match discover_fixed_point(
            ctx,
            &self.workspace,
            mode,
            &files.module,
            &mut overrides,
            &mut observations,
        )
        .await
        {
            Ok(entries) => entries,
            Err(outcome) => return outcome,
        };
        finish_select_stage(
            select_graph(entries, &candidate_overrides, &overrides),
            observations,
        )
    }
}

fn project_legacy_graph(result: GraphResult) -> GraphOutcome {
    SourcePreparationOutcome::Complete(result)
}

#[async_trait]
impl Key for HostSelectedModuleGraphKey {
    type Value = GraphOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match self.drive(ctx, HostSelectedModuleGraphMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => project_legacy_graph(result),
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy selected graph has no observed frontier")
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
impl Key for HostSelectedModuleGraphObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostSelectedModuleGraph, HostSelectedModuleGraphObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match self
            .0
            .drive(ctx, HostSelectedModuleGraphMode::Observed)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedModuleGraph::new(
                    result,
                    observations,
                )))
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
    use std::collections::BTreeMap;

    use dice::DetectCycles;
    use dice::Dice;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::interim_module::NonrootModuleBuilder;
    use crate::module_eval::RegistryMultipleOverride;
    use crate::module_eval::RootModuleRegistrations;
    use crate::registry::RegistryBaseUrl;
    use crate::source_preparation::HostDiscoveredModuleProvenance;

    fn version(value: &str) -> BazelModuleVersion {
        BazelModuleVersion::parse(value).unwrap()
    }

    fn key(name: &str, value: &str) -> HostGraphModuleKey {
        HostGraphModuleKey::module(name.into(), version(value))
    }

    fn source() -> HostGraphModuleSource {
        HostGraphModuleSource::Root(Arc::new(EvaluatedRootModule {
            header: None,
            dependencies: Arc::new([]),
            registrations: RootModuleRegistrations::default(),
        }))
    }

    fn dependency(apparent: &str, target: HostGraphModuleKey) -> RawDependency {
        RawDependency {
            apparent_name: Some(apparent.into()),
            requested: target.clone(),
            transformed: target,
        }
    }

    fn module(
        key: HostGraphModuleKey,
        dependencies: Vec<RawDependency>,
        nodep_dependencies: Vec<RawDependency>,
    ) -> RawModule {
        RawModule {
            key,
            source: source(),
            original_dependencies: dependencies.clone(),
            dependencies,
            nodep_dependencies,
        }
    }

    fn discovered(name: &str, value: &str) -> Arc<HostDiscoveredModule> {
        Arc::new(HostDiscoveredModule {
            module: NonrootModuleBuilder::new(
                NonrootModuleKey::new(name, value),
                name,
                value,
                name,
            )
            .build()
            .unwrap(),
            provenance: HostDiscoveredModuleProvenance::Registry {
                selected_registry: RegistryBaseUrl::new("https://registry.invalid"),
                module_file_attempts: Arc::new([]),
            },
        })
    }

    #[test]
    fn completed_leaf_errors_remain_structurally_typed() {
        let module = key("dep", "1.0");
        let missing = HostSelectedModuleGraphError::DiscoveryLeaf {
            module: module.clone(),
            error: HostDiscoveredModuleError::MissingVersion {
                module_name: "dep".into(),
            },
        };
        let builtin = HostSelectedModuleGraphError::DiscoveryLeaf {
            module,
            error: HostDiscoveredModuleError::InvalidBuiltinVersion {
                version: "1.0".into(),
            },
        };
        assert_ne!(missing, builtin);
        let left = SourcePreparationOutcome::Complete(Arc::new(Err::<HostSelectedModuleGraph, _>(
            missing,
        )));
        let right = SourcePreparationOutcome::Complete(Arc::new(
            Err::<HostSelectedModuleGraph, _>(builtin),
        ));
        assert!(!left.complete_eq(&right));
    }

    #[test]
    fn observed_graph_identity_arc_epoch_and_full_horizon_merge_are_exact() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let graph_key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());
        let other = HostSelectedModuleGraphObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |value: &HostSelectedModuleGraphObservationKey| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        assert_ne!(graph_key, other);
        assert_ne!(hash(&graph_key), hash(&other));
        assert_eq!(
            graph_key.to_string(),
            "observed-host-selected-module-graph:\"/workspace\""
        );

        let epoch = |name: &str, inode: u64| {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/{name}")).unwrap(),
                PathObservationOperation::Lstat,
            );
            let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, inode as i64, 4, 0o644),
            )));
            (
                demand.dupe(),
                result.dupe(),
                PathObservationEpoch::from_shared([(demand, result)]).unwrap(),
            )
        };
        let (demand, first, prefix) = epoch("root", 1);
        let duplicate = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let merged = merge_graph_prefix(&prefix, &duplicate).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
        let (_, _, conflict) = epoch("root", 2);
        assert!(matches!(
            merge_graph_prefix(&prefix, &conflict),
            Err(HostSelectedModuleGraphObservationError::Merge(
                ObservedPathFrontierError::Epoch(
                    slug_workspace_v2::PathObservationEpochError::ConflictingDemand(_)
                )
            ))
        ));

        let result = Arc::new(Ok(HostSelectedModuleGraph {
            resolved: Arc::new([]),
            unpruned: Arc::new([]),
        }));
        let complete = SourcePreparationOutcome::Complete(Ok(
            ObservedHostSelectedModuleGraph::new(result.dupe(), prefix.dupe()),
        ));
        let SourcePreparationOutcome::Complete(Ok(observed)) = &complete else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(observed.result(), &result));
        assert_eq!(observed.observations(), &prefix);
        assert!(HostSelectedModuleGraphObservationKey::validity(&complete));
        assert!(HostSelectedModuleGraphObservationKey::equality(
            &complete, &complete
        ));
        let projected = project_legacy_graph(result.dupe());
        let SourcePreparationOutcome::Complete(projected) = projected else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(&projected, &result));

        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(demand.dupe()),
        ));
        assert!(!HostSelectedModuleGraphObservationKey::validity(&need));
        assert!(!HostSelectedModuleGraphObservationKey::equality(
            &need, &need
        ));
        let outer = SourcePreparationOutcome::Complete(Err(
            HostSelectedModuleGraphObservationError::Merge(ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                    demand,
                    result_operation: PathObservationOperation::FileBytes,
                },
            )),
        ));
        assert!(HostSelectedModuleGraphObservationKey::validity(&outer));
        assert!(HostSelectedModuleGraphObservationKey::equality(
            &outer, &outer
        ));

        let modules = [key("a", "1"), key("b", "1"), key("c", "1")];
        let mut outcomes = SmallMap::new();
        let mut expected = prefix.dupe();
        for (index, module) in modules.iter().enumerate() {
            let (_, _, incoming) = epoch(&format!("leaf-{index}"), index as u64 + 10);
            expected = merge_graph_prefix(&expected, &incoming).unwrap();
            outcomes.insert(
                module.clone(),
                Ok(SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostDiscoveredModuleError::MissingVersion {
                        module_name: module.name().unwrap().into(),
                    })),
                    incoming,
                )))),
            );
        }
        let Err(SourcePreparationOutcome::Complete(Ok((result, actual)))) =
            finish_horizon(&modules, &outcomes, &prefix)
        else {
            panic!("first semantic leaf error must retain every complete sibling epoch")
        };
        assert!(matches!(
            result.as_ref(),
            Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. })
                if module == &modules[0]
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn graph_stage_and_horizon_terminals_preserve_full_prefix_and_order() {
        let epoch = |name: &str, inode: i64| {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/{name}")).unwrap(),
                PathObservationOperation::Lstat,
            );
            let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, inode, 4, 0o644),
            )));
            PathObservationEpoch::from_shared([(demand, result)]).unwrap()
        };
        let root = epoch("root", 1);
        let policy = epoch("policy", 2);
        let effective = epoch("effective", 3);
        let root_policy = merge_graph_prefix(&root, &policy).unwrap();
        let full = merge_graph_prefix(&root_policy, &effective).unwrap();
        let SourcePreparationOutcome::Complete(Ok((result, actual))) =
            root_compute_error(HostSelectedModuleGraphError::Input {
                owner: "root MODULE files compute".into(),
                message: "compute".into(),
            })
        else {
            panic!("root compute failures are semantic")
        };
        assert!(result.is_err());
        assert_eq!(actual, PathObservationEpoch::empty());

        type StageProjector =
            fn(HostSelectedModuleGraphError, PathObservationEpoch) -> GraphDriverOutcome;
        for (project, owner, prefix) in [
            (
                root_semantic_error as StageProjector,
                "root MODULE files semantic",
                root.dupe(),
            ),
            (
                policy_compute_error as StageProjector,
                "command policy",
                root.dupe(),
            ),
            (
                effective_stage_error as StageProjector,
                "effective override first",
                root_policy.dupe(),
            ),
            (
                transform_stage_error as StageProjector,
                "transform",
                full.dupe(),
            ),
            (
                (|error, observations| finish_select_stage(Err(error), observations))
                    as StageProjector,
                "select",
                full.dupe(),
            ),
        ] {
            let SourcePreparationOutcome::Complete(Ok((result, actual))) = project(
                HostSelectedModuleGraphError::Input {
                    owner: owner.into(),
                    message: "compute".into(),
                },
                prefix.dupe(),
            ) else {
                panic!("stage compute failures are semantic")
            };
            assert!(
                matches!(result.as_ref(), Err(HostSelectedModuleGraphError::Input { owner: actual, .. }) if actual == owner)
            );
            assert_eq!(actual, prefix);
        }
        assert!(matches!(
            merge_graph_prefix(&root, &epoch("root", 99)),
            Err(HostSelectedModuleGraphObservationError::Merge(_))
        ));

        let modules = [key("a", "1"), key("b", "1"), key("c", "1")];
        let incoming = [
            epoch("leaf-a", 10),
            epoch("leaf-b", 11),
            epoch("leaf-c", 12),
        ];
        let complete = |module: &HostGraphModuleKey, observations: PathObservationEpoch| {
            Ok(SourcePreparationOutcome::Complete(Ok((
                Arc::new(Ok(discovered(module.name().unwrap(), "1").as_ref().clone())),
                observations,
            ))))
        };
        let expected = incoming.iter().fold(root.dupe(), |prefix, next| {
            merge_graph_prefix(&prefix, next).unwrap()
        });
        for position in 0..modules.len() {
            let mut outcomes = modules
                .iter()
                .zip(incoming.iter())
                .map(|(module, observations)| {
                    (module.clone(), complete(module, observations.dupe()))
                })
                .collect::<SmallMap<_, _>>();
            outcomes.insert(modules[position].clone(), Err("compute".into()));
            let Err(SourcePreparationOutcome::Complete(Ok((result, actual)))) =
                finish_horizon(&modules, &outcomes, &root)
            else {
                panic!("horizon compute failure is semantic")
            };
            assert!(
                matches!(result.as_ref(), Err(HostSelectedModuleGraphError::DiscoveryCompute { module, .. }) if module == &modules[position])
            );
            let expected_without = incoming
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != position)
                .fold(root.dupe(), |prefix, (_, next)| {
                    merge_graph_prefix(&prefix, next).unwrap()
                });
            assert_eq!(actual, expected_without);

            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/outer-{position}")).unwrap(),
                PathObservationOperation::Lstat,
            );
            let outer =
                HostSelectedModuleGraphObservationError::Merge(ObservedPathFrontierError::Epoch(
                    slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                        demand,
                        result_operation: PathObservationOperation::FileBytes,
                    },
                ));
            outcomes.insert(
                modules[position].clone(),
                Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
            );
            let Err(SourcePreparationOutcome::Complete(Err(actual))) =
                finish_horizon(&modules, &outcomes, &root)
            else {
                panic!("horizon outer must be carrierless")
            };
            assert_eq!(actual, outer);
        }

        for position in 0..modules.len() {
            let mut outcomes = modules
                .iter()
                .zip(incoming.iter())
                .map(|(module, observations)| {
                    (module.clone(), complete(module, observations.dupe()))
                })
                .collect::<SmallMap<_, _>>();
            outcomes.insert(
                modules[position].clone(),
                Ok(SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostDiscoveredModuleError::MissingVersion {
                        module_name: modules[position].name().unwrap().into(),
                    })),
                    incoming[position].dupe(),
                )))),
            );
            let Err(SourcePreparationOutcome::Complete(Ok((result, actual)))) =
                finish_horizon(&modules, &outcomes, &root)
            else {
                panic!("semantic leaf errors retain every complete epoch")
            };
            assert!(
                matches!(result.as_ref(), Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. }) if module == &modules[position])
            );
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn production_effective_finisher_preserves_prefix_need_outer_and_first_arc() {
        let epoch = |name: &str, inode: i64| {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/{name}")).unwrap(),
                PathObservationOperation::Lstat,
            );
            let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, inode, 4, 0o644),
            )));
            (
                demand.dupe(),
                result.dupe(),
                PathObservationEpoch::from_shared([(demand, result)]).unwrap(),
            )
        };
        let (root_demand, root_arc, root) = epoch("root", 1);
        let (_, _, policy) = epoch("policy", 2);
        let (_, _, effective) = epoch("effective", 3);
        let prefixes = [
            root.dupe(),
            merge_graph_prefix(&root, &policy).unwrap(),
            merge_graph_prefix(&merge_graph_prefix(&root, &policy).unwrap(), &effective).unwrap(),
        ];
        let need = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/need").unwrap(),
                PathObservationOperation::Lstat,
            ),
        ));
        let mismatch = ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: root_demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        for prefix in prefixes {
            assert!(matches!(
                finish_observed_effective(SourcePreparationOutcome::Need(need.clone()), &prefix,),
                Err(SourcePreparationOutcome::Need(_))
            ));
            assert!(matches!(
                finish_observed_effective(
                    SourcePreparationOutcome::Complete(Err(mismatch.dupe())),
                    &prefix,
                ),
                Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedModuleGraphObservationError::Effective(actual)
                ))) if actual == mismatch
            ));
            let observed = ObservedHostEffectiveModuleOverride::new(
                Arc::new(Ok(HostEffectiveModuleOverride::None)),
                root.dupe(),
            );
            let (_, merged) = finish_observed_effective(
                SourcePreparationOutcome::Complete(Ok(observed)),
                &prefix,
            )
            .unwrap();
            assert!(Arc::ptr_eq(merged.get(&root_demand).unwrap(), &root_arc));
        }

        let (_, _, conflict) = epoch("root", 99);
        assert!(matches!(
            finish_observed_effective(
                SourcePreparationOutcome::Complete(Ok(ObservedHostEffectiveModuleOverride::new(
                    Arc::new(Ok(HostEffectiveModuleOverride::None)),
                    conflict,
                ))),
                &root,
            ),
            Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedModuleGraphObservationError::Merge(_)
            )))
        ));
    }

    #[test]
    fn discovery_outer_and_merge_outer_keep_first_horizon_order() {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/shared").unwrap(),
            PathObservationOperation::Lstat,
        );
        let result = |inode| {
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, inode, 4, 0o644),
            )))
        };
        let prefix = PathObservationEpoch::from_shared([(demand.dupe(), result(1))]).unwrap();
        let conflict = PathObservationEpoch::from_shared([(demand.dupe(), result(2))]).unwrap();
        let mismatch = ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        let discovery = HostSelectedModuleGraphObservationError::Discovery(
            HostDiscoveredModuleObservationError::EffectiveFrontier(mismatch),
        );
        let modules = [key("a", "1"), key("b", "1"), key("c", "1")];
        let complete = |module: &HostGraphModuleKey, epoch: PathObservationEpoch| {
            Ok(SourcePreparationOutcome::Complete(Ok((
                Arc::new(Ok(discovered(module.name().unwrap(), "1").as_ref().clone())),
                epoch,
            ))))
        };

        let mut child_first = SmallMap::from_iter([
            (
                modules[0].clone(),
                Ok(SourcePreparationOutcome::Complete(Err(discovery.dupe()))),
            ),
            (modules[1].clone(), complete(&modules[1], conflict.dupe())),
            (
                modules[2].clone(),
                complete(&modules[2], PathObservationEpoch::empty()),
            ),
        ]);
        assert!(matches!(
            finish_horizon(&modules, &child_first, &prefix),
            Err(SourcePreparationOutcome::Complete(Err(actual))) if actual == discovery
        ));

        child_first.insert(modules[0].clone(), complete(&modules[0], conflict));
        child_first.insert(
            modules[1].clone(),
            Ok(SourcePreparationOutcome::Complete(Err(discovery.dupe()))),
        );
        assert!(matches!(
            finish_horizon(&modules, &child_first, &prefix),
            Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedModuleGraphObservationError::Merge(_)
            )))
        ));
    }
    #[test]
    fn horizon_complete_error_wins_over_need_and_compatible_needs_union() {
        let a = key("a", "1");
        let b = key("b", "1");
        let path_need = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/a").unwrap(),
                PathObservationOperation::Lstat,
            ),
        ));
        let bootstrap_need =
            SourcePreparationNeeds::root_module_bootstrap(crate::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            });
        let leaf_error = |name: &str| {
            Ok(SourcePreparationOutcome::Complete(Ok((
                Arc::new(Err(HostDiscoveredModuleError::MissingVersion {
                    module_name: name.into(),
                })),
                PathObservationEpoch::empty(),
            ))))
        };
        let semantic = |outcome: Result<_, GraphDriverOutcome>| {
            let Err(SourcePreparationOutcome::Complete(Ok((result, _)))) = outcome else {
                panic!("horizon semantic terminal must retain a graph carrier");
            };
            result.as_ref().clone().unwrap_err()
        };

        let mut mixed = SmallMap::new();
        mixed.insert(
            a.clone(),
            Ok(SourcePreparationOutcome::Need(path_need.clone())),
        );
        mixed.insert(b.clone(), leaf_error("b"));
        assert!(matches!(
            semantic(finish_horizon(
                &[a.clone(), b.clone()],
                &mixed,
                &PathObservationEpoch::empty()
            )),
            HostSelectedModuleGraphError::DiscoveryLeaf { module, .. } if module == b
        ));

        let mut compatible = SmallMap::new();
        compatible.insert(a.clone(), Ok(SourcePreparationOutcome::Need(path_need)));
        compatible.insert(
            b.clone(),
            Ok(SourcePreparationOutcome::Need(bootstrap_need)),
        );
        let Err(SourcePreparationOutcome::Need(needs)) = finish_horizon(
            &[a.clone(), b.clone()],
            &compatible,
            &PathObservationEpoch::empty(),
        ) else {
            panic!("compatible horizon Needs must remain transient");
        };
        assert!(needs.path_observations().is_some());
        assert!(needs.root_module_bootstrap_request().is_some());

        let incompatible_left =
            SourcePreparationNeeds::root_module_bootstrap(crate::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/left").unwrap(),
            });
        let incompatible_right =
            SourcePreparationNeeds::root_module_bootstrap(crate::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/right").unwrap(),
            });
        let mut incompatible = SmallMap::new();
        incompatible.insert(
            a.clone(),
            Ok(SourcePreparationOutcome::Need(incompatible_left)),
        );
        incompatible.insert(
            b.clone(),
            Ok(SourcePreparationOutcome::Need(incompatible_right)),
        );
        assert!(matches!(
            semantic(finish_horizon(
                &[a.clone(), b.clone()],
                &incompatible,
                &PathObservationEpoch::empty()
            )),
            HostSelectedModuleGraphError::IncompatibleNeeds(_)
        ));
        let c = key("c", "1");
        incompatible.insert(c.clone(), leaf_error("c"));
        assert!(matches!(
            semantic(finish_horizon(
                &[a, b, c.clone()],
                &incompatible,
                &PathObservationEpoch::empty()
            )),
            HostSelectedModuleGraphError::DiscoveryLeaf { module, .. } if module == c
        ));
    }

    #[test]
    fn horizon_is_first_seen_deduplicated_and_nodep_name_gated() {
        let a = key("a", "1");
        let b = key("b", "1");
        let root = module(
            HostGraphModuleKey::Root,
            vec![dependency("a", a.clone()), dependency("again", a.clone())],
            vec![dependency("nodep", b.clone())],
        );
        let seen = SmallSet::from_iter([HostGraphModuleKey::Root]);
        assert_eq!(
            next_horizon(
                &[root.clone()],
                &[HostGraphModuleKey::Root],
                &SmallSet::new(),
                &seen
            ),
            vec![a.clone()]
        );
        assert_eq!(
            next_horizon(
                &[root],
                &[HostGraphModuleKey::Root],
                &SmallSet::from_iter(["b".into()]),
                &seen
            ),
            vec![a, b]
        );
    }

    #[test]
    fn selection_uses_highest_version_and_preserves_bfs_and_unpruned_order() {
        let a1 = key("a", "1");
        let a2 = key("a", "2");
        let b = key("b", "1");
        let graph = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![dependency("a", a1.clone()), dependency("b", b.clone())],
                    vec![],
                ),
                module(a1.clone(), vec![], vec![]),
                module(b.clone(), vec![dependency("a", a2.clone())], vec![]),
                module(a2.clone(), vec![], vec![]),
            ],
            &[],
            &SmallMap::new(),
        )
        .unwrap();
        assert_eq!(
            graph
                .resolved
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            vec![HostGraphModuleKey::Root, a2.clone(), b]
        );
        assert_eq!(
            graph
                .unpruned
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            vec![HostGraphModuleKey::Root, a1, key("b", "1"), a2]
        );
        assert_eq!(graph.resolved[0].dependencies[0].key, key("a", "2"));
        assert_eq!(
            graph.resolved[0].original_dependencies[0].key,
            key("a", "1")
        );
    }

    #[test]
    fn nodep_validates_but_does_not_create_final_reachability() {
        let a = key("a", "1");
        let graph = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![],
                    vec![dependency("nodep", a.clone())],
                ),
                module(a, vec![], vec![]),
            ],
            &[],
            &SmallMap::new(),
        )
        .unwrap();
        assert_eq!(graph.resolved.len(), 1);
        assert_eq!(graph.unpruned.len(), 2);
    }

    #[test]
    fn unreachable_multiple_version_no_ceiling_is_retained_unpruned() {
        let a = key("a", "1");
        let z = key("z", "1");
        let m1 = key("m", "1");
        let m2 = key("m", "2");
        let m3 = key("m", "3");
        let mut overrides = SmallMap::new();
        overrides.insert(
            "m".into(),
            HostEffectiveModuleOverride::Root {
                override_: RootModuleOverride::RegistryMultiple(RegistryMultipleOverride {
                    versions: Arc::from(["1".into(), "2".into()]),
                    registry: "".into(),
                }),
            },
        );
        let graph = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![dependency("a", a.clone())],
                    vec![],
                ),
                module(a, vec![], vec![]),
                module(z, vec![dependency("m", m3.clone())], vec![]),
                module(m1, vec![], vec![]),
                module(m2, vec![], vec![]),
            ],
            &["m".into()],
            &overrides,
        )
        .unwrap();
        assert_eq!(graph.resolved.len(), 2);
        assert!(graph.unpruned.iter().any(|entry| {
            entry
                .dependencies
                .iter()
                .any(|dependency| dependency.key == m3)
        }));
    }

    #[test]
    fn duplicate_apparent_names_for_one_selected_dependency_fail() {
        let a = key("a", "1");
        let error = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![
                        dependency("first", a.clone()),
                        dependency("second", a.clone()),
                    ],
                    vec![],
                ),
                module(a, vec![], vec![]),
            ],
            &[],
            &SmallMap::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            HostSelectedModuleGraphError::DuplicateDependency {
                first_apparent_name,
                second_apparent_name,
                ..
            } if first_apparent_name == "first" && second_apparent_name == "second"
        ));
    }

    #[test]
    fn multiple_version_terminals_and_lowest_allowed_ceiling_are_distinct() {
        let mut overrides = SmallMap::new();
        overrides.insert(
            "m".into(),
            HostEffectiveModuleOverride::Root {
                override_: RootModuleOverride::RegistryMultiple(RegistryMultipleOverride {
                    versions: Arc::from(["2".into(), "4".into()]),
                    registry: "".into(),
                }),
            },
        );
        let missing = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![dependency("m", key("m", "1"))],
                    vec![],
                ),
                module(key("m", "1"), vec![], vec![]),
                module(key("m", "2"), vec![], vec![]),
            ],
            &["m".into()],
            &overrides,
        )
        .unwrap_err();
        assert!(matches!(
            missing,
            HostSelectedModuleGraphError::AllowedVersionMissing {
                version: missing_version,
                ..
            } if missing_version == version("4")
        ));

        let entries = vec![
            module(
                HostGraphModuleKey::Root,
                vec![dependency("m", key("m", "5"))],
                vec![],
            ),
            module(key("m", "2"), vec![], vec![]),
            module(key("m", "4"), vec![], vec![]),
            module(key("m", "5"), vec![], vec![]),
        ];
        let no_ceiling = select_graph(entries, &["m".into()], &overrides).unwrap_err();
        assert!(matches!(
            no_ceiling,
            HostSelectedModuleGraphError::MultipleVersionNoCeiling {
                version: excess_version,
                ..
            } if excess_version == version("5")
        ));

        let success = select_graph(
            vec![
                module(
                    HostGraphModuleKey::Root,
                    vec![dependency("m", key("m", "3"))],
                    vec![],
                ),
                module(key("m", "2"), vec![], vec![]),
                module(key("m", "3"), vec![], vec![]),
                module(key("m", "4"), vec![], vec![]),
            ],
            &["m".into()],
            &overrides,
        )
        .unwrap();
        assert_eq!(success.resolved[0].dependencies[0].key, key("m", "4"));
    }

    #[test]
    fn diamond_cycle_bfs_order_is_stable_and_finite() {
        let a = key("a", "1");
        let b = key("b", "1");
        let c = key("c", "1");
        let root = module(
            HostGraphModuleKey::Root,
            vec![dependency("a", a.clone()), dependency("b", b.clone())],
            vec![],
        );
        let a_entry = module(a.clone(), vec![dependency("c", c.clone())], vec![]);
        let b_entry = module(b.clone(), vec![dependency("c", c.clone())], vec![]);
        let c_entry = module(c.clone(), vec![dependency("a", a.clone())], vec![]);
        let graph =
            select_graph(vec![root, a_entry, b_entry, c_entry], &[], &SmallMap::new()).unwrap();
        assert_eq!(
            graph
                .resolved
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            vec![HostGraphModuleKey::Root, a, b, c]
        );
    }

    struct RegistryIo(BTreeMap<String, Arc<[u8]>>);

    #[async_trait]
    impl crate::RegistryIo for RegistryIo {
        async fn read_exact(
            &self,
            url: &crate::RegistryFileUrl,
        ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
            Ok(self
                .0
                .get(url.as_str())
                .map_or(crate::RegistryIoOutcome::NotFound, |bytes| {
                    crate::RegistryIoOutcome::Found(bytes.clone())
                }))
        }
    }

    async fn compute_graph(dice: &Arc<Dice>, root_source: &str, generation: u64) -> GraphOutcome {
        compute_graph_with_overrides(dice, root_source, generation, &[]).await
    }

    async fn compute_graph_with_overrides(
        dice: &Arc<Dice>,
        root_source: &str,
        generation: u64,
        overrides: &[&str],
    ) -> GraphOutcome {
        let workspace = NormalizedAbsolutePath::new("/selected-graph-test").unwrap();
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_path_buf(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        WorkspaceFileValue::Present(Arc::new(root_source.to_owned())),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_path_buf(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        crate::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                workspace.as_path(),
                overrides.iter().copied(),
            )
            .unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        crate::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::RegistryUrls::new(["https://registry.invalid"]),
            crate::RegistryRequestGeneration(generation),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                crate::RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                crate::RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        updater
            .commit()
            .await
            .compute(&HostSelectedModuleGraphKey::new(workspace))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn retained_dice_normalization_semantic_aba_and_warm_reuse() {
        let mut builder = Dice::builder();
        crate::install_registry_io(
            &mut builder,
            Arc::new(RegistryIo(
                [
                    (
                        "https://registry.invalid/modules/dep/1/MODULE.bazel".to_owned(),
                        Arc::from(&b"module(name='dep', version='1')\n"[..]),
                    ),
                    (
                        "https://registry.invalid/modules/dep/2/MODULE.bazel".to_owned(),
                        Arc::from(&b"module(name='dep', version='2')\n"[..]),
                    ),
                ]
                .into_iter()
                .collect(),
            )),
        );
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let a = compute_graph(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1+root')\n",
            1,
        )
        .await;
        let equivalent_a = compute_graph(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1+other')\n",
            1,
        )
        .await;
        let b = compute_graph(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='2')\n",
            2,
        )
        .await;
        let restored_a = compute_graph(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep', version='1+final')\n",
            3,
        )
        .await;
        assert!(HostSelectedModuleGraphKey::equality(&a, &equivalent_a));
        assert!(
            !HostSelectedModuleGraphKey::equality(&a, &b),
            "A={a:#?}\nB={b:#?}"
        );
        assert!(HostSelectedModuleGraphKey::equality(&a, &restored_a));
        assert!(HostSelectedModuleGraphKey::validity(&a));
        assert!(matches!(
            a,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap().resolved.len() == 2
        ));
    }

    #[tokio::test]
    async fn builtin_default_command_bypass_and_explicit_root_terminal() {
        let tools_root = "/selected-graph-test/tools";
        let mut builder = Dice::builder();
        crate::install_registry_io(&mut builder, Arc::new(RegistryIo(BTreeMap::new())));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));

        let default = compute_graph(&dice, "module(name='root')\n", 1).await;
        assert!(matches!(
            default,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostSelectedModuleGraphError::DiscoveryLeaf {
                        module: HostGraphModuleKey::Module { name, .. },
                        ..
                    }) if name == "rules_license"
                )
        ));

        let command = compute_graph_with_overrides(
            &dice,
            "module(name='root')\n",
            2,
            &[&format!("bazel_tools={tools_root}")],
        )
        .await;
        assert!(matches!(command, SourcePreparationOutcome::Need(_)));
        assert!(!HostSelectedModuleGraphKey::validity(&command));
        assert!(!HostSelectedModuleGraphKey::equality(&command, &command));

        let explicit_root = compute_graph(
            &dice,
            "module(name='root')\nlocal_path_override(module_name='bazel_tools', path='tools')\n",
            3,
        )
        .await;
        assert!(matches!(
            explicit_root,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostSelectedModuleGraphError::DiscoveryLeaf {
                        error: HostDiscoveredModuleError::ExplicitBuiltinOverride,
                        ..
                    })
                )
        ));
    }

    #[tokio::test]
    async fn real_dice_nodep_name_admission_requires_a_second_graph_round() {
        let mut builder = Dice::builder();
        crate::install_registry_io(
            &mut builder,
            Arc::new(RegistryIo(
                [
                    (
                        "https://registry.invalid/modules/a/1/MODULE.bazel".to_owned(),
                        Arc::from(
                            &b"module(name='a', version='1')\nbazel_dep(name='b', version='1')\nbazel_dep(name='b', version='2', repo_name=None)\n"[..],
                        ),
                    ),
                    (
                        "https://registry.invalid/modules/b/1/MODULE.bazel".to_owned(),
                        Arc::from(&b"module(name='b', version='1')\n"[..]),
                    ),
                    (
                        "https://registry.invalid/modules/b/2/MODULE.bazel".to_owned(),
                        Arc::from(&b"module(name='b', version='2')\n"[..]),
                    ),
                ]
                .into_iter()
                .collect(),
            )),
        );
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let outcome = compute_graph(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='a', version='1')\n",
            1,
        )
        .await;
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("registry-only nodep graph must complete");
        };
        let graph = value.as_ref().as_ref().unwrap();
        assert_eq!(
            graph
                .unpruned
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            vec![
                HostGraphModuleKey::Root,
                key("a", "1"),
                key("b", "1"),
                key("b", "2")
            ]
        );
        assert_eq!(
            graph
                .resolved
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            vec![HostGraphModuleKey::Root, key("a", "1"), key("b", "2")]
        );
    }
}
