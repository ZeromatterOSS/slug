/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

//! Loading-owned expansion of selected MODULE registration patterns.

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::HostCanonicalRepositorySourceInput;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedRegistrationPatternView;
use slug_bzlmod_v2::HostSelectedRegistrationPatterns;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsError;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsKey;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsObservationError;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsObservationKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationDiagnosticLevel;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::CanonicalTargetPattern;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetPatternWildcard;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::ExternalSubtreePackageSetError;
use crate::ExternalSubtreePackageSetKey;
use crate::ExternalSubtreePackageSetObservationKey;
use crate::HostCanonicalRepositoryLoadRouteError;
use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::LoadedPackage;
use crate::PackageTarget;
use crate::PackageTargetKind;
use crate::RepositoryPackageLoadError;
use crate::RootPackageLoadError;
use crate::RootPackageLoadKey;
use crate::RootPackageLoadObservationKey;
use crate::RootSubtreePackageSetError;
use crate::RootSubtreePackageSetKey;
use crate::RootSubtreePackageSetObservationKey;
use crate::bzl_module::RepositoryPackageInventoryKey;
use crate::bzl_module::RepositoryPackageInventoryObservationKey;
use crate::package::NativeToolchainTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum ModuleRegistrationFamily {
    Toolchains,
    ExecutionPlatforms,
}

impl fmt::Display for ModuleRegistrationFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Toolchains => "toolchains",
            Self::ExecutionPlatforms => "execution-platforms",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleRegistrationAmbiguity {
    family: ModuleRegistrationFamily,
    row: u32,
    raw_pattern: CompactString,
    wildcard: CompactString,
    conflict_target: CanonicalLabel,
}

impl ModuleRegistrationAmbiguity {
    pub fn family(&self) -> ModuleRegistrationFamily {
        self.family
    }

    pub fn row(&self) -> u32 {
        self.row
    }

    pub fn raw_pattern(&self) -> &str {
        &self.raw_pattern
    }

    pub fn wildcard(&self) -> &str {
        &self.wildcard
    }

    pub fn conflict_target(&self) -> &CanonicalLabel {
        &self.conflict_target
    }

