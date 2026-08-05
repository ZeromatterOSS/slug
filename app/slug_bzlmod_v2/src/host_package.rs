/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host root-module packets.

#[cfg(unix)]
use std::ffi::OsString;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;

use crate::RootPackageLookupInputsProjectionKey;
use crate::RootPackagePolicyProjectionError;
use crate::RootRepositoryRoute;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileError;
use crate::package_policy::CanonicalDeletedPackagesProjectionKey;
use crate::repository_ignore::HostRepositoryIgnoreError;
use crate::repository_ignore::HostRepositoryIgnoreKey;
use crate::repository_ignore::HostRouteRepositoryIgnoreKey;
use crate::source_preparation::DirectLocalModuleSupport;
use crate::source_preparation::DirectLocalModuleSupportError;
use crate::source_preparation::DirectLocalUnsupportedCycle;
use crate::source_preparation::HostRepositoryPathKey;
use crate::source_preparation::HostRepositorySourceFileKey;
use crate::source_preparation::HostRepositorySourceFileValue;
use crate::source_preparation::RepositorySourceFileError;
use crate::source_preparation::SourcePreparationNeeds;
use crate::source_preparation::SourcePreparationOutcome;
use crate::source_preparation::direct_local_module_support;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostBuildFileName {
    BuildDotBazel,
    Build,
}

impl HostBuildFileName {
    fn as_str(self) -> &'static str {
        match self {
            Self::BuildDotBazel => "BUILD.bazel",
            Self::Build => "BUILD",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPackage {
    /// The selected package-path entry. Consumers append package and target.
    package_root: NormalizedAbsolutePath,
    build_file_name: HostBuildFileName,
}

impl HostPackage {
    pub(crate) fn package_root(&self) -> &NormalizedAbsolutePath {
        &self.package_root
    }

    pub(crate) fn build_file_name(&self) -> HostBuildFileName {
        self.build_file_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostRootPackageLookup {
    Package(HostPackage),
    NoBuildFile,
    Deleted,
    InvalidPackageName { message: Arc<str> },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootPackageLookupError {
    PolicyInput(RootPackagePolicyProjectionError),
    RepositoryIgnore(HostRepositoryIgnoreError),
    Resolution {
        logical_path: NormalizedAbsolutePath,
        error: PathResolutionError,
    },
}

impl fmt::Display for HostRootPackageLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyInput(error) => error.fmt(f),
            Self::RepositoryIgnore(error) => error.fmt(f),
            Self::Resolution {
                logical_path,
                error,
            } => write!(
                f,
                "failed to resolve package marker {}: {error:?}",
                logical_path.as_path().display()
            ),
        }
    }
}

impl std::error::Error for HostRootPackageLookupError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRootPackageLookupKey {
    workspace: NormalizedAbsolutePath,
    package: PackagePath,
}

impl HostRootPackageLookupKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

impl fmt::Display for HostRootPackageLookupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-root-package-lookup:{}//{}",
            self.workspace, self.package
        )
    }
}

fn invalid_package_name(package: &PackagePath) -> Option<Arc<str>> {
    let value = package.as_str();
    if !value
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b':' | b'\\'))
    {
        let reason = r##"package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##;
        return Some(Arc::from(format!(
            "Invalid package name '{value}': {reason}"
        )));
    }
    if value
        .split('/')
        .any(|component| !component.is_empty() && component.bytes().all(|byte| byte == b'.'))
    {
        return Some(Arc::from(format!(
            "Invalid package name '{value}': package name component contains only '.' characters"
        )));
    }
    None
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host package-lookup DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for HostRootPackageLookupKey {
    type Value = PathOutcome<Arc<Result<HostRootPackageLookup, HostRootPackageLookupError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let inputs = match dice_invariant(
            ctx.compute(&RootPackageLookupInputsProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(inputs) => inputs,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    HostRootPackageLookupError::PolicyInput(error),
                )));
            }
        };

        if let Some(message) = invalid_package_name(&self.package) {
            return PathOutcome::Complete(Arc::new(Ok(
                HostRootPackageLookup::InvalidPackageName { message },
            )));
        }

        let package_id = PackageIdentifier::new(CanonicalRepoName::root(), self.package.clone());
        if inputs.deleted_packages().contains(&package_id) {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::Deleted)));
        }
        if self.package.as_str() == "external" {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::NoBuildFile)));
        }

        let repository_ignore = match dice_invariant(
            ctx.compute(&HostRepositoryIgnoreKey::new(self.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return PathOutcome::Complete(Arc::new(Err(
                        HostRootPackageLookupError::RepositoryIgnore(error.clone()),
                    )));
                }
            },
        };
        if repository_ignore.matching_entry(&self.package).is_some() {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::Deleted)));
        }

        for root in inputs.package_roots() {
            for build_file_name in [HostBuildFileName::BuildDotBazel, HostBuildFileName::Build] {
                let logical_path = NormalizedAbsolutePath::new(
                    root.as_path()
                        .join(self.package.as_str())
                        .join(build_file_name.as_str()),
                )
                .expect("joining package and marker to a normalized root remains absolute");
                let resolved = match dice_invariant(
                    ctx.compute(&ResolvedPathKey::new(
                        PathObservationNamespace::Host,
                        logical_path.dupe(),
                    ))
                    .await,
                ) {
                    PathOutcome::Need(need) => return PathOutcome::Need(need),
                    PathOutcome::Complete(Ok(resolved)) => resolved,
                    PathOutcome::Complete(Err(error)) => {
                        return PathOutcome::Complete(Arc::new(Err(
                            HostRootPackageLookupError::Resolution {
                                logical_path,
                                error,
                            },
                        )));
                    }
                };
                match resolved.state() {
                    ResolvedPathState::Present(lstat)
                        if matches!(
                            lstat.kind(),
                            PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                        ) =>
                    {
                        return PathOutcome::Complete(Arc::new(Ok(
                            HostRootPackageLookup::Package(HostPackage {
                                package_root: root.dupe(),
                                build_file_name,
                            }),
                        )));
                    }
                    ResolvedPathState::Present(lstat) if lstat.kind() == PathNodeKind::Symlink => {
                        unreachable!("ResolvedPathKey returns the terminal symlink kind")
                    }
                    ResolvedPathState::Present(_) | ResolvedPathState::Missing => {}
                }
            }
        }

        PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::NoBuildFile)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum ExternalRepositoryPackageLookup {
    Package(HostBuildFileName),
    NoBuildFile,
    Deleted,
    InvalidPackageName { message: Arc<str> },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum ExternalRepositoryPackageLookupError {
    PolicyInput(RootPackagePolicyProjectionError),
    RepositoryIgnore(HostRepositoryIgnoreError),
    Path(RepositorySourceFileError),
}

impl fmt::Display for ExternalRepositoryPackageLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyInput(error) => error.fmt(f),
            Self::RepositoryIgnore(error) => error.fmt(f),
            Self::Path(error) => write!(f, "failed to inspect routed package marker: {error:?}"),
        }
    }
}

impl std::error::Error for ExternalRepositoryPackageLookupError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct ExternalRepositoryPackageLookupKey {
    route: RootRepositoryRoute,
    package: PackageIdentifier,
}

impl ExternalRepositoryPackageLookupKey {
    pub(crate) fn new(route: RootRepositoryRoute, package: PackageIdentifier) -> Option<Self> {
        (package.repo() == route.canonical_repo()).then_some(Self { route, package })
    }
}

impl Hash for ExternalRepositoryPackageLookupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.package.hash(state);
    }
}

impl fmt::Display for ExternalRepositoryPackageLookupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external-repository-package-lookup:{:?}", self.package)
    }
}

#[async_trait]
impl Key for ExternalRepositoryPackageLookupKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<ExternalRepositoryPackageLookup, ExternalRepositoryPackageLookupError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        if let Some(message) = invalid_package_name(self.package.package()) {
            return SourcePreparationOutcome::Complete(Arc::new(Ok(
                ExternalRepositoryPackageLookup::InvalidPackageName { message },
            )));
        }
        let deleted = match dice_invariant(
            ctx.compute(&CanonicalDeletedPackagesProjectionKey::new(
                self.route.workspace().dupe(),
            ))
            .await,
        ) {
            Ok(deleted) => deleted,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    ExternalRepositoryPackageLookupError::PolicyInput(error),
                )));
            }
        };
        if deleted.contains(&self.package) {
            return SourcePreparationOutcome::Complete(Arc::new(Ok(
                ExternalRepositoryPackageLookup::Deleted,
            )));
        }
        let repository_ignore = match dice_invariant(
            ctx.compute(&HostRouteRepositoryIgnoreKey::new(self.route.clone()))
                .await,
        ) {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        ExternalRepositoryPackageLookupError::RepositoryIgnore(error.clone()),
                    )));
                }
            },
        };
        if repository_ignore
            .matching_entry(self.package.package())
            .is_some()
        {
            return SourcePreparationOutcome::Complete(Arc::new(Ok(
                ExternalRepositoryPackageLookup::Deleted,
            )));
        }

        for build_file_name in [HostBuildFileName::BuildDotBazel, HostBuildFileName::Build] {
            let marker =
                PathBuf::from(self.package.package().as_str()).join(build_file_name.as_str());
            let path = match dice_invariant(
                ctx.compute(&HostRepositoryPathKey::new(self.route.clone(), marker))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        ExternalRepositoryPackageLookupError::Path(error),
                    )));
                }
                SourcePreparationOutcome::Complete(Ok(path)) => path,
            };
            match path.resolved().state() {
                ResolvedPathState::Present(lstat)
                    if matches!(
                        lstat.kind(),
                        PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                    ) =>
                {
                    return SourcePreparationOutcome::Complete(Arc::new(Ok(
                        ExternalRepositoryPackageLookup::Package(build_file_name),
                    )));
                }
                ResolvedPathState::Missing | ResolvedPathState::Present(_) => {}
            }
        }
        SourcePreparationOutcome::Complete(Arc::new(Ok(
            ExternalRepositoryPackageLookup::NoBuildFile,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

/// DICE identity for the selected BUILD source of one routed external package.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryPackageSourceKey {
    route: RootRepositoryRoute,
    package: PackageIdentifier,
}

impl RepositoryPackageSourceKey {
    pub fn new(route: RootRepositoryRoute, package: PackageIdentifier) -> Option<Self> {
        (package.repo() == route.canonical_repo()).then_some(Self { route, package })
    }
}

impl Hash for RepositoryPackageSourceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.package.hash(state);
    }
}

