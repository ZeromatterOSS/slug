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
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::interim_module::NonrootDependency;
use crate::interim_module::NonrootModuleKey;
use crate::module_eval::EvaluatedRootModule;
use crate::module_eval::HostEffectiveModuleOverride;
use crate::module_eval::HostEffectiveModuleOverrideKey;
use crate::module_eval::RegistryMultipleOverride;
use crate::module_eval::RootModuleCommandPolicyKey;
use crate::module_eval::RootModuleFilesKey;
use crate::module_eval::RootModuleOverride;
use crate::module_version::BazelModuleVersion;
use crate::source_preparation::HostDiscoveredModule;
use crate::source_preparation::HostDiscoveredModuleError;
use crate::source_preparation::HostDiscoveredModuleKey;
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

type GraphOutcome =
    SourcePreparationOutcome<Arc<Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>>>;

fn complete_error(error: HostSelectedModuleGraphError) -> GraphOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
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
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    module_name: &CompactString,
) -> Result<HostEffectiveModuleOverride, HostSelectedModuleGraphError> {
    if let Some(value) = cache.get(module_name.as_str()) {
        return Ok(value.clone());
    }
    let value = ctx
        .compute(&HostEffectiveModuleOverrideKey::new(
            workspace.dupe(),
            module_name.clone(),
        ))
        .await
        .map_err(|error| HostSelectedModuleGraphError::Input {
            owner: format!("effective override {module_name}").into(),
            message: error.to_string().into(),
        })?;
    let value = value
        .as_ref()
        .clone()
        .map_err(|error| HostSelectedModuleGraphError::Input {
            owner: format!("effective override {module_name}").into(),
            message: error.to_string().into(),
        })?;
    cache.insert(module_name.clone(), value.clone());
    Ok(value)
}