    fn event(&self) -> EvaluationEvent {
        EvaluationEvent::Diagnostic {
            level: EvaluationDiagnosticLevel::Warning,
            text: CompactString::new(format!(
                "target pattern `{}` is ambiguous; using explicit target {}",
                self.raw_pattern, self.conflict_target
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleRegistrationExpansionErrorKind {
    Selected(HostSelectedRegistrationPatternsError),
    Parse(Arc<str>),
    RowOverflow,
    CanonicalRoute(HostCanonicalRepositoryLoadRouteError),
    RootSubtree(RootSubtreePackageSetError),
    CanonicalSubtree(ExternalSubtreePackageSetError),
    RootPackage(RootPackageLoadError),
    CanonicalPackage(RepositoryPackageLoadError),
    MissingTarget(CanonicalLabel),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleRegistrationExpansionError {
    family: ModuleRegistrationFamily,
    row: Option<u32>,
    kind: ModuleRegistrationExpansionErrorKind,
}

impl ModuleRegistrationExpansionError {
    pub fn family(&self) -> ModuleRegistrationFamily {
        self.family
    }

    pub fn row(&self) -> Option<u32> {
        self.row
    }

    pub fn kind(&self) -> &ModuleRegistrationExpansionErrorKind {
        &self.kind
    }
}

impl fmt::Display for ModuleRegistrationExpansionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} registration", self.family)?;
        if let Some(row) = self.row {
            write!(f, " row {row}")?;
        }
        write!(f, ": {:?}", self.kind)
    }
}

impl std::error::Error for ModuleRegistrationExpansionError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleRegistrationExpansion {
    labels: Result<Arc<[CanonicalLabel]>, ModuleRegistrationExpansionError>,
    ambiguities: Arc<[ModuleRegistrationAmbiguity]>,
}

impl ModuleRegistrationExpansion {
    pub fn labels(&self) -> Result<&Arc<[CanonicalLabel]>, &ModuleRegistrationExpansionError> {
        self.labels.as_ref()
    }

    pub fn ambiguities(&self) -> &Arc<[ModuleRegistrationAmbiguity]> {
        &self.ambiguities
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum ModuleRegistrationExpansionObservationError {
    Selected(HostSelectedRegistrationPatternsObservationError),
    CanonicalRoute(HostCanonicalRepositoryLoadRouteObservationError),
    Frontier(ObservedPathFrontierError),
}

impl fmt::Display for ModuleRegistrationExpansionObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ModuleRegistrationExpansionObservationError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleRegistrationExpansionKey {
    workspace: NormalizedAbsolutePath,
    family: ModuleRegistrationFamily,
}

impl ModuleRegistrationExpansionKey {
    pub fn toolchains(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            family: ModuleRegistrationFamily::Toolchains,
        }
    }

    pub fn execution_platforms(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            family: ModuleRegistrationFamily::ExecutionPlatforms,
        }
    }

    pub fn family(&self) -> ModuleRegistrationFamily {
        self.family
    }
}

impl fmt::Display for ModuleRegistrationExpansionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "module-registration-expansion:{}:{}",
            self.workspace, self.family
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleRegistrationExpansionObservationKey(ModuleRegistrationExpansionKey);

impl ModuleRegistrationExpansionObservationKey {
    pub fn toolchains(workspace: NormalizedAbsolutePath) -> Self {
        Self(ModuleRegistrationExpansionKey::toolchains(workspace))
    }

    pub fn execution_platforms(workspace: NormalizedAbsolutePath) -> Self {
        Self(ModuleRegistrationExpansionKey::execution_platforms(
            workspace,
        ))
    }

    pub fn family(&self) -> ModuleRegistrationFamily {
        self.0.family
    }
}

impl fmt::Display for ModuleRegistrationExpansionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedModuleRegistrationExpansion {
    result: Arc<ModuleRegistrationExpansion>,
    observations: PathObservationEpoch,
}

impl ObservedModuleRegistrationExpansion {
    pub fn result(&self) -> &Arc<ModuleRegistrationExpansion> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationMode {
    Legacy,
    Observed,
}

enum DriverBreak {
    Need(SourcePreparationNeeds),
    Semantic(ModuleRegistrationExpansionError),
    Outer(ModuleRegistrationExpansionObservationError),
}

type Step<T> = Result<T, DriverBreak>;
type DriverOutcome = SourcePreparationOutcome<
    Result<
        (Arc<ModuleRegistrationExpansion>, PathObservationEpoch),
        ModuleRegistrationExpansionObservationError,
    >,
>;
type SelectedCarrier =
    Arc<Result<HostSelectedRegistrationPatterns, HostSelectedRegistrationPatternsError>>;

#[derive(Clone)]
enum PackageCarrier {
    Root(Arc<Result<LoadedPackage, RootPackageLoadError>>),
    Canonical(Arc<Result<LoadedPackage, RepositoryPackageLoadError>>),
}

impl PackageCarrier {
    fn loaded(&self) -> &LoadedPackage {
        match self {
            Self::Root(value) => value.as_ref().as_ref().unwrap(),
            Self::Canonical(value) => value.as_ref().as_ref().unwrap(),
        }
    }
}

struct ExpansionScratch {
    workspace: NormalizedAbsolutePath,
    family: ModuleRegistrationFamily,
    mode: ObservationMode,
    labels: Vec<CanonicalLabel>,
    seen: SmallSet<CanonicalLabel>,
    ambiguities: Vec<ModuleRegistrationAmbiguity>,
    routes: SmallMap<CanonicalRepoName, HostCanonicalRepositorySourceInput>,
    packages: SmallMap<PackageIdentifier, PackageCarrier>,
    subtrees: SmallMap<PackageIdentifier, Arc<[CompactString]>>,
    observations: PathObservationEpoch,
}

impl ExpansionScratch {
    fn new(key: &ModuleRegistrationExpansionKey, mode: ObservationMode) -> Self {
        Self {
            workspace: key.workspace.dupe(),
            family: key.family,
            mode,
            labels: Vec::new(),
            seen: SmallSet::new(),
            ambiguities: Vec::new(),
            routes: SmallMap::new(),
            packages: SmallMap::new(),
            subtrees: SmallMap::new(),
            observations: PathObservationEpoch::empty(),
        }
    }

    fn error(&self, row: Option<u32>, kind: ModuleRegistrationExpansionErrorKind) -> DriverBreak {
        DriverBreak::Semantic(ModuleRegistrationExpansionError {
            family: self.family,
            row,
            kind,
        })
    }

    fn merge(&mut self, incoming: &PathObservationEpoch) -> Step<()> {
        self.observations = PathObservationEpoch::from_shared(
            self.observations
                .observations()
                .iter()
                .chain(incoming.observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .map_err(|error| {
            DriverBreak::Outer(ModuleRegistrationExpansionObservationError::Frontier(
                error.into(),
            ))
        })?;
        Ok(())
    }

    fn append(&mut self, label: CanonicalLabel) {
        if self.seen.insert(label.clone()) {
            self.labels.push(label);
        }
    }

    fn finish(self, error: Option<ModuleRegistrationExpansionError>) -> DriverOutcome {
        let labels = match error {
            Some(error) => Err(error),
            None => Ok(self.labels.into()),
        };
        SourcePreparationOutcome::Complete(Ok((
            Arc::new(ModuleRegistrationExpansion {
                labels,
                ambiguities: self.ambiguities.into(),
            }),
            self.observations,
        )))
    }
}

async fn selected_patterns(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
) -> Step<SelectedCarrier> {
    match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&HostSelectedRegistrationPatternsKey::new(
                scratch.workspace.dupe(),
            ))
            .await
            .expect("selected registration patterns DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(_) => Ok(value),
                Err(error) => Err(scratch.error(
                    None,
                    ModuleRegistrationExpansionErrorKind::Selected(error.clone()),
                )),
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&HostSelectedRegistrationPatternsObservationKey::new(
                scratch.workspace.dupe(),
            ))
            .await
            .expect("observed selected registration patterns DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => Err(DriverBreak::Outer(
                ModuleRegistrationExpansionObservationError::Selected(error),
            )),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(_) => Ok(observed.result().dupe()),
                    Err(error) => Err(scratch.error(
                        None,
                        ModuleRegistrationExpansionErrorKind::Selected(error.clone()),
                    )),
                }
            }
        },
    }
}