impl fmt::Display for RepositoryPackageSourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-package-source:{}", self.package)
    }
}

/// The selected BUILD identity and bytes required by external package loading.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryPackageSource {
    logical_path: NormalizedAbsolutePath,
    build_file_name: HostBuildFileName,
    bytes: Arc<[u8]>,
}

impl RepositoryPackageSource {
    pub fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }

    pub fn build_file_name(&self) -> &'static str {
        self.build_file_name.as_str()
    }

    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RepositoryPackageSourceErrorInner {
    ModuleEvaluation {
        error: DirectLocalModuleSupportError,
    },
    Unsupported {
        cycle: DirectLocalUnsupportedCycle,
    },
    InvalidPackageName {
        package: PackageIdentifier,
        message: Arc<str>,
    },
    Deleted {
        package: PackageIdentifier,
    },
    NoBuildFile {
        package: PackageIdentifier,
    },
    Lookup {
        package: PackageIdentifier,
        error: ExternalRepositoryPackageLookupError,
    },
    LookupCompute {
        package: PackageIdentifier,
        message: Arc<str>,
    },
    Source {
        logical_path: Arc<PathBuf>,
        error: RepositorySourceFileError,
    },
    SourceCompute {
        logical_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    SelectedSourceAbsent {
        logical_path: Arc<PathBuf>,
    },
}

/// Opaque typed failure while selecting or reading an external BUILD source.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryPackageSourceError {
    inner: RepositoryPackageSourceErrorInner,
}

impl RepositoryPackageSourceError {
    fn new(inner: RepositoryPackageSourceErrorInner) -> Self {
        Self { inner }
    }

    pub fn is_unsupported_feature(&self) -> bool {
        matches!(
            self.inner,
            RepositoryPackageSourceErrorInner::Unsupported { .. }
        )
    }
}

impl fmt::Display for RepositoryPackageSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            RepositoryPackageSourceErrorInner::ModuleEvaluation { error } => error.fmt(f),
            RepositoryPackageSourceErrorInner::Unsupported { cycle } => cycle.fmt(f),
            RepositoryPackageSourceErrorInner::InvalidPackageName { package, message } => {
                write!(f, "invalid external package {package}: {message}")
            }
            RepositoryPackageSourceErrorInner::Deleted { package } => {
                write!(f, "external package {package} is deleted")
            }
            RepositoryPackageSourceErrorInner::NoBuildFile { package } => write!(
                f,
                "no such package '{package}': BUILD file not found in directory '{}' of external repository {}. Add a BUILD file to a directory to mark it as a package.",
                package.package(),
                package.repo()
            ),
            RepositoryPackageSourceErrorInner::Lookup { package, error } => {
                write!(f, "selecting BUILD source for {package}: {error}")
            }
            RepositoryPackageSourceErrorInner::LookupCompute { package, message } => {
                write!(f, "computing BUILD selection for {package}: {message}")
            }
            RepositoryPackageSourceErrorInner::Source {
                logical_path,
                error,
            } => write!(
                f,
                "reading selected BUILD source {}: {error:?}",
                logical_path.display()
            ),
            RepositoryPackageSourceErrorInner::SourceCompute {
                logical_path,
                message,
            } => write!(
                f,
                "computing selected BUILD source {}: {message}",
                logical_path.display()
            ),
            RepositoryPackageSourceErrorInner::SelectedSourceAbsent { logical_path } => write!(
                f,
                "selected BUILD source {} became absent",
                logical_path.display()
            ),
        }
    }
}

impl std::error::Error for RepositoryPackageSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            RepositoryPackageSourceErrorInner::ModuleEvaluation { error } => Some(error),
            RepositoryPackageSourceErrorInner::Lookup { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[async_trait]
impl Key for RepositoryPackageSourceKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RepositoryPackageSource, RepositoryPackageSourceError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match direct_local_module_support(ctx, &self.route).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(DirectLocalModuleSupport::Supported) => {}
                Ok(DirectLocalModuleSupport::Unsupported(cycle)) => {
                    return repository_package_source_complete(Err(
                        RepositoryPackageSourceError::new(
                            RepositoryPackageSourceErrorInner::Unsupported {
                                cycle: cycle.clone(),
                            },
                        ),
                    ));
                }
                Err(error) => {
                    return repository_package_source_complete(Err(
                        RepositoryPackageSourceError::new(
                            RepositoryPackageSourceErrorInner::ModuleEvaluation {
                                error: error.clone(),
                            },
                        ),
                    ));
                }
            },
        }
        let lookup = match ctx
            .compute(
                &ExternalRepositoryPackageLookupKey::new(self.route.clone(), self.package.clone())
                    .expect("public source key enforces route/package identity"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => value,
            Err(error) => {
                return repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::LookupCompute {
                        package: self.package.clone(),
                        message: Arc::from(error.to_string()),
                    },
                )));
            }
        };
        let build_file_name = match lookup.as_ref() {
            Ok(ExternalRepositoryPackageLookup::Package(name)) => *name,
            Ok(ExternalRepositoryPackageLookup::InvalidPackageName { message }) => {
                return repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::InvalidPackageName {
                        package: self.package.clone(),
                        message: message.clone(),
                    },
                )));
            }
            Ok(ExternalRepositoryPackageLookup::Deleted) => {
                return repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::Deleted {
                        package: self.package.clone(),
                    },
                )));
            }
            Ok(ExternalRepositoryPackageLookup::NoBuildFile) => {
                return repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::NoBuildFile {
                        package: self.package.clone(),
                    },
                )));
            }
            Err(error) => {
                return repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::Lookup {
                        package: self.package.clone(),
                        error: error.clone(),
                    },
                )));
            }
        };
        let logical_path =
            Arc::new(PathBuf::from(self.package.package().as_str()).join(build_file_name.as_str()));
        match ctx
            .compute(&HostRepositorySourceFileKey::new(
                self.route.clone(),
                logical_path.as_ref().clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Ok(
                HostRepositorySourceFileValue::Present {
                    bytes,
                    logical_path,
                },
            ))) => repository_package_source_complete(Ok(RepositoryPackageSource {
                logical_path,
                build_file_name,
                bytes,
            })),
            Ok(SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Absent))) => {
                repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::SelectedSourceAbsent { logical_path },
                )))
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::Source {
                        logical_path,
                        error,
                    },
                )))
            }
            Err(error) => {
                repository_package_source_complete(Err(RepositoryPackageSourceError::new(
                    RepositoryPackageSourceErrorInner::SourceCompute {
                        logical_path,
                        message: Arc::from(error.to_string()),
                    },
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

fn repository_package_source_complete(
    value: Result<RepositoryPackageSource, RepositoryPackageSourceError>,
) -> SourcePreparationOutcome<Arc<Result<RepositoryPackageSource, RepositoryPackageSourceError>>> {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

/// A validated root-repository `.bzl` target in Bazel's internal byte shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe)]
pub struct RootPackageBzlTarget {
    raw: Arc<[u8]>,
}

impl RootPackageBzlTarget {
    pub fn parse(value: &str) -> Result<Self, RootPackageBzlTargetError> {
        let target = TargetName::parse(value).map_err(|message| {
            RootPackageBzlTargetError::InvalidTarget {
                target: Arc::from(value),
                message: Arc::from(message),
            }
        })?;
        if target.as_str() != value {
            return Err(RootPackageBzlTargetError::InvalidTarget {
                target: Arc::from(value),
                message: Arc::from("target path must use its canonical spelling"),
            });
        }
        if !value.ends_with(".bzl") {
            return Err(RootPackageBzlTargetError::InvalidTarget {
                target: Arc::from(value),
                message: Arc::from("load target must end with `.bzl`"),
            });
        }
        let mut raw = Vec::with_capacity(value.len());
        for scalar in value.chars() {
            let byte = u8::try_from(u32::from(scalar)).map_err(|_| {
                RootPackageBzlTargetError::NonLatin1Scalar {
                    target: Arc::from(value),
                    scalar: u32::from(scalar),
                }
            })?;
            raw.push(byte);
        }
        if raw.is_empty()
            || raw.first() == Some(&b'/')
            || raw.last() == Some(&b'/')
            || raw
                .split(|byte| *byte == b'/')
                .any(|component| component.is_empty() || matches!(component, b"." | b".."))
            || raw
                .iter()
                .any(|byte| *byte < b' ' || *byte == b'\x7f' || matches!(byte, b':' | b'\\'))
        {
            return Err(RootPackageBzlTargetError::InvalidTarget {
                target: Arc::from(value),
                message: Arc::from("target is not a normalized relative `.bzl` path"),
            });
        }
        Ok(Self { raw: raw.into() })
    }

    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    fn internal_string(&self) -> String {
        self.raw.iter().copied().map(char::from).collect()
    }
}

impl fmt::Display for RootPackageBzlTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.internal_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RootPackageBzlTargetError {
    InvalidTarget { target: Arc<str>, message: Arc<str> },
    NonLatin1Scalar { target: Arc<str>, scalar: u32 },
}

impl fmt::Display for RootPackageBzlTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget { target, message } => {
                write!(f, "invalid root .bzl target `{target}`: {message}")
            }
            Self::NonLatin1Scalar { target, scalar } => write!(
                f,
                "root .bzl target `{target}` contains non-Latin-1 scalar U+{:04X}",
                scalar
            ),
        }
    }
}

impl std::error::Error for RootPackageBzlTargetError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum RootPackageSourceRequest {
    Build(PackagePath),
    Bzl {
        package: PackagePath,
        target: RootPackageBzlTarget,
    },
}

impl RootPackageSourceRequest {
    fn package(&self) -> &PackagePath {
        match self {
            Self::Build(package) | Self::Bzl { package, .. } => package,
        }
    }
}

/// Selects and reads one root-package BUILD or `.bzl` source through Host DICE
/// owners, including Bazel package-boundary and special-file behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootPackageSourceKey {
    workspace: NormalizedAbsolutePath,
    request: RootPackageSourceRequest,
}

impl RootPackageSourceKey {
    pub fn for_build(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self {
            workspace,
            request: RootPackageSourceRequest::Build(package),
        }
    }

    pub fn for_bzl(
        workspace: NormalizedAbsolutePath,
        package: PackagePath,
        target: RootPackageBzlTarget,
    ) -> Self {
        Self {
            workspace,
            request: RootPackageSourceRequest::Bzl { package, target },
        }
    }
}

impl fmt::Display for RootPackageSourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.request {
            RootPackageSourceRequest::Build(package) => {
                write!(
                    f,
                    "root-package-source:{}//{}:<BUILD>",
                    self.workspace, package
                )
            }
            RootPackageSourceRequest::Bzl { package, target } => write!(
                f,
                "root-package-source:{}//{}:{}",
                self.workspace, package, target
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootPackageSource {
    package_root: NormalizedAbsolutePath,
    logical_path: NormalizedAbsolutePath,
    relative_path: Arc<[u8]>,
    bytes: Arc<[u8]>,
}

impl RootPackageSource {
    pub fn package_root(&self) -> &NormalizedAbsolutePath {
        &self.package_root
    }

    pub fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }

    pub fn relative_path(&self) -> &Arc<[u8]> {
        &self.relative_path
    }

    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootPackageSourceErrorInner {
    PackageLookup {
        package: PackagePath,
        error: HostRootPackageLookupError,
    },
    NoBuildFile {
        package: PackagePath,
    },
    DeletedPackage {
        package: PackagePath,
    },
    InvalidPackageName {
        package: PackagePath,
        message: Arc<str>,
    },
    LabelCrossesPackageBoundary {
        package: PackagePath,
        containing_package: PackagePath,
    },
    Source {
        logical_path: NormalizedAbsolutePath,
        error: HostFileError,
    },
    Missing {
        logical_path: NormalizedAbsolutePath,
    },
    UnsupportedPlatformPath {
        target: RootPackageBzlTarget,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootPackageSourceError {
    inner: RootPackageSourceErrorInner,
}

impl RootPackageSourceError {
    fn new(inner: RootPackageSourceErrorInner) -> Self {
        Self { inner }
    }

    /// Whether the requested source was semantically absent.
    pub fn is_missing(&self) -> bool {
        matches!(&self.inner, RootPackageSourceErrorInner::Missing { .. })
    }
}

impl fmt::Display for RootPackageSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            RootPackageSourceErrorInner::PackageLookup { package, error } => {
                write!(f, "looking up root package //{package}: {error}")
            }
            RootPackageSourceErrorInner::NoBuildFile { package } => {
                write!(f, "no BUILD.bazel or BUILD file in package //{package}")
            }
            RootPackageSourceErrorInner::DeletedPackage { package } => {
                write!(f, "package //{package} is deleted or ignored")
            }
            RootPackageSourceErrorInner::InvalidPackageName { message, .. } => f.write_str(message),
            RootPackageSourceErrorInner::LabelCrossesPackageBoundary {
                package,
                containing_package,
            } => write!(
                f,
                "label in package //{package} crosses boundary of subpackage //{containing_package}"
            ),
            RootPackageSourceErrorInner::Source {
                logical_path,
                error,
            } => write!(
                f,
                "reading root package source {}: {error:?}",
                logical_path.as_path().display()
            ),
            RootPackageSourceErrorInner::Missing { logical_path } => write!(
                f,
                "root package source is missing: {}",
                logical_path.as_path().display()
            ),
            RootPackageSourceErrorInner::UnsupportedPlatformPath { target } => write!(
                f,
                "root .bzl target cannot be represented on this platform: {target}"
            ),
        }
    }
}

impl std::error::Error for RootPackageSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            RootPackageSourceErrorInner::PackageLookup { error, .. } => Some(error),
            RootPackageSourceErrorInner::NoBuildFile { .. }
            | RootPackageSourceErrorInner::DeletedPackage { .. }
            | RootPackageSourceErrorInner::InvalidPackageName { .. }
            | RootPackageSourceErrorInner::LabelCrossesPackageBoundary { .. }
            | RootPackageSourceErrorInner::Source { .. }
            | RootPackageSourceErrorInner::Missing { .. }
            | RootPackageSourceErrorInner::UnsupportedPlatformPath { .. } => None,
        }
    }
}