async fn transform_request(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    module_name: CompactString,
    version: BazelModuleVersion,
) -> Result<HostGraphModuleKey, HostSelectedModuleGraphError> {
    if root_name == Some(module_name.as_str()) {
        return Ok(HostGraphModuleKey::Root);
    }
    let effective = effective_override(ctx, workspace, cache, &module_name).await?;
    let version = match effective.override_() {
        Some(RootModuleOverride::NonRegistry(_)) => BazelModuleVersion::empty(),
        Some(RootModuleOverride::RegistrySingle(single)) if !single.version.is_empty() => {
            parse_version(&module_name, &single.version)?
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
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    apparent_name: Option<CompactString>,
    dependency: &NonrootDependency,
) -> Result<RawDependency, HostSelectedModuleGraphError> {
    let version = parse_version(&dependency.name, &dependency.version)?;
    let requested = HostGraphModuleKey::module(dependency.name.clone(), version.clone());
    let transformed = transform_request(
        ctx,
        workspace,
        root_name,
        cache,
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
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    dependencies: Vec<(Option<CompactString>, NonrootDependency)>,
) -> Result<Vec<RawDependency>, HostSelectedModuleGraphError> {
    let mut result = Vec::new();
    for (apparent_name, dependency) in dependencies {
        result.push(
            raw_dependency(ctx, workspace, root_name, cache, apparent_name, &dependency).await?,
        );
    }
    Ok(result)
}

async fn raw_root(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    module: &EvaluatedRootModule,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
) -> Result<RawModule, HostSelectedModuleGraphError> {
    let root_name = module.header.as_ref().map(|header| header.name.as_str());
    let mut dependencies = Vec::new();
    let mut nodep_dependencies = Vec::new();
    for dependency in module.dependencies.iter() {
        let version = parse_version(&dependency.name, &dependency.version)?;
        let requested = HostGraphModuleKey::module(dependency.name.clone(), version.clone());
        let transformed = transform_request(
            ctx,
            workspace,
            root_name,
            cache,
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
            root_name,
            cache,
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
    root_name: Option<&str>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
    key: HostGraphModuleKey,
    module: Arc<HostDiscoveredModule>,
) -> Result<RawModule, HostSelectedModuleGraphError> {
    let base = &module.module.base;
    let dependencies = raw_dependencies(
        ctx,
        workspace,
        root_name,
        cache,
        base.dependencies
            .iter()
            .map(|(name, dependency)| (Some(name.clone()), dependency.clone()))
            .collect(),
    )
    .await?;
    let original_dependencies = raw_dependencies(
        ctx,
        workspace,
        root_name,
        cache,
        base.original_dependencies
            .iter()
            .map(|(name, dependency)| (Some(name.clone()), dependency.clone()))
            .collect(),
    )
    .await?;
    let nodep_dependencies = raw_dependencies(
        ctx,
        workspace,
        root_name,
        cache,
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

type LeafOutcome = Result<
    SourcePreparationOutcome<Arc<Result<HostDiscoveredModule, HostDiscoveredModuleError>>>,
    CompactString,
>;

fn finish_horizon(
    next: &[HostGraphModuleKey],
    outcomes: &SmallMap<HostGraphModuleKey, LeafOutcome>,
) -> Result<
    SourcePreparationOutcome<Vec<(HostGraphModuleKey, Arc<HostDiscoveredModule>)>>,
    HostSelectedModuleGraphError,
> {
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
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => complete.push((module.clone(), Arc::new(value.clone()))),
                Err(error) if first_error.is_none() => {
                    first_error = Some(HostSelectedModuleGraphError::DiscoveryLeaf {
                        module: module.clone(),
                        error: error.clone(),
                    });
                }
                Err(_) => {}
            },
        }
    }
    match (first_error, need_error, needs) {
        (Some(error), _, _) | (None, Some(error), _) => Err(error),
        (None, None, Some(need)) => Ok(SourcePreparationOutcome::Need(need)),
        (None, None, None) => Ok(SourcePreparationOutcome::Complete(complete)),
    }
}

async fn discover_round(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    root: RawModule,
    root_name: Option<&str>,
    prior_names: &SmallSet<CompactString>,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
) -> Result<SourcePreparationOutcome<Vec<RawModule>>, HostSelectedModuleGraphError> {
    let mut entries = vec![root];
    let mut seen = SmallSet::from_iter([HostGraphModuleKey::Root]);
    let mut frontier = vec![HostGraphModuleKey::Root];
    loop {
        let next = next_horizon(&entries, &frontier, prior_names, &seen);
        if next.is_empty() {
            return Ok(SourcePreparationOutcome::Complete(entries));
        }
        let computed = ctx
            .compute_join(next.iter().cloned(), |ctx, module| {
                let workspace = workspace.dupe();
                Box::pin(async move {
                    let key = match &module {
                        HostGraphModuleKey::Module { name, version } => {
                            HostDiscoveredModuleKey::try_new(
                                workspace,
                                NonrootModuleKey::new(name.clone(), version.normalized()),
                            )
                            .expect("typed graph versions construct checked Host keys")
                        }
                        HostGraphModuleKey::Root => unreachable!("root is never a leaf horizon"),
                    };
                    let value = ctx
                        .compute(&key)
                        .await
                        .map_err(|error| CompactString::new(error.to_string()));
                    (module, value)
                })
            })
            .await;
        let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
        let complete = match finish_horizon(&next, &outcomes)? {
            SourcePreparationOutcome::Complete(complete) => complete,
            SourcePreparationOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(need));
            }
        };
        frontier.clear();
        for (key, module) in complete {
            seen.insert(key.clone());
            entries
                .push(raw_discovered(ctx, workspace, root_name, cache, key.clone(), module).await?);
            frontier.push(key);
        }
    }
}

async fn discover_fixed_point(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    root_module: &EvaluatedRootModule,
    cache: &mut SmallMap<CompactString, HostEffectiveModuleOverride>,
) -> Result<SourcePreparationOutcome<Vec<RawModule>>, HostSelectedModuleGraphError> {
    let mut prior_names = SmallSet::new();
    let mut previous_keys: Option<SmallSet<HostGraphModuleKey>> = None;
    loop {
        let root = raw_root(ctx, workspace, root_module, cache).await?;
        let entries = match discover_round(
            ctx,
            workspace,
            root,
            root_module
                .header
                .as_ref()
                .map(|header| header.name.as_str()),
            &prior_names,
            cache,
        )
        .await?
        {
            SourcePreparationOutcome::Complete(entries) => entries,
            SourcePreparationOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(need));
            }
        };
        let keys = entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<SmallSet<_>>();
        if previous_keys.as_ref() == Some(&keys) {
            return Ok(SourcePreparationOutcome::Complete(entries));
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

#[async_trait]
impl Key for HostSelectedModuleGraphKey {
    type Value = GraphOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let files = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.as_path().to_owned(),
            })
            .await
        {
            Ok(value) => match value.as_ref() {
                Ok(files) => files.clone(),
                Err(error) => {
                    return complete_error(HostSelectedModuleGraphError::Input {
                        owner: "root MODULE files".into(),
                        message: error.clone(),
                    });
                }
            },
            Err(error) => {
                return complete_error(HostSelectedModuleGraphError::Input {
                    owner: "root MODULE files".into(),
                    message: error.to_string().into(),
                });
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
                return complete_error(HostSelectedModuleGraphError::Input {
                    owner: "command policy".into(),
                    message: error.to_string().into(),
                });
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
            match effective_override(ctx, &self.workspace, &mut overrides, name).await {
                Ok(_) => {}
                Err(error) => return complete_error(error),
            }
        }
        let entries =
            match discover_fixed_point(ctx, &self.workspace, &files.module, &mut overrides).await {
                Ok(SourcePreparationOutcome::Complete(entries)) => entries,
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Err(error) => return complete_error(error),
            };
        SourcePreparationOutcome::Complete(Arc::new(select_graph(
            entries,
            &candidate_overrides,
            &overrides,
        )))
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
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::module_eval::RegistryMultipleOverride;
    use crate::module_eval::RootModuleRegistrations;

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
        let mut mixed = SmallMap::new();
        mixed.insert(
            a.clone(),
            Ok(SourcePreparationOutcome::Need(path_need.clone())),
        );
        mixed.insert(
            b.clone(),
            Ok(SourcePreparationOutcome::Complete(Arc::new(Err(
                HostDiscoveredModuleError::MissingVersion {
                    module_name: "b".into(),
                },
            )))),
        );
        assert!(matches!(
            finish_horizon(&[a.clone(), b.clone()], &mixed),
            Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. }) if module == b
        ));

        let mut compatible = SmallMap::new();
        compatible.insert(a.clone(), Ok(SourcePreparationOutcome::Need(path_need)));
        compatible.insert(
            b.clone(),
            Ok(SourcePreparationOutcome::Need(bootstrap_need)),
        );
        let outcome = finish_horizon(&[a.clone(), b.clone()], &compatible).unwrap();
        let SourcePreparationOutcome::Need(needs) = &outcome else {
            panic!("compatible horizon Needs must remain transient");
        };
        assert!(needs.path_observations().is_some());
        assert!(needs.root_module_bootstrap_request().is_some());
        assert!(!outcome.is_complete());
        assert!(!outcome.complete_eq(&outcome));

        let c = key("c", "1");
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
            Ok(SourcePreparationOutcome::Need(incompatible_left.clone())),
        );
        incompatible.insert(
            b.clone(),
            Ok(SourcePreparationOutcome::Need(incompatible_right.clone())),
        );
        assert!(matches!(
            finish_horizon(&[a.clone(), b.clone()], &incompatible),
            Err(HostSelectedModuleGraphError::IncompatibleNeeds(_))
        ));
        incompatible.insert(
            c.clone(),
            Ok(SourcePreparationOutcome::Complete(Arc::new(Err(
                HostDiscoveredModuleError::MissingVersion {
                    module_name: "c".into(),
                },
            )))),
        );
        assert!(matches!(
            finish_horizon(&[a, b, c.clone()], &incompatible),
            Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. }) if module == c
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