async fn canonical_input(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    repo: &CanonicalRepoName,
    row: u32,
) -> Step<HostCanonicalRepositorySourceInput> {
    if let Some(input) = scratch.routes.get(repo) {
        return Ok(input.clone());
    }
    let input = match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                scratch.workspace.dupe(),
                repo.clone(),
            ))
            .await
            .expect("canonical registration route DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => return Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(route) => route.input().clone(),
                Err(error) => {
                    return Err(scratch.error(
                        Some(row),
                        ModuleRegistrationExpansionErrorKind::CanonicalRoute(error.clone()),
                    ));
                }
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                scratch.workspace.dupe(),
                repo.clone(),
            ))
            .await
            .expect("observed canonical registration route DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => return Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return Err(DriverBreak::Outer(
                    ModuleRegistrationExpansionObservationError::CanonicalRoute(error),
                ));
            }
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(route) => route.input().clone(),
                    Err(error) => {
                        return Err(scratch.error(
                            Some(row),
                            ModuleRegistrationExpansionErrorKind::CanonicalRoute(error.clone()),
                        ));
                    }
                }
            }
        },
    };
    scratch.routes.insert(repo.clone(), input.clone());
    Ok(input)
}

async fn root_package(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<PackageCarrier> {
    match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&RootPackageLoadKey::new(
                scratch.workspace.dupe(),
                package.package().clone(),
            ))
            .await
            .expect("root registration package DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(_) => Ok(PackageCarrier::Root(value)),
                Err(error) => Err(scratch.error(
                    Some(row),
                    ModuleRegistrationExpansionErrorKind::RootPackage(error.clone()),
                )),
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&RootPackageLoadObservationKey::new(
                scratch.workspace.dupe(),
                package.package().clone(),
            ))
            .await
            .expect("observed root registration package DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => Err(DriverBreak::Outer(
                ModuleRegistrationExpansionObservationError::Frontier(error),
            )),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(_) => Ok(PackageCarrier::Root(observed.result().dupe())),
                    Err(error) => Err(scratch.error(
                        Some(row),
                        ModuleRegistrationExpansionErrorKind::RootPackage(error.clone()),
                    )),
                }
            }
        },
    }
}