fn containing_package_candidates(
    package: &PackagePath,
    target: &RootPackageBzlTarget,
) -> Vec<PackagePath> {
    let raw = target.raw_bytes();
    let parent = raw
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(&[][..], |index| &raw[..index]);
    let parent: String = parent.iter().copied().map(char::from).collect();
    let mut candidate = if parent.is_empty() {
        package.as_str().to_owned()
    } else if package.as_str().is_empty() {
        parent
    } else {
        format!("{}/{parent}", package.as_str())
    };
    let mut candidates = Vec::new();
    loop {
        let parsed = PackagePath::parse(&candidate)
            .expect("validated target parents remain normalized package paths");
        let is_declared = &parsed == package;
        candidates.push(parsed);
        if is_declared {
            break;
        }
        candidate.truncate(
            candidate
                .rfind('/')
                .expect("a target parent below its declared package has a slash"),
        );
    }
    candidates
}

fn append_bzl_target(
    mut package_dir: PathBuf,
    target: &RootPackageBzlTarget,
) -> Result<PathBuf, RootPackageSourceError> {
    for component in target.raw_bytes().split(|byte| *byte == b'/') {
        #[cfg(unix)]
        package_dir.push(OsString::from_vec(component.to_vec()));
        #[cfg(not(unix))]
        {
            let component = std::str::from_utf8(component).map_err(|_| {
                RootPackageSourceError::new(RootPackageSourceErrorInner::UnsupportedPlatformPath {
                    target: target.dupe(),
                })
            })?;
            package_dir.push(component);
        }
    }
    Ok(package_dir)
}

fn source_complete_error(
    inner: RootPackageSourceErrorInner,
) -> SourcePreparationOutcome<Arc<Result<RootPackageSource, RootPackageSourceError>>> {
    SourcePreparationOutcome::Complete(Arc::new(Err(RootPackageSourceError::new(inner))))
}