async fn canonical_package(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<PackageCarrier> {
    let input = canonical_input(ctx, scratch, package.repo(), row).await?;
    let route = HostRepositorySourceRoute::canonical(input);
    match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&RepositoryPackageInventoryKey::new(
                route,
                package.package().clone(),
            ))
            .await
            .expect("canonical registration package inventory DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(_) => Ok(PackageCarrier::Canonical(value)),
                Err(error) => Err(scratch.error(
                    Some(row),
                    ModuleRegistrationExpansionErrorKind::CanonicalPackage(error.clone()),
                )),
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&RepositoryPackageInventoryObservationKey::new(
                route,
                package.package().clone(),
            ))
            .await
            .expect("observed canonical registration package inventory DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => Err(DriverBreak::Outer(
                ModuleRegistrationExpansionObservationError::Frontier(error),
            )),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(_) => Ok(PackageCarrier::Canonical(observed.result().dupe())),
                    Err(error) => Err(scratch.error(
                        Some(row),
                        ModuleRegistrationExpansionErrorKind::CanonicalPackage(error.clone()),
                    )),
                }
            }
        },
    }
}

async fn loaded_package(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<PackageCarrier> {
    if let Some(value) = scratch.packages.get(package) {
        return Ok(value.clone());
    }
    let value = if package.repo().is_root() {
        root_package(ctx, scratch, package, row).await?
    } else {
        canonical_package(ctx, scratch, package, row).await?
    };
    scratch.packages.insert(package.clone(), value.clone());
    Ok(value)
}

async fn root_subtree(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<Arc<[CompactString]>> {
    match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&RootSubtreePackageSetKey::new(
                scratch.workspace.dupe(),
                package.package().clone(),
            ))
            .await
            .expect("root registration subtree DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(subtree) => Ok(subtree.packages().dupe()),
                Err(error) => Err(scratch.error(
                    Some(row),
                    ModuleRegistrationExpansionErrorKind::RootSubtree(error.clone()),
                )),
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&RootSubtreePackageSetObservationKey::new(
                scratch.workspace.dupe(),
                package.package().clone(),
            ))
            .await
            .expect("observed root registration subtree DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => Err(DriverBreak::Outer(
                ModuleRegistrationExpansionObservationError::Frontier(error),
            )),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(subtree) => Ok(subtree.packages().dupe()),
                    Err(error) => Err(scratch.error(
                        Some(row),
                        ModuleRegistrationExpansionErrorKind::RootSubtree(error.clone()),
                    )),
                }
            }
        },
    }
}

async fn canonical_subtree(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<Arc<[CompactString]>> {
    let input = canonical_input(ctx, scratch, package.repo(), row).await?;
    match scratch.mode {
        ObservationMode::Legacy => match ctx
            .compute(&ExternalSubtreePackageSetKey::new_canonical(
                input,
                package.package().clone(),
            ))
            .await
            .expect("canonical registration subtree DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(subtree) => Ok(subtree.packages().dupe()),
                Err(error) => Err(scratch.error(
                    Some(row),
                    ModuleRegistrationExpansionErrorKind::CanonicalSubtree(error.clone()),
                )),
            },
        },
        ObservationMode::Observed => match ctx
            .compute(&ExternalSubtreePackageSetObservationKey::new_canonical(
                input,
                package.package().clone(),
            ))
            .await
            .expect("observed canonical registration subtree DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => Err(DriverBreak::Need(need)),
            SourcePreparationOutcome::Complete(Err(error)) => Err(DriverBreak::Outer(
                ModuleRegistrationExpansionObservationError::Frontier(error),
            )),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                scratch.merge(observed.observations())?;
                match observed.result().as_ref() {
                    Ok(subtree) => Ok(subtree.packages().dupe()),
                    Err(error) => Err(scratch.error(
                        Some(row),
                        ModuleRegistrationExpansionErrorKind::CanonicalSubtree(error.clone()),
                    )),
                }
            }
        },
    }
}

async fn subtree_packages(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: &PackageIdentifier,
    row: u32,
) -> Step<Arc<[CompactString]>> {
    if let Some(value) = scratch.subtrees.get(package) {
        return Ok(value.dupe());
    }
    let value = if package.repo().is_root() {
        root_subtree(ctx, scratch, package, row).await?
    } else {
        canonical_subtree(ctx, scratch, package, row).await?
    };
    scratch.subtrees.insert(package.clone(), value.dupe());
    Ok(value)
}

fn family_candidate(family: ModuleRegistrationFamily, target: &PackageTarget) -> bool {
    match (family, &target.kind) {
        (
            ModuleRegistrationFamily::Toolchains,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain { .. }),
        )
        | (
            ModuleRegistrationFamily::ExecutionPlatforms,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform { .. })
            | PackageTargetKind::Alias { .. },
        ) => true,
        _ => false,
    }
}

fn label_in_package(package: &PackageIdentifier, target: &str) -> CanonicalLabel {
    CanonicalLabel::parse(&format!("{package}:{target}"))
        .expect("a loaded target has valid canonical label components")
}

pub(crate) fn package_postorder(left: &PackagePath, right: &PackagePath) -> Ordering {
    let mut left = left
        .as_str()
        .split('/')
        .filter(|component| !component.is_empty());
    let mut right = right
        .as_str()
        .split('/')
        .filter(|component| !component.is_empty());
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) => match left.cmp(right) {
                Ordering::Equal => {}
                ordering => return ordering,
            },
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

async fn expand_wildcard(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    package: PackageIdentifier,
    conflict: Option<CanonicalLabel>,
    raw: &CompactString,
    wildcard: TargetPatternWildcard,
    row: u32,
) -> Step<()> {
    let carrier = loaded_package(ctx, scratch, &package, row).await?;
    if let Some(conflict) = conflict {
        if carrier
            .loaded()
            .targets
            .iter()
            .any(|target| target.name == conflict.target().as_str())
        {
            scratch.append(conflict.clone());
            scratch.ambiguities.push(ModuleRegistrationAmbiguity {
                family: scratch.family,
                row,
                raw_pattern: raw.clone(),
                wildcard: CompactString::new(wildcard.as_str()),
                conflict_target: conflict,
            });
            return Ok(());
        }
    }
    let mut candidates = carrier
        .loaded()
        .targets
        .iter()
        .filter(|target| family_candidate(scratch.family, target))
        .collect::<Vec<_>>();
    candidates.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    for target in candidates {
        scratch.append(label_in_package(&package, &target.name));
    }
    Ok(())
}