#[async_trait]
impl Key for RootPackageSourceKey {
    type Value = SourcePreparationOutcome<Arc<Result<RootPackageSource, RootPackageSourceError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let declared_package = self.request.package();
        let candidates = match &self.request {
            RootPackageSourceRequest::Build(package) => vec![package.clone()],
            RootPackageSourceRequest::Bzl { package, target } => {
                containing_package_candidates(package, target)
            }
        };
        let mut selected = None;
        for candidate in candidates {
            let lookup = dice_invariant(
                ctx.compute(&HostRootPackageLookupKey::new(
                    self.workspace.dupe(),
                    candidate.clone(),
                ))
                .await,
            );
            let lookup = match lookup {
                PathOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need));
                }
                PathOutcome::Complete(value) => value,
            };
            match lookup.as_ref() {
                Err(error) => {
                    return source_complete_error(RootPackageSourceErrorInner::PackageLookup {
                        package: candidate,
                        error: error.clone(),
                    });
                }
                Ok(HostRootPackageLookup::Package(package)) => {
                    if &candidate != declared_package {
                        return source_complete_error(
                            RootPackageSourceErrorInner::LabelCrossesPackageBoundary {
                                package: declared_package.clone(),
                                containing_package: candidate,
                            },
                        );
                    }
                    selected = Some(package.dupe());
                    break;
                }
                Ok(HostRootPackageLookup::NoBuildFile) if &candidate == declared_package => {
                    return source_complete_error(RootPackageSourceErrorInner::NoBuildFile {
                        package: candidate,
                    });
                }
                Ok(HostRootPackageLookup::Deleted) if &candidate == declared_package => {
                    return source_complete_error(RootPackageSourceErrorInner::DeletedPackage {
                        package: candidate,
                    });
                }
                Ok(HostRootPackageLookup::InvalidPackageName { message })
                    if &candidate == declared_package =>
                {
                    return source_complete_error(
                        RootPackageSourceErrorInner::InvalidPackageName {
                            package: candidate,
                            message: message.clone(),
                        },
                    );
                }
                Ok(HostRootPackageLookup::NoBuildFile)
                | Ok(HostRootPackageLookup::Deleted)
                | Ok(HostRootPackageLookup::InvalidPackageName { .. }) => {}
            }
        }
        let selected = selected.expect("declared package candidate returns or selects a package");
        let package_dir = selected
            .package_root()
            .as_path()
            .join(declared_package.as_str());
        let (logical_path, relative_path): (PathBuf, Arc<[u8]>) = match &self.request {
            RootPackageSourceRequest::Build(_) => {
                let name = selected.build_file_name().as_str();
                (package_dir.join(name), Arc::from(name.as_bytes()))
            }
            RootPackageSourceRequest::Bzl { target, .. } => (
                match append_bzl_target(package_dir, target) {
                    Ok(path) => path,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Arc::new(Err(error)));
                    }
                },
                target.raw.clone(),
            ),
        };
        let logical_path = NormalizedAbsolutePath::new(logical_path)
            .expect("selected package roots and validated target remain normalized absolute");
        let source = dice_invariant(
            ctx.compute(&HostFileBytesKey::new(logical_path.dupe()))
                .await,
        );
        match source {
            PathOutcome::Need(need) => {
                SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
            }
            PathOutcome::Complete(Err(error)) => {
                source_complete_error(RootPackageSourceErrorInner::Source {
                    logical_path,
                    error,
                })
            }
            PathOutcome::Complete(Ok(HostFileBytes::Missing)) => {
                source_complete_error(RootPackageSourceErrorInner::Missing { logical_path })
            }
            PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => {
                SourcePreparationOutcome::Complete(Arc::new(Ok(RootPackageSource {
                    package_root: selected.package_root().dupe(),
                    logical_path,
                    relative_path,
                    bytes,
                })))
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
    #[cfg(unix)]
    use std::fmt;
    #[cfg(unix)]
    use std::hash::Hash;
    #[cfg(unix)]
    use std::hash::Hasher;
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::Mutex;
    #[cfg(unix)]
    use std::sync::atomic::AtomicUsize;
    #[cfg(unix)]
    use std::sync::atomic::Ordering;

    #[cfg(unix)]
    use allocative::Allocative;
    #[cfg(unix)]
    use async_trait::async_trait;
    #[cfg(unix)]
    use compact_str::CompactString;
    #[cfg(unix)]
    use dice::ActivationData;
    #[cfg(unix)]
    use dice::ActivationKind;
    #[cfg(unix)]
    use dice::ActivationTracker;
    #[cfg(unix)]
    use dice::DetectCycles;
    #[cfg(unix)]
    use dice::Dice;
    #[cfg(unix)]
    use dice::DiceComputations;
    #[cfg(unix)]
    use dice::DiceTransaction;
    #[cfg(unix)]
    use dice::DynKey;
    #[cfg(unix)]
    use dice::Key;
    #[cfg(unix)]
    use dice::RichActivation;
    #[cfg(unix)]
    use dice::UserComputationData;
    #[cfg(unix)]
    use dice_futures::cancellation::CancellationContext;
    #[cfg(unix)]
    use dupe::Dupe;
    #[cfg(unix)]
    use slug_events_v2::CaptureEvaluationEvents;
    #[cfg(unix)]
    use slug_events_v2::EvaluationEvent;
    #[cfg(unix)]
    use slug_events_v2::EventBatch;
    #[cfg(unix)]
    use slug_identity_v2::ApparentRepoName;
    #[cfg(unix)]
    use slug_identity_v2::CanonicalLabel;
    #[cfg(unix)]
    use slug_identity_v2::CanonicalRepoName;
    #[cfg(unix)]
    use slug_identity_v2::PackageIdentifier;
    #[cfg(unix)]
    use slug_identity_v2::PackagePath;
    #[cfg(unix)]
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::PathIoErrorKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationError;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationNamespace;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationOperation;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOperationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOutcome;
    #[cfg(unix)]
    use starlark_map::small_map::SmallMap;

    #[cfg(unix)]
    use super::ExternalRepositoryPackageLookup;
    #[cfg(unix)]
    use super::ExternalRepositoryPackageLookupError;
    #[cfg(unix)]
    use super::ExternalRepositoryPackageLookupKey;
    #[cfg(unix)]
    use super::HostBuildFileName;
    #[cfg(unix)]
    use super::HostRootPackageLookup;
    #[cfg(unix)]
    use super::HostRootPackageLookupKey;
    #[cfg(unix)]
    use super::RepositoryPackageSourceError;
    #[cfg(unix)]
    use super::RepositoryPackageSourceErrorInner;
    #[cfg(unix)]
    use super::RepositoryPackageSourceKey;
    #[cfg(unix)]
    use super::RootPackageBzlTarget;
    #[cfg(unix)]
    use super::RootPackageSource;
    #[cfg(unix)]
    use super::RootPackageSourceError;
    #[cfg(unix)]
    use super::RootPackageSourceKey;
    #[cfg(unix)]
    use crate::BzlmodCommandPolicyKey;
    #[cfg(unix)]
    use crate::BzlmodEnvironmentPolicyKey;
    #[cfg(unix)]
    use crate::LockfileMode;
    #[cfg(unix)]
    use crate::OverrideAttributeValue;
    #[cfg(unix)]
    use crate::RepoSpec;
    #[cfg(unix)]
    use crate::RepositorySourceFileError;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::RootRepositoryRoute;
    #[cfg(unix)]
    use crate::inject_root_module_request_inputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;
    #[cfg(unix)]
    use crate::repo_file::HostRouteRepoFileKey;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationEpochEntry;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationKind;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationRequest;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationRequestId;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResult;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpoch;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpochKey;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationSuccess;
    #[cfg(unix)]
    use crate::source_preparation::SourcePreparationOutcome;

    #[cfg(unix)]
    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    #[cfg(unix)]
    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    #[cfg(unix)]
    fn lstat(kind: PathNodeKind, variant: i64) -> PathLstat {
        PathLstat::new(kind, variant, variant, variant, variant, 0o755)
    }

    #[cfg(unix)]
    fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    }

    #[cfg(unix)]
    fn observed_lstat(value: &str, result: PathOperationResult<PathLstat>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(result),
        )
    }

    #[cfg(unix)]
    fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
        observed_lstat(value, PathOperationResult::Present(lstat(kind, variant)))
    }

    #[cfg(unix)]
    fn missing(value: &str) -> ScriptEntry {
        observed_lstat(value, PathOperationResult::Missing)
    }

    #[cfg(unix)]
    fn lstat_error(value: &str) -> ScriptEntry {
        observed_lstat(
            value,
            PathOperationResult::Error(PathObservationError::Io {
                kind: PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            }),
        )
    }

    #[cfg(unix)]
    fn bytes(value: &str, contents: &'static [u8]) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(contents))),
        )
    }

    #[cfg(unix)]
    fn missing_bytes(value: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Missing),
        )
    }

    #[cfg(unix)]
    fn read_link(value: &str, target: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                target,
            )))),
        )
    }

    #[cfg(unix)]
    fn missing_read_link(value: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Missing),
        )
    }

    #[cfg(unix)]
    fn inputs(roots: &[&str], deleted: &[&str], vendor: Option<&str>) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            path("/workspace"),
            roots.iter().map(|root| path(root)).collect::<Vec<_>>(),
            deleted,
            vendor.map(path),
            Some("warning"),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn local_route(root: &str) -> RootRepositoryRoute {
        RootRepositoryRoute::for_test(
            path("/workspace"),
            ApparentRepoName::new("dep").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::new("path"),
                    OverrideAttributeValue::String(root.into()),
                )])),
            },
        )
    }

    #[cfg(unix)]
    fn route_materialization(root: &str) -> RepositoryMaterializationResultEpoch {
        let route = local_route(root);
        RepositoryMaterializationResultEpoch::new(
            path("/workspace"),
            [RepositoryMaterializationEpochEntry {
                request: Arc::new(RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: path("/workspace"),
                        canonical_repo: route.canonical_repo().clone(),
                    },
                    repo_spec: route.repo_spec().clone(),
                    kind: RepositoryMaterializationKind::Local {
                        logical_root: path(&format!("/workspace/{root}")),
                    },
                }),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn route_prelude(
        root: &str,
        repo: Option<&'static [u8]>,
        ignore: Option<&'static [u8]>,
        variant: i64,
    ) -> Vec<ScriptEntry> {
        let root = format!("/workspace/{root}");
        let mut entries = vec![
            present("/", PathNodeKind::Directory, variant),
            present("/workspace", PathNodeKind::Directory, variant),
            present(&root, PathNodeKind::Directory, variant),
        ];
        entries.extend([
            present(
                "/workspace/MODULE.bazel",
                PathNodeKind::RegularFile,
                variant,
            ),
            bytes(
                "/workspace/MODULE.bazel",
                Box::leak(
                    format!(
                        "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"{}\")\n",
                        root.strip_prefix("/workspace/").unwrap()
                    )
                    .into_bytes()
                    .into_boxed_slice(),
                ),
            ),
            present(
                &format!("{root}/MODULE.bazel"),
                PathNodeKind::RegularFile,
                variant,
            ),
            bytes(
                &format!("{root}/MODULE.bazel"),
                b"module(name = \"dep\", version = \"1.0.0\")\n",
            ),
        ]);
        for (name, source) in [("REPO.bazel", repo), (".bazelignore", ignore)] {
            let logical = format!("{root}/{name}");
            match source {
                Some(source) => {
                    entries.push(present(&logical, PathNodeKind::RegularFile, variant));
                    entries.push(bytes(&logical, source));
                }
                None => entries.push(missing(&logical)),
            }
        }
        entries
    }

    #[cfg(unix)]
    fn external_key(root: &str, package: &str) -> ExternalRepositoryPackageLookupKey {
        let route = local_route(root);
        ExternalRepositoryPackageLookupKey::new(
            route.clone(),
            PackageIdentifier::new(
                route.canonical_repo().clone(),
                PackagePath::parse(package).unwrap(),
            ),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn external_source_key(root: &str, package: &str) -> RepositoryPackageSourceKey {
        let route = local_route(root);
        RepositoryPackageSourceKey::new(
            route.clone(),
            PackageIdentifier::new(
                route.canonical_repo().clone(),
                PackagePath::parse(package).unwrap(),
            ),
        )
        .unwrap()
    }

    #[cfg(unix)]
    async fn external_transaction(
        dice: &Arc<Dice>,
        root: &str,
        deleted: &[&str],
        entries: Vec<ScriptEntry>,
        tracker: Option<Arc<RouteRepoEventTracker>>,
    ) -> DiceTransaction {
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_root_package_policy_inputs(&mut updater, inputs(&[], deleted, None)).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            std::path::Path::new("/workspace"),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(&entries))])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: path("/workspace"),
                },
                route_materialization(root),
            )])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    async fn route_repo_value(
        dice: &Arc<Dice>,
        entries: Vec<ScriptEntry>,
        materialized: bool,
        capture: bool,
        tracker: Arc<RouteRepoEventTracker>,
    ) -> <HostRouteRepoFileKey as Key>::Value {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture {
            data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(data);
        inject_root_package_policy_inputs(&mut updater, inputs(&[], &[], None)).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(&entries))])
            .unwrap();
        let materialization = if materialized {
            route_materialization("dep")
        } else {
            RepositoryMaterializationResultEpoch::new(path("/workspace"), []).unwrap()
        };
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: path("/workspace"),
                },
                materialization,
            )])
            .unwrap();
        updater
            .commit()
            .await
            .compute(&HostRouteRepoFileKey::new(local_route("dep")))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    fn external_value(
        outcome: SourcePreparationOutcome<
            Arc<
                Result<
                    ExternalRepositoryPackageLookup,
                    super::ExternalRepositoryPackageLookupError,
                >,
            >,
        >,
    ) -> ExternalRepositoryPackageLookup {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("external lookup returned Need");
        };
        value.as_ref().as_ref().unwrap().dupe()
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RouteRepoActivation {
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[cfg(unix)]
    #[derive(Default)]
    struct RouteRepoEventTracker(Mutex<Vec<RouteRepoActivation>>);

    #[cfg(unix)]
    impl RouteRepoEventTracker {
        fn take(&self) -> Vec<RouteRepoActivation> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    #[cfg(unix)]
    impl ActivationTracker for RouteRepoEventTracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<HostRouteRepoFileKey>().is_some() {
                self.0.lock().unwrap().push(RouteRepoActivation {
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn route_repo_event_lifecycle_is_exact() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(RouteRepoEventTracker::default());
        let source = Some(b"print('captured')\n".as_slice());

        let value = route_repo_value(
            &dice,
            route_prelude("dep", source, None, 50),
            true,
            true,
            tracker.clone(),
        )
        .await;
        assert!(matches!(value, SourcePreparationOutcome::Complete(value) if value.is_ok()));
        let activations = tracker.take();
        assert!(matches!(
            activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if matches!(
                batch.events(),
                [EvaluationEvent::StarlarkPrint { text, .. }] if text == "captured"
            )
        ));

        route_repo_value(
            &dice,
            route_prelude("dep", source, None, 50),
            true,
            true,
            tracker.clone(),
        )
        .await;
        assert_eq!(
            tracker.take(),
            [RouteRepoActivation {
                kind: ActivationKind::Reused,
                batch: None,
            }]
        );

        let absent = route_repo_value(
            &dice,
            route_prelude("dep", None, None, 51),
            true,
            true,
            tracker.clone(),
        )
        .await;
        assert!(matches!(absent, SourcePreparationOutcome::Complete(value) if value.is_ok()));
        let activations = tracker.take();
        assert!(matches!(
            activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if batch.events().is_empty()
        ));

        let repo_path = path("/workspace/dep/REPO.bazel");
        let mut entries = route_prelude("dep", None, None, 52);
        entries.retain(|(demand, _)| demand.path() != &repo_path);
        entries.push(present(
            "/workspace/dep/REPO.bazel",
            PathNodeKind::Directory,
            52,
        ));
        let failed = route_repo_value(&dice, entries, true, true, tracker.clone()).await;
        assert!(matches!(
            failed,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(crate::repo_file::HostRouteRepoFileError::Source(_)))
        ));
        let activations = tracker.take();
        assert!(matches!(
            activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if batch.events().is_empty()
        ));

        let need = route_repo_value(&dice, Vec::new(), false, true, tracker.clone()).await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert_eq!(
            tracker.take(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: None,
            }]
        );

        let uncaptured = route_repo_value(
            &dice,
            route_prelude("dep", Some(b"print('DIRECT')\n"), None, 53),
            true,
            false,
            tracker.clone(),
        )
        .await;
        assert!(matches!(uncaptured, SourcePreparationOutcome::Complete(value) if value.is_ok()));
        assert_eq!(
            tracker.take(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: None,
            }]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn external_lookup_validates_and_canonically_deletes_without_route_or_event() {
        let route = local_route("dep");
        assert!(
            ExternalRepositoryPackageLookupKey::new(
                route.clone(),
                PackageIdentifier::new(
                    CanonicalRepoName::new("other+").unwrap(),
                    PackagePath::parse("pkg").unwrap(),
                ),
            )
            .is_none()
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        let invalid = external_key("dep", "bad:name");
        assert!(matches!(
            external_value(transaction.compute(&invalid).await.unwrap()),
            ExternalRepositoryPackageLookup::InvalidPackageName { .. }
        ));

        let tracker = Arc::new(RouteRepoEventTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_root_package_policy_inputs(&mut updater, inputs(&[], &["@dep+//pkg"], None))
            .unwrap();
        let mut transaction = updater.commit().await;
        assert_eq!(
            external_value(
                transaction
                    .compute(&external_key("dep", "pkg"))
                    .await
                    .unwrap()
            ),
            ExternalRepositoryPackageLookup::Deleted
        );
        assert!(tracker.take().is_empty());

        let mut updater = transaction.into_updater();
        inject_root_package_policy_inputs(&mut updater, inputs(&[], &["@dep//pkg"], None)).unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: path("/workspace"),
                },
                RepositoryMaterializationResultEpoch::new(path("/workspace"), []).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction
            .compute(&external_key("dep", "pkg"))
            .await
            .unwrap();
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
        assert!(!ExternalRepositoryPackageLookupKey::validity(&outcome));
        assert!(!ExternalRepositoryPackageLookupKey::equality(
            &outcome, &outcome
        ));
        assert_eq!(
            tracker.take(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: None,
            }]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn external_lookup_reacts_to_repo_and_bazelignore_edits_with_child_events() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(RouteRepoEventTracker::default());
        let mut entries = route_prelude(
            "dep",
            Some(b"print('one')\nignore_directories(['repo_deleted'])\n"),
            Some(b"file_deleted\n"),
            1,
        );
        let mut transaction = external_transaction(
            &dice,
            "dep",
            &[],
            std::mem::take(&mut entries),
            Some(tracker.clone()),
        )
        .await;
        for package in ["repo_deleted", "file_deleted"] {
            assert_eq!(
                external_value(
                    transaction
                        .compute(&external_key("dep", package))
                        .await
                        .unwrap()
                ),
                ExternalRepositoryPackageLookup::Deleted
            );
        }
        let activations = tracker.take();
        assert!(matches!(
            activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if matches!(
                batch.events(),
                [EvaluationEvent::StarlarkPrint { text, .. }] if text == "one"
            )
        ));

        let mut entries = route_prelude(
            "dep",
            Some(b"print('two')\nignore_directories(['other'])\n"),
            Some(b"also_other\n"),
            2,
        );
        for package in ["repo_deleted", "file_deleted"] {
            entries.extend([
                present(
                    &format!("/workspace/dep/{package}"),
                    PathNodeKind::Directory,
                    2,
                ),
                present(
                    &format!("/workspace/dep/{package}/BUILD.bazel"),
                    PathNodeKind::RegularFile,
                    2,
                ),
            ]);
        }
        let mut transaction =
            external_transaction(&dice, "dep", &[], entries, Some(tracker.clone())).await;
        for package in ["repo_deleted", "file_deleted"] {
            assert_eq!(
                external_value(
                    transaction
                        .compute(&external_key("dep", package))
                        .await
                        .unwrap()
                ),
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel)
            );
        }
        let activations = tracker.take();
        assert!(
            matches!(
                activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }, RouteRepoActivation {
                kind: ActivationKind::Reused,
                batch: None,
            }] if matches!(
                batch.events(),
                [EvaluationEvent::StarlarkPrint { text, .. }] if text == "two"
            )
            ),
            "{activations:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn external_lookup_build_priority_kinds_symlink_errors_and_lifecycle() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = external_key("dep", "pkg");
        let cases = [
            (
                Some(PathNodeKind::RegularFile),
                Some(PathNodeKind::RegularFile),
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            ),
            (
                Some(PathNodeKind::Directory),
                Some(PathNodeKind::SpecialFile),
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::Build),
            ),
            (
                None,
                Some(PathNodeKind::RegularFile),
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::Build),
            ),
            (None, None, ExternalRepositoryPackageLookup::NoBuildFile),
            (
                Some(PathNodeKind::RegularFile),
                None,
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            ),
        ];
        let mut observed = Vec::new();
        for (variant, (primary, fallback, expected)) in cases.into_iter().enumerate() {
            let variant = variant as i64 + 10;
            let mut entries = route_prelude("dep", None, None, variant);
            entries.push(present(
                "/workspace/dep/pkg",
                PathNodeKind::Directory,
                variant,
            ));
            for (name, kind) in [("BUILD.bazel", primary), ("BUILD", fallback)] {
                let marker = format!("/workspace/dep/pkg/{name}");
                entries.push(match kind {
                    Some(kind) => present(&marker, kind, variant),
                    None => missing(&marker),
                });
            }
            let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
            let value = external_value(transaction.compute(&key).await.unwrap());
            assert_eq!(value, expected);
            observed.push(value);
        }
        assert_eq!(observed[0], observed[4]);

        let mut entries = route_prelude("dep", None, None, 20);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 20),
            present("/workspace/dep/pkg/BUILD.bazel", PathNodeKind::Symlink, 20),
            read_link("/workspace/dep/pkg/BUILD.bazel", "/physical/selected"),
            present("/physical", PathNodeKind::Directory, 20),
            present("/physical/selected", PathNodeKind::RegularFile, 20),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        assert_eq!(
            external_value(transaction.compute(&key).await.unwrap()),
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel)
        );

        let mut entries = route_prelude("dep", None, None, 21);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 21),
            lstat_error("/workspace/dep/pkg/BUILD.bazel"),
            present("/workspace/dep/pkg/BUILD", PathNodeKind::RegularFile, 21),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(super::ExternalRepositoryPackageLookupError::Path(_)))
        ));

        let mut entries = route_prelude("dep", None, None, 22);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 22),
            present("/workspace/dep/pkg/BUILD", PathNodeKind::RegularFile, 22),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("missing primary observation must stop before fallback");
        };
        let demands = needs.path_observations().unwrap().demands();
        assert!(
            demands
                .iter()
                .any(|demand| demand.path() == &path("/workspace/dep/pkg/BUILD.bazel"))
        );
        assert!(
            demands
                .iter()
                .all(|demand| demand.path() != &path("/workspace/dep/pkg/BUILD"))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn external_lookup_retains_typed_repo_and_ignore_errors() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = external_key("dep", "pkg");
        let tracker = Arc::new(RouteRepoEventTracker::default());
        let entries = route_prelude("dep", Some(b"x =\n"), None, 40);
        let mut transaction =
            external_transaction(&dice, "dep", &[], entries, Some(tracker.clone())).await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::ExternalRepositoryPackageLookupError::RepositoryIgnore(
                        crate::repository_ignore::HostRepositoryIgnoreError::RouteRepoFile(
                            crate::repo_file::HostRouteRepoFileError::Evaluation(
                                crate::repo_file::HostRepoFileError::Syntax { .. }
                            )
                        )
                    ))
                )
        ));
        let activations = tracker.take();
        assert!(matches!(
            activations.as_slice(),
            [RouteRepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if matches!(batch.events(), [EvaluationEvent::Diagnostic { .. }])
        ));

        let entries = route_prelude("dep", None, Some(b"/absolute\n"), 41);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::ExternalRepositoryPackageLookupError::RepositoryIgnore(
                        crate::repository_ignore::HostRepositoryIgnoreError::InvalidAbsolute { .. }
                    ))
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn external_lookup_route_identity_is_a_to_b_to_a() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut seen = Vec::new();
        for root in ["dep-a", "dep-b", "dep-a"] {
            let key = external_key(root, "pkg");
            let mut entries = route_prelude(root, None, None, 30);
            entries.extend([
                present(
                    &format!("/workspace/{root}/pkg"),
                    PathNodeKind::Directory,
                    30,
                ),
                present(
                    &format!("/workspace/{root}/pkg/BUILD.bazel"),
                    PathNodeKind::RegularFile,
                    30,
                ),
            ]);
            let mut transaction = external_transaction(&dice, root, &[], entries, None).await;
            let outcome = transaction.compute(&key).await.unwrap();
            assert_eq!(
                external_value(outcome.clone()),
                ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel)
            );
            seen.push((key, outcome));
        }
        assert_ne!(seen[0].0, seen[1].0);
        assert_eq!(seen[0].0, seen[2].0);
        assert!(ExternalRepositoryPackageLookupKey::equality(
            &seen[0].1, &seen[2].1
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repository_package_source_selects_one_marker_and_replays_its_lifecycle() {
        let route = local_route("dep");
        assert!(
            RepositoryPackageSourceKey::new(
                route,
                PackageIdentifier::new(
                    CanonicalRepoName::new("other+").unwrap(),
                    PackagePath::parse("pkg").unwrap(),
                ),
            )
            .is_none()
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = external_source_key("dep", "pkg");
        for (variant, primary, fallback, expected_name, expected_bytes) in [
            (
                60,
                Some(b"primary".as_slice()),
                Some(b"fallback".as_slice()),
                "BUILD.bazel",
                b"primary".as_slice(),
            ),
            (
                61,
                Some(b"edited".as_slice()),
                Some(b"fallback".as_slice()),
                "BUILD.bazel",
                b"edited".as_slice(),
            ),
            (
                62,
                None,
                Some(b"fallback".as_slice()),
                "BUILD",
                b"fallback".as_slice(),
            ),
            (
                63,
                Some(b"restored".as_slice()),
                None,
                "BUILD.bazel",
                b"restored".as_slice(),
            ),
        ] {
            let mut entries = route_prelude("dep", None, None, variant);
            entries.push(present(
                "/workspace/dep/pkg",
                PathNodeKind::Directory,
                variant,
            ));
            for (name, source) in [("BUILD.bazel", primary), ("BUILD", fallback)] {
                let marker = format!("/workspace/dep/pkg/{name}");
                if let Some(source) = source {
                    entries.push(present(&marker, PathNodeKind::RegularFile, variant));
                    entries.push(bytes(&marker, source));
                } else {
                    entries.push(missing(&marker));
                }
            }
            let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(value) = &outcome else {
                panic!("complete selected source epoch returned Need");
            };
            let source = value.as_ref().as_ref().unwrap();
            assert_eq!(source.build_file_name(), expected_name);
            assert_eq!(source.bytes().as_ref(), expected_bytes);
            assert_eq!(
                source.logical_path(),
                &path(&format!("/workspace/dep/pkg/{expected_name}"))
            );
            assert!(RepositoryPackageSourceKey::validity(&outcome));
            assert!(RepositoryPackageSourceKey::equality(&outcome, &outcome));
        }

        let mut entries = route_prelude("dep", None, None, 64);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 64),
            present(
                "/workspace/dep/pkg/BUILD.bazel",
                PathNodeKind::RegularFile,
                64,
            ),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let need = transaction.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!RepositoryPackageSourceKey::validity(&need));
        assert!(!RepositoryPackageSourceKey::equality(&need, &need));

        let mut entries = route_prelude("dep", None, None, 65);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 65),
            present(
                "/workspace/dep/pkg/BUILD.bazel",
                PathNodeKind::RegularFile,
                65,
            ),
            missing_bytes("/workspace/dep/pkg/BUILD.bazel"),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let absent = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            &absent,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    &value.as_ref().as_ref().unwrap_err().inner,
                    RepositoryPackageSourceErrorInner::Source {
                        logical_path,
                        error: RepositorySourceFileError::InconsistentState { .. },
                    } if logical_path.as_path() == PathBuf::from("pkg/BUILD.bazel")
                )
        ));

        let mut entries = route_prelude("dep", None, None, 66);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 66),
            missing("/workspace/dep/pkg/BUILD.bazel"),
            missing("/workspace/dep/pkg/BUILD"),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let deleted = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    &value.as_ref().as_ref().unwrap_err().inner,
                    RepositoryPackageSourceErrorInner::NoBuildFile { .. }
                )
        ));

        let mut entries = route_prelude("dep", None, None, 67);
        entries.extend([
            present("/workspace/dep/pkg", PathNodeKind::Directory, 67),
            present(
                "/workspace/dep/pkg/BUILD.bazel",
                PathNodeKind::RegularFile,
                67,
            ),
            bytes("/workspace/dep/pkg/BUILD.bazel", b"recreated"),
        ]);
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let recreated = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            recreated,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap().bytes().as_ref() == b"recreated"
        ));

        let tracker = Arc::new(RouteRepoEventTracker::default());
        let mut transaction = external_transaction(
            &dice,
            "dep",
            &["@dep+//pkg"],
            route_prelude("dep", None, None, 68),
            Some(tracker.clone()),
        )
        .await;
        let deleted = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap_err().to_string().contains("is deleted")
        ));
        assert!(tracker.take().is_empty());

        let mut routed = Vec::new();
        for (variant, root) in [(68, "dep-a"), (69, "dep-b"), (68, "dep-a")] {
            let key = external_source_key(root, "pkg");
            let marker = format!("/workspace/{root}/pkg/BUILD.bazel");
            let mut entries = route_prelude(root, None, None, variant);
            entries.extend([
                present(
                    &format!("/workspace/{root}/pkg"),
                    PathNodeKind::Directory,
                    variant,
                ),
                present(&marker, PathNodeKind::RegularFile, variant),
                bytes(&marker, b"same"),
            ]);
            let mut transaction = external_transaction(&dice, root, &[], entries, None).await;
            routed.push((key.clone(), transaction.compute(&key).await.unwrap()));
        }
        assert_ne!(routed[0].0, routed[1].0);
        assert_eq!(routed[0].0, routed[2].0);
        assert!(!RepositoryPackageSourceKey::equality(
            &routed[0].1,
            &routed[1].1
        ));
        assert!(RepositoryPackageSourceKey::equality(
            &routed[0].1,
            &routed[2].1
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repository_package_source_retains_complete_module_evaluation_error_chain() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = external_source_key("dep", "pkg");
        let mut entries = route_prelude("dep", None, None, 72);
        let module_bytes = demand(
            "/workspace/dep/MODULE.bazel",
            PathObservationOperation::FileBytes,
        );
        entries.retain(|(incoming, _)| incoming != &module_bytes);
        entries.push(bytes(
            "/workspace/dep/MODULE.bazel",
            b"module(name = 'dep', version = '1.0.0')\nfail('ordinary-module-error')\n",
        ));
        let mut transaction = external_transaction(&dice, "dep", &[], entries, None).await;
        let SourcePreparationOutcome::Complete(value) = transaction.compute(&key).await.unwrap()
        else {
            panic!("ordinary module failure must be Complete")
        };
        let error = value.as_ref().as_ref().unwrap_err();
        assert!(!error.is_unsupported_feature());
        assert!(error.to_string().contains("ordinary-module-error"));
        let support = std::error::Error::source(error).expect("source retains support error");
        assert!(support.to_string().contains("ordinary-module-error"));
        let evaluation = support.source().expect("support retains evaluation owner");
        assert!(evaluation.to_string().contains("ordinary-module-error"));
        assert!(
            evaluation.source().is_some(),
            "evaluation owner retains the interpreter error"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_package_source_errors_preserve_structural_classes_and_chain() {
        let package = PackageIdentifier::new(
            CanonicalRepoName::new("dep+").unwrap(),
            PackagePath::parse("pkg").unwrap(),
        );
        let marker = Arc::new(PathBuf::from("pkg/BUILD.bazel"));
        let lookup = RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::Lookup {
            package: package.clone(),
            error: ExternalRepositoryPackageLookupError::Path(
                RepositorySourceFileError::InvalidRepoRelativePath {
                    requested_path: marker.clone(),
                },
            ),
        });
        assert_eq!(lookup, lookup.clone());
        assert!(lookup.to_string().contains("selecting BUILD source"));
        assert!(std::error::Error::source(&lookup).is_some());

        let errors = [
            RepositoryPackageSourceError::new(
                RepositoryPackageSourceErrorInner::InvalidPackageName {
                    package: package.clone(),
                    message: Arc::from("bad name"),
                },
            ),
            RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::Deleted {
                package: package.clone(),
            }),
            RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::NoBuildFile {
                package: package.clone(),
            }),
            RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::LookupCompute {
                package,
                message: Arc::from("lookup compute"),
            }),
            RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::Source {
                logical_path: marker.clone(),
                error: RepositorySourceFileError::InvalidRepoRelativePath {
                    requested_path: marker.clone(),
                },
            }),
            RepositoryPackageSourceError::new(RepositoryPackageSourceErrorInner::SourceCompute {
                logical_path: marker.clone(),
                message: Arc::from("source compute"),
            }),
            RepositoryPackageSourceError::new(
                RepositoryPackageSourceErrorInner::SelectedSourceAbsent {
                    logical_path: marker,
                },
            ),
        ];
        for error in errors {
            assert_eq!(error, error.clone());
            assert!(!error.to_string().is_empty());
            assert!(std::error::Error::source(&error).is_none());
        }
    }

    #[cfg(unix)]
    fn repository_prelude(roots: &[&str], variant: i64) -> Vec<ScriptEntry> {
        let mut entries = vec![
            present("/", PathNodeKind::Directory, variant),
            present("/workspace", PathNodeKind::Directory, variant),
            missing("/workspace/REPO.bazel"),
        ];
        for root in roots {
            entries.push(present(root, PathNodeKind::Directory, variant));
            entries.push(missing(&format!("{root}/.bazelignore")));
        }
        entries
    }

    #[cfg(unix)]
    fn epoch(entries: &[ScriptEntry]) -> PathObservationEpoch {
        PathObservationEpoch::new(
            entries
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap()
    }

    #[cfg(unix)]
    async fn lookup(
        policy: RootPackagePolicyInputs,
        entries: Vec<ScriptEntry>,
        package: &str,
    ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, policy).unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(entries).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootPackageLookupKey::new(
                path("/workspace"),
                PackagePath::parse(package).unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    async fn lookup_without_observations(
        policy: Option<RootPackagePolicyInputs>,
        package: &str,
    ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        if let Some(policy) = policy {
            inject_root_package_policy_inputs(&mut updater, policy).unwrap();
        }
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootPackageLookupKey::new(
                path("/workspace"),
                PackagePath::parse(package).unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    async fn source(
        policy: RootPackagePolicyInputs,
        entries: Vec<ScriptEntry>,
        key: RootPackageSourceKey,
    ) -> SourcePreparationOutcome<Arc<Result<RootPackageSource, RootPackageSourceError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, policy).unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(entries).unwrap(),
            )])
            .unwrap();
        updater.commit().await.compute(&key).await.unwrap()
    }

    #[cfg(unix)]
    fn package(
        outcome: &PathOutcome<
            Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>,
        >,
    ) -> &super::HostPackage {
        let PathOutcome::Complete(value) = outcome else {
            panic!("complete script returned an observation Need");
        };
        let Ok(HostRootPackageLookup::Package(package)) = value.as_ref() else {
            panic!("expected package, got {value:?}");
        };
        package
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, Allocative)]
    struct LookupCounterKey {
        lookup: HostRootPackageLookupKey,
        #[allocative(skip)]
        counter: Arc<AtomicUsize>,
    }

    #[cfg(unix)]
    impl PartialEq for LookupCounterKey {
        fn eq(&self, other: &Self) -> bool {
            self.lookup == other.lookup && Arc::ptr_eq(&self.counter, &other.counter)
        }
    }

    #[cfg(unix)]
    impl Eq for LookupCounterKey {}

    #[cfg(unix)]
    impl Hash for LookupCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.lookup.hash(state);
            Arc::as_ptr(&self.counter).hash(state);
        }
    }

    #[cfg(unix)]
    impl fmt::Display for LookupCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "host-package-lookup-counter:{}:{:p}",
                self.lookup,
                Arc::as_ptr(&self.counter)
            )
        }
    }

    #[cfg(unix)]
    #[async_trait]
    impl Key for LookupCounterKey {
        type Value = PathOutcome<usize>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            ctx.compute(&self.lookup)
                .await
                .unwrap()
                .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }

    #[cfg(unix)]
    async fn update_epoch(
        transaction: DiceTransaction,
        entries: &[ScriptEntry],
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(entries))])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ordered_roots_are_outer_to_build_file_name_priority() {
        let roots = ["/root-a", "/root-b"];
        let mut entries = repository_prelude(&roots, 1);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            missing("/root-a/pkg/BUILD.bazel"),
            present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 1),
            present("/root-b/pkg", PathNodeKind::Directory, 1),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 1),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        let selected = package(&outcome);
        assert_eq!(selected.package_root(), &path("/root-a"));
        assert_eq!(selected.build_file_name(), HostBuildFileName::Build);

        let roots = ["/root-a"];
        let mut entries = repository_prelude(&roots, 2);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::RegularFile, 2),
            present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 2),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );

        let roots = ["/root-a", "/root-b"];
        let mut entries = repository_prelude(&roots, 3);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 3),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Directory, 3),
            missing("/root-a/pkg/BUILD"),
            present("/root-b/pkg", PathNodeKind::Directory, 3),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 3),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        let selected = package(&outcome);
        assert_eq!(selected.package_root(), &path("/root-b"));
        assert_eq!(selected.build_file_name(), HostBuildFileName::BuildDotBazel);
    }

    #[cfg(unix)]
    #[test]
    fn root_bzl_target_is_validated_before_key_identity() {
        let target = RootPackageBzlTarget::parse("defs/\u{e9}.bzl").unwrap();
        assert_eq!(target.raw_bytes(), b"defs/\xe9.bzl");
        assert_eq!(target.to_string(), "defs/\u{e9}.bzl");

        for invalid in [
            "",
            "/x.bzl",
            "../x.bzl",
            "./x.bzl",
            "a/../x.bzl",
            "a/./x.bzl",
            "a//x.bzl",
            "a\\x.bzl",
            "a:x.bzl",
            "a/\u{1}x.bzl",
            "a/x.bzl/",
            "a/x.scl",
            "\u{100}.bzl",
        ] {
            assert!(
                RootPackageBzlTarget::parse(invalid).is_err(),
                "{invalid:?} entered source-key identity"
            );
        }

        let package = PackagePath::parse("pkg").unwrap();
        let key = RootPackageSourceKey::for_bzl(
            path("/workspace"),
            package.clone(),
            RootPackageBzlTarget::parse("defs/a.bzl").unwrap(),
        );
        assert_ne!(
            key,
            RootPackageSourceKey::for_bzl(
                path("/workspace"),
                package.clone(),
                RootPackageBzlTarget::parse("defs/b.bzl").unwrap(),
            )
        );
        assert_ne!(
            key,
            RootPackageSourceKey::for_build(path("/workspace"), package.clone())
        );
        assert_ne!(
            key,
            RootPackageSourceKey::for_bzl(
                path("/other"),
                package,
                RootPackageBzlTarget::parse("defs/a.bzl").unwrap(),
            )
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_projection_selects_special_build_and_nested_bzl_bytes() {
        let build_roots = ["/root-a", "/root-b"];
        let mut build_entries = repository_prelude(&build_roots, 31);
        build_entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 31),
            missing("/root-a/pkg/BUILD.bazel"),
            present("/root-a/pkg/BUILD", PathNodeKind::SpecialFile, 31),
            bytes("/root-a/pkg/BUILD", b"build-source"),
            present("/root-b/pkg", PathNodeKind::Directory, 31),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 31),
        ]);
        let build = source(
            inputs(&build_roots, &[], None),
            build_entries,
            RootPackageSourceKey::for_build(path("/workspace"), PackagePath::parse("pkg").unwrap()),
        )
        .await;
        let SourcePreparationOutcome::Complete(build) = &build else {
            panic!("complete BUILD observations returned Need");
        };
        let build = build.as_ref().as_ref().unwrap();
        assert_eq!(build.package_root(), &path("/root-a"));
        assert_eq!(build.logical_path(), &path("/root-a/pkg/BUILD"));
        assert_eq!(build.relative_path().as_ref(), b"BUILD");
        assert_eq!(build.bytes().as_ref(), b"build-source");
        assert!(RootPackageSourceKey::validity(
            &SourcePreparationOutcome::Complete(Arc::new(Ok(build.dupe())))
        ));

        let bzl_roots = ["/root"];
        let mut bzl_entries = repository_prelude(&bzl_roots, 32);
        bzl_entries.extend([
            present("/root/pkg", PathNodeKind::Directory, 32),
            present("/root/pkg/defs", PathNodeKind::Directory, 32),
            missing("/root/pkg/defs/BUILD.bazel"),
            missing("/root/pkg/defs/BUILD"),
            present("/root/pkg/BUILD.bazel", PathNodeKind::RegularFile, 32),
            present("/root/pkg/defs/lib.bzl", PathNodeKind::SpecialFile, 32),
            bytes("/root/pkg/defs/lib.bzl", b"bzl-source"),
        ]);
        let bzl = source(
            inputs(&bzl_roots, &[], None),
            bzl_entries,
            RootPackageSourceKey::for_bzl(
                path("/workspace"),
                PackagePath::parse("pkg").unwrap(),
                RootPackageBzlTarget::parse("defs/lib.bzl").unwrap(),
            ),
        )
        .await;
        let SourcePreparationOutcome::Complete(bzl) = bzl else {
            panic!("complete .bzl observations returned Need");
        };
        let bzl = bzl.as_ref().as_ref().unwrap();
        assert_eq!(bzl.logical_path(), &path("/root/pkg/defs/lib.bzl"));
        assert_eq!(bzl.relative_path().as_ref(), b"defs/lib.bzl");
        assert_eq!(bzl.bytes().as_ref(), b"bzl-source");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_projection_preserves_package_policy_and_missing_source_errors() {
        let roots = ["/root"];
        let key =
            RootPackageSourceKey::for_build(path("/workspace"), PackagePath::parse("pkg").unwrap());
        let deleted = source(inputs(&roots, &["//pkg"], None), Vec::new(), key).await;
        let SourcePreparationOutcome::Complete(deleted) = deleted else {
            panic!("deleted package requested observations");
        };
        assert_eq!(
            deleted.as_ref().as_ref().unwrap_err().to_string(),
            "package //pkg is deleted or ignored"
        );

        let invalid = source(
            inputs(&roots, &[], None),
            Vec::new(),
            RootPackageSourceKey::for_build(
                path("/workspace"),
                PackagePath::parse("bad:name").unwrap(),
            ),
        )
        .await;
        let SourcePreparationOutcome::Complete(invalid) = invalid else {
            panic!("invalid package requested observations");
        };
        assert!(
            invalid
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .starts_with("Invalid package name 'bad:name':")
        );

        let mut missing_entries = repository_prelude(&roots, 33);
        missing_entries.extend([
            present("/root/pkg", PathNodeKind::Directory, 33),
            present("/root/pkg/BUILD.bazel", PathNodeKind::RegularFile, 33),
            missing("/root/pkg/missing.bzl"),
        ]);
        let missing = source(
            inputs(&roots, &[], None),
            missing_entries,
            RootPackageSourceKey::for_bzl(
                path("/workspace"),
                PackagePath::parse("pkg").unwrap(),
                RootPackageBzlTarget::parse("missing.bzl").unwrap(),
            ),
        )
        .await;
        let SourcePreparationOutcome::Complete(missing) = missing else {
            panic!("complete missing-source observations returned Need");
        };
        assert_eq!(
            missing.as_ref().as_ref().unwrap_err().to_string(),
            "root package source is missing: /root/pkg/missing.bzl"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_projection_rejects_nested_package_and_keeps_need_transient() {
        let roots = ["/root"];
        let mut entries = repository_prelude(&roots, 41);
        entries.extend([
            present("/root/pkg", PathNodeKind::Directory, 41),
            present("/root/pkg/sub", PathNodeKind::Directory, 41),
            present("/root/pkg/sub/BUILD.bazel", PathNodeKind::RegularFile, 41),
        ]);
        let key = RootPackageSourceKey::for_bzl(
            path("/workspace"),
            PackagePath::parse("pkg").unwrap(),
            RootPackageBzlTarget::parse("sub/lib.bzl").unwrap(),
        );
        let crossing = source(inputs(&roots, &[], None), entries, key.clone()).await;
        let SourcePreparationOutcome::Complete(crossing) = crossing else {
            panic!("subpackage marker observations returned Need");
        };
        let error = crossing.as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "label in package //pkg crosses boundary of subpackage //pkg/sub"
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs(&roots, &[], None)).unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        assert!(!RootPackageSourceKey::validity(&need));
        assert!(!RootPackageSourceKey::equality(&need, &need));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_roots_and_missing_markers_are_no_build_file() {
        let outcome = lookup(inputs(&[], &[], None), repository_prelude(&[], 1), "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));

        let roots = ["/root-a"];
        let mut entries = repository_prelude(&roots, 2);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            missing("/root-a/pkg/BUILD.bazel"),
            missing("/root-a/pkg/BUILD"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn deletion_and_external_precede_every_observation() {
        let deleted =
            lookup_without_observations(Some(inputs(&["/root-a"], &["//pkg"], None)), "pkg").await;
        assert!(matches!(
            deleted,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let external =
            lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), "external").await;
        assert!(matches!(
            external,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));

        let deleted_external = lookup_without_observations(
            Some(inputs(&["/root-a"], &["//external"], None)),
            "external",
        )
        .await;
        assert!(matches!(
            deleted_external,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let nonmain_only = lookup(
            inputs(&[], &["@other//pkg"], None),
            repository_prelude(&[], 1),
            "pkg",
        )
        .await;
        assert!(matches!(
            nonmain_only,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_package_name_precedes_every_observation() {
        for (package, expected) in [
            (
                "bad:name",
                r##"Invalid package name 'bad:name': package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##,
            ),
            (
                "...",
                "Invalid package name '...': package name component contains only '.' characters",
            ),
            (
                ".../bad:name",
                r##"Invalid package name '.../bad:name': package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##,
            ),
        ] {
            let invalid =
                lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), package).await;
            let PathOutcome::Complete(value) = invalid else {
                panic!("invalid package name must not request observations");
            };
            let Ok(HostRootPackageLookup::InvalidPackageName { message }) = value.as_ref() else {
                panic!("expected invalid package name, got {value:?}");
            };
            assert_eq!(message.as_ref(), expected);
            assert!(HostRootPackageLookupKey::equality(
                &PathOutcome::Complete(value.dupe()),
                &PathOutcome::Complete(value)
            ));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repo_bazelignore_and_contained_vendor_all_delete_packages() {
        let roots = ["/root-a"];

        let repo = vec![
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 1),
            present("/workspace/REPO.bazel", PathNodeKind::RegularFile, 1),
            bytes(
                "/workspace/REPO.bazel",
                b"ignore_directories(['repo/**'])\n",
            ),
            present("/root-a", PathNodeKind::Directory, 1),
            missing("/root-a/.bazelignore"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), repo, "repo/child").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let bazelignore = vec![
            present("/", PathNodeKind::Directory, 2),
            present("/workspace", PathNodeKind::Directory, 2),
            missing("/workspace/REPO.bazel"),
            present("/root-a", PathNodeKind::Directory, 2),
            present("/root-a/.bazelignore", PathNodeKind::RegularFile, 2),
            bytes("/root-a/.bazelignore", b"ignored\n"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), bazelignore, "ignored/child").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let vendor = repository_prelude(&roots, 3);
        let outcome = lookup(
            inputs(&roots, &[], Some("/root-a/vendor")),
            vendor,
            "vendor/child",
        )
        .await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn special_and_symlink_terminal_markers_are_files() {
        let roots = ["/root-a"];
        let mut special = repository_prelude(&roots, 1);
        special.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::SpecialFile, 1),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), special, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );

        let mut symlink = repository_prelude(&roots, 2);
        symlink.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 2),
            read_link("/root-a/pkg/BUILD.bazel", "/outside/marker"),
            present("/outside", PathNodeKind::Directory, 2),
            present("/outside/marker", PathNodeKind::SpecialFile, 2),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), symlink, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn marker_metadata_is_pruned_from_successful_equality() {
        async fn with_variant(
            variant: i64,
        ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>>
        {
            let roots = ["/root-a"];
            let mut entries = repository_prelude(&roots, variant);
            entries.extend([
                present("/root-a/pkg", PathNodeKind::Directory, variant),
                present(
                    "/root-a/pkg/BUILD.bazel",
                    PathNodeKind::RegularFile,
                    variant,
                ),
            ]);
            lookup(inputs(&roots, &[], None), entries, "pkg").await
        }

        let first = with_variant(1).await;
        let changed_metadata = with_variant(99).await;
        assert!(HostRootPackageLookupKey::equality(
            &first,
            &changed_metadata
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn retained_lifecycle_prunes_metadata_and_replays_create_delete_restore() {
        fn script(marker: Option<i64>, variant: i64) -> Vec<ScriptEntry> {
            let roots = ["/root-a"];
            let mut entries = repository_prelude(&roots, variant);
            entries.push(present("/root-a/pkg", PathNodeKind::Directory, variant));
            match marker {
                Some(marker_variant) => entries.push(present(
                    "/root-a/pkg/BUILD.bazel",
                    PathNodeKind::RegularFile,
                    marker_variant,
                )),
                None => {
                    entries.push(missing("/root-a/pkg/BUILD.bazel"));
                    entries.push(missing("/root-a/pkg/BUILD"));
                }
            }
            entries
        }

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs(&["/root-a"], &[], None)).unwrap();
        let mut transaction = updater.commit().await;
        let lookup =
            HostRootPackageLookupKey::new(path("/workspace"), PackagePath::parse("pkg").unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let counter = LookupCounterKey {
            lookup: lookup.clone(),
            counter: count.dupe(),
        };

        transaction = update_epoch(transaction, &script(None, 1)).await;
        let missing_value = transaction.compute(&lookup).await.unwrap();
        assert!(matches!(
            &missing_value,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(1)
        ));

        transaction = update_epoch(transaction, &script(Some(2), 2)).await;
        let created = transaction.compute(&lookup).await.unwrap();
        assert_eq!(package(&created).package_root(), &path("/root-a"));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(2)
        ));

        transaction = update_epoch(transaction, &script(Some(99), 99)).await;
        let metadata_changed = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(
            &created,
            &metadata_changed
        ));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(2)
        ));
        assert_eq!(count.load(Ordering::SeqCst), 2);

        transaction = update_epoch(transaction, &script(None, 3)).await;
        let deleted = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(&missing_value, &deleted));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(3)
        ));

        transaction = update_epoch(transaction, &script(Some(4), 4)).await;
        let restored = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(&created, &restored));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(4)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn policy_ignore_and_resolution_failures_remain_typed() {
        let missing_input = lookup_without_observations(None, "pkg").await;
        assert!(matches!(
            missing_input,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::PolicyInput(_))
                )
        ));

        let roots = ["/root-a"];
        let ignore_failure = vec![
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 1),
            lstat_error("/workspace/REPO.bazel"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), ignore_failure, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::RepositoryIgnore(_))
                )
        ));

        let mut resolution_failure = repository_prelude(&roots, 2);
        resolution_failure.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            lstat_error("/root-a/pkg/BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), resolution_failure, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::Observation { .. },
                        ..
                    })
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cycle_and_infinite_expansion_failures_remain_discriminating() {
        let roots = ["/root-a"];
        let mut inconsistent = repository_prelude(&roots, 0);
        inconsistent.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 0),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 0),
            missing_read_link("/root-a/pkg/BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), inconsistent, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::InconsistentState { .. },
                        ..
                    })
                )
        ));

        let mut cycle = repository_prelude(&roots, 1);
        cycle.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 1),
            read_link("/root-a/pkg/BUILD.bazel", "BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), cycle, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::Cycle { .. },
                        ..
                    })
                )
        ));

        let mut expansion = repository_prelude(&roots, 2);
        expansion.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 2),
            read_link("/root-a/pkg/BUILD.bazel", "/a"),
            present("/a", PathNodeKind::Symlink, 2),
            read_link("/a", "/a/child"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), expansion, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::InfiniteExpansion { .. },
                        ..
                    })
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn incomplete_observation_is_transient_invalid_and_self_unequal() {
        let outcome =
            lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), "pkg").await;
        assert!(matches!(outcome, PathOutcome::Need(_)));
        assert!(!HostRootPackageLookupKey::validity(&outcome));
        assert!(!HostRootPackageLookupKey::equality(&outcome, &outcome));
    }
}