async fn expand_pattern(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    pattern: CanonicalTargetPattern,
    raw: &CompactString,
    row: u32,
) -> Step<()> {
    match pattern {
        CanonicalTargetPattern::Single(label) => {
            let carrier = loaded_package(ctx, scratch, label.package(), row).await?;
            if !carrier
                .loaded()
                .targets
                .iter()
                .any(|target| target.name == label.target().as_str())
            {
                return Err(scratch.error(
                    Some(row),
                    ModuleRegistrationExpansionErrorKind::MissingTarget(label),
                ));
            }
            scratch.append(label);
        }
        CanonicalTargetPattern::PackageWildcard {
            package,
            wildcard,
            conflict_target,
        } => {
            expand_wildcard(ctx, scratch, package, conflict_target, raw, wildcard, row).await?;
        }
        CanonicalTargetPattern::Recursive { package, wildcard } => {
            let subtree = subtree_packages(ctx, scratch, &package, row).await?;
            let mut packages = subtree
                .iter()
                .map(|value| PackagePath::parse(value).expect("subtree owns valid package paths"))
                .collect::<Vec<_>>();
            packages.sort_unstable_by(package_postorder);
            let wildcard = wildcard.unwrap_or(TargetPatternWildcard::All);
            for path in packages {
                expand_wildcard(
                    ctx,
                    scratch,
                    PackageIdentifier::new(package.repo().clone(), path),
                    None,
                    raw,
                    wildcard,
                    row,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn expand_rows<'a>(
    ctx: &mut DiceComputations<'_>,
    scratch: &mut ExpansionScratch,
    rows: impl Iterator<Item = HostSelectedRegistrationPatternView<'a>>,
) -> Step<()> {
    for (ordinal, view) in rows.enumerate() {
        let row = u32::try_from(ordinal)
            .map_err(|_| scratch.error(None, ModuleRegistrationExpansionErrorKind::RowOverflow))?;
        let raw = CompactString::new(view.raw_pattern());
        let pattern = CanonicalTargetPattern::parse(&raw, view.canonical_repo(), |apparent| {
            view.mapping_target(apparent)
        })
        .map_err(|error| {
            scratch.error(
                Some(row),
                ModuleRegistrationExpansionErrorKind::Parse(error.into()),
            )
        })?;
        expand_pattern(ctx, scratch, pattern, &raw, row).await?;
    }
    Ok(())
}

async fn drive_registration_expansion(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleRegistrationExpansionKey,
    mode: ObservationMode,
) -> DriverOutcome {
    let mut scratch = ExpansionScratch::new(key, mode);
    let result = async {
        let selected = selected_patterns(ctx, &mut scratch).await?;
        let selected = selected.as_ref().as_ref().unwrap();
        match key.family {
            ModuleRegistrationFamily::Toolchains => {
                expand_rows(ctx, &mut scratch, selected.toolchains()).await
            }
            ModuleRegistrationFamily::ExecutionPlatforms => {
                expand_rows(ctx, &mut scratch, selected.execution_platforms()).await
            }
        }
    }
    .await;
    match result {
        Ok(()) => scratch.finish(None),
        Err(DriverBreak::Need(need)) => SourcePreparationOutcome::Need(need),
        Err(DriverBreak::Outer(error)) => SourcePreparationOutcome::Complete(Err(error)),
        Err(DriverBreak::Semantic(error)) => scratch.finish(Some(error)),
    }
}

fn event_batch(value: &ModuleRegistrationExpansion) -> EventBatch {
    EventBatch::from_events(
        value
            .ambiguities
            .iter()
            .map(ModuleRegistrationAmbiguity::event),
    )
}

#[async_trait]
impl Key for ModuleRegistrationExpansionKey {
    type Value = SourcePreparationOutcome<Arc<ModuleRegistrationExpansion>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = drive_registration_expansion(ctx, self, ObservationMode::Legacy).await;
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy module registration expansion produced outer error: {error}")
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                if ctx
                    .per_transaction_data()
                    .data
                    .get::<CaptureEvaluationEvents>()
                    .is_ok()
                {
                    ctx.store_evaluation_data(event_batch(&result))
                        .expect("registration expansion stores one local Complete event batch");
                }
                SourcePreparationOutcome::Complete(result)
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
impl Key for ModuleRegistrationExpansionObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedModuleRegistrationExpansion, ModuleRegistrationExpansionObservationError>,
    >;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match drive_registration_expansion(ctx, &self.0, ObservationMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                if ctx
                    .per_transaction_data()
                    .data
                    .get::<CaptureEvaluationEvents>()
                    .is_ok()
                {
                    ctx.store_evaluation_data(event_batch(&result)).expect(
                        "observed registration expansion stores one local Complete event batch",
                    );
                }
                SourcePreparationOutcome::Complete(Ok(ObservedModuleRegistrationExpansion {
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
