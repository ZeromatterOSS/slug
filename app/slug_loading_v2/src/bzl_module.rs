/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::ffi::OsString;
use std::fmt;
use std::hash::Hash;
use std::ops::ControlFlow;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceTransaction;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::RunfilesRepositoryMapping;
use slug_bzlmod_v2::HostRepositorySourceFileValue;
use slug_bzlmod_v2::HostRepositorySourceObservation;
use slug_bzlmod_v2::HostRepositorySourceObservationError;
use slug_bzlmod_v2::HostRepositorySourceReadKey;
use slug_bzlmod_v2::HostRepositorySourceReadObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostRootRepositoryMappingKey;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequests;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsError;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsKey;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionSource;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInput;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequests;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsError;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsKey;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsObservationKey;
use slug_bzlmod_v2::HostSelectedObservationFrontier;
use slug_bzlmod_v2::LogicalSpan;
use slug_bzlmod_v2::RepositoryPackageSource;
use slug_bzlmod_v2::RepositoryPackageSourceAddress;
use slug_bzlmod_v2::RepositoryPackageSourceError;
use slug_bzlmod_v2::RepositoryPackageSourceKey;
use slug_bzlmod_v2::RepositoryPackageSourceObservationKey;
use slug_bzlmod_v2::RepositorySourceFileError;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorError;
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorObservationKey;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::RootPackageBzlTargetError;
use slug_bzlmod_v2::RootPackageSource;
use slug_bzlmod_v2::RootPackageSourceError;
use slug_bzlmod_v2::RootPackageSourceKey;
use slug_bzlmod_v2::RootPackageSourceObservationKey;
use slug_bzlmod_v2::RootRepositoryBzlLoadRoute;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_relative_path;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::StringEncoding;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::HostRootRepositoryLoadRouteError;
use crate::HostRootRepositoryLoadRouteKey;
use crate::HostRootRepositoryLoadRouteObservationKey;
use crate::bzl_visibility::BzlLoadVisibility;
use crate::bzl_visibility::BzlLoadVisibilityError;
use crate::bzl_visibility::validate_bzl_load_visibility;
use crate::cycle_detector::BzlLoadCycle;
use crate::cycle_detector::BzlLoadCycleGuard;
use crate::cycle_detector::ExternalBzlLoadCycle;
use crate::cycle_detector::ExternalBzlLoadCycleGuard;
use crate::cycle_detector::HostBzlLoadCycle;
use crate::cycle_detector::HostBzlLoadCycleGuard;
use crate::glob::PackageListing;
use crate::host_glob::HostGlobBoundaryScope;
use crate::host_glob::HostGlobLoadingRequest;
use crate::host_glob::HostGlobPrepared;
use crate::host_glob::HostGlobRequestInputError;
use crate::host_glob::compute_host_glob_request;
use crate::keys::BzlModuleEvalKey;
use crate::keys::BzlParseKey;
use crate::keys::LoadLabelResolutionKey;
use crate::keys::PackageListingKey;
use crate::keys::PackageLoadKey;
use crate::keys::WorkspaceDirectoryEntryKind;
use crate::keys::WorkspaceDirectoryKey;
use crate::keys::WorkspaceDirectoryValue;
use crate::keys::WorkspaceFileKey;
use crate::keys::WorkspaceFileValue;
use crate::load_label::LoadLabel;
use crate::package::FrozenModuleExtensionDefinition;
use crate::package::HostGlobAttemptControl;
use crate::package::HostGlobAttemptError;
use crate::package::LegacyLoadedPackage;
use crate::package::LoadedPackage;
use crate::package::ModuleExtensionDefinitionProjection;
use crate::package::ModuleExtensionTagCoercionError;
use crate::package::PackageRecorder;
use crate::package::PackageTargetKind;
use crate::package::build_file_loading_globals;
use crate::package::bzlmod_loading_globals;
use crate::package::loading_globals;
use crate::package::prepare_module_extension_tag_attributes;
use crate::package::validate_module_extension_tag_schema;
use crate::provider::BzlEvaluationContext;
use crate::visibility::RuleVisibility;
use crate::visibility::VisibilitySource;

/// Local-root loading operations over a caller-owned DICE transaction.
///
/// This intentionally owns no DICE instance or asynchronous runtime. The
/// workspace runtime supplies one committed transaction containing all file
/// observations for root and package loading. Loading-capable transactions
/// must install [`crate::bzl_load_cycle_detector`] in their
/// `UserComputationData`; the modern DICE engine does not detect key cycles
/// itself.
#[derive(Clone)]
pub struct BzlModuleEvaluator {
    workspace: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBzlModule {
    pub path: PathBuf,
    pub loads: Vec<String>,
    pub manifest: BzlLoadManifest,
}

#[derive(Debug, Default)]
pub(crate) struct LoadingPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl LoadingPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }

    pub(crate) fn drain_batch(&self) -> EventBatch {
        EventBatch::from_events(self.events.borrow_mut().drain(..).collect::<Vec<_>>())
    }
}

impl PrintHandler for LoadingPrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }
}

/// Stable identity for one `.bzl` module evaluated through a logical source path.
///
/// `label` is the canonical repository label and `workspace_path` is the
/// normalized absolute path DICE evaluated. `repository_mapping` is the
/// defining module's already-selected apparent-to-canonical mapping. The value
/// intentionally carries no evaluator handle, so it can be used at semantic
/// equality boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlModuleIdentity {
    pub label: CanonicalLabel,
    pub workspace_path: PathBuf,
    pub repository_mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
}

/// Flat, immutable loading provenance for one evaluated `.bzl` root.
///
/// Direct children retain source order with label-first deduplication.
/// `reachable` is a label-first, first-seen closure and is
/// deliberately flat: consumers do not need to walk an `Arc` DAG to compare
/// the loading semantics.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BzlLoadManifest {
    pub root: BzlModuleIdentity,
    pub direct_children: Arc<[BzlModuleIdentity]>,
    pub reachable: Arc<[BzlModuleIdentity]>,
    pub fingerprint: [u8; 32],
}

/// Parse-independent discovery result for a package's active BUILD basename.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BuildFileCompanion {
    pub label: CanonicalLabel,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[doc(hidden)]
pub struct LoadingError {
    message: String,
    absent: bool,
    #[allocative(skip)]
    cycle: Option<BzlLoadCycle>,
}

impl LoadingError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            absent: false,
            cycle: None,
        }
    }

    fn absent(path: &Path) -> Self {
        Self {
            message: format!("workspace file is absent: {}", path.display()),
            absent: true,
            cycle: None,
        }
    }

    fn is_absent(&self) -> bool {
        self.absent
    }

    fn load_cycle(cycle: BzlLoadCycle) -> Self {
        Self {
            message: "cycle detected in extension files".to_owned(),
            absent: false,
            cycle: Some(cycle),
        }
    }

    fn with_package_cycle_origin(&self, workspace: &Path, package: &Path) -> Self {
        let Some(cycle) = &self.cycle else {
            return self.clone();
        };
        let package = package
            .strip_prefix(workspace)
            .unwrap_or(package)
            .to_string_lossy()
            .replace('\\', "/");
        let origin = if package.is_empty() {
            "BUILD".to_owned()
        } else {
            format!("{package}/BUILD")
        };
        let path = cycle
            .path
            .iter()
            .map(|key| {
                bzl_source_label(workspace, &key.path)
                    .unwrap_or_else(|_| key.path.display().to_string())
            })
            .collect::<Vec<_>>();
        let keys = cycle
            .keys
            .iter()
            .map(|key| {
                bzl_source_label(workspace, &key.path)
                    .unwrap_or_else(|_| key.path.display().to_string())
            })
            .collect::<Vec<_>>();
        Self::new(render_bzl_cycle(&origin, &path, &keys))
    }
}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadingError {}

fn load_error(load_label: &str, error: &LoadingError) -> LoadingError {
    if error.is_absent() {
        LoadingError::new(format!("cannot load '{load_label}': no such file"))
    } else {
        error.clone()
    }
}

fn render_bzl_cycle(origin: &str, path: &[String], cycle: &[String]) -> String {
    // Bazel 9 source of truth:
    // BzlLoadCycleReporter + AbstractLabelCycleReporter::printCycle.
    let mut message = format!("cycle detected in extension files: \n    {origin}");
    for label in path {
        message.push_str("\n    ");
        message.push_str(label);
    }
    let Some((first, rest)) = cycle.split_first() else {
        return message;
    };
    message.push_str("\n.-> ");
    message.push_str(first);
    if rest.is_empty() {
        message.push_str(" [self-edge]\n`--");
        return message;
    }
    for label in rest {
        message.push_str("\n|   ");
        message.push_str(label);
    }
    message.push_str("\n`-- ");
    message.push_str(first);
    message
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct BzlLoadCyclePoisonKey;

impl fmt::Display for BzlLoadCyclePoisonKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bzl-load-cycle-poison")
    }
}

#[async_trait]
impl Key for BzlLoadCyclePoisonKey {
    type Value = ();

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }

    fn validity(_value: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for PackageListingKey {
    type Value = LoadResult<PackageListing>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if !self.workspace.is_absolute()
            || !self.package.is_absolute()
            || !self.package.starts_with(&self.workspace)
            || self.package.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Arc::new(Err(LoadingError::new(format!(
                "package listing key requires a normalized absolute package inside workspace {}: {}",
                self.workspace.display(),
                self.package.display()
            ))));
        }

        let mut regular_files = Vec::new();
        let mut directories = Vec::new();
        let mut watched_directories = Vec::new();
        let mut subpackages = Vec::new();
        let mut pending = vec![(self.package.clone(), String::new())];

        while let Some((directory, relative_directory)) = pending.pop() {
            let value = match ctx
                .compute(&WorkspaceDirectoryKey {
                    workspace: self.workspace.clone(),
                    directory: directory.clone(),
                })
                .await
            {
                Ok(value) => value,
                Err(error) => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "reading package directory {} through DICE: {error}",
                        directory.display()
                    ))));
                }
            };
            let entries = match value {
                WorkspaceDirectoryValue::Present(entries) => entries,
                WorkspaceDirectoryValue::Absent => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "package directory is absent: {}",
                        directory.display()
                    ))));
                }
                WorkspaceDirectoryValue::ReadError(error) => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "reading package directory {}: {error}",
                        directory.display()
                    ))));
                }
            };

            watched_directories.push(CompactString::new(&relative_directory));
            let is_subpackage = !relative_directory.is_empty()
                && entries.iter().any(|entry| {
                    matches!(entry.name.as_str(), "BUILD.bazel" | "BUILD")
                        && entry.kind == WorkspaceDirectoryEntryKind::RegularFile
                });
            if is_subpackage {
                subpackages.push(CompactString::new(&relative_directory));
                continue;
            }

            if !relative_directory.is_empty() {
                directories.push(CompactString::new(&relative_directory));
            }
            for entry in entries.iter().rev() {
                let relative = if relative_directory.is_empty() {
                    entry.name.to_string()
                } else {
                    format!("{relative_directory}/{}", entry.name)
                };
                match entry.kind {
                    WorkspaceDirectoryEntryKind::RegularFile => {
                        regular_files.push(CompactString::new(&relative));
                    }
                    WorkspaceDirectoryEntryKind::Directory => {
                        pending.push((directory.join(entry.name.as_str()), relative));
                    }
                    WorkspaceDirectoryEntryKind::Symlink => {
                        return Arc::new(Err(LoadingError::new(format!(
                            "symlink entries are unsupported while listing package {}: {relative}",
                            self.package.display()
                        ))));
                    }
                    WorkspaceDirectoryEntryKind::Other => {
                        return Arc::new(Err(LoadingError::new(format!(
                            "special filesystem entries are unsupported while listing package {}: {relative}",
                            self.package.display()
                        ))));
                    }
                }
            }
        }

        Arc::new(Ok(PackageListing::new(
            regular_files,
            directories,
            watched_directories,
            subpackages,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x.as_ref(), y.as_ref()), (Ok(x), Ok(y)) if x == y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

async fn observed_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    path: &Path,
) -> Result<Arc<String>, LoadingError> {
    let value = ctx
        .compute(&WorkspaceFileKey {
            workspace: workspace.to_path_buf(),
            path: path.to_path_buf(),
        })
        .await
        .map_err(|error| {
            LoadingError::new(format!("reading {} through DICE: {error}", path.display()))
        })?;
    match value {
        WorkspaceFileValue::Present(source) => Ok(source.clone()),
        WorkspaceFileValue::Absent => Err(LoadingError::absent(path)),
        WorkspaceFileValue::ReadError(error) => Err(LoadingError::new(format!(
            "reading {}: {error}",
            path.display()
        ))),
    }
}

#[derive(Allocative, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct ParsedBzl {
    source: String,
    loads: Vec<String>,
    source_digest: [u8; 32],
}

#[derive(Debug, Allocative, Clone)]
#[doc(hidden)]
pub struct FrozenBzlModule {
    #[allocative(skip)]
    pub(crate) module: FrozenModule,
    path: PathBuf,
    loads: Vec<String>,
    pub(crate) manifest: BzlLoadManifest,
    pub(crate) bzl_load_visibility: BzlLoadVisibility,
    /// Kept separately from `manifest`: frozen module pointers are lifetime
    /// ownership, never semantic equality.
    retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
}

/// Lifetime-only pairing kept structurally aligned with its identity.
#[derive(Debug, Allocative, Clone)]
pub(crate) struct FrozenBzlLifetimeEntry {
    identity: BzlModuleIdentity,
    #[allocative(skip)]
    module: FrozenModule,
}

impl PartialEq for FrozenBzlModule {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest && self.bzl_load_visibility == other.bzl_load_visibility
    }
}

impl Eq for FrozenBzlModule {}

impl FrozenBzlModule {
    fn lifetime_modules(&self) -> impl Iterator<Item = (&BzlModuleIdentity, FrozenModule)> {
        std::iter::once((&self.manifest.root, self.module.dupe())).chain(
            self.retained_bzl_modules
                .iter()
                .map(|entry| (&entry.identity, entry.module.dupe())),
        )
    }
}

fn validate_direct_bzl_load_visibilities(
    importer: &PackageIdentifier,
    loaded_modules: &[(String, FrozenBzlModule)],
) -> Result<(), BzlLoadVisibilityError> {
    for (_, module) in loaded_modules {
        validate_bzl_load_visibility(
            importer,
            &module.manifest.root.label,
            &module.bzl_load_visibility,
        )?;
    }
    Ok(())
}

impl BzlLoadManifest {
    fn new(
        root: BzlModuleIdentity,
        source_digest: [u8; 32],
        direct_modules: impl IntoIterator<Item = impl std::borrow::Borrow<FrozenBzlModule>>,
    ) -> Self {
        let direct_modules = direct_modules.into_iter().collect::<Vec<_>>();
        let mut direct_indices = Vec::with_capacity(direct_modules.len());
        let mut direct_children = Vec::with_capacity(direct_modules.len());
        let mut direct_seen_labels = SmallSet::with_capacity(direct_modules.len());
        for (index, module) in direct_modules.iter().enumerate() {
            let identity = &module.borrow().manifest.root;
            if direct_seen_labels.insert(identity.label.clone()) {
                direct_children.push(identity.clone());
                direct_indices.push(index);
            }
        }
        let mut reachable = vec![root.clone()];
        let mut seen_labels = SmallSet::with_capacity(direct_children.len() + 1);
        seen_labels.insert(root.label.clone());
        for module in &direct_modules {
            for identity in module.borrow().manifest.reachable.iter() {
                if seen_labels.insert(identity.label.clone()) {
                    reachable.push(identity.clone());
                }
            }
        }
        let fingerprint = manifest_fingerprint(
            source_digest,
            direct_indices
                .iter()
                .map(|index| direct_modules[*index].borrow()),
        );
        Self {
            root,
            direct_children: direct_children.into(),
            reachable: reachable.into(),
            fingerprint,
        }
    }
}

type LoadResult<T> = Arc<Result<T, LoadingError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)] // Private dormant owner; activated only by a future Host package key.
pub(crate) enum HostPackageAttemptError {
    Loading(LoadingError),
    Glob(HostGlobAttemptError),
    Input(HostGlobRequestInputError),
    Invariant(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
struct HostPackageAttemptTerminal {
    result: Result<LoadedPackage, HostPackageAttemptError>,
    event_batch: EventBatch,
}

#[allow(dead_code)]
type HostPackageAttemptOutcome = SourcePreparationOutcome<Arc<HostPackageAttemptTerminal>>;

type HostPackageAttemptDriverOutcome = SourcePreparationOutcome<
    Result<(Arc<HostPackageAttemptTerminal>, PathObservationEpoch), ObservedPathFrontierError>,
>;

#[derive(Clone, Copy)]
#[allow(dead_code)] // Private observed sibling is callerless until a later cutover packet.
enum HostPackageLoadMode {
    Legacy,
    Observed,
}

#[cfg(test)]
struct ForceRootPackageObservationOuter(ObservedPathFrontierError);

#[allow(dead_code)]
struct HostPackageAttemptInput<'a> {
    workspace: NormalizedAbsolutePath,
    logical_package_root: NormalizedAbsolutePath,
    package: PackagePath,
    package_identifier: PackageIdentifier,
    package_dir: PathBuf,
    build_file: PathBuf,
    source: Arc<String>,
    package_label: CompactString,
    loaded_modules: &'a [(String, FrozenBzlModule)],
    capture_events: bool,
}

#[allow(dead_code)]
enum HostPackageAttemptStep {
    Pending {
        request: HostGlobLoadingRequest,
        event_batch: EventBatch,
    },
    Terminal(HostPackageAttemptTerminal),
}

#[allow(dead_code)]
fn host_package_terminal(
    result: Result<LoadedPackage, HostPackageAttemptError>,
    event_batch: EventBatch,
) -> HostPackageAttemptStep {
    HostPackageAttemptStep::Terminal(HostPackageAttemptTerminal {
        result,
        event_batch,
    })
}

fn host_package_attempt_source_name(input: &HostPackageAttemptInput<'_>) -> String {
    if input.package_identifier.repo().is_root() {
        return input.build_file.display().to_string();
    }
    format!(
        "{}//{}:{}",
        input.package_identifier.repo(),
        input.package,
        input
            .build_file
            .file_name()
            .expect("repository BUILD file has a basename")
            .to_string_lossy()
    )
}

#[allow(dead_code)]
fn evaluate_host_package_attempt(
    input: &HostPackageAttemptInput<'_>,
    prepared: Arc<SmallMap<HostGlobLoadingRequest, HostGlobPrepared>>,
    repository_mapping: Arc<RunfilesRepositoryMapping>,
) -> HostPackageAttemptStep {
    let source_name = host_package_attempt_source_name(input);
    let ast = match AstModule::parse_with_string_encoding(
        &source_name,
        input.source.as_ref().clone(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(error) => {
            return host_package_terminal(
                Err(HostPackageAttemptError::Loading(LoadingError::new(
                    error.to_string(),
                ))),
                EventBatch::empty(),
            );
        }
    };
    if let Err(error) =
        validate_direct_bzl_load_visibilities(&input.package_identifier, input.loaded_modules)
    {
        return host_package_terminal(
            Err(HostPackageAttemptError::Loading(LoadingError::new(
                error.to_string(),
            ))),
            EventBatch::empty(),
        );
    }
    let print_capture = input
        .capture_events
        .then(|| Rc::new(LoadingPrintCapture::default()));
    let recorder = PackageRecorder::new_host(
        prepared,
        input.package_identifier.clone(),
        repository_mapping,
    )
    .with_print_capture(print_capture.clone());
    let module = Module::new();
    let loader = LocalBzlLoader {
        modules: input
            .loaded_modules
            .iter()
            .map(|(load, module)| (load.as_str(), module.module.dupe()))
            .collect(),
    };
    let globals = build_file_loading_globals();
    let evaluation = {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&recorder);
        evaluator.set_loader(&loader);
        if let Some(print_capture) = print_capture.as_deref() {
            evaluator.set_print_handler(print_capture);
        }
        evaluator.eval_module(ast, &globals).map(|_| ())
    };
    let event_batch = print_capture
        .as_deref()
        .map(LoadingPrintCapture::drain_batch)
        .unwrap_or_else(EventBatch::empty);
    let control = recorder.take_host_glob_control();

    match (evaluation, control) {
        (Ok(()), None) => {
            let direct_load_roots = first_seen_direct_roots(input.loaded_modules);
            let retained_bzl_modules = flattened_lifetime_closure(input.loaded_modules);
            let reachable_loads = retained_bzl_modules
                .iter()
                .map(|entry| entry.identity.clone())
                .collect::<Vec<_>>();
            let load_fingerprint = package_load_fingerprint(input.loaded_modules);
            host_package_terminal(
                Ok(recorder.finish(
                    input.package_dir.clone(),
                    input.build_file.clone(),
                    direct_load_roots.into(),
                    reachable_loads.into(),
                    load_fingerprint,
                    retained_bzl_modules.into(),
                )),
                event_batch,
            )
        }
        (Ok(()), Some(_)) => host_package_terminal(
            Err(HostPackageAttemptError::Invariant(
                "successful package attempt retained Host glob control",
            )),
            event_batch,
        ),
        (Err(error), Some(control)) => {
            if !PackageRecorder::is_host_glob_control_error(&error) {
                return host_package_terminal(
                    Err(HostPackageAttemptError::Invariant(
                        "Host glob control accompanied a different evaluator error",
                    )),
                    event_batch,
                );
            }
            match control {
                HostGlobAttemptControl::Pending(request) => HostPackageAttemptStep::Pending {
                    request,
                    event_batch,
                },
                HostGlobAttemptControl::Terminal(error) => {
                    host_package_terminal(Err(HostPackageAttemptError::Glob(error)), event_batch)
                }
            }
        }
        (Err(error), None) => {
            let error = if PackageRecorder::is_host_glob_control_error(&error) {
                HostPackageAttemptError::Invariant(
                    "Host glob control error had no attempt-local control",
                )
            } else {
                HostPackageAttemptError::Loading(LoadingError::new(error.to_string()))
            };
            host_package_terminal(Err(error), event_batch)
        }
    }
}

#[allow(dead_code)]
async fn evaluate_host_package_attempts_driver(
    ctx: &mut DiceComputations<'_>,
    input: HostPackageAttemptInput<'_>,
    mode: HostPackageLoadMode,
    repository_mapping: Arc<RunfilesRepositoryMapping>,
    boundary_scope: HostGlobBoundaryScope,
) -> HostPackageAttemptDriverOutcome {
    let mut prepared = Arc::new(SmallMap::new());
    let mut observations = PathObservationEpoch::empty();
    loop {
        // The synchronous attempt returns only compact terminal state or one
        // request, so no evaluator/module/recorder borrow can cross this await.
        match evaluate_host_package_attempt(&input, prepared.dupe(), repository_mapping.dupe()) {
            HostPackageAttemptStep::Terminal(terminal) => {
                return SourcePreparationOutcome::Complete(Ok((Arc::new(terminal), observations)));
            }
            HostPackageAttemptStep::Pending {
                request,
                event_batch,
            } => {
                let outcome = compute_host_glob_request(
                    ctx,
                    input.workspace.dupe(),
                    boundary_scope.clone(),
                    input.logical_package_root.dupe(),
                    input.package.clone(),
                    request.dupe(),
                    matches!(mode, HostPackageLoadMode::Observed),
                )
                .await;
                let outcome = match outcome {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Ok((
                            Arc::new(HostPackageAttemptTerminal {
                                result: Err(HostPackageAttemptError::Input(error)),
                                event_batch,
                            }),
                            observations,
                        )));
                    }
                };
                match outcome {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(Err(error)) => {
                        return SourcePreparationOutcome::Complete(Err(error));
                    }
                    SourcePreparationOutcome::Complete(Ok((value, incoming))) => {
                        observations =
                            match merge_root_package_observations(mode, observations, &incoming) {
                                Ok(observations) => observations,
                                Err(error) => {
                                    return SourcePreparationOutcome::Complete(Err(error));
                                }
                            };
                        let replaced = Arc::make_mut(&mut prepared).insert(request, value);
                        debug_assert!(replaced.is_none());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
async fn evaluate_host_package_attempts(
    ctx: &mut DiceComputations<'_>,
    input: HostPackageAttemptInput<'_>,
) -> HostPackageAttemptOutcome {
    match evaluate_host_package_attempts_driver(
        ctx,
        input,
        HostPackageLoadMode::Legacy,
        Arc::new(RunfilesRepositoryMapping::empty()),
        HostGlobBoundaryScope::Root,
    )
    .await
    {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((terminal, observations))) => {
            debug_assert!(observations.observations().is_empty());
            SourcePreparationOutcome::Complete(terminal)
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            panic!("legacy Host package attempt produced frontier error: {error}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRootBzlLabel {
    package: PackagePath,
    target: RootPackageBzlTarget,
}

impl HostRootBzlLabel {
    pub(crate) fn new(package: PackagePath, target: RootPackageBzlTarget) -> Self {
        Self { package, target }
    }

    fn path_fragment(&self) -> String {
        if self.package.as_str().is_empty() {
            self.target.to_string()
        } else {
            format!("{}/{}", self.package, self.target)
        }
    }

    fn canonical_label(&self) -> CanonicalLabel {
        CanonicalLabel::parse(&format!("@@//{}:{}", self.package, self.target))
            .expect("validated root .bzl identity is a canonical label")
    }
}

impl fmt::Display for HostRootBzlLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "//{}:{}", self.package, self.target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostLoadLabelError {
    Invalid {
        load: Arc<str>,
        message: Arc<str>,
    },
    Target {
        load: Arc<str>,
        error: RootPackageBzlTargetError,
    },
    UnsupportedExternalRepository {
        load: Arc<str>,
    },
    CanonicalExternalRepository {
        load: Arc<str>,
    },
    ExternalPackage {
        load: Arc<str>,
    },
}

impl fmt::Display for HostLoadLabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { load, message } => {
                write!(f, "invalid load label `{load}`: {message}")
            }
            Self::Target { load, error } => {
                write!(f, "invalid load label `{load}`: {error}")
            }
            Self::UnsupportedExternalRepository { load } => write!(
                f,
                "external repository load is not available in the root Host loader: {load}"
            ),
            Self::CanonicalExternalRepository { load } => write!(
                f,
                "canonical nonroot repository load is deferred in a root BUILD file: {load}"
            ),
            Self::ExternalPackage { load } => {
                write!(
                    f,
                    "Starlark files may not be loaded from //external: {load}"
                )
            }
        }
    }
}

impl std::error::Error for HostLoadLabelError {}

fn resolve_host_load_label(
    requesting_package: &PackagePath,
    load: &str,
) -> Result<HostRootBzlLabel, HostLoadLabelError> {
    let (package, target) = if let Some(target) = load.strip_prefix(':') {
        (
            requesting_package.clone(),
            RootPackageBzlTarget::parse(target).map_err(|error| HostLoadLabelError::Target {
                load: Arc::from(load),
                error,
            })?,
        )
    } else {
        if load.starts_with("@@") && !load.starts_with("@@//") {
            return Err(HostLoadLabelError::CanonicalExternalRepository {
                load: Arc::from(load),
            });
        }
        if load.starts_with('@') && !load.starts_with("@@//") && !load.starts_with("@//") {
            return Err(HostLoadLabelError::UnsupportedExternalRepository {
                load: Arc::from(load),
            });
        }
        let apparent = if let Some(root) = load.strip_prefix("@@//") {
            format!("//{root}")
        } else {
            load.to_owned()
        };
        let label =
            ApparentLabel::parse(&apparent).map_err(|message| HostLoadLabelError::Invalid {
                load: Arc::from(load),
                message: Arc::from(message),
            })?;
        if !label.repo().is_root() {
            return Err(HostLoadLabelError::UnsupportedExternalRepository {
                load: Arc::from(load),
            });
        }
        (
            label.package().clone(),
            RootPackageBzlTarget::parse(label.target().as_str()).map_err(|error| {
                HostLoadLabelError::Target {
                    load: Arc::from(load),
                    error,
                }
            })?,
        )
    };
    if package.as_str() == "external" {
        return Err(HostLoadLabelError::ExternalPackage {
            load: Arc::from(load),
        });
    }
    Ok(HostRootBzlLabel::new(package, target))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct RepositoryBzlLabel {
    package: PackagePath,
    target: RootPackageBzlTarget,
}

impl RepositoryBzlLabel {
    pub(crate) fn new(
        package: PackagePath,
        target: RootPackageBzlTarget,
    ) -> Result<Self, ExternalLoadLabelError> {
        if target.raw_bytes().contains(&b'/') {
            return Err(ExternalLoadLabelError::SlashTarget {
                target: target.to_string().into(),
            });
        }
        Ok(Self { package, target })
    }

    fn canonical_label(&self, route: &HostRepositorySourceRoute) -> CanonicalLabel {
        CanonicalLabel::parse(&format!(
            "{}//{}:{}",
            route.canonical_repo(),
            self.package,
            self.target
        ))
        .expect("typed external bzl label is canonical")
    }

    fn repository_relative_path(&self) -> PathBuf {
        let target = repository_bzl_target_os_string(&self.target);
        let mut path = PathBuf::from(self.package.as_str());
        path.push(target);
        path
    }
}

#[cfg(unix)]
fn repository_bzl_target_os_string(target: &RootPackageBzlTarget) -> OsString {
    OsString::from_vec(target.raw_bytes().to_vec())
}

#[cfg(not(unix))]
fn repository_bzl_target_os_string(target: &RootPackageBzlTarget) -> OsString {
    target
        .raw_bytes()
        .iter()
        .copied()
        .map(char::from)
        .collect::<String>()
        .into()
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum ExternalLoadLabelError {
    Invalid {
        load: Arc<str>,
        message: Arc<str>,
    },
    Target {
        load: Arc<str>,
        error: RootPackageBzlTargetError,
    },
    Repository {
        load: Arc<str>,
    },
    CrossPackage {
        load: Arc<str>,
        requesting_package: PackagePath,
        loaded_package: PackagePath,
    },
    SlashTarget {
        target: Arc<str>,
    },
}

impl fmt::Display for ExternalLoadLabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { load, message } => {
                write!(f, "invalid external load label `{load}`: {message}")
            }
            Self::Target { load, error } => {
                write!(f, "invalid external load label `{load}`: {error}")
            }
            Self::Repository { load } => {
                write!(f, "repository-qualified external load is deferred: {load}")
            }
            Self::CrossPackage {
                load,
                requesting_package,
                loaded_package,
            } => write!(
                f,
                "cross-package external load is deferred: {load} ({requesting_package} -> {loaded_package})"
            ),
            Self::SlashTarget { target } => write!(
                f,
                "external load target must be a direct file in its package: {target}"
            ),
        }
    }
}

fn resolve_external_load_label(
    requesting_package: &PackagePath,
    load: &str,
) -> Result<RepositoryBzlLabel, ExternalLoadLabelError> {
    if load.starts_with('@') {
        return Err(ExternalLoadLabelError::Repository {
            load: Arc::from(load),
        });
    }
    let (package, target) = if let Some(target) = load.strip_prefix(':') {
        (
            requesting_package.clone(),
            RootPackageBzlTarget::parse(target).map_err(|error| {
                ExternalLoadLabelError::Target {
                    load: Arc::from(load),
                    error,
                }
            })?,
        )
    } else {
        let label =
            ApparentLabel::parse(load).map_err(|message| ExternalLoadLabelError::Invalid {
                load: Arc::from(load),
                message: Arc::from(message),
            })?;
        if !label.repo().is_root() {
            return Err(ExternalLoadLabelError::Repository {
                load: Arc::from(load),
            });
        }
        if label.package() != requesting_package {
            return Err(ExternalLoadLabelError::CrossPackage {
                load: Arc::from(load),
                requesting_package: requesting_package.clone(),
                loaded_package: label.package().clone(),
            });
        }
        (
            label.package().clone(),
            RootPackageBzlTarget::parse(label.target().as_str()).map_err(|error| {
                ExternalLoadLabelError::Target {
                    load: Arc::from(load),
                    error,
                }
            })?,
        )
    };
    RepositoryBzlLabel::new(package, target)
}

#[derive(Debug, Clone)]
struct ResolvedExternalBzlLoad {
    route: HostRepositorySourceRoute,
    label: RepositoryBzlLabel,
}

enum ExternalBzlChildLoad {
    Canonical(String),
    Resolved(String, ResolvedExternalBzlLoad),
}

fn resolve_external_bzl_load_label(
    route: &RootRepositoryRoute,
    package: &PackagePath,
    load: &str,
) -> Result<(RootRepositoryBzlLoadRoute, RepositoryBzlLabel), ExternalLoadLabelError> {
    if !matches!(
        route.source(),
        slug_bzlmod_v2::RootRepositorySource::SelectedRegistry(_)
    ) {
        return resolve_external_load_label(package, load)
            .map(|label| (RootRepositoryBzlLoadRoute::Root(route.clone()), label));
    }
    if let Some(target) = load.strip_prefix(':') {
        return RepositoryBzlLabel::new(
            package.clone(),
            RootPackageBzlTarget::parse(target).map_err(|error| {
                ExternalLoadLabelError::Target {
                    load: Arc::from(load),
                    error,
                }
            })?,
        )
        .map(|label| (RootRepositoryBzlLoadRoute::Root(route.clone()), label));
    }
    let label = ApparentLabel::parse(load).map_err(|message| ExternalLoadLabelError::Invalid {
        load: Arc::from(load),
        message: Arc::from(message),
    })?;
    let child_route = if label.repo().is_root() {
        RootRepositoryBzlLoadRoute::Root(route.clone())
    } else {
        route.selected_bzl_load_route(label.repo()).ok_or_else(|| {
            ExternalLoadLabelError::Repository {
                load: Arc::from(load),
            }
        })?
    };
    RepositoryBzlLabel::new(
        label.package().clone(),
        RootPackageBzlTarget::parse(label.target().as_str()).map_err(|error| {
            ExternalLoadLabelError::Target {
                load: Arc::from(load),
                error,
            }
        })?,
    )
    .map(|label| (child_route, label))
}

pub(crate) fn resolve_canonical_external_bzl_load_label(
    route: &slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
    package: &PackagePath,
    load: &str,
) -> Result<(Option<CanonicalRepoName>, RepositoryBzlLabel), ExternalLoadLabelError> {
    if let Some(target) = load.strip_prefix(':') {
        return RepositoryBzlLabel::new(
            package.clone(),
            RootPackageBzlTarget::parse(target).map_err(|error| {
                ExternalLoadLabelError::Target {
                    load: Arc::from(load),
                    error,
                }
            })?,
        )
        .map(|label| (None, label));
    }
    let apparent =
        ApparentLabel::parse(load).map_err(|message| ExternalLoadLabelError::Invalid {
            load: Arc::from(load),
            message: Arc::from(message),
        })?;
    let child = if apparent.repo().is_root() {
        None
    } else {
        Some(
            route
                .view()
                .route()
                .mapping_target(apparent.repo())
                .cloned()
                .ok_or_else(|| ExternalLoadLabelError::Repository {
                    load: Arc::from(load),
                })?,
        )
    };
    RepositoryBzlLabel::new(
        apparent.package().clone(),
        RootPackageBzlTarget::parse(apparent.target().as_str()).map_err(|error| {
            ExternalLoadLabelError::Target {
                load: Arc::from(load),
                error,
            }
        })?,
    )
    .map(|label| (child, label))
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostSourceInputError {
    UnsupportedSourceEncoding {
        logical_path: NormalizedAbsolutePath,
    },
    #[cfg(not(unix))]
    UnsupportedPathEncoding {
        logical_path: NormalizedAbsolutePath,
    },
}

impl fmt::Display for HostSourceInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSourceEncoding { logical_path } => write!(
                f,
                "Host Starlark source is not valid UTF-8: {}",
                logical_path.as_path().display()
            ),
            #[cfg(not(unix))]
            Self::UnsupportedPathEncoding { logical_path } => write!(
                f,
                "Host Starlark source path is not representable without loss: {}",
                logical_path.as_path().display()
            ),
        }
    }
}

fn host_source_text(source: &RootPackageSource) -> Result<Arc<String>, HostSourceInputError> {
    String::from_utf8(source.bytes().to_vec())
        .map(Arc::new)
        .map_err(|_| HostSourceInputError::UnsupportedSourceEncoding {
            logical_path: source.logical_path().dupe(),
        })
}

fn host_source_name(source: &RootPackageSource) -> Result<String, HostSourceInputError> {
    #[cfg(unix)]
    {
        Ok(starlark_source_name(source.logical_path().as_path()).expect("Unix paths are accepted"))
    }
    #[cfg(not(unix))]
    {
        starlark_source_name(source.logical_path().as_path()).ok_or_else(|| {
            HostSourceInputError::UnsupportedPathEncoding {
                logical_path: source.logical_path().dupe(),
            }
        })
    }
}

pub(crate) fn starlark_source_name(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        Some(
            path.as_os_str()
                .as_bytes()
                .iter()
                .copied()
                .map(char::from)
                .collect(),
        )
    }
    #[cfg(not(unix))]
    {
        path.to_str().map(str::to_owned)
    }
}

pub(crate) fn manifest_starlark_sources(
    manifest: &BzlLoadManifest,
) -> Arc<[(CompactString, BzlModuleIdentity)]> {
    manifest
        .reachable
        .iter()
        .map(|identity| {
            let source = starlark_source_name(&identity.workspace_path)
                .expect("manifest paths were accepted as Starlark source names");
            (source.into(), identity.clone())
        })
        .collect::<Vec<_>>()
        .into()
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostBzlModuleError {
    Source(RootPackageSourceError),
    Input(HostSourceInputError),
    Parse {
        label: HostRootBzlLabel,
        message: Arc<str>,
    },
    LoadLabel {
        source: HostRootBzlLabel,
        error: HostLoadLabelError,
    },
    Child {
        load: Arc<str>,
        label: HostRootBzlLabel,
        error: Arc<HostBzlModuleError>,
    },
    Cycle(HostBzlLoadCycle),
    Evaluation(LoadingError),
    Freeze {
        label: HostRootBzlLabel,
        message: Arc<str>,
    },
}

impl fmt::Display for HostBzlModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Input(error) => error.fmt(f),
            Self::Parse { label, message } => write!(
                f,
                "parsing {label}: {message}\ncompilation of module '{}' failed",
                label.path_fragment()
            ),
            Self::LoadLabel { source, error } => {
                write!(f, "resolving a load in {source}: {error}")
            }
            Self::Child { load, error, .. } => {
                write!(f, "loading `{load}`: {error}")
            }
            Self::Cycle(_) => f.write_str("cycle detected in extension files"),
            Self::Evaluation(error) => error.fmt(f),
            Self::Freeze { label, message } => write!(f, "freezing {label}: {message}"),
        }
    }
}

impl std::error::Error for HostBzlModuleError {}

#[allow(dead_code)]
impl HostBzlModuleError {
    fn cycle(&self) -> Option<&HostBzlLoadCycle> {
        match self {
            Self::Cycle(cycle) => Some(cycle),
            Self::Child { error, .. } => error.cycle(),
            Self::Source(_)
            | Self::Input(_)
            | Self::Parse { .. }
            | Self::LoadLabel { .. }
            | Self::Evaluation(_)
            | Self::Freeze { .. } => None,
        }
    }

    fn missing_label<'a>(
        &'a self,
        current_label: &'a HostRootBzlLabel,
    ) -> Option<&'a HostRootBzlLabel> {
        match self {
            Self::Source(error) if error.is_missing() => Some(current_label),
            Self::Child { label, error, .. } => error.missing_label(label),
            Self::Source(_) => None,
            Self::Input(_)
            | Self::Parse { .. }
            | Self::LoadLabel { .. }
            | Self::Cycle(_)
            | Self::Evaluation(_)
            | Self::Freeze { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
enum BzlModuleContext {
    Build,
    Bzlmod,
}

impl BzlModuleContext {
    fn globals(self) -> starlark::environment::Globals {
        match self {
            Self::Build => loading_globals(),
            Self::Bzlmod => bzlmod_loading_globals(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostBzlModuleEvalKey {
    workspace: NormalizedAbsolutePath,
    label: HostRootBzlLabel,
    context: BzlModuleContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostBzlCycleIdentity {
    workspace: NormalizedAbsolutePath,
    label: HostRootBzlLabel,
    context: BzlModuleContext,
}

impl HostBzlCycleIdentity {
    fn source_key(&self) -> RootPackageSourceObservationKey {
        RootPackageSourceObservationKey::for_bzl(
            self.workspace.dupe(),
            self.label.package.clone(),
            self.label.target.dupe(),
        )
    }
}

impl HostBzlModuleEvalKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self::with_context(workspace, label, BzlModuleContext::Build)
    }

    pub(crate) fn new_bzlmod(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self::with_context(workspace, label, BzlModuleContext::Bzlmod)
    }

    fn with_context(
        workspace: NormalizedAbsolutePath,
        label: HostRootBzlLabel,
        context: BzlModuleContext,
    ) -> Self {
        Self {
            workspace,
            label,
            context,
        }
    }

    pub(crate) fn cycle_identity(&self) -> HostBzlCycleIdentity {
        HostBzlCycleIdentity {
            workspace: self.workspace.dupe(),
            label: self.label.clone(),
            context: self.context,
        }
    }
}

impl fmt::Display for HostBzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.context {
            BzlModuleContext::Build => {
                write!(f, "host-bzl-module:{}:{}", self.workspace, self.label)
            }
            BzlModuleContext::Bzlmod => {
                write!(
                    f,
                    "bzlmod-host-bzl-module:{}:{}",
                    self.workspace, self.label
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostBzlModule {
    result: Arc<Result<FrozenBzlModule, HostBzlModuleError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostBzlModule {
    pub(crate) fn result(&self) -> &Result<FrozenBzlModule, HostBzlModuleError> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostBzlModuleObservationKey {
    workspace: NormalizedAbsolutePath,
    label: HostRootBzlLabel,
    context: BzlModuleContext,
}

impl HostBzlModuleObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self::with_context(workspace, label, BzlModuleContext::Build)
    }

    pub(crate) fn new_bzlmod(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self::with_context(workspace, label, BzlModuleContext::Bzlmod)
    }

    fn with_context(
        workspace: NormalizedAbsolutePath,
        label: HostRootBzlLabel,
        context: BzlModuleContext,
    ) -> Self {
        Self {
            workspace,
            label,
            context,
        }
    }

    pub(crate) fn cycle_identity(&self) -> HostBzlCycleIdentity {
        HostBzlCycleIdentity {
            workspace: self.workspace.dupe(),
            label: self.label.clone(),
            context: self.context,
        }
    }
}

impl fmt::Display for HostBzlModuleObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.context {
            BzlModuleContext::Build => {
                write!(
                    f,
                    "observed-host-bzl-module:{}:{}",
                    self.workspace, self.label
                )
            }
            BzlModuleContext::Bzlmod => write!(
                f,
                "observed-bzlmod-host-bzl-module:{}:{}",
                self.workspace, self.label
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ExternalBzlModuleEvalKey {
    route: HostRepositorySourceRoute,
    label: RepositoryBzlLabel,
    context: BzlModuleContext,
}

impl ExternalBzlModuleEvalKey {
    pub(crate) fn new(route: RootRepositoryRoute, label: RepositoryBzlLabel) -> Self {
        Self::from_source_route(
            HostRepositorySourceRoute::root(route),
            label,
            BzlModuleContext::Build,
        )
    }
    pub(crate) fn new_bzlmod(route: RootRepositoryRoute, label: RepositoryBzlLabel) -> Self {
        Self::from_source_route(
            HostRepositorySourceRoute::root(route),
            label,
            BzlModuleContext::Bzlmod,
        )
    }
    #[allow(dead_code)] // Direct canonical entry is exercised by the packet proof surface.
    pub(crate) fn new_canonical(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        label: RepositoryBzlLabel,
    ) -> Self {
        Self::from_source_route(
            HostRepositorySourceRoute::canonical(input),
            label,
            BzlModuleContext::Build,
        )
    }
    pub(crate) fn new_canonical_bzlmod(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        label: RepositoryBzlLabel,
    ) -> Self {
        Self::from_source_route(
            HostRepositorySourceRoute::canonical(input),
            label,
            BzlModuleContext::Bzlmod,
        )
    }
    fn from_source_route(
        route: HostRepositorySourceRoute,
        label: RepositoryBzlLabel,
        context: BzlModuleContext,
    ) -> Self {
        Self {
            route,
            label,
            context,
        }
    }
    fn canonical_label(&self) -> CanonicalLabel {
        self.label.canonical_label(&self.route)
    }
    pub(crate) fn cycle_identity(&self) -> ExternalBzlCycleIdentity {
        ExternalBzlCycleIdentity {
            route: self.route.clone(),
            label: self.label.clone(),
            context: self.context,
        }
    }
}
impl fmt::Display for ExternalBzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.context {
            BzlModuleContext::Build => {
                write!(f, "external-bzl-module:{}", self.canonical_label())
            }
            BzlModuleContext::Bzlmod => {
                write!(f, "bzlmod-external-bzl-module:{}", self.canonical_label())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ExternalBzlCycleIdentity {
    route: HostRepositorySourceRoute,
    label: RepositoryBzlLabel,
    context: BzlModuleContext,
}
impl ExternalBzlCycleIdentity {
    pub(crate) fn canonical_label(&self) -> CanonicalLabel {
        self.label.canonical_label(&self.route)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ExternalBzlModuleObservationKey(ExternalBzlModuleEvalKey);
impl ExternalBzlModuleObservationKey {
    pub(crate) fn new(route: RootRepositoryRoute, label: RepositoryBzlLabel) -> Self {
        Self(ExternalBzlModuleEvalKey::new(route, label))
    }
    pub(crate) fn new_bzlmod(route: RootRepositoryRoute, label: RepositoryBzlLabel) -> Self {
        Self(ExternalBzlModuleEvalKey::new_bzlmod(route, label))
    }
    #[allow(dead_code)] // Direct canonical entry is exercised by the packet proof surface.
    pub(crate) fn new_canonical(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        label: RepositoryBzlLabel,
    ) -> Self {
        Self(ExternalBzlModuleEvalKey::new_canonical(input, label))
    }
    pub(crate) fn new_canonical_bzlmod(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        label: RepositoryBzlLabel,
    ) -> Self {
        Self(ExternalBzlModuleEvalKey::new_canonical_bzlmod(input, label))
    }
    fn from_source_route(
        route: HostRepositorySourceRoute,
        label: RepositoryBzlLabel,
        context: BzlModuleContext,
    ) -> Self {
        Self(ExternalBzlModuleEvalKey::from_source_route(
            route, label, context,
        ))
    }
    pub(crate) fn cycle_identity(&self) -> ExternalBzlCycleIdentity {
        self.0.cycle_identity()
    }
}
impl fmt::Display for ExternalBzlModuleObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedExternalBzlModule {
    result: Arc<Result<FrozenBzlModule, ExternalBzlModuleError>>,
    observations: PathObservationEpoch,
}

impl ObservedExternalBzlModule {
    pub(crate) fn result(&self) -> &Arc<Result<FrozenBzlModule, ExternalBzlModuleError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum ExternalBzlModuleError {
    SourceCompute {
        label: CanonicalLabel,
        message: Arc<str>,
    },
    Source {
        label: CanonicalLabel,
        error: RepositorySourceFileError,
    },
    SourceObservation {
        label: CanonicalLabel,
        error: HostRepositorySourceObservationError,
    },
    Route {
        source: CanonicalLabel,
        load: Arc<str>,
        message: Arc<str>,
    },
    Absent {
        label: CanonicalLabel,
    },
    Encoding {
        label: CanonicalLabel,
    },
    Parse {
        label: CanonicalLabel,
        message: Arc<str>,
    },
    LoadLabel {
        source: CanonicalLabel,
        error: ExternalLoadLabelError,
    },
    Child {
        raw_load: Arc<str>,
        canonical_label: CanonicalLabel,
        error: Arc<ExternalBzlModuleError>,
    },
    Cycle(ExternalBzlLoadCycle),
    Evaluation {
        label: CanonicalLabel,
        message: Arc<str>,
    },
    Freeze {
        label: CanonicalLabel,
        message: Arc<str>,
    },
}

impl fmt::Display for ExternalBzlModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceCompute { label, message } => {
                write!(f, "computing source for {label}: {message}")
            }
            Self::Source { label, error } => write!(f, "reading {label}: {error:?}"),
            Self::SourceObservation { label, error } => write!(f, "reading {label}: {error}"),
            Self::Route {
                source,
                load,
                message,
            } => write!(f, "resolving `{load}` from {source}: {message}"),
            Self::Absent { label } => write!(f, "cannot load '{label}': no such file"),
            Self::Encoding { label } => {
                write!(
                    f,
                    "external Starlark source bytes or parser path have unsupported encoding: {label}"
                )
            }
            Self::Parse { label, message } => write!(f, "parsing {label}: {message}"),
            Self::LoadLabel { source, error } => {
                write!(f, "resolving a load in {source}: {error}")
            }
            Self::Child {
                raw_load, error, ..
            } => write!(f, "loading `{raw_load}`: {error}"),
            Self::Cycle(_) => f.write_str("cycle detected in extension files"),
            Self::Evaluation { label, message } => write!(f, "evaluating {label}: {message}"),
            Self::Freeze { label, message } => write!(f, "freezing {label}: {message}"),
        }
    }
}

impl std::error::Error for ExternalBzlModuleError {}

impl ExternalBzlModuleError {
    pub(crate) fn cycle(&self) -> Option<&ExternalBzlLoadCycle> {
        match self {
            Self::Cycle(cycle) => Some(cycle),
            Self::Child { error, .. } => error.cycle(),
            Self::SourceCompute { .. }
            | Self::Source { .. }
            | Self::SourceObservation { .. }
            | Self::Route { .. }
            | Self::Absent { .. }
            | Self::Encoding { .. }
            | Self::Parse { .. }
            | Self::LoadLabel { .. }
            | Self::Evaluation { .. }
            | Self::Freeze { .. } => None,
        }
    }

    fn missing_label(&self) -> Option<&CanonicalLabel> {
        match self {
            Self::Absent { label } => Some(label),
            Self::Child { error, .. } => error.missing_label(),
            Self::SourceCompute { .. }
            | Self::Source { .. }
            | Self::SourceObservation { .. }
            | Self::Route { .. }
            | Self::Encoding { .. }
            | Self::Parse { .. }
            | Self::LoadLabel { .. }
            | Self::Cycle(_)
            | Self::Evaluation { .. }
            | Self::Freeze { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootPackageLoadErrorInner {
    RootModule(RootModuleLoadingAnchorError),
    Source(RootPackageSourceError),
    Input(HostSourceInputError),
    Parse {
        package: PackagePath,
        message: Arc<str>,
    },
    LoadLabel {
        package: PackagePath,
        error: HostLoadLabelError,
    },
    RepositoryMapping {
        message: Arc<str>,
    },
    RepositoryMappingInfrastructure {
        message: Arc<str>,
    },
    Bzl {
        origin: Arc<str>,
        load: Arc<str>,
        label: HostRootBzlLabel,
        error: Arc<HostBzlModuleError>,
    },
    ExternalRoute {
        load: Arc<str>,
        apparent_repo: slug_identity_v2::ApparentRepoName,
        error: HostRootRepositoryLoadRouteError,
    },
    ExternalInfrastructure {
        load: Arc<str>,
        message: Arc<str>,
    },
    ExternalBzl {
        origin: Arc<str>,
        load: Arc<str>,
        label: CanonicalLabel,
        error: Arc<ExternalBzlModuleError>,
    },
    Attempt(HostPackageAttemptError),
}

/// Terminal root-package loading failure.
///
/// The concrete loading stages remain private so downstream typed callers
/// cannot couple themselves to Host implementation details.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootPackageLoadError {
    inner: RootPackageLoadErrorInner,
}

impl RootPackageLoadError {
    fn new(inner: RootPackageLoadErrorInner) -> Self {
        Self { inner }
    }
}

impl fmt::Display for RootPackageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            RootPackageLoadErrorInner::RootModule(error) => error.fmt(f),
            RootPackageLoadErrorInner::Source(error) => error.fmt(f),
            RootPackageLoadErrorInner::Input(error) => error.fmt(f),
            RootPackageLoadErrorInner::Parse { package, message } => {
                write!(f, "parsing BUILD file for //{package}: {message}")
            }
            RootPackageLoadErrorInner::LoadLabel { package, error } => {
                write!(f, "resolving a load in //{package}: {error}")
            }
            RootPackageLoadErrorInner::RepositoryMapping { message } => {
                write!(f, "resolving the root repository mapping: {message}")
            }
            RootPackageLoadErrorInner::RepositoryMappingInfrastructure { message } => {
                write!(f, "computing the root repository mapping: {message}")
            }
            RootPackageLoadErrorInner::Bzl {
                origin,
                load,
                label,
                error,
            } => {
                if let Some(cycle) = error.cycle() {
                    let path = cycle
                        .path
                        .iter()
                        .map(|key| key.label.to_string())
                        .collect::<Vec<_>>();
                    let keys = cycle
                        .keys
                        .iter()
                        .map(|key| key.label.to_string())
                        .collect::<Vec<_>>();
                    f.write_str(&render_bzl_cycle(origin, &path, &keys))
                } else if let Some(missing) = error.missing_label(label) {
                    write!(f, "cannot load '{missing}': no such file")
                } else {
                    write!(f, "loading `{load}`: {error}")
                }
            }
            RootPackageLoadErrorInner::ExternalRoute { error, .. } => error.fmt(f),
            RootPackageLoadErrorInner::ExternalInfrastructure { message, .. } => {
                f.write_str(message)
            }
            RootPackageLoadErrorInner::ExternalBzl { load, error, .. } => {
                write!(f, "loading `{load}`: {error}")
            }
            RootPackageLoadErrorInner::Attempt(error) => write!(f, "{error:?}"),
        }
    }
}

enum RootPackageDirectLoad {
    Root(HostRootBzlLabel),
    External {
        apparent_repo: slug_identity_v2::ApparentRepoName,
        label: RepositoryBzlLabel,
    },
}

fn resolve_root_package_direct_load(
    requesting_package: &PackagePath,
    load: &str,
) -> Result<RootPackageDirectLoad, HostLoadLabelError> {
    if !load.starts_with('@') || load.starts_with("@@//") {
        return resolve_host_load_label(requesting_package, load).map(RootPackageDirectLoad::Root);
    }
    if load.starts_with("@@") {
        return Err(HostLoadLabelError::CanonicalExternalRepository {
            load: Arc::from(load),
        });
    }
    let label = ApparentLabel::parse(load).map_err(|message| HostLoadLabelError::Invalid {
        load: Arc::from(load),
        message: Arc::from(message),
    })?;
    if label.repo().is_root() {
        return resolve_host_load_label(requesting_package, load).map(RootPackageDirectLoad::Root);
    }
    let target = RootPackageBzlTarget::parse(label.target().as_str()).map_err(|error| {
        HostLoadLabelError::Target {
            load: Arc::from(load),
            error,
        }
    })?;
    let label = RepositoryBzlLabel::new(label.package().clone(), target).map_err(|error| {
        HostLoadLabelError::Invalid {
            load: Arc::from(load),
            message: Arc::from(error.to_string()),
        }
    })?;
    Ok(RootPackageDirectLoad::External {
        apparent_repo: label_for_root_build_repo(load)?,
        label,
    })
}

fn label_for_root_build_repo(
    load: &str,
) -> Result<slug_identity_v2::ApparentRepoName, HostLoadLabelError> {
    ApparentLabel::parse(load)
        .map_err(|message| HostLoadLabelError::Invalid {
            load: Arc::from(load),
            message: Arc::from(message),
        })
        .map(|label| label.repo().clone())
}

impl std::error::Error for RootPackageLoadError {}

/// DICE identity for loading one root-repository package through Host inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootPackageLoadKey {
    workspace: NormalizedAbsolutePath,
    package: PackagePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling is callerless until a later cutover packet.
#[doc(hidden)]
pub struct RootPackageLoadObservationKey(RootPackageLoadKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed key.
#[doc(hidden)]
pub struct ObservedRootPackageLoad {
    result: Arc<Result<LoadedPackage, RootPackageLoadError>>,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedRootPackageLoad {
    #[doc(hidden)]
    pub fn result(&self) -> &Arc<Result<LoadedPackage, RootPackageLoadError>> {
        &self.result
    }

    #[doc(hidden)]
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RepositoryPackageLoadErrorInner {
    Source {
        error: RepositoryPackageSourceError,
    },
    SourceCompute {
        canonical_repo: CompactString,
        package: PackagePath,
        message: Arc<str>,
    },
    Encoding {
        path: PathBuf,
    },
    Parse {
        canonical_repo: CompactString,
        package: PackagePath,
        message: Arc<str>,
    },
    #[allow(dead_code)] // Retained for exact typed root-route error compatibility.
    LoadLabel {
        canonical_repo: CompactString,
        package: PackagePath,
        error: ExternalLoadLabelError,
    },
    Bzl {
        origin: Arc<str>,
        raw_load: Arc<str>,
        canonical_label: CanonicalLabel,
        error: Arc<ExternalBzlModuleError>,
    },
    LoadedTargetKind {
        canonical_repo: CompactString,
        package: PackagePath,
        target: Arc<str>,
        kind: Arc<str>,
    },
    LoadedStarlarkRule {
        canonical_repo: CompactString,
        package: PackagePath,
        target: Arc<str>,
        reason: LoadedStarlarkRuleReason,
    },
    GlobSourceRoot {
        canonical_repo: CompactString,
        package: PackagePath,
        build_file: PathBuf,
    },
    Attempt(HostPackageAttemptError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum LoadedStarlarkRuleReason {
    AdditionalTargets(usize),
    Visibility,
    Test,
    Executable,
    OrdinaryDependencies,
    SchemaValues,
    ReachableLabels(Arc<str>),
}

impl fmt::Display for LoadedStarlarkRuleReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdditionalTargets(count) => write!(f, "package contains {count} targets"),
            Self::Visibility => f.write_str("visibility is not explicitly public"),
            Self::Test => f.write_str("test rules are deferred"),
            Self::Executable => f.write_str("executable rules are deferred"),
            Self::OrdinaryDependencies => f.write_str("ordinary dependencies are deferred"),
            Self::SchemaValues => f.write_str("schema/value relationship is malformed"),
            Self::ReachableLabels(attribute) => {
                write!(f, "attribute `{attribute}` contains a reachable label")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryPackageLoadError {
    inner: RepositoryPackageLoadErrorInner,
}

impl RepositoryPackageLoadError {
    fn new(inner: RepositoryPackageLoadErrorInner) -> Self {
        Self { inner }
    }

    pub fn is_unsupported_feature(&self) -> bool {
        matches!(
            &self.inner,
            RepositoryPackageLoadErrorInner::Source { error }
                if error.is_unsupported_feature()
        )
    }
}

impl fmt::Display for RepositoryPackageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            RepositoryPackageLoadErrorInner::Source { error } => error.fmt(f),
            RepositoryPackageLoadErrorInner::SourceCompute {
                canonical_repo,
                package,
                message,
            } => write!(
                f,
                "computing external repository BUILD source for @@{canonical_repo}//{package}: {message}"
            ),
            RepositoryPackageLoadErrorInner::Encoding { path } => {
                write!(
                    f,
                    "external repository BUILD file is not UTF-8: {}",
                    path.display()
                )
            }
            RepositoryPackageLoadErrorInner::Parse {
                canonical_repo,
                package,
                message,
            } => write!(
                f,
                "parsing BUILD file for @@{canonical_repo}//{package}: {message}"
            ),
            RepositoryPackageLoadErrorInner::LoadLabel {
                canonical_repo,
                package,
                error,
            } => write!(
                f,
                "resolving a load in @@{canonical_repo}//{package}: {error}"
            ),
            RepositoryPackageLoadErrorInner::Bzl {
                origin,
                raw_load,
                canonical_label: _,
                error,
            } => {
                if let Some(cycle) = error.cycle() {
                    let path = cycle
                        .path
                        .iter()
                        .map(|key| key.canonical_label().to_string())
                        .collect::<Vec<_>>();
                    let keys = cycle
                        .keys
                        .iter()
                        .map(|key| key.canonical_label().to_string())
                        .collect::<Vec<_>>();
                    f.write_str(&render_bzl_cycle(origin, &path, &keys))
                } else if let Some(missing) = error.missing_label() {
                    write!(f, "cannot load '{missing}': no such file")
                } else {
                    write!(f, "loading `{raw_load}`: {error}")
                }
            }
            RepositoryPackageLoadErrorInner::LoadedTargetKind {
                canonical_repo,
                package,
                target,
                kind,
            } => write!(
                f,
                "loaded external package @@{canonical_repo}//{package} produced unsupported target `{target}` of kind {kind}"
            ),
            RepositoryPackageLoadErrorInner::LoadedStarlarkRule {
                canonical_repo,
                package,
                target,
                reason,
            } => write!(
                f,
                "loaded external Starlark rule @@{canonical_repo}//{package}:{target} is deferred: {reason}"
            ),
            RepositoryPackageLoadErrorInner::GlobSourceRoot {
                canonical_repo,
                package,
                build_file,
            } => write!(
                f,
                "external repository BUILD source does not identify the materialized root for @@{canonical_repo}//{package}: {}",
                build_file.display()
            ),
            RepositoryPackageLoadErrorInner::Attempt(error) => write!(f, "{error:?}"),
        }
    }
}

impl std::error::Error for RepositoryPackageLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            RepositoryPackageLoadErrorInner::Source { error } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryPackageInventoryKey {
    route: HostRepositorySourceRoute,
    package: PackagePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct RepositoryPackageInventoryObservationKey(RepositoryPackageInventoryKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedRepositoryPackageInventory {
    result: Arc<Result<LoadedPackage, RepositoryPackageLoadError>>,
    observations: PathObservationEpoch,
}

impl ObservedRepositoryPackageInventory {
    pub(crate) fn result(&self) -> &Arc<Result<LoadedPackage, RepositoryPackageLoadError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryPackageLoadKey {
    inventory: RepositoryPackageInventoryKey,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryPackageLoadObservationKey(RepositoryPackageLoadKey);

impl RepositoryPackageLoadObservationKey {
    pub fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self(RepositoryPackageLoadKey::new(route, package))
    }

    pub fn new_canonical(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        package: PackagePath,
    ) -> Self {
        Self(RepositoryPackageLoadKey::new_canonical(input, package))
    }
}

impl fmt::Display for RepositoryPackageLoadObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedRepositoryPackageLoad {
    result: Arc<Result<LoadedPackage, RepositoryPackageLoadError>>,
    observations: PathObservationEpoch,
}

impl ObservedRepositoryPackageLoad {
    pub fn result(&self) -> &Arc<Result<LoadedPackage, RepositoryPackageLoadError>> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

impl RepositoryPackageLoadKey {
    pub fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self {
            inventory: RepositoryPackageInventoryKey::new(
                HostRepositorySourceRoute::root(route),
                package,
            ),
        }
    }

    pub fn new_canonical(
        input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        package: PackagePath,
    ) -> Self {
        Self {
            inventory: RepositoryPackageInventoryKey::new(
                HostRepositorySourceRoute::canonical(input),
                package,
            ),
        }
    }
}

impl RepositoryPackageInventoryKey {
    pub(crate) fn new(route: HostRepositorySourceRoute, package: PackagePath) -> Self {
        Self { route, package }
    }
}

impl RepositoryPackageInventoryObservationKey {
    pub(crate) fn new(route: HostRepositorySourceRoute, package: PackagePath) -> Self {
        Self(RepositoryPackageInventoryKey::new(route, package))
    }
}

impl std::hash::Hash for RepositoryPackageInventoryKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.package.hash(state);
    }
}

impl fmt::Display for RepositoryPackageLoadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-package-load:{}//{}",
            self.inventory.route.canonical_repo(),
            self.inventory.package
        )
    }
}

impl fmt::Display for RepositoryPackageInventoryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-package-inventory:{}//{}",
            self.route.canonical_repo(),
            self.package
        )
    }
}

impl fmt::Display for RepositoryPackageInventoryObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

impl RootPackageLoadKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

#[allow(dead_code)]
impl RootPackageLoadObservationKey {
    #[doc(hidden)]
    pub fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self(RootPackageLoadKey::new(workspace, package))
    }
}

impl fmt::Display for RootPackageLoadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-package-load:{}//{}", self.workspace, self.package)
    }
}

impl fmt::Display for RootPackageLoadObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[track_caller]
fn host_dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host loading DICE invariant failed: {error:?}"))
}

type HostBzlModuleCarrier = Arc<Result<FrozenBzlModule, HostBzlModuleError>>;
type HostBzlModuleProjection = (HostBzlModuleCarrier, PathObservationEpoch);
type HostBzlModuleDriverOutcome =
    SourcePreparationOutcome<Result<HostBzlModuleProjection, ObservedPathFrontierError>>;

#[derive(Clone, Copy)]
enum HostBzlModuleMode {
    Legacy,
    Observed,
}

#[cfg(test)]
struct ForceHostBzlFreezeFailure;

fn host_bzl_complete(
    result: Result<FrozenBzlModule, HostBzlModuleError>,
    observations: PathObservationEpoch,
) -> HostBzlModuleDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn union_host_observations(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                right
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(ObservedPathFrontierError::from)
}

async fn complete_observed_host_bzl_cycle(
    ctx: &mut DiceComputations<'_>,
    current: &HostBzlCycleIdentity,
    cycle: &HostBzlLoadCycle,
    mut observations: PathObservationEpoch,
) -> HostBzlModuleDriverOutcome {
    let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
    if !cycle.keys.is_empty() {
        let current_index = cycle
            .keys
            .iter()
            .position(|identity| identity == current)
            .expect("detected Host bzl cycle contains the current observed module");
        for offset in 1..cycle.keys.len() {
            let identity = &cycle.keys[(current_index + offset) % cycle.keys.len()];
            let source = match host_dice_invariant(ctx.compute(&identity.source_key()).await) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(source)) => source,
            };
            observations = match union_host_observations(&observations, source.observations()) {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
            if let Err(error) = source.result() {
                return host_bzl_complete(
                    Err(HostBzlModuleError::Source(error.clone())),
                    observations,
                );
            }
        }
    }
    host_bzl_complete(Err(HostBzlModuleError::Cycle(cycle.clone())), observations)
}

async fn compute_host_bzl_module(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: &HostRootBzlLabel,
    context: BzlModuleContext,
    mode: HostBzlModuleMode,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> HostBzlModuleDriverOutcome {
    let source = match mode {
        HostBzlModuleMode::Legacy => match host_dice_invariant(
            ctx.compute(&RootPackageSourceKey::for_bzl(
                workspace.dupe(),
                label.package.clone(),
                label.target.dupe(),
            ))
            .await,
        ) {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => SourcePreparationOutcome::Complete(Ok((
                result.as_ref().clone(),
                PathObservationEpoch::empty(),
            ))),
        },
        HostBzlModuleMode::Observed => match host_dice_invariant(
            ctx.compute(&RootPackageSourceObservationKey::for_bzl(
                workspace.dupe(),
                label.package.clone(),
                label.target.dupe(),
            ))
            .await,
        ) {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok(source)) => SourcePreparationOutcome::Complete(
                Ok((source.result().clone(), source.observations().dupe())),
            ),
        },
    };
    let (source, observations) = match source {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    let source = match source {
        Ok(source) => source,
        Err(error) => {
            return host_bzl_complete(Err(HostBzlModuleError::Source(error)), observations);
        }
    };
    let source_text = match host_source_text(&source) {
        Ok(source) => source,
        Err(error) => {
            return host_bzl_complete(Err(HostBzlModuleError::Input(error)), observations);
        }
    };
    let source_name = match host_source_name(&source) {
        Ok(name) => name,
        Err(error) => {
            return host_bzl_complete(Err(HostBzlModuleError::Input(error)), observations);
        }
    };
    let ast = match AstModule::parse_with_string_encoding(
        &source_name,
        source_text.as_ref().clone(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(error) => {
            return host_bzl_complete(
                Err(HostBzlModuleError::Parse {
                    label: label.clone(),
                    message: Arc::from(error.to_string()),
                }),
                observations,
            );
        }
    };
    let loads = ast
        .loads()
        .into_iter()
        .map(|load| load.module_id.to_owned())
        .collect::<Vec<_>>();
    let mut loaded_modules = Vec::with_capacity(loads.len());
    let mut observations = observations;
    for load in &loads {
        let child_label = match resolve_host_load_label(&label.package, load) {
            Ok(label) => label,
            Err(error) => {
                return host_bzl_complete(
                    Err(HostBzlModuleError::LoadLabel {
                        source: label.clone(),
                        error,
                    }),
                    observations,
                );
            }
        };
        let guard = host_dice_invariant(ctx.cycle_guard::<HostBzlLoadCycleGuard>())
            .expect("Host bzl loading requires the request cycle detector");
        let child = match mode {
            HostBzlModuleMode::Legacy => {
                let child = HostBzlModuleEvalKey::with_context(
                    workspace.dupe(),
                    child_label.clone(),
                    context,
                );
                match guard.guard_this(ctx.compute(&child)).await {
                    Ok(result) => host_dice_invariant(result)
                        .map(|value| Ok((value.as_ref().clone(), PathObservationEpoch::empty()))),
                    Err(cycle) => {
                        let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                        return host_bzl_complete(
                            Err(HostBzlModuleError::Cycle(cycle)),
                            observations,
                        );
                    }
                }
            }
            HostBzlModuleMode::Observed => {
                let child = HostBzlModuleObservationKey::with_context(
                    workspace.dupe(),
                    child_label.clone(),
                    context,
                );
                match guard.guard_this(ctx.compute(&child)).await {
                    Ok(result) => match host_dice_invariant(result) {
                        SourcePreparationOutcome::Need(need) => {
                            SourcePreparationOutcome::Need(need)
                        }
                        SourcePreparationOutcome::Complete(Err(error)) => {
                            SourcePreparationOutcome::Complete(Err(error))
                        }
                        SourcePreparationOutcome::Complete(Ok(value)) => {
                            SourcePreparationOutcome::Complete(Ok((
                                value.result().clone(),
                                value.observations().dupe(),
                            )))
                        }
                    },
                    Err(cycle) => {
                        return complete_observed_host_bzl_cycle(
                            ctx,
                            &HostBzlCycleIdentity {
                                workspace: workspace.dupe(),
                                label: label.clone(),
                                context,
                            },
                            &cycle,
                            observations,
                        )
                        .await;
                    }
                }
            }
        };
        let (child, incoming) = match child {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok(value)) => value,
        };
        observations = match mode {
            HostBzlModuleMode::Legacy => {
                debug_assert!(incoming.observations().is_empty());
                observations
            }
            HostBzlModuleMode::Observed => {
                match union_host_observations(&observations, &incoming) {
                    Ok(observations) => observations,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                }
            }
        };
        let module = match child {
            Ok(module) => module,
            Err(error) => {
                return host_bzl_complete(
                    Err(HostBzlModuleError::Child {
                        load: Arc::from(load.as_str()),
                        label: child_label,
                        error: Arc::new(error),
                    }),
                    observations,
                );
            }
        };
        loaded_modules.push((load.clone(), module));
    }

    let module = Module::new();
    let manifest = BzlLoadManifest::new(
        BzlModuleIdentity {
            label: label.canonical_label(),
            workspace_path: source.logical_path().as_path().to_path_buf(),
            repository_mapping: Arc::from([]),
        },
        digest(source_text.as_str()),
        loaded_modules.iter().map(|(_, module)| module),
    );
    if let Err(error) =
        validate_direct_bzl_load_visibilities(manifest.root.label.package(), &loaded_modules)
    {
        return host_bzl_complete(
            Err(HostBzlModuleError::Evaluation(LoadingError::new(
                error.to_string(),
            ))),
            observations,
        );
    }
    let loader = LocalBzlLoader {
        modules: loaded_modules
            .iter()
            .map(|(load, module)| (load.as_str(), module.module.dupe()))
            .collect(),
    };
    let evaluation_context = BzlEvaluationContext::from_manifest(&manifest);
    let print_capture = capture_events.then(LoadingPrintCapture::default);
    let globals = context.globals();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&evaluation_context);
        evaluator.set_loader(&loader);
        if let Some(print_capture) = print_capture.as_ref() {
            evaluator.set_print_handler(print_capture);
        }
        let evaluation = evaluator.eval_module(ast, &globals).map(|_| ());
        drop(evaluator);
        *event_batch = print_capture.map(LoadingPrintCapture::into_batch);
        if let Err(error) = evaluation {
            return host_bzl_complete(
                Err(HostBzlModuleError::Evaluation(LoadingError::new(
                    error.to_string(),
                ))),
                observations,
            );
        }
    }
    #[cfg(test)]
    if ctx
        .per_transaction_data()
        .data
        .get::<ForceHostBzlFreezeFailure>()
        .is_ok()
    {
        return host_bzl_complete(
            Err(HostBzlModuleError::Freeze {
                label: label.clone(),
                message: Arc::from("forced test freeze failure"),
            }),
            observations,
        );
    }
    let module = match module.freeze() {
        Ok(module) => module,
        Err(error) => {
            return host_bzl_complete(
                Err(HostBzlModuleError::Freeze {
                    label: label.clone(),
                    message: Arc::from(format!("{error:?}")),
                }),
                observations,
            );
        }
    };
    host_bzl_complete(
        Ok(FrozenBzlModule {
            module,
            path: source.logical_path().as_path().to_path_buf(),
            loads,
            retained_bzl_modules: retained_module_closure(&loaded_modules),
            bzl_load_visibility: evaluation_context.bzl_load_visibility(),
            manifest,
        }),
        observations,
    )
}

fn stores_host_bzl_event_batch(value: &HostBzlModuleDriverOutcome) -> bool {
    matches!(value, SourcePreparationOutcome::Complete(Ok(_)))
}

#[async_trait]
impl Key for HostBzlModuleEvalKey {
    type Value = SourcePreparationOutcome<HostBzlModuleCarrier>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_host_bzl_module(
            ctx,
            &self.workspace,
            &self.label,
            self.context,
            HostBzlModuleMode::Legacy,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_host_bzl_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("HostBzlModuleEvalKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy Host bzl module produced frontier error: {error}")
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
impl Key for HostBzlModuleObservationKey {
    type Value = SourcePreparationOutcome<Result<ObservedHostBzlModule, ObservedPathFrontierError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_host_bzl_module(
            ctx,
            &self.workspace,
            &self.label,
            self.context,
            HostBzlModuleMode::Observed,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_host_bzl_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("HostBzlModuleObservationKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostBzlModule {
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)] // Callerless owner; activated only by a future extension evaluator.
pub(crate) struct HostLoadedModuleExtensionDefinition {
    request: HostSelectedExtensionDefinitionLoadRequest,
    pub(crate) manifest: BzlLoadManifest,
    pub(crate) definition: ModuleExtensionDefinitionProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)] // Callerless owner; activated only by a future extension evaluator.
pub(crate) struct HostLoadedModuleExtensionDefinitions {
    requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
    pub(crate) definitions: Arc<[HostLoadedModuleExtensionDefinition]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)] // Callerless owner; activated only by a future extension evaluator.
pub(crate) enum HostLoadedModuleExtensionDefinitionsError {
    Requests(HostSelectedExtensionDefinitionLoadRequestsError),
    RequestsCompute(CompactString),
    Request {
        requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
        request: HostSelectedExtensionDefinitionLoadRequest,
        error: HostLoadedModuleExtensionDefinitionError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostLoadedModuleExtensionDefinitionError {
    Label {
        label: CanonicalLabel,
        message: CompactString,
    },
    Bzl {
        label: CanonicalLabel,
        error: HostBzlModuleError,
    },
    SelectedBzl {
        label: CanonicalLabel,
        error: ExternalBzlModuleError,
    },
    Export {
        label: CanonicalLabel,
        name: CompactString,
        message: CompactString,
    },
    WrongKind {
        label: CanonicalLabel,
        name: CompactString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum DefinitionBzlModuleCarrier {
    Root(HostBzlModuleCarrier),
    Selected(ExternalBzlModuleCarrier),
}

impl fmt::Display for HostLoadedModuleExtensionDefinitionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostLoadedModuleExtensionDefinitionsError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Callerless owner; activated only by a future extension evaluator.
pub(crate) struct HostLoadedModuleExtensionDefinitionsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostLoadedModuleExtensionDefinitionsKey {
    #[allow(dead_code)]
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostLoadedModuleExtensionDefinitionsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-loaded-module-extension-definitions:{}",
            self.workspace
        )
    }
}

#[allow(dead_code)]
type HostLoadedModuleExtensionDefinitionsOutcome = SourcePreparationOutcome<
    Arc<Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>>,
>;

#[async_trait]
impl Key for HostLoadedModuleExtensionDefinitionsKey {
    type Value = HostLoadedModuleExtensionDefinitionsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_loaded_extension_definitions(
            drive_loaded_extension_definitions(
                ctx,
                self,
                LoadedModuleExtensionDefinitionsMode::Legacy,
            )
            .await,
        )
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling; a later packet owns consumer activation.
struct HostLoadedModuleExtensionDefinitionsObservationKey(HostLoadedModuleExtensionDefinitionsKey);

impl HostLoadedModuleExtensionDefinitionsObservationKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostLoadedModuleExtensionDefinitionsKey::new(workspace))
    }
}

impl fmt::Display for HostLoadedModuleExtensionDefinitionsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed sibling.
struct ObservedHostLoadedModuleExtensionDefinitions {
    result: Arc<
        Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>,
    >,
    observations: PathObservationEpoch,
}

impl ObservedHostLoadedModuleExtensionDefinitions {
    fn result(
        &self,
    ) -> &Arc<Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>>
    {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
enum LoadedModuleExtensionDefinitionsObservationStage {
    Bzl,
    Merge,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum LoadedModuleExtensionDefinitionsObservationError {
    Requests(HostSelectedExtensionDefinitionLoadRequestsObservationError),
    Request {
        requests: Arc<HostSelectedExtensionDefinitionLoadRequests>,
        request: HostSelectedExtensionDefinitionLoadRequest,
        stage: LoadedModuleExtensionDefinitionsObservationStage,
        error: ObservedPathFrontierError,
    },
}

impl Dupe for LoadedModuleExtensionDefinitionsObservationError {}

type LoadedModuleExtensionDefinitionsResult =
    Arc<Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>>;
type LoadedModuleExtensionDefinitionsDriverOutcome = SourcePreparationOutcome<
    Result<
        (LoadedModuleExtensionDefinitionsResult, PathObservationEpoch),
        LoadedModuleExtensionDefinitionsObservationError,
    >,
>;

#[derive(Clone, Copy)]
enum LoadedModuleExtensionDefinitionsMode {
    Legacy,
    Observed,
}

fn loaded_extension_definitions_driver_complete(
    value: Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>,
    observations: PathObservationEpoch,
) -> LoadedModuleExtensionDefinitionsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

async fn loaded_extension_definition_requests(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: LoadedModuleExtensionDefinitionsMode,
) -> Result<
    (
        Arc<
            Result<
                HostSelectedExtensionDefinitionLoadRequests,
                HostSelectedExtensionDefinitionLoadRequestsError,
            >,
        >,
        PathObservationEpoch,
    ),
    LoadedModuleExtensionDefinitionsDriverOutcome,
> {
    match mode {
        LoadedModuleExtensionDefinitionsMode::Legacy => match ctx
            .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => Err(loaded_extension_definitions_driver_complete(
                Err(HostLoadedModuleExtensionDefinitionsError::RequestsCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )),
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(result)) => {
                Ok((result, PathObservationEpoch::empty()))
            }
        },
        LoadedModuleExtensionDefinitionsMode::Observed => match ctx
            .compute(
                &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe()),
            )
            .await
        {
            Err(error) => Err(loaded_extension_definitions_driver_complete(
                Err(HostLoadedModuleExtensionDefinitionsError::RequestsCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )),
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                Err(SourcePreparationOutcome::Complete(Err(
                    LoadedModuleExtensionDefinitionsObservationError::Requests(error),
                )))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                Ok((observed.result().dupe(), observed.observations().dupe()))
            }
        },
    }
}

async fn loaded_extension_definition_bzl(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    request: &HostSelectedExtensionDefinitionLoadRequest,
    target: RootPackageBzlTarget,
    mode: LoadedModuleExtensionDefinitionsMode,
) -> Result<
    (DefinitionBzlModuleCarrier, PathObservationEpoch),
    SourcePreparationOutcome<ObservedPathFrontierError>,
> {
    let root_label =
        HostRootBzlLabel::new(request.parts().0.package().package().clone(), target.dupe());
    let selected_label =
        RepositoryBzlLabel::new(request.parts().0.package().package().clone(), target)
            .expect("the definition target was parsed before selected-source dispatch");
    let selected_route = match request.source() {
        HostSelectedExtensionDefinitionSource::Root => None,
        source => RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), source),
    };
    match (mode, selected_route) {
        (LoadedModuleExtensionDefinitionsMode::Legacy, None) => {
            match host_dice_invariant(
                ctx.compute(&HostBzlModuleEvalKey::new_bzlmod(
                    workspace.dupe(),
                    root_label,
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => Err(SourcePreparationOutcome::Need(need)),
                SourcePreparationOutcome::Complete(result) => Ok((
                    DefinitionBzlModuleCarrier::Root(result),
                    PathObservationEpoch::empty(),
                )),
            }
        }
        (LoadedModuleExtensionDefinitionsMode::Observed, None) => {
            match host_dice_invariant(
                ctx.compute(&HostBzlModuleObservationKey::new_bzlmod(
                    workspace.dupe(),
                    root_label,
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => Err(SourcePreparationOutcome::Need(need)),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    Err(SourcePreparationOutcome::Complete(error))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => Ok((
                    DefinitionBzlModuleCarrier::Root(observed.result.dupe()),
                    observed.observations().dupe(),
                )),
            }
        }
        (LoadedModuleExtensionDefinitionsMode::Legacy, Some(route)) => match ctx
            .compute(&ExternalBzlModuleEvalKey::new_bzlmod(
                route,
                selected_label.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(result)) => Ok((
                DefinitionBzlModuleCarrier::Selected(result),
                PathObservationEpoch::empty(),
            )),
            Err(error) => Ok((
                DefinitionBzlModuleCarrier::Selected(Arc::new(Err(
                    ExternalBzlModuleError::SourceCompute {
                        label: request.parts().0.clone(),
                        message: Arc::from(error.to_string()),
                    },
                ))),
                PathObservationEpoch::empty(),
            )),
        },
        (LoadedModuleExtensionDefinitionsMode::Observed, Some(route)) => match ctx
            .compute(&ExternalBzlModuleObservationKey::new_bzlmod(
                route,
                selected_label,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                Err(SourcePreparationOutcome::Complete(error))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => Ok((
                DefinitionBzlModuleCarrier::Selected(observed.result().dupe()),
                observed.observations().dupe(),
            )),
            Err(error) => Ok((
                DefinitionBzlModuleCarrier::Selected(Arc::new(Err(
                    ExternalBzlModuleError::SourceCompute {
                        label: request.parts().0.clone(),
                        message: Arc::from(error.to_string()),
                    },
                ))),
                PathObservationEpoch::empty(),
            )),
        },
    }
}

fn finish_loaded_extension_definition_observed_child(
    requests: &Arc<HostSelectedExtensionDefinitionLoadRequests>,
    request: &HostSelectedExtensionDefinitionLoadRequest,
    current: PathObservationEpoch,
    child: SourcePreparationOutcome<
        Result<(DefinitionBzlModuleCarrier, PathObservationEpoch), ObservedPathFrontierError>,
    >,
) -> Result<
    (DefinitionBzlModuleCarrier, PathObservationEpoch),
    LoadedModuleExtensionDefinitionsDriverOutcome,
> {
    let (carrier, incoming) = match child {
        SourcePreparationOutcome::Need(need) => return Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                LoadedModuleExtensionDefinitionsObservationError::Request {
                    requests: requests.dupe(),
                    request: request.clone(),
                    stage: LoadedModuleExtensionDefinitionsObservationStage::Bzl,
                    error,
                },
            )));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    let observations = union_host_observations(&current, &incoming).map_err(|error| {
        SourcePreparationOutcome::Complete(Err(
            LoadedModuleExtensionDefinitionsObservationError::Request {
                requests: requests.dupe(),
                request: request.clone(),
                stage: LoadedModuleExtensionDefinitionsObservationStage::Merge,
                error,
            },
        ))
    })?;
    Ok((carrier, observations))
}

async fn drive_loaded_extension_definitions(
    ctx: &mut DiceComputations<'_>,
    key: &HostLoadedModuleExtensionDefinitionsKey,
    mode: LoadedModuleExtensionDefinitionsMode,
) -> LoadedModuleExtensionDefinitionsDriverOutcome {
    let (requests_result, mut observations) =
        match loaded_extension_definition_requests(ctx, &key.workspace, mode).await {
            Ok(value) => value,
            Err(terminal) => return terminal,
        };
    let requests = match requests_result.as_ref() {
        Ok(requests) => Arc::new(requests.clone()),
        Err(error) => {
            return loaded_extension_definitions_driver_complete(
                Err(HostLoadedModuleExtensionDefinitionsError::Requests(
                    error.clone(),
                )),
                observations,
            );
        }
    };
    drop(requests_result);

    let mut definitions = Vec::new();
    for request in requests.parts().1 {
        let (label, export, _, _) = request.parts();
        let target = match RootPackageBzlTarget::parse(label.target().as_str()) {
            Ok(target) => target,
            Err(error) => {
                return loaded_extension_definitions_driver_complete(
                    Err(HostLoadedModuleExtensionDefinitionsError::Request {
                        requests: requests.clone(),
                        request: request.clone(),
                        error: HostLoadedModuleExtensionDefinitionError::Label {
                            label: label.clone(),
                            message: error.to_string().into(),
                        },
                    }),
                    observations,
                );
            }
        };
        let child =
            match loaded_extension_definition_bzl(ctx, &key.workspace, request, target, mode).await
            {
                Ok(value) => SourcePreparationOutcome::Complete(Ok(value)),
                Err(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
                Err(SourcePreparationOutcome::Complete(error)) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
            };
        let (module_result, merged) = match finish_loaded_extension_definition_observed_child(
            &requests,
            request,
            observations,
            child,
        ) {
            Ok(observations) => observations,
            Err(terminal) => return terminal,
        };
        observations = merged;
        let module = match module_result {
            DefinitionBzlModuleCarrier::Root(result) => match result.as_ref() {
                Ok(module) => module.clone(),
                Err(error) => {
                    return loaded_extension_definitions_driver_complete(
                        Err(HostLoadedModuleExtensionDefinitionsError::Request {
                            requests: requests.clone(),
                            request: request.clone(),
                            error: HostLoadedModuleExtensionDefinitionError::Bzl {
                                label: label.clone(),
                                error: error.clone(),
                            },
                        }),
                        observations,
                    );
                }
            },
            DefinitionBzlModuleCarrier::Selected(result) => match result.as_ref() {
                Ok(module) => module.clone(),
                Err(error) => {
                    return loaded_extension_definitions_driver_complete(
                        Err(HostLoadedModuleExtensionDefinitionsError::Request {
                            requests: requests.clone(),
                            request: request.clone(),
                            error: HostLoadedModuleExtensionDefinitionError::SelectedBzl {
                                label: label.clone(),
                                error: error.clone(),
                            },
                        }),
                        observations,
                    );
                }
            },
        };
        let exported = match module.module.get_assigned(export) {
            Ok((value, _visibility)) => value,
            Err(error) => {
                return loaded_extension_definitions_driver_complete(
                    Err(HostLoadedModuleExtensionDefinitionsError::Request {
                        requests: requests.clone(),
                        request: request.clone(),
                        error: HostLoadedModuleExtensionDefinitionError::Export {
                            label: label.clone(),
                            name: export.into(),
                            message: error.to_string().into(),
                        },
                    }),
                    observations,
                );
            }
        };
        let exported = match exported.downcast::<FrozenModuleExtensionDefinition>() {
            Ok(value) => value,
            Err(_) => {
                return loaded_extension_definitions_driver_complete(
                    Err(HostLoadedModuleExtensionDefinitionsError::Request {
                        requests: requests.clone(),
                        request: request.clone(),
                        error: HostLoadedModuleExtensionDefinitionError::WrongKind {
                            label: label.clone(),
                            name: export.into(),
                        },
                    }),
                    observations,
                );
            }
        };
        definitions.push(HostLoadedModuleExtensionDefinition {
            request: request.clone(),
            manifest: module.manifest.clone(),
            definition: exported.projection(),
        });
    }
    loaded_extension_definitions_driver_complete(
        Ok(HostLoadedModuleExtensionDefinitions {
            requests,
            definitions: definitions.into(),
        }),
        observations,
    )
}

fn project_legacy_loaded_extension_definitions(
    outcome: LoadedModuleExtensionDefinitionsDriverOutcome,
) -> HostLoadedModuleExtensionDefinitionsOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            debug_assert!(observations.observations().is_empty());
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy loaded module extension definitions have no observed frontier")
        }
    }
}

#[async_trait]
impl Key for HostLoadedModuleExtensionDefinitionsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostLoadedModuleExtensionDefinitions,
            LoadedModuleExtensionDefinitionsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_loaded_extension_definitions(
            ctx,
            &self.0,
            LoadedModuleExtensionDefinitionsMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostLoadedModuleExtensionDefinitions {
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
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct PreparedModuleExtensionTag {
    pub(crate) tag_class: CompactString,
    pub(crate) attributes: Arc<[(CompactString, crate::attrs::CoercedAttributeValue)]>,
    pub(crate) dev_dependency: bool,
    pub(crate) location: LogicalSpan,
    pub(crate) module_index: usize,
    pub(crate) tag_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct PreparedModuleExtensionInput {
    pub(crate) input: HostSelectedExtensionEvaluationInput,
    pub(crate) tags: Arc<[PreparedModuleExtensionTag]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) struct HostPreparedModuleExtensionInputs {
    pub(crate) raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
    pub(crate) definitions: Arc<HostLoadedModuleExtensionDefinitions>,
    pub(crate) inputs: Arc<[PreparedModuleExtensionInput]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)]
pub(crate) enum HostPreparedModuleExtensionInputsError {
    Raw(HostSelectedExtensionEvaluationInputRequestsError),
    RawCompute(CompactString),
    Definitions {
        raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
        error: Result<HostLoadedModuleExtensionDefinitionsError, CompactString>,
    },
    AfterInputs {
        raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
        definitions: Arc<HostLoadedModuleExtensionDefinitions>,
        request: Option<HostSelectedExtensionDefinitionLoadRequest>,
        error: HostPreparedModuleExtensionInputError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPreparedModuleExtensionInputError {
    Join(CompactString),
    UnknownTagClass(CompactString),
    Attribute(ModuleExtensionTagCoercionError),
}

fn prepare_module_extension_inputs(
    raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
    definitions: Arc<HostLoadedModuleExtensionDefinitions>,
) -> Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError> {
    let after = |request: Option<&HostSelectedExtensionDefinitionLoadRequest>, error| {
        HostPreparedModuleExtensionInputsError::AfterInputs {
            raw: raw.clone(),
            definitions: definitions.clone(),
            request: request.cloned(),
            error,
        }
    };
    if definitions.requests.as_ref() != raw.parts().0 {
        return Err(after(
            None,
            HostPreparedModuleExtensionInputError::Join(
                "definition and raw-input request aggregates differ".into(),
            ),
        ));
    }
    let raw_inputs = raw.parts().1;
    if raw_inputs.len() != definitions.definitions.len() {
        return Err(after(
            None,
            HostPreparedModuleExtensionInputError::Join(
                "definition and raw-input counts differ".into(),
            ),
        ));
    }
    let mut inputs = Vec::with_capacity(raw_inputs.len());
    for (input, loaded) in raw_inputs.iter().zip(definitions.definitions.iter()) {
        let (request, _, _, _, _, tags) = input.parts();
        if request != &loaded.request {
            return Err(after(
                Some(request),
                HostPreparedModuleExtensionInputError::Join(
                    "definition and raw-input request order differs".into(),
                ),
            ));
        }
        for (_, schema) in loaded.definition.tag_classes.iter() {
            validate_module_extension_tag_schema(schema).map_err(|error| {
                after(
                    Some(request),
                    HostPreparedModuleExtensionInputError::Attribute(error),
                )
            })?;
        }
        let (_, _, context_repo, mapping) = request.parts();
        let mut prepared_tags = Vec::with_capacity(tags.len());
        for (tag_index, tag) in tags.iter().enumerate() {
            let schema = loaded
                .definition
                .tag_classes
                .iter()
                .find_map(|(name, schema)| (name == &tag.tag_class).then_some(schema.as_ref()))
                .ok_or_else(|| {
                    after(
                        Some(request),
                        HostPreparedModuleExtensionInputError::UnknownTagClass(
                            tag.tag_class.clone(),
                        ),
                    )
                })?;
            let attributes = prepare_module_extension_tag_attributes(
                schema,
                &tag.attributes,
                context_repo,
                mapping,
            )
            .map_err(|error| {
                after(
                    Some(request),
                    HostPreparedModuleExtensionInputError::Attribute(error),
                )
            })?;
            prepared_tags.push(PreparedModuleExtensionTag {
                tag_class: tag.tag_class.clone(),
                attributes,
                dev_dependency: tag.dev_dependency,
                location: tag.location.clone(),
                module_index: 0,
                tag_index,
            });
        }
        inputs.push(PreparedModuleExtensionInput {
            input: input.clone(),
            tags: prepared_tags.into(),
        });
    }
    Ok(HostPreparedModuleExtensionInputs {
        raw,
        definitions,
        inputs: inputs.into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct HostPreparedModuleExtensionInputsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostPreparedModuleExtensionInputsKey {
    #[allow(dead_code)]
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostPreparedModuleExtensionInputsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-prepared-module-extension-inputs:{}",
            self.workspace
        )
    }
}

type HostPreparedModuleExtensionInputsOutcome = SourcePreparationOutcome<
    Arc<Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling; a later packet owns consumer activation.
pub(crate) struct HostPreparedModuleExtensionInputsObservationKey(
    HostPreparedModuleExtensionInputsKey,
);

#[allow(dead_code)]
impl HostPreparedModuleExtensionInputsObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostPreparedModuleExtensionInputsKey::new(workspace))
    }
}

impl fmt::Display for HostPreparedModuleExtensionInputsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed sibling.
pub(crate) struct ObservedHostPreparedModuleExtensionInputs {
    result: PreparedModuleExtensionInputsResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostPreparedModuleExtensionInputs {
    pub(crate) fn result(
        &self,
    ) -> &Arc<Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>>
    {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum PreparedModuleExtensionInputsObservationError {
    Raw(HostSelectedExtensionEvaluationInputRequestsObservationError),
    Definitions {
        raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
        error: LoadedModuleExtensionDefinitionsObservationError,
    },
    Merge {
        raw: Arc<HostSelectedExtensionEvaluationInputRequests>,
        error: ObservedPathFrontierError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPreparedModuleExtensionInputsObservationError(
    PreparedModuleExtensionInputsObservationError,
);

type PreparedModuleExtensionInputsResult =
    Arc<Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>>;
type PreparedModuleExtensionInputsDriverOutcome = SourcePreparationOutcome<
    Result<
        (PreparedModuleExtensionInputsResult, PathObservationEpoch),
        PreparedModuleExtensionInputsObservationError,
    >,
>;
type PreparedChildOutcome<T, E, O> =
    SourcePreparationOutcome<Result<(Arc<Result<T, E>>, PathObservationEpoch), O>>;

#[derive(Clone, Copy)]
enum PreparedModuleExtensionInputsMode {
    Legacy,
    Observed,
}

fn prepared_module_extension_inputs_complete(
    value: Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>,
    observations: PathObservationEpoch,
) -> PreparedModuleExtensionInputsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

fn finish_prepared_module_extension_raw(
    child: PreparedChildOutcome<
        HostSelectedExtensionEvaluationInputRequests,
        HostSelectedExtensionEvaluationInputRequestsError,
        HostSelectedExtensionEvaluationInputRequestsObservationError,
    >,
) -> Result<
    (
        Arc<HostSelectedExtensionEvaluationInputRequests>,
        PathObservationEpoch,
    ),
    PreparedModuleExtensionInputsDriverOutcome,
> {
    let (result, observations) = match child {
        SourcePreparationOutcome::Need(need) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                PreparedModuleExtensionInputsObservationError::Raw(error),
            )));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    match result.as_ref() {
        Ok(raw) => Ok((Arc::new(raw.clone()), observations)),
        Err(error) => Err(prepared_module_extension_inputs_complete(
            Err(HostPreparedModuleExtensionInputsError::Raw(error.clone())),
            observations,
        )),
    }
}

async fn prepared_module_extension_raw(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: PreparedModuleExtensionInputsMode,
) -> Result<
    (
        Arc<HostSelectedExtensionEvaluationInputRequests>,
        PathObservationEpoch,
    ),
    PreparedModuleExtensionInputsDriverOutcome,
> {
    let child = match mode {
        PreparedModuleExtensionInputsMode::Legacy => match ctx
            .compute(&HostSelectedExtensionEvaluationInputRequestsKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => {
                return Err(prepared_module_extension_inputs_complete(
                    Err(HostPreparedModuleExtensionInputsError::RawCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => {
                SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
            }
        },
        PreparedModuleExtensionInputsMode::Observed => match ctx
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe()),
            )
            .await
        {
            Err(error) => {
                return Err(prepared_module_extension_inputs_complete(
                    Err(HostPreparedModuleExtensionInputsError::RawCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                SourcePreparationOutcome::Complete(Ok((
                    observed.result().dupe(),
                    observed.observations().dupe(),
                )))
            }
        },
    };
    finish_prepared_module_extension_raw(child)
}

fn finish_prepared_module_extension_definitions(
    raw: &Arc<HostSelectedExtensionEvaluationInputRequests>,
    observations: PathObservationEpoch,
    child: PreparedChildOutcome<
        HostLoadedModuleExtensionDefinitions,
        HostLoadedModuleExtensionDefinitionsError,
        LoadedModuleExtensionDefinitionsObservationError,
    >,
) -> Result<
    (
        Arc<HostLoadedModuleExtensionDefinitions>,
        PathObservationEpoch,
    ),
    PreparedModuleExtensionInputsDriverOutcome,
> {
    let (result, incoming) = match child {
        SourcePreparationOutcome::Need(need) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                PreparedModuleExtensionInputsObservationError::Definitions {
                    raw: raw.dupe(),
                    error,
                },
            )));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    let merged = union_host_observations(&observations, &incoming).map_err(|error| {
        SourcePreparationOutcome::Complete(Err(
            PreparedModuleExtensionInputsObservationError::Merge {
                raw: raw.dupe(),
                error,
            },
        ))
    })?;
    match result.as_ref() {
        Ok(definitions) => Ok((Arc::new(definitions.clone()), merged)),
        Err(error) => Err(prepared_module_extension_inputs_complete(
            Err(HostPreparedModuleExtensionInputsError::Definitions {
                raw: raw.dupe(),
                error: Ok(error.clone()),
            }),
            merged,
        )),
    }
}

async fn prepared_module_extension_definitions(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    raw: &Arc<HostSelectedExtensionEvaluationInputRequests>,
    observations: PathObservationEpoch,
    mode: PreparedModuleExtensionInputsMode,
) -> Result<
    (
        Arc<HostLoadedModuleExtensionDefinitions>,
        PathObservationEpoch,
    ),
    PreparedModuleExtensionInputsDriverOutcome,
> {
    let child = match mode {
        PreparedModuleExtensionInputsMode::Legacy => match ctx
            .compute(&HostLoadedModuleExtensionDefinitionsKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => {
                return Err(prepared_module_extension_inputs_complete(
                    Err(HostPreparedModuleExtensionInputsError::Definitions {
                        raw: raw.dupe(),
                        error: Err(error.to_string().into()),
                    }),
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => {
                SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
            }
        },
        PreparedModuleExtensionInputsMode::Observed => match ctx
            .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                workspace.dupe(),
            ))
            .await
        {
            Err(error) => {
                return Err(prepared_module_extension_inputs_complete(
                    Err(HostPreparedModuleExtensionInputsError::Definitions {
                        raw: raw.dupe(),
                        error: Err(error.to_string().into()),
                    }),
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                SourcePreparationOutcome::Complete(Ok((
                    observed.result().dupe(),
                    observed.observations().dupe(),
                )))
            }
        },
    };
    finish_prepared_module_extension_definitions(raw, observations, child)
}

async fn drive_prepared_module_extension_inputs(
    ctx: &mut DiceComputations<'_>,
    key: &HostPreparedModuleExtensionInputsKey,
    mode: PreparedModuleExtensionInputsMode,
) -> PreparedModuleExtensionInputsDriverOutcome {
    let (raw, observations) = match prepared_module_extension_raw(ctx, &key.workspace, mode).await {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let (definitions, observations) =
        match prepared_module_extension_definitions(ctx, &key.workspace, &raw, observations, mode)
            .await
        {
            Ok(value) => value,
            Err(terminal) => return terminal,
        };
    prepared_module_extension_inputs_complete(
        prepare_module_extension_inputs(raw, definitions),
        observations,
    )
}

#[async_trait]
impl Key for HostPreparedModuleExtensionInputsKey {
    type Value = HostPreparedModuleExtensionInputsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_prepared_module_extension_inputs(
            ctx,
            self,
            PreparedModuleExtensionInputsMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy prepared module extension inputs have no observed frontier")
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
impl Key for HostPreparedModuleExtensionInputsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostPreparedModuleExtensionInputs,
            HostPreparedModuleExtensionInputsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_prepared_module_extension_inputs(
            ctx,
            &self.0,
            PreparedModuleExtensionInputsMode::Observed,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostPreparedModuleExtensionInputsObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostPreparedModuleExtensionInputs {
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

type ExternalBzlModuleCarrier = Arc<Result<FrozenBzlModule, ExternalBzlModuleError>>;
type ExternalBzlDriverOutcome = SourcePreparationOutcome<
    Result<(ExternalBzlModuleCarrier, PathObservationEpoch), ObservedPathFrontierError>,
>;

#[derive(Clone, Copy)]
enum ExternalBzlModuleMode {
    Legacy,
    Observed,
}

enum ExternalBzlSourceChild {
    Root(
        Result<HostRepositorySourceFileValue, RepositorySourceFileError>,
        PathObservationEpoch,
    ),
    Observation(
        Arc<Result<HostRepositorySourceObservation, HostRepositorySourceObservationError>>,
        PathObservationEpoch,
    ),
    Compute(Arc<str>),
}

fn external_bzl_observed_child<T>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
) -> ControlFlow<SourcePreparationOutcome<Result<(), ObservedPathFrontierError>>, T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(value)) => ControlFlow::Continue(value),
    }
}

fn external_bzl_complete(
    result: Result<FrozenBzlModule, ExternalBzlModuleError>,
    observations: PathObservationEpoch,
) -> ExternalBzlDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}
#[derive(Debug)]
struct ExternalBzlSourceValue {
    bytes: Arc<[u8]>,
    source_name: String,
    presentation_path: PathBuf,
    observations: PathObservationEpoch,
}

fn finish_external_bzl_source(
    child: ExternalBzlSourceChild,
    label: CanonicalLabel,
) -> Result<ExternalBzlSourceValue, ExternalBzlDriverOutcome> {
    match child {
        ExternalBzlSourceChild::Root(result, observations) => {
            finish_root_external_bzl_source(result, label, observations)
        }
        ExternalBzlSourceChild::Observation(result, observations) => {
            let source_name = label.to_string();
            let presentation_path = PathBuf::from(&source_name);
            match result.as_ref() {
                Ok(HostRepositorySourceObservation::Builtin(value)) => Ok(ExternalBzlSourceValue {
                    bytes: value.bytes_arc().dupe(),
                    source_name,
                    presentation_path,
                    observations,
                }),
                Ok(HostRepositorySourceObservation::Request(
                    HostRepositorySourceFileValue::Present { bytes, .. },
                )) => Ok(ExternalBzlSourceValue {
                    bytes: bytes.dupe(),
                    source_name,
                    presentation_path,
                    observations,
                }),
                Ok(HostRepositorySourceObservation::Request(
                    HostRepositorySourceFileValue::Absent,
                )) => Err(external_bzl_complete(
                    Err(ExternalBzlModuleError::Absent { label }),
                    observations,
                )),
                Err(error) => Err(external_bzl_complete(
                    Err(ExternalBzlModuleError::SourceObservation {
                        label,
                        error: error.clone(),
                    }),
                    observations,
                )),
            }
        }
        ExternalBzlSourceChild::Compute(message) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::SourceCompute { label, message }),
            PathObservationEpoch::empty(),
        )),
    }
}

fn finish_root_external_bzl_source(
    result: Result<HostRepositorySourceFileValue, RepositorySourceFileError>,
    label: CanonicalLabel,
    observations: PathObservationEpoch,
) -> Result<ExternalBzlSourceValue, ExternalBzlDriverOutcome> {
    match result {
        Ok(HostRepositorySourceFileValue::Present {
            bytes,
            logical_path,
        }) => match external_source_name(&logical_path) {
            Ok(source_name) => Ok(ExternalBzlSourceValue {
                bytes,
                source_name,
                presentation_path: logical_path.as_path().to_path_buf(),
                observations,
            }),
            Err(()) => Err(external_bzl_complete(
                Err(ExternalBzlModuleError::Encoding { label }),
                observations,
            )),
        },
        Ok(HostRepositorySourceFileValue::Absent) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::Absent { label }),
            observations,
        )),
        Err(error) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::Source { label, error }),
            observations,
        )),
    }
}

async fn compute_external_bzl_source(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalBzlModuleEvalKey,
    mode: ExternalBzlModuleMode,
) -> SourcePreparationOutcome<Result<ExternalBzlSourceChild, ObservedPathFrontierError>> {
    let source_path = key.label.repository_relative_path();
    let relative = host_repository_relative_path(source_path)
        .expect("typed external bzl labels form repository-relative paths");
    match mode {
        ExternalBzlModuleMode::Legacy => match key.route.source_read_key(relative) {
            HostRepositorySourceReadKey::RootRequest(source_key) => {
                match ctx.compute(&source_key).await {
                    Ok(SourcePreparationOutcome::Need(need)) => {
                        SourcePreparationOutcome::Need(need)
                    }
                    Ok(SourcePreparationOutcome::Complete(result)) => {
                        SourcePreparationOutcome::Complete(Ok(ExternalBzlSourceChild::Root(
                            result,
                            PathObservationEpoch::empty(),
                        )))
                    }
                    Err(error) => SourcePreparationOutcome::Complete(Ok(
                        ExternalBzlSourceChild::Compute(Arc::from(error.to_string())),
                    )),
                }
            }
            HostRepositorySourceReadKey::Observation(source_key) => {
                match ctx.compute(&source_key).await {
                    Ok(SourcePreparationOutcome::Need(need)) => {
                        SourcePreparationOutcome::Need(need)
                    }
                    Ok(SourcePreparationOutcome::Complete(result)) => {
                        SourcePreparationOutcome::Complete(Ok(ExternalBzlSourceChild::Observation(
                            result,
                            PathObservationEpoch::empty(),
                        )))
                    }
                    Err(error) => SourcePreparationOutcome::Complete(Ok(
                        ExternalBzlSourceChild::Compute(Arc::from(error.to_string())),
                    )),
                }
            }
        },
        ExternalBzlModuleMode::Observed => match key.route.source_read_observation_key(relative) {
            HostRepositorySourceReadObservationKey::RootRequest(source_key) => {
                match ctx.compute(&source_key).await {
                    Ok(outcome) => match external_bzl_observed_child(outcome) {
                        ControlFlow::Continue(source) => {
                            SourcePreparationOutcome::Complete(Ok(ExternalBzlSourceChild::Root(
                                source.result().as_ref().clone(),
                                source.observations().dupe(),
                            )))
                        }
                        ControlFlow::Break(outcome) => {
                            outcome.map(|result| result.map(|()| unreachable!()))
                        }
                    },
                    Err(error) => SourcePreparationOutcome::Complete(Ok(
                        ExternalBzlSourceChild::Compute(Arc::from(error.to_string())),
                    )),
                }
            }
            HostRepositorySourceReadObservationKey::Observation(source_key) => {
                match ctx.compute(&source_key).await {
                    Ok(outcome) => match external_bzl_observed_child(outcome) {
                        ControlFlow::Continue(source) => SourcePreparationOutcome::Complete(Ok(
                            ExternalBzlSourceChild::Observation(
                                source.result().dupe(),
                                source.observations().dupe(),
                            ),
                        )),
                        ControlFlow::Break(outcome) => {
                            outcome.map(|result| result.map(|()| unreachable!()))
                        }
                    },
                    Err(error) => SourcePreparationOutcome::Complete(Ok(
                        ExternalBzlSourceChild::Compute(Arc::from(error.to_string())),
                    )),
                }
            }
        },
    }
}

async fn complete_observed_external_bzl_cycle(
    ctx: &mut DiceComputations<'_>,
    current: &ExternalBzlCycleIdentity,
    cycle: &ExternalBzlLoadCycle,
    mut observations: PathObservationEpoch,
) -> ExternalBzlDriverOutcome {
    let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
    if !cycle.keys.is_empty() {
        let current_index = cycle
            .keys
            .iter()
            .position(|identity| identity == current)
            .expect("detected external bzl cycle contains the current observed module");
        for offset in 1..cycle.keys.len() {
            let identity = &cycle.keys[(current_index + offset) % cycle.keys.len()];
            let source_key = ExternalBzlModuleEvalKey::from_source_route(
                identity.route.clone(),
                identity.label.clone(),
                identity.context,
            );
            let source = match compute_external_bzl_source(
                ctx,
                &source_key,
                ExternalBzlModuleMode::Observed,
            )
            .await
            {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(source)) => source,
            };
            let finished = match finish_external_bzl_source(source, identity.canonical_label()) {
                Ok(source) => source,
                Err(value) => return value,
            };
            observations = match union_host_observations(&observations, &finished.observations) {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
        }
    }
    external_bzl_complete(
        Err(ExternalBzlModuleError::Cycle(cycle.clone())),
        observations,
    )
}

enum ExternalBzlRecursiveChild {
    Value(ExternalBzlModuleCarrier, PathObservationEpoch),
    Compute(Arc<str>),
    Cycle(ExternalBzlLoadCycle),
}

async fn compute_external_bzl_child(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    label: RepositoryBzlLabel,
    context: BzlModuleContext,
    mode: ExternalBzlModuleMode,
) -> SourcePreparationOutcome<Result<ExternalBzlRecursiveChild, ObservedPathFrontierError>> {
    let guard = host_dice_invariant(ctx.cycle_guard::<ExternalBzlLoadCycleGuard>())
        .expect("external Bzl loading requires the request cycle detector");
    match mode {
        ExternalBzlModuleMode::Legacy => {
            let child = ExternalBzlModuleEvalKey::from_source_route(route.clone(), label, context);
            match guard.guard_this(ctx.compute(&child)).await {
                Ok(Ok(SourcePreparationOutcome::Need(need))) => {
                    SourcePreparationOutcome::Need(need)
                }
                Ok(Ok(SourcePreparationOutcome::Complete(value))) => {
                    SourcePreparationOutcome::Complete(Ok(ExternalBzlRecursiveChild::Value(
                        value,
                        PathObservationEpoch::empty(),
                    )))
                }
                Ok(Err(error)) => SourcePreparationOutcome::Complete(Ok(
                    ExternalBzlRecursiveChild::Compute(Arc::from(error.to_string())),
                )),
                Err(cycle) => {
                    SourcePreparationOutcome::Complete(Ok(ExternalBzlRecursiveChild::Cycle(cycle)))
                }
            }
        }
        ExternalBzlModuleMode::Observed => {
            let child =
                ExternalBzlModuleObservationKey::from_source_route(route.clone(), label, context);
            match guard.guard_this(ctx.compute(&child)).await {
                Ok(Ok(outcome)) => match external_bzl_observed_child(outcome) {
                    ControlFlow::Continue(value) => {
                        SourcePreparationOutcome::Complete(Ok(ExternalBzlRecursiveChild::Value(
                            value.result().dupe(),
                            value.observations().dupe(),
                        )))
                    }
                    ControlFlow::Break(SourcePreparationOutcome::Need(need)) => {
                        SourcePreparationOutcome::Need(need)
                    }
                    ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))) => {
                        SourcePreparationOutcome::Complete(Err(error))
                    }
                    ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(()))) => {
                        unreachable!()
                    }
                },
                Ok(Err(error)) => SourcePreparationOutcome::Complete(Ok(
                    ExternalBzlRecursiveChild::Compute(Arc::from(error.to_string())),
                )),
                Err(cycle) => {
                    SourcePreparationOutcome::Complete(Ok(ExternalBzlRecursiveChild::Cycle(cycle)))
                }
            }
        }
    }
}

fn external_source_name(logical_path: &NormalizedAbsolutePath) -> Result<String, ()> {
    starlark_source_name(logical_path.as_path()).ok_or(())
}

fn external_repository_mapping(
    route: &HostRepositorySourceRoute,
) -> Arc<[(ApparentRepoName, CanonicalRepoName)]> {
    match route {
        HostRepositorySourceRoute::Root(route) => route.bzl_repository_mapping(),
        HostRepositorySourceRoute::Canonical(input) => {
            input.view().route().bzl_repository_mapping()
        }
    }
}

fn external_runfiles_repository_mapping(
    route: &HostRepositorySourceRoute,
) -> Arc<RunfilesRepositoryMapping> {
    Arc::new(RunfilesRepositoryMapping::new(
        external_repository_mapping(route),
        route.runfiles_mapping_compact_group(),
    ))
}

fn external_load_resolution_error(
    source: CanonicalLabel,
    load: &str,
    message: impl Into<Arc<str>>,
    observations: PathObservationEpoch,
) -> ExternalBzlDriverOutcome {
    external_bzl_complete(
        Err(ExternalBzlModuleError::Route {
            source,
            load: Arc::from(load),
            message: message.into(),
        }),
        observations,
    )
}

async fn compute_canonical_external_child_input(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    child_repo: CanonicalRepoName,
    source: CanonicalLabel,
    load: &str,
    mode: ExternalBzlModuleMode,
    observations: PathObservationEpoch,
) -> ControlFlow<
    ExternalBzlDriverOutcome,
    (
        slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        PathObservationEpoch,
    ),
> {
    match mode {
        ExternalBzlModuleMode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                workspace, child_repo,
            ))
            .await
        {
            Err(error) => ControlFlow::Break(external_load_resolution_error(
                source,
                load,
                Arc::from(format!("{error:?}")),
                observations,
            )),
            Ok(SourcePreparationOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::Need(need))
            }
            Ok(SourcePreparationOutcome::Complete(result)) => match result.as_ref() {
                Ok(route) => ControlFlow::Continue((route.input().clone(), observations)),
                Err(error) => ControlFlow::Break(external_load_resolution_error(
                    source,
                    load,
                    Arc::from(error.to_string()),
                    observations,
                )),
            },
        },
        ExternalBzlModuleMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                workspace, child_repo,
            ))
            .await
        {
            Err(error) => ControlFlow::Break(external_load_resolution_error(
                source,
                load,
                Arc::from(format!("{error:?}")),
                observations,
            )),
            Ok(SourcePreparationOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::Need(need))
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                ControlFlow::Break(external_load_resolution_error(
                    source,
                    load,
                    Arc::from(format!("{error:?}")),
                    observations,
                ))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                let observations =
                    match union_host_observations(&observations, observed.observations()) {
                        Ok(observations) => observations,
                        Err(error) => {
                            return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
                                error,
                            )));
                        }
                    };
                match observed.result().as_ref() {
                    Ok(route) => ControlFlow::Continue((route.input().clone(), observations)),
                    Err(error) => ControlFlow::Break(external_load_resolution_error(
                        source,
                        load,
                        Arc::from(error.to_string()),
                        observations,
                    )),
                }
            }
        },
    }
}

fn admitted_builtin_external_repository_load(loads: &[String]) -> Option<&str> {
    let [load] = loads else {
        return None;
    };
    let apparent = load.strip_prefix('@')?;
    if apparent.starts_with('@') {
        return None;
    }
    let (repository, _) = apparent.split_once("//")?;
    (!repository.is_empty() && ApparentRepoName::new(repository).is_ok()).then_some(load.as_str())
}

async fn compute_external_bzl_effective_route(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalBzlModuleEvalKey,
    loads: &[String],
    source: CanonicalLabel,
    mode: ExternalBzlModuleMode,
    observations: PathObservationEpoch,
) -> ControlFlow<ExternalBzlDriverOutcome, (HostRepositorySourceRoute, PathObservationEpoch)> {
    let HostRepositorySourceRoute::Root(route) = &key.route else {
        return ControlFlow::Continue((key.route.clone(), observations));
    };
    if !route.is_builtin_bazel_tools() {
        return ControlFlow::Continue((key.route.clone(), observations));
    }
    let Some(load) = admitted_builtin_external_repository_load(loads) else {
        return ControlFlow::Continue((key.route.clone(), observations));
    };
    let (input, observations) = match compute_canonical_external_child_input(
        ctx,
        route.workspace().dupe(),
        route.canonical_repo().clone(),
        source,
        load,
        mode,
        observations,
    )
    .await
    {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
    };
    ControlFlow::Continue((HostRepositorySourceRoute::canonical(input), observations))
}

async fn resolve_external_bzl_child_route(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    package: &PackagePath,
    source: CanonicalLabel,
    mode: ExternalBzlModuleMode,
    load: &str,
    observations: PathObservationEpoch,
) -> ControlFlow<ExternalBzlDriverOutcome, (ResolvedExternalBzlLoad, PathObservationEpoch)> {
    let HostRepositorySourceRoute::Canonical(input) = route else {
        let HostRepositorySourceRoute::Root(route) = route else {
            unreachable!()
        };
        let (child_route, label) = match resolve_external_bzl_load_label(route, package, load) {
            Ok(resolved) => resolved,
            Err(error) => {
                return ControlFlow::Break(external_bzl_complete(
                    Err(ExternalBzlModuleError::LoadLabel { source, error }),
                    observations,
                ));
            }
        };
        return match child_route {
            RootRepositoryBzlLoadRoute::Root(child_route) => ControlFlow::Continue((
                ResolvedExternalBzlLoad {
                    route: HostRepositorySourceRoute::root(child_route),
                    label,
                },
                observations,
            )),
            RootRepositoryBzlLoadRoute::Canonical(child_repo) => {
                let (child_input, observations) = match compute_canonical_external_child_input(
                    ctx,
                    route.workspace().dupe(),
                    child_repo,
                    source,
                    load,
                    mode,
                    observations,
                )
                .await
                {
                    ControlFlow::Continue(value) => value,
                    ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                };
                ControlFlow::Continue((
                    ResolvedExternalBzlLoad {
                        route: HostRepositorySourceRoute::canonical(child_input),
                        label,
                    },
                    observations,
                ))
            }
        };
    };
    let (child_repo, label) = match resolve_canonical_external_bzl_load_label(input, package, load)
    {
        Ok(resolved) => resolved,
        Err(error) => {
            return ControlFlow::Break(external_bzl_complete(
                Err(ExternalBzlModuleError::LoadLabel { source, error }),
                observations,
            ));
        }
    };
    let Some(child_repo) = child_repo else {
        return ControlFlow::Continue((
            ResolvedExternalBzlLoad {
                route: route.clone(),
                label,
            },
            observations,
        ));
    };
    let (child_input, observations) = match compute_canonical_external_child_input(
        ctx,
        route.workspace().dupe(),
        child_repo,
        source,
        load,
        mode,
        observations,
    )
    .await
    {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
    };
    ControlFlow::Continue((
        ResolvedExternalBzlLoad {
            route: HostRepositorySourceRoute::canonical(child_input),
            label,
        },
        observations,
    ))
}

async fn compute_external_bzl_children(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalBzlModuleEvalKey,
    route: &HostRepositorySourceRoute,
    mode: ExternalBzlModuleMode,
    loads: Vec<ExternalBzlChildLoad>,
    mut observations: PathObservationEpoch,
) -> ControlFlow<ExternalBzlDriverOutcome, (Vec<(String, FrozenBzlModule)>, PathObservationEpoch)> {
    let mut loaded_modules = Vec::with_capacity(loads.len());
    for load in loads {
        let (raw_load, resolved, incoming) = match load {
            ExternalBzlChildLoad::Resolved(raw_load, resolved) => {
                (raw_load, resolved, observations)
            }
            ExternalBzlChildLoad::Canonical(raw_load) => {
                let (resolved, incoming) = match resolve_external_bzl_child_route(
                    ctx,
                    route,
                    &key.label.package,
                    key.canonical_label(),
                    mode,
                    &raw_load,
                    observations,
                )
                .await
                {
                    ControlFlow::Continue(resolved) => resolved,
                    ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                };
                (raw_load, resolved, incoming)
            }
        };
        observations = incoming;
        let child_label = resolved.label.canonical_label(&resolved.route);
        let child = match compute_external_bzl_child(
            ctx,
            &resolved.route,
            resolved.label,
            key.context,
            mode,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => {
                return ControlFlow::Break(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)));
            }
            SourcePreparationOutcome::Complete(Ok(child)) => child,
        };
        let (value, incoming) = match child {
            ExternalBzlRecursiveChild::Value(value, incoming) => (value, incoming),
            ExternalBzlRecursiveChild::Compute(message) => {
                return ControlFlow::Break(external_bzl_complete(
                    Err(ExternalBzlModuleError::Child {
                        raw_load: Arc::from(raw_load.as_str()),
                        canonical_label: child_label.clone(),
                        error: Arc::new(ExternalBzlModuleError::SourceCompute {
                            label: child_label,
                            message,
                        }),
                    }),
                    observations,
                ));
            }
            ExternalBzlRecursiveChild::Cycle(cycle) => match mode {
                ExternalBzlModuleMode::Legacy => {
                    let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                    return ControlFlow::Break(external_bzl_complete(
                        Err(ExternalBzlModuleError::Cycle(cycle)),
                        observations,
                    ));
                }
                ExternalBzlModuleMode::Observed => {
                    return ControlFlow::Break(
                        complete_observed_external_bzl_cycle(
                            ctx,
                            &key.cycle_identity(),
                            &cycle,
                            observations,
                        )
                        .await,
                    );
                }
            },
        };
        observations = match mode {
            ExternalBzlModuleMode::Legacy => {
                debug_assert!(incoming.observations().is_empty());
                observations
            }
            ExternalBzlModuleMode::Observed => {
                match union_host_observations(&observations, &incoming) {
                    Ok(observations) => observations,
                    Err(error) => {
                        return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)));
                    }
                }
            }
        };
        match value.as_ref() {
            Ok(module) => loaded_modules.push((raw_load, module.clone())),
            Err(error) => {
                return ControlFlow::Break(external_bzl_complete(
                    Err(ExternalBzlModuleError::Child {
                        raw_load: Arc::from(raw_load.as_str()),
                        canonical_label: child_label,
                        error: Arc::new(error.clone()),
                    }),
                    observations,
                ));
            }
        }
    }
    ControlFlow::Continue((loaded_modules, observations))
}

fn prepare_external_bzl_child_loads(
    key: &ExternalBzlModuleEvalKey,
    effective_route: &HostRepositorySourceRoute,
    loads: &[String],
    source: CanonicalLabel,
    observations: &PathObservationEpoch,
) -> Result<Vec<ExternalBzlChildLoad>, ExternalBzlDriverOutcome> {
    let HostRepositorySourceRoute::Root(route) = effective_route else {
        return Ok(loads
            .iter()
            .cloned()
            .map(ExternalBzlChildLoad::Canonical)
            .collect());
    };
    loads
        .iter()
        .map(|load| {
            resolve_external_bzl_load_label(route, &key.label.package, load)
                .map(|(route, label)| match route {
                    RootRepositoryBzlLoadRoute::Root(route) => ExternalBzlChildLoad::Resolved(
                        load.clone(),
                        ResolvedExternalBzlLoad {
                            route: HostRepositorySourceRoute::root(route),
                            label,
                        },
                    ),
                    RootRepositoryBzlLoadRoute::Canonical(_) => {
                        ExternalBzlChildLoad::Canonical(load.clone())
                    }
                })
                .map_err(|error| {
                    external_bzl_complete(
                        Err(ExternalBzlModuleError::LoadLabel {
                            source: source.clone(),
                            error,
                        }),
                        observations.dupe(),
                    )
                })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn evaluate_external_bzl_module(
    route: &HostRepositorySourceRoute,
    context: BzlModuleContext,
    canonical_label: CanonicalLabel,
    presentation_path: PathBuf,
    source_text: Arc<String>,
    ast: AstModule,
    loads: Vec<String>,
    loaded_modules: Vec<(String, FrozenBzlModule)>,
    observations: PathObservationEpoch,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> ExternalBzlDriverOutcome {
    let module = Module::new();
    let manifest = BzlLoadManifest::new(
        BzlModuleIdentity {
            label: canonical_label.clone(),
            workspace_path: presentation_path.clone(),
            repository_mapping: external_repository_mapping(route),
        },
        digest(source_text.as_str()),
        loaded_modules.iter().map(|(_, module)| module),
    );
    if let Err(error) =
        validate_direct_bzl_load_visibilities(manifest.root.label.package(), &loaded_modules)
    {
        return external_bzl_complete(
            Err(ExternalBzlModuleError::Evaluation {
                label: canonical_label,
                message: Arc::from(error.to_string()),
            }),
            observations,
        );
    }
    let loader = LocalBzlLoader {
        modules: loaded_modules
            .iter()
            .map(|(load, module)| (load.as_str(), module.module.dupe()))
            .collect(),
    };
    let evaluation_context = BzlEvaluationContext::from_manifest(&manifest);
    let print_capture = capture_events.then(LoadingPrintCapture::default);
    let globals = context.globals();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&evaluation_context);
        evaluator.set_loader(&loader);
        if let Some(print_capture) = print_capture.as_ref() {
            evaluator.set_print_handler(print_capture);
        }
        let evaluation = evaluator.eval_module(ast, &globals).map(|_| ());
        drop(evaluator);
        *event_batch = print_capture.map(LoadingPrintCapture::into_batch);
        if let Err(error) = evaluation {
            return external_bzl_complete(
                Err(ExternalBzlModuleError::Evaluation {
                    label: canonical_label,
                    message: Arc::from(error.to_string()),
                }),
                observations,
            );
        }
    }
    let module = match module.freeze() {
        Ok(module) => module,
        Err(error) => {
            return external_bzl_complete(
                Err(ExternalBzlModuleError::Freeze {
                    label: canonical_label,
                    message: Arc::from(format!("{error:?}")),
                }),
                observations,
            );
        }
    };
    external_bzl_complete(
        Ok(FrozenBzlModule {
            module,
            path: presentation_path,
            loads,
            retained_bzl_modules: retained_module_closure(&loaded_modules),
            bzl_load_visibility: evaluation_context.bzl_load_visibility(),
            manifest,
        }),
        observations,
    )
}

async fn compute_external_bzl_module(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalBzlModuleEvalKey,
    mode: ExternalBzlModuleMode,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> ExternalBzlDriverOutcome {
    let canonical_label = key.canonical_label();
    let source = match compute_external_bzl_source(ctx, key, mode).await {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(child)) => {
            match finish_external_bzl_source(child, canonical_label.clone()) {
                Ok(source) => source,
                Err(value) => return value,
            }
        }
    };
    let source_text = match String::from_utf8(source.bytes.to_vec()) {
        Ok(source) => Arc::new(source),
        Err(_) => {
            return external_bzl_complete(
                Err(ExternalBzlModuleError::Encoding {
                    label: canonical_label,
                }),
                source.observations,
            );
        }
    };
    let ast = match AstModule::parse_with_string_encoding(
        &source.source_name,
        source_text.as_ref().clone(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(error) => {
            return external_bzl_complete(
                Err(ExternalBzlModuleError::Parse {
                    label: canonical_label,
                    message: Arc::from(error.to_string()),
                }),
                source.observations,
            );
        }
    };
    let loads = ast
        .loads()
        .into_iter()
        .map(|load| load.module_id.to_owned())
        .collect::<Vec<_>>();
    let (effective_route, observations) = match compute_external_bzl_effective_route(
        ctx,
        key,
        &loads,
        canonical_label.clone(),
        mode,
        source.observations,
    )
    .await
    {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return outcome,
    };
    let child_loads = match prepare_external_bzl_child_loads(
        key,
        &effective_route,
        &loads,
        canonical_label.clone(),
        &observations,
    ) {
        Ok(child_loads) => child_loads,
        Err(outcome) => return outcome,
    };
    let (loaded_modules, observations) = match compute_external_bzl_children(
        ctx,
        key,
        &effective_route,
        mode,
        child_loads,
        observations,
    )
    .await
    {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(value) => return value,
    };

    evaluate_external_bzl_module(
        &effective_route,
        key.context,
        canonical_label,
        source.presentation_path,
        source_text,
        ast,
        loads,
        loaded_modules,
        observations,
        capture_events,
        event_batch,
    )
}

fn stores_external_bzl_event_batch(value: &ExternalBzlDriverOutcome) -> bool {
    matches!(value, SourcePreparationOutcome::Complete(Ok(_)))
}

#[async_trait]
impl Key for ExternalBzlModuleEvalKey {
    type Value = SourcePreparationOutcome<ExternalBzlModuleCarrier>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_external_bzl_module(
            ctx,
            self,
            ExternalBzlModuleMode::Legacy,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_external_bzl_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ExternalBzlModuleEvalKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy external bzl module produced frontier error: {error}")
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
impl Key for ExternalBzlModuleObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedExternalBzlModule, ObservedPathFrontierError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_external_bzl_module(
            ctx,
            &self.0,
            ExternalBzlModuleMode::Observed,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_external_bzl_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ExternalBzlModuleObservationKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedExternalBzlModule {
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

type RootPackageLoadCarrier = Arc<Result<LoadedPackage, RootPackageLoadError>>;
type RootPackageLoadDriverOutcome = SourcePreparationOutcome<
    Result<(RootPackageLoadCarrier, PathObservationEpoch), ObservedPathFrontierError>,
>;

fn root_package_driver_complete(
    result: Result<LoadedPackage, RootPackageLoadError>,
    observations: PathObservationEpoch,
) -> RootPackageLoadDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn merge_root_package_observations(
    mode: HostPackageLoadMode,
    current: PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    match mode {
        HostPackageLoadMode::Legacy => {
            debug_assert!(incoming.observations().is_empty());
            Ok(current)
        }
        HostPackageLoadMode::Observed => union_host_observations(&current, incoming),
    }
}

type RootPackageModuleOutcome = Result<FrozenBzlModule, RootPackageLoadDriverOutcome>;

fn root_package_load_terminal(
    error: RootPackageLoadErrorInner,
    observations: &PathObservationEpoch,
) -> RootPackageLoadDriverOutcome {
    root_package_driver_complete(Err(RootPackageLoadError::new(error)), observations.dupe())
}

async fn load_root_package_host_bzl(
    key: &RootPackageLoadKey,
    ctx: &mut DiceComputations<'_>,
    mode: HostPackageLoadMode,
    label: HostRootBzlLabel,
    origin: Arc<str>,
    load: &str,
    observations: &mut PathObservationEpoch,
) -> RootPackageModuleOutcome {
    let child = match mode {
        HostPackageLoadMode::Legacy => host_dice_invariant(
            ctx.compute(&HostBzlModuleEvalKey::new(
                key.workspace.dupe(),
                label.clone(),
            ))
            .await,
        )
        .map(|result| Ok((result.as_ref().clone(), PathObservationEpoch::empty()))),
        HostPackageLoadMode::Observed => host_dice_invariant(
            ctx.compute(&HostBzlModuleObservationKey::new(
                key.workspace.dupe(),
                label.clone(),
            ))
            .await,
        )
        .map(|value| {
            value.map(|observed| (observed.result().clone(), observed.observations().dupe()))
        }),
    };
    let (child, incoming) = match child {
        SourcePreparationOutcome::Need(need) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(SourcePreparationOutcome::Complete(Err(error)));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    *observations = merge_root_package_observations(mode, observations.dupe(), &incoming)
        .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
    child.map_err(|error| {
        root_package_load_terminal(
            RootPackageLoadErrorInner::Bzl {
                origin,
                load: Arc::from(load),
                label,
                error: Arc::new(error),
            },
            observations,
        )
    })
}

async fn load_root_package_external_route(
    key: &RootPackageLoadKey,
    ctx: &mut DiceComputations<'_>,
    mode: HostPackageLoadMode,
    apparent_repo: &slug_identity_v2::ApparentRepoName,
    load: &str,
    observations: &mut PathObservationEpoch,
) -> Result<HostRepositorySourceRoute, RootPackageLoadDriverOutcome> {
    let route = match mode {
        HostPackageLoadMode::Legacy => match ctx
            .compute(
                &HostRootRepositoryLoadRouteKey::for_root_build(
                    key.workspace.dupe(),
                    apparent_repo.clone(),
                )
                .expect("root BUILD load has nonroot repo"),
            )
            .await
        {
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::ExternalInfrastructure {
                        load: Arc::from(load),
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(route)) => route.as_ref().clone(),
        },
        HostPackageLoadMode::Observed => match ctx
            .compute(
                &HostRootRepositoryLoadRouteObservationKey::for_root_build(
                    key.workspace.dupe(),
                    apparent_repo.clone(),
                )
                .expect("root BUILD load has nonroot repo"),
            )
            .await
        {
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::ExternalInfrastructure {
                        load: Arc::from(load),
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(match error.selected_frontier() {
                    HostSelectedObservationFrontier::Path(error) => {
                        SourcePreparationOutcome::Complete(Err(error))
                    }
                    HostSelectedObservationFrontier::Infrastructure(message) => {
                        root_package_load_terminal(
                            RootPackageLoadErrorInner::ExternalInfrastructure {
                                load: Arc::from(load),
                                message,
                            },
                            observations,
                        )
                    }
                });
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                *observations = merge_root_package_observations(
                    mode,
                    observations.dupe(),
                    observed.observations(),
                )
                .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
                observed.result().as_ref().clone()
            }
        },
    };
    route.map(|route| route.source().clone()).map_err(|error| {
        root_package_load_terminal(
            RootPackageLoadErrorInner::ExternalRoute {
                load: Arc::from(load),
                apparent_repo: apparent_repo.clone(),
                error,
            },
            observations,
        )
    })
}

async fn load_root_package_external_bzl(
    ctx: &mut DiceComputations<'_>,
    mode: HostPackageLoadMode,
    route: HostRepositorySourceRoute,
    label: RepositoryBzlLabel,
    origin: Arc<str>,
    load: &str,
    observations: &mut PathObservationEpoch,
) -> RootPackageModuleOutcome {
    let canonical = label.canonical_label(&route);
    let child = match mode {
        HostPackageLoadMode::Legacy => match ctx
            .compute(&ExternalBzlModuleEvalKey::from_source_route(
                route,
                label,
                BzlModuleContext::Build,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => result.as_ref().clone(),
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::ExternalInfrastructure {
                        load: Arc::from(load),
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
        },
        HostPackageLoadMode::Observed => match ctx
            .compute(&ExternalBzlModuleObservationKey::from_source_route(
                route,
                label,
                BzlModuleContext::Build,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(error)));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                *observations = merge_root_package_observations(
                    mode,
                    observations.dupe(),
                    observed.observations(),
                )
                .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
                observed.result().as_ref().clone()
            }
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::ExternalInfrastructure {
                        load: Arc::from(load),
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
        },
    };
    child.map_err(|error| {
        root_package_load_terminal(
            RootPackageLoadErrorInner::ExternalBzl {
                origin,
                load: Arc::from(load),
                label: canonical,
                error: Arc::new(error),
            },
            observations,
        )
    })
}

async fn load_root_runfiles_repository_mapping(
    key: &RootPackageLoadKey,
    ctx: &mut DiceComputations<'_>,
    mode: HostPackageLoadMode,
    observations: &mut PathObservationEpoch,
) -> Result<Arc<RunfilesRepositoryMapping>, RootPackageLoadDriverOutcome> {
    let result = match mode {
        HostPackageLoadMode::Legacy => match ctx
            .compute(&HostRootRepositoryMappingKey::new(key.workspace.dupe()))
            .await
        {
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::RepositoryMappingInfrastructure {
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => result,
        },
        HostPackageLoadMode::Observed => match ctx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                key.workspace.dupe(),
            ))
            .await
        {
            Err(error) => {
                return Err(root_package_load_terminal(
                    RootPackageLoadErrorInner::RepositoryMappingInfrastructure {
                        message: Arc::from(error.to_string()),
                    },
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(match error.selected_frontier() {
                    HostSelectedObservationFrontier::Path(error) => {
                        SourcePreparationOutcome::Complete(Err(error))
                    }
                    HostSelectedObservationFrontier::Infrastructure(message) => {
                        root_package_load_terminal(
                            RootPackageLoadErrorInner::RepositoryMappingInfrastructure { message },
                            observations,
                        )
                    }
                });
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                *observations = merge_root_package_observations(
                    mode,
                    observations.dupe(),
                    observed.observations(),
                )
                .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
                observed.result().dupe()
            }
        },
    };
    let mapping = result.as_ref().as_ref().map_err(|error| {
        root_package_load_terminal(
            RootPackageLoadErrorInner::RepositoryMapping {
                message: Arc::from(error.to_string()),
            },
            observations,
        )
    })?;
    let view = mapping.view().ok_or_else(|| {
        root_package_load_terminal(
            RootPackageLoadErrorInner::RepositoryMapping {
                message: Arc::from("selected root mapping has no root repository view"),
            },
            observations,
        )
    })?;
    if !view.canonical_repo().is_root() || !view.mapping_context().is_root() {
        return Err(root_package_load_terminal(
            RootPackageLoadErrorInner::RepositoryMapping {
                message: Arc::from("selected root mapping has a nonroot context"),
            },
            observations,
        ));
    }
    Ok(Arc::new(RunfilesRepositoryMapping::new(
        view.mapping()
            .map(|(apparent, canonical)| (apparent.clone(), canonical.clone()))
            .collect::<Vec<_>>()
            .into(),
        None,
    )))
}

async fn compute_root_package(
    key: &RootPackageLoadKey,
    ctx: &mut DiceComputations<'_>,
    mode: HostPackageLoadMode,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> RootPackageLoadDriverOutcome {
    let anchor = match mode {
        HostPackageLoadMode::Legacy => host_dice_invariant(
            ctx.compute(&RootModuleLoadingAnchorKey::new(key.workspace.dupe()))
                .await,
        )
        .map(|result| Ok((result.as_ref().clone(), PathObservationEpoch::empty()))),
        HostPackageLoadMode::Observed => host_dice_invariant(
            ctx.compute(&RootModuleLoadingAnchorObservationKey::new(
                key.workspace.dupe(),
            ))
            .await,
        )
        .map(|value| {
            value.map(|observed| (observed.result().clone(), observed.observations().dupe()))
        }),
    };
    let (anchor, mut observations) = match anchor {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    if let Err(error) = anchor {
        return root_package_driver_complete(
            Err(RootPackageLoadError::new(
                RootPackageLoadErrorInner::RootModule(error),
            )),
            observations,
        );
    }

    let source = match mode {
        HostPackageLoadMode::Legacy => host_dice_invariant(
            ctx.compute(&RootPackageSourceKey::for_build(
                key.workspace.dupe(),
                key.package.clone(),
            ))
            .await,
        )
        .map(|result| Ok((result.as_ref().clone(), PathObservationEpoch::empty()))),
        HostPackageLoadMode::Observed => host_dice_invariant(
            ctx.compute(&RootPackageSourceObservationKey::for_build(
                key.workspace.dupe(),
                key.package.clone(),
            ))
            .await,
        )
        .map(|value| {
            value.map(|observed| (observed.result().clone(), observed.observations().dupe()))
        }),
    };
    let (source, incoming) = match source {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    observations = match merge_root_package_observations(mode, observations, &incoming) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    #[cfg(test)]
    if matches!(mode, HostPackageLoadMode::Observed) {
        if let Ok(forced) = ctx
            .per_transaction_data()
            .data
            .get::<ForceRootPackageObservationOuter>()
        {
            return SourcePreparationOutcome::Complete(Err(forced.0.clone()));
        }
    }
    let source = match source {
        Ok(source) => source,
        Err(error) => {
            return root_package_driver_complete(
                Err(RootPackageLoadError::new(
                    RootPackageLoadErrorInner::Source(error),
                )),
                observations,
            );
        }
    };
    let source_text = match host_source_text(&source) {
        Ok(source) => source,
        Err(error) => {
            return root_package_driver_complete(
                Err(RootPackageLoadError::new(RootPackageLoadErrorInner::Input(
                    error,
                ))),
                observations,
            );
        }
    };
    let source_name = match host_source_name(&source) {
        Ok(name) => name,
        Err(error) => {
            return root_package_driver_complete(
                Err(RootPackageLoadError::new(RootPackageLoadErrorInner::Input(
                    error,
                ))),
                observations,
            );
        }
    };
    let ast = match AstModule::parse_with_string_encoding(
        &source_name,
        source_text.as_ref().clone(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(error) => {
            return root_package_driver_complete(
                Err(RootPackageLoadError::new(
                    RootPackageLoadErrorInner::Parse {
                        package: key.package.clone(),
                        message: Arc::from(error.to_string()),
                    },
                )),
                observations,
            );
        }
    };
    let mut loaded_modules = Vec::new();
    for load in ast.loads() {
        let load = load.module_id.to_owned();
        let resolved = match resolve_root_package_direct_load(&key.package, &load) {
            Ok(label) => label,
            Err(error) => {
                return root_package_driver_complete(
                    Err(RootPackageLoadError::new(
                        RootPackageLoadErrorInner::LoadLabel {
                            package: key.package.clone(),
                            error,
                        },
                    )),
                    observations,
                );
            }
        };
        let build_name: String = source
            .relative_path()
            .iter()
            .copied()
            .map(char::from)
            .collect();
        let origin: Arc<str> = Arc::from(if key.package.as_str().is_empty() {
            build_name
        } else {
            format!("{}/{build_name}", key.package)
        });
        let module = match resolved {
            RootPackageDirectLoad::Root(label) => {
                load_root_package_host_bzl(key, ctx, mode, label, origin, &load, &mut observations)
                    .await
            }
            RootPackageDirectLoad::External {
                apparent_repo,
                label,
            } => match load_root_package_external_route(
                key,
                ctx,
                mode,
                &apparent_repo,
                &load,
                &mut observations,
            )
            .await
            {
                Ok(route) => {
                    load_root_package_external_bzl(
                        ctx,
                        mode,
                        route,
                        label,
                        origin,
                        &load,
                        &mut observations,
                    )
                    .await
                }
                Err(outcome) => Err(outcome),
            },
        };
        let module = match module {
            Ok(module) => module,
            Err(outcome) => return outcome,
        };
        loaded_modules.push((load, module));
    }
    let repository_mapping =
        match load_root_runfiles_repository_mapping(key, ctx, mode, &mut observations).await {
            Ok(mapping) => mapping,
            Err(outcome) => return outcome,
        };
    let package_dir = source.package_root().as_path().join(key.package.as_str());
    let attempts = evaluate_host_package_attempts_driver(
        ctx,
        HostPackageAttemptInput {
            workspace: key.workspace.dupe(),
            logical_package_root: source.package_root().dupe(),
            package: key.package.clone(),
            package_identifier: PackageIdentifier::new(
                CanonicalRepoName::root(),
                key.package.clone(),
            ),
            package_dir,
            build_file: source.logical_path().as_path().to_path_buf(),
            source: source_text,
            package_label: CompactString::new(key.package.as_str()),
            loaded_modules: &loaded_modules,
            capture_events,
        },
        mode,
        repository_mapping,
        HostGlobBoundaryScope::Root,
    )
    .await;
    let (terminal, incoming) = match attempts {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    observations = match merge_root_package_observations(mode, observations, &incoming) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    *event_batch = Some(terminal.event_batch.clone());
    root_package_driver_complete(
        terminal
            .result
            .clone()
            .map_err(|error| RootPackageLoadError::new(RootPackageLoadErrorInner::Attempt(error))),
        observations,
    )
}

fn stores_root_package_event_batch(value: &RootPackageLoadDriverOutcome) -> bool {
    matches!(value, SourcePreparationOutcome::Complete(Ok(_)))
}

#[async_trait]
impl Key for RootPackageLoadKey {
    type Value = SourcePreparationOutcome<RootPackageLoadCarrier>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_root_package(
            self,
            ctx,
            HostPackageLoadMode::Legacy,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_root_package_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("RootPackageLoadKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy root package produced frontier error: {error}")
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
impl Key for RootPackageLoadObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedRootPackageLoad, ObservedPathFrontierError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = compute_root_package(
            &self.0,
            ctx,
            HostPackageLoadMode::Observed,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && stores_root_package_event_batch(&value) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("RootPackageLoadObservationKey stores one local Complete event batch");
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedRootPackageLoad {
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

fn loaded_external_target_kind(kind: &PackageTargetKind) -> Option<&'static str> {
    match kind {
        PackageTargetKind::ExportedFile
        | PackageTargetKind::Filegroup { .. }
        | PackageTargetKind::Alias { .. }
        | PackageTargetKind::ConfigSetting { .. } => None,
        // Native toolchain targets are retained as typed native targets even
        // when the external BUILD uses `load()`. The load gate only protects
        // target kinds whose loaded form is not yet represented.
        PackageTargetKind::NativeToolchain(_) => None,
        PackageTargetKind::TestSuite { .. } => Some("test_suite"),
        PackageTargetKind::PackageGroup { .. } => Some("package_group"),
        PackageTargetKind::GeneratedFile { .. } => Some("generated file"),
        PackageTargetKind::StarlarkRule(_) => Some("Starlark rule"),
    }
}

fn loaded_external_starlark_rule_reason(
    targets: &[crate::package::PackageTarget],
) -> Option<(&str, LoadedStarlarkRuleReason)> {
    let target = targets
        .iter()
        .find(|target| matches!(target.kind, PackageTargetKind::StarlarkRule(_)))?;
    let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
        unreachable!()
    };
    let reason = if !matches!(
        &target.visibility,
        VisibilitySource::Declared(RuleVisibility::Public)
    ) {
        Some(LoadedStarlarkRuleReason::Visibility)
    } else if target
        .rule_capability()
        .is_some_and(|capability| capability.test_kind.is_some())
    {
        Some(LoadedStarlarkRuleReason::Test)
    } else if target
        .rule_capability()
        .is_some_and(|capability| capability.executable)
    {
        Some(LoadedStarlarkRuleReason::Executable)
    } else if !implementation.dependencies().is_empty() {
        Some(LoadedStarlarkRuleReason::OrdinaryDependencies)
    } else if implementation.schema().len() != implementation.values().len()
        || implementation
            .schema()
            .iter()
            .zip(implementation.values())
            .any(|(schema, value)| schema.declaration_name() != value.declaration_name)
    {
        Some(LoadedStarlarkRuleReason::SchemaValues)
    } else {
        implementation
            .schema()
            .iter()
            .zip(implementation.values())
            .filter(|(schema, _)| {
                schema.dependency_reachable()
                    && (!schema.is_builtin() || schema.ordinary_dependency())
            })
            .find_map(|(schema, value)| {
                let mut labels = Vec::new();
                value.value.labels(&mut labels);
                (!labels.is_empty()).then(|| {
                    LoadedStarlarkRuleReason::ReachableLabels(Arc::from(schema.query_name()))
                })
            })
    };
    reason
        .or_else(|| {
            (targets.len() != 1).then(|| LoadedStarlarkRuleReason::AdditionalTargets(targets.len()))
        })
        .map(|reason| (target.name.as_str(), reason))
}

#[derive(Clone, Copy)]
enum RepositoryPackageInventoryMode {
    Legacy,
    Observed,
}

type RepositoryPackageInventoryCarrier = Arc<Result<LoadedPackage, RepositoryPackageLoadError>>;
type RepositoryPackageInventoryDriverOutcome = SourcePreparationOutcome<
    Result<(RepositoryPackageInventoryCarrier, PathObservationEpoch), ObservedPathFrontierError>,
>;

fn repository_package_driver_complete(
    result: Result<LoadedPackage, RepositoryPackageLoadError>,
    observations: PathObservationEpoch,
) -> RepositoryPackageInventoryDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn finish_repository_package_observed_child<T>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
) -> ControlFlow<RepositoryPackageInventoryDriverOutcome, T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(value)) => ControlFlow::Continue(value),
    }
}

fn merge_repository_package_observations(
    current: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    union_host_observations(current, incoming)
}

async fn compute_repository_package_source(
    ctx: &mut DiceComputations<'_>,
    key: &RepositoryPackageInventoryKey,
    mode: RepositoryPackageInventoryMode,
) -> ControlFlow<
    RepositoryPackageInventoryDriverOutcome,
    (RepositoryPackageSource, PathObservationEpoch),
> {
    let package = PackageIdentifier::new(key.route.canonical_repo().clone(), key.package.clone());
    let mut observations = PathObservationEpoch::empty();
    let result = match mode {
        RepositoryPackageInventoryMode::Legacy => {
            let source = match &key.route {
                HostRepositorySourceRoute::Root(route) => {
                    RepositoryPackageSourceKey::new(route.clone(), package)
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    RepositoryPackageSourceKey::new_canonical(input.clone(), package)
                }
            }
            .expect("repository package load route and package agree");
            match ctx.compute(&source).await {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return ControlFlow::Break(SourcePreparationOutcome::Need(need));
                }
                Ok(SourcePreparationOutcome::Complete(value)) => value.dupe(),
                Err(error) => {
                    return ControlFlow::Break(repository_package_driver_complete(
                        Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::SourceCompute {
                                canonical_repo: CompactString::new(
                                    key.route.canonical_repo().as_str(),
                                ),
                                package: key.package.clone(),
                                message: Arc::from(error.to_string()),
                            },
                        )),
                        observations,
                    ));
                }
            }
        }
        RepositoryPackageInventoryMode::Observed => {
            let source = match &key.route {
                HostRepositorySourceRoute::Root(route) => {
                    RepositoryPackageSourceObservationKey::new(route.clone(), package)
                }
                HostRepositorySourceRoute::Canonical(input) => {
                    RepositoryPackageSourceObservationKey::new_canonical(input.clone(), package)
                }
            }
            .expect("repository package load route and package agree");
            match ctx.compute(&source).await {
                Ok(outcome) => match finish_repository_package_observed_child(outcome) {
                    ControlFlow::Continue(value) => {
                        observations = value.observations().dupe();
                        value.result().dupe()
                    }
                    ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                },
                Err(error) => {
                    return ControlFlow::Break(repository_package_driver_complete(
                        Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::SourceCompute {
                                canonical_repo: CompactString::new(
                                    key.route.canonical_repo().as_str(),
                                ),
                                package: key.package.clone(),
                                message: Arc::from(error.to_string()),
                            },
                        )),
                        observations,
                    ));
                }
            }
        }
    };
    match result.as_ref() {
        Ok(source) => ControlFlow::Continue((source.dupe(), observations)),
        Err(error) => ControlFlow::Break(repository_package_driver_complete(
            Err(RepositoryPackageLoadError::new(
                RepositoryPackageLoadErrorInner::Source {
                    error: error.clone(),
                },
            )),
            observations,
        )),
    }
}

async fn compute_repository_package_child(
    ctx: &mut DiceComputations<'_>,
    mode: RepositoryPackageInventoryMode,
    resolved: ResolvedExternalBzlLoad,
    raw_load: &str,
    build_origin: &Arc<str>,
    mut observations: PathObservationEpoch,
) -> ControlFlow<RepositoryPackageInventoryDriverOutcome, (FrozenBzlModule, PathObservationEpoch)> {
    let canonical_label = resolved.label.canonical_label(&resolved.route);
    let child_result = match mode {
        RepositoryPackageInventoryMode::Legacy => {
            let child = ExternalBzlModuleEvalKey::from_source_route(
                resolved.route,
                resolved.label,
                BzlModuleContext::Build,
            );
            match ctx.compute(&child).await {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return ControlFlow::Break(SourcePreparationOutcome::Need(need));
                }
                Ok(SourcePreparationOutcome::Complete(value)) => value,
                Err(error) => {
                    return ControlFlow::Break(repository_package_driver_complete(
                        Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::Bzl {
                                origin: build_origin.dupe(),
                                raw_load: Arc::from(raw_load),
                                canonical_label: canonical_label.clone(),
                                error: Arc::new(ExternalBzlModuleError::SourceCompute {
                                    label: canonical_label,
                                    message: Arc::from(error.to_string()),
                                }),
                            },
                        )),
                        observations,
                    ));
                }
            }
        }
        RepositoryPackageInventoryMode::Observed => {
            let child = ExternalBzlModuleObservationKey::from_source_route(
                resolved.route,
                resolved.label,
                BzlModuleContext::Build,
            );
            match ctx.compute(&child).await {
                Ok(outcome) => match finish_repository_package_observed_child(outcome) {
                    ControlFlow::Continue(value) => {
                        observations = match merge_repository_package_observations(
                            &observations,
                            value.observations(),
                        ) {
                            Ok(observations) => observations,
                            Err(error) => {
                                return ControlFlow::Break(SourcePreparationOutcome::Complete(
                                    Err(error),
                                ));
                            }
                        };
                        value.result().dupe()
                    }
                    ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                },
                Err(error) => {
                    return ControlFlow::Break(repository_package_driver_complete(
                        Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::Bzl {
                                origin: build_origin.dupe(),
                                raw_load: Arc::from(raw_load),
                                canonical_label: canonical_label.clone(),
                                error: Arc::new(ExternalBzlModuleError::SourceCompute {
                                    label: canonical_label,
                                    message: Arc::from(error.to_string()),
                                }),
                            },
                        )),
                        observations,
                    ));
                }
            }
        }
    };
    match child_result.as_ref() {
        Ok(module) => ControlFlow::Continue((module.clone(), observations)),
        Err(error) => ControlFlow::Break(repository_package_driver_complete(
            Err(RepositoryPackageLoadError::new(
                RepositoryPackageLoadErrorInner::Bzl {
                    origin: build_origin.dupe(),
                    raw_load: Arc::from(raw_load),
                    canonical_label,
                    error: Arc::new(error.clone()),
                },
            )),
            observations,
        )),
    }
}

enum RepositoryPackageChildLoad {
    Resolved(String, ResolvedExternalBzlLoad),
    Canonical(String),
}

async fn compute_repository_package_children(
    ctx: &mut DiceComputations<'_>,
    key: &RepositoryPackageInventoryKey,
    mode: RepositoryPackageInventoryMode,
    loads: Vec<RepositoryPackageChildLoad>,
    source_label: CanonicalLabel,
    build_origin: &Arc<str>,
    mut observations: PathObservationEpoch,
) -> ControlFlow<
    RepositoryPackageInventoryDriverOutcome,
    (Vec<(String, FrozenBzlModule)>, PathObservationEpoch),
> {
    let mut loaded_modules = Vec::with_capacity(loads.len());
    for load in loads {
        let (raw_load, resolved, incoming) = match load {
            RepositoryPackageChildLoad::Resolved(raw_load, resolved) => {
                (raw_load, resolved, observations)
            }
            RepositoryPackageChildLoad::Canonical(raw_load) => {
                let external_mode = match mode {
                    RepositoryPackageInventoryMode::Legacy => ExternalBzlModuleMode::Legacy,
                    RepositoryPackageInventoryMode::Observed => ExternalBzlModuleMode::Observed,
                };
                match resolve_external_bzl_child_route(
                    ctx,
                    &key.route,
                    &key.package,
                    source_label.clone(),
                    external_mode,
                    &raw_load,
                    observations,
                )
                .await
                {
                    ControlFlow::Continue((resolved, incoming)) => (raw_load, resolved, incoming),
                    ControlFlow::Break(outcome) => {
                        return ControlFlow::Break(project_repository_load_resolution_error(
                            outcome,
                            build_origin,
                            &raw_load,
                            source_label,
                        ));
                    }
                }
            }
        };
        match compute_repository_package_child(
            ctx,
            mode,
            resolved,
            &raw_load,
            build_origin,
            incoming,
        )
        .await
        {
            ControlFlow::Continue((module, incoming)) => {
                observations = incoming;
                loaded_modules.push((raw_load, module));
            }
            ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
        }
    }
    ControlFlow::Continue((loaded_modules, observations))
}

fn project_repository_load_resolution_error(
    outcome: ExternalBzlDriverOutcome,
    build_origin: &Arc<str>,
    raw_load: &str,
    source_label: CanonicalLabel,
) -> RepositoryPackageInventoryDriverOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(error))
        }
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            let error = result
                .as_ref()
                .as_ref()
                .err()
                .expect("load resolution break contains an external module error")
                .clone();
            repository_package_driver_complete(
                Err(RepositoryPackageLoadError::new(
                    RepositoryPackageLoadErrorInner::Bzl {
                        origin: build_origin.dupe(),
                        raw_load: Arc::from(raw_load),
                        canonical_label: source_label,
                        error: Arc::new(error),
                    },
                )),
                observations,
            )
        }
    }
}

struct PreparedRepositoryPackageEvaluation {
    source_text: Arc<String>,
    glob_source: RepositoryPackageGlobSource,
    logical_package_dir: PathBuf,
    logical_build_file: PathBuf,
    build_label: CanonicalLabel,
    child_loads: Vec<RepositoryPackageChildLoad>,
    build_origin: Arc<str>,
    observations: PathObservationEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepositoryPackageGlobSource {
    Host(NormalizedAbsolutePath),
    BuiltinCatalog,
}

fn repository_package_glob_source(
    address: &RepositoryPackageSourceAddress,
    build_file_name: &str,
    package: &PackagePath,
) -> Result<RepositoryPackageGlobSource, PathBuf> {
    let RepositoryPackageSourceAddress::Host(build_file) = address else {
        return Ok(RepositoryPackageGlobSource::BuiltinCatalog);
    };
    let fail = || build_file.as_path().to_path_buf();
    if build_file.as_path().file_name() != Some(std::ffi::OsStr::new(build_file_name)) {
        return Err(fail());
    }
    let mut directory = build_file.as_path().parent().ok_or_else(&fail)?;
    for component in package
        .as_str()
        .split('/')
        .rev()
        .filter(|component| !component.is_empty())
    {
        if directory.file_name() != Some(std::ffi::OsStr::new(component)) {
            return Err(fail());
        }
        directory = directory.parent().ok_or_else(&fail)?;
    }
    NormalizedAbsolutePath::new(directory.to_path_buf())
        .map(RepositoryPackageGlobSource::Host)
        .map_err(|_| fail())
}

fn prepare_repository_package_evaluation(
    key: &RepositoryPackageInventoryKey,
    source: RepositoryPackageSource,
    observations: PathObservationEpoch,
) -> Result<PreparedRepositoryPackageEvaluation, RepositoryPackageInventoryDriverOutcome> {
    let relative_build_file = PathBuf::from(key.package.as_str()).join(source.build_file_name());
    let canonical_repo = CompactString::new(key.route.canonical_repo().as_str());
    let glob_source =
        repository_package_glob_source(source.address(), source.build_file_name(), &key.package)
            .map_err(|build_file| {
                repository_package_driver_complete(
                    Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::GlobSourceRoot {
                            canonical_repo: canonical_repo.clone(),
                            package: key.package.clone(),
                            build_file,
                        },
                    )),
                    observations.dupe(),
                )
            })?;
    let source_text = match std::str::from_utf8(source.bytes().as_ref()) {
        Ok(source) => Arc::new(source.to_owned()),
        Err(_) => {
            let path = match source.address() {
                RepositoryPackageSourceAddress::Host(path) => path.as_path().to_path_buf(),
                RepositoryPackageSourceAddress::BuiltinCatalog(path) => {
                    path.as_path().to_path_buf()
                }
            };
            return Err(repository_package_driver_complete(
                Err(RepositoryPackageLoadError::new(
                    RepositoryPackageLoadErrorInner::Encoding { path },
                )),
                observations,
            ));
        }
    };
    let logical_package_dir = PathBuf::from("<output_base>")
        .join("external")
        .join(canonical_repo.as_str())
        .join(key.package.as_str());
    let logical_build_file = logical_package_dir.join(
        relative_build_file
            .file_name()
            .expect("BUILD candidate has a basename"),
    );
    let build_label = CanonicalLabel::parse(&format!(
        "{}//{}:{}",
        key.route.canonical_repo(),
        key.package,
        source.build_file_name()
    ))
    .expect("typed repository BUILD identity is canonical");
    let source_name = match (&key.route, source.address()) {
        (HostRepositorySourceRoute::Root(_), RepositoryPackageSourceAddress::Host(path)) => {
            starlark_source_name(path.as_path())
                .expect("accepted root repository source paths have parser names")
        }
        (HostRepositorySourceRoute::Canonical(_), _) => build_label.to_string(),
        _ => unreachable!("root repository sources retain Host addresses"),
    };
    let ast = AstModule::parse_with_string_encoding(
        &source_name,
        source_text.as_ref().clone(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    )
    .map_err(|error| {
        repository_package_driver_complete(
            Err(RepositoryPackageLoadError::new(
                RepositoryPackageLoadErrorInner::Parse {
                    canonical_repo: canonical_repo.clone(),
                    package: key.package.clone(),
                    message: Arc::from(error.to_string()),
                },
            )),
            observations.dupe(),
        )
    })?;
    let loads = ast
        .loads()
        .into_iter()
        .map(|load| load.module_id.to_owned())
        .collect::<Vec<_>>();
    let child_loads =
        prepare_repository_package_child_loads(key, &loads, &canonical_repo, &observations)?;
    let build_basename = relative_build_file
        .file_name()
        .expect("BUILD candidate has a basename")
        .to_string_lossy();
    let build_origin = Arc::from(format!(
        "@@{canonical_repo}//{}/{}",
        key.package, build_basename
    ));
    Ok(PreparedRepositoryPackageEvaluation {
        source_text,
        glob_source,
        logical_package_dir,
        logical_build_file,
        build_label,
        child_loads,
        build_origin,
        observations,
    })
}

fn prepare_repository_package_child_loads(
    key: &RepositoryPackageInventoryKey,
    loads: &[String],
    canonical_repo: &CompactString,
    observations: &PathObservationEpoch,
) -> Result<Vec<RepositoryPackageChildLoad>, RepositoryPackageInventoryDriverOutcome> {
    let HostRepositorySourceRoute::Root(route) = &key.route else {
        return Ok(loads
            .iter()
            .cloned()
            .map(RepositoryPackageChildLoad::Canonical)
            .collect());
    };
    loads
        .iter()
        .map(|load| {
            resolve_external_load_label(&key.package, load)
                .map(|label| {
                    RepositoryPackageChildLoad::Resolved(
                        load.clone(),
                        ResolvedExternalBzlLoad {
                            route: HostRepositorySourceRoute::root(route.clone()),
                            label,
                        },
                    )
                })
                .map_err(|error| {
                    repository_package_driver_complete(
                        Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::LoadLabel {
                                canonical_repo: canonical_repo.clone(),
                                package: key.package.clone(),
                                error,
                            },
                        )),
                        observations.dupe(),
                    )
                })
        })
        .collect()
}

async fn evaluate_repository_package(
    ctx: &mut DiceComputations<'_>,
    key: &RepositoryPackageInventoryKey,
    prepared: PreparedRepositoryPackageEvaluation,
    loaded_modules: &[(String, FrozenBzlModule)],
    mode: RepositoryPackageInventoryMode,
    observations: PathObservationEpoch,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> RepositoryPackageInventoryDriverOutcome {
    let (logical_package_root, boundary_scope) = match &prepared.glob_source {
        RepositoryPackageGlobSource::Host(root) => (
            root.dupe(),
            HostGlobBoundaryScope::External(key.route.clone()),
        ),
        RepositoryPackageGlobSource::BuiltinCatalog => (
            key.route.workspace().dupe(),
            HostGlobBoundaryScope::BuiltinCatalog(key.route.clone()),
        ),
    };
    let input = HostPackageAttemptInput {
        workspace: key.route.workspace().dupe(),
        logical_package_root,
        package: key.package.clone(),
        package_identifier: PackageIdentifier::new(
            key.route.canonical_repo().clone(),
            key.package.clone(),
        ),
        package_dir: prepared.logical_package_dir,
        build_file: prepared.logical_build_file,
        source: prepared.source_text,
        package_label: CompactString::new(key.package.as_str()),
        loaded_modules,
        capture_events,
    };
    let host_mode = match mode {
        RepositoryPackageInventoryMode::Legacy => HostPackageLoadMode::Legacy,
        RepositoryPackageInventoryMode::Observed => HostPackageLoadMode::Observed,
    };
    let attempts = evaluate_host_package_attempts_driver(
        ctx,
        input,
        host_mode,
        external_runfiles_repository_mapping(&key.route),
        boundary_scope,
    )
    .await;
    let (terminal, incoming) = match attempts {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    let observations = match merge_repository_package_observations(&observations, &incoming) {
        Ok(observations) => observations,
        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
    };
    *event_batch = Some(terminal.event_batch.clone());
    let result = terminal.result.clone().map_err(|error| {
        RepositoryPackageLoadError::new(RepositoryPackageLoadErrorInner::Attempt(error))
    });
    repository_package_driver_complete(result, observations)
}

fn validate_loaded_repository_package(
    key: &RepositoryPackageInventoryKey,
    loaded: &LoadedPackage,
) -> Result<(), RepositoryPackageLoadError> {
    // Package evaluation retains one direct root for every first-seen BUILD
    // load, so emptiness preserves the former syntactic `has_loads` gate.
    if loaded.direct_load_roots.is_empty() {
        return Ok(());
    }
    if let Some((target, reason)) = loaded_external_starlark_rule_reason(&loaded.targets) {
        return Err(RepositoryPackageLoadError::new(
            RepositoryPackageLoadErrorInner::LoadedStarlarkRule {
                canonical_repo: CompactString::new(key.route.canonical_repo().as_str()),
                package: key.package.clone(),
                target: Arc::from(target),
                reason,
            },
        ));
    }
    if matches!(
        loaded.targets.as_slice(),
        [crate::package::PackageTarget {
            kind: PackageTargetKind::StarlarkRule(_),
            ..
        }]
    ) {
        return Ok(());
    }
    if let Some((target, kind)) = loaded.targets.iter().find_map(|target| {
        loaded_external_target_kind(&target.kind).map(|kind| (target.name.as_str(), kind))
    }) {
        return Err(RepositoryPackageLoadError::new(
            RepositoryPackageLoadErrorInner::LoadedTargetKind {
                canonical_repo: CompactString::new(key.route.canonical_repo().as_str()),
                package: key.package.clone(),
                target: Arc::from(target),
                kind: Arc::from(kind),
            },
        ));
    }
    Ok(())
}

impl RepositoryPackageInventoryKey {
    async fn compute_mode(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: RepositoryPackageInventoryMode,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> RepositoryPackageInventoryDriverOutcome {
        let (source, observations) = match compute_repository_package_source(ctx, self, mode).await
        {
            ControlFlow::Continue(value) => value,
            ControlFlow::Break(value) => return value,
        };
        let mut prepared = match prepare_repository_package_evaluation(self, source, observations) {
            Ok(prepared) => prepared,
            Err(outcome) => return outcome,
        };
        let child_loads = std::mem::take(&mut prepared.child_loads);
        let (loaded_modules, observations) = match compute_repository_package_children(
            ctx,
            self,
            mode,
            child_loads,
            prepared.build_label.clone(),
            &prepared.build_origin,
            prepared.observations.dupe(),
        )
        .await
        {
            ControlFlow::Continue(value) => value,
            ControlFlow::Break(value) => return value,
        };
        evaluate_repository_package(
            ctx,
            self,
            prepared,
            &loaded_modules,
            mode,
            observations,
            capture_events,
            event_batch,
        )
        .await
    }
}

fn project_legacy_repository_package_inventory(
    value: RepositoryPackageInventoryDriverOutcome,
) -> SourcePreparationOutcome<RepositoryPackageInventoryCarrier> {
    match value {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            debug_assert!(observations.observations().is_empty());
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            panic!("legacy repository package inventory produced frontier error: {error}")
        }
    }
}

#[async_trait]
impl Key for RepositoryPackageInventoryKey {
    type Value = SourcePreparationOutcome<RepositoryPackageInventoryCarrier>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = self
            .compute_mode(
                ctx,
                RepositoryPackageInventoryMode::Legacy,
                capture_events,
                &mut event_batch,
            )
            .await;
        if capture_events && matches!(value, SourcePreparationOutcome::Complete(Ok(_))) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("RepositoryPackageInventoryKey stores one local Complete event batch");
        }
        project_legacy_repository_package_inventory(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for RepositoryPackageLoadObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedRepositoryPackageLoad, ObservedPathFrontierError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = ctx
            .compute(&RepositoryPackageInventoryObservationKey(
                self.0.inventory.clone(),
            ))
            .await
            .expect("observed repository package inventory DICE invariant");
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok(inventory)) => {
                SourcePreparationOutcome::Complete(Ok(ObservedRepositoryPackageLoad {
                    result: project_repository_package_policy(&self.0.inventory, inventory.result),
                    observations: inventory.observations,
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
impl Key for RepositoryPackageInventoryObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedRepositoryPackageInventory, ObservedPathFrontierError>,
    >;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = self
            .0
            .compute_mode(
                ctx,
                RepositoryPackageInventoryMode::Observed,
                capture_events,
                &mut event_batch,
            )
            .await;
        if capture_events && matches!(value, SourcePreparationOutcome::Complete(Ok(_))) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect(
                    "RepositoryPackageInventoryObservationKey stores one local Complete event batch",
                );
        }
        match value {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedRepositoryPackageInventory {
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

fn project_repository_package_policy(
    key: &RepositoryPackageInventoryKey,
    result: RepositoryPackageInventoryCarrier,
) -> RepositoryPackageInventoryCarrier {
    let policy_error = result
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|loaded| validate_loaded_repository_package(key, loaded).err());
    policy_error.map_or(result, |error| Arc::new(Err(error)))
}

#[async_trait]
impl Key for RepositoryPackageLoadKey {
    type Value = SourcePreparationOutcome<RepositoryPackageInventoryCarrier>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&self.inventory)
            .await
            .expect("repository package inventory DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => SourcePreparationOutcome::Complete(
                project_repository_package_policy(&self.inventory, result),
            ),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl BzlModuleEvaluator {
    pub fn new(workspace: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let workspace = workspace.into();
        let original_workspace = workspace.clone();
        let workspace = workspace.canonicalize().map_err(|error| {
            anyhow::anyhow!(
                "canonicalizing workspace for .bzl loading: {}: {error}",
                original_workspace.display()
            )
        })?;
        Ok(Self { workspace })
    }

    /// Evaluate a Bazel local load label from one package directory.
    ///
    pub async fn evaluate_load(
        &self,
        transaction: &mut DiceTransaction,
        requesting_package: impl AsRef<Path>,
        load: impl AsRef<str>,
    ) -> anyhow::Result<EvaluatedBzlModule> {
        let requesting_package = requesting_package.as_ref().to_path_buf();
        self.ensure_package(&requesting_package)?;
        let workspace = self.workspace.clone();
        let load = load.as_ref().to_owned();
        let path = transaction
            .compute(&LoadLabelResolutionKey {
                workspace: workspace.clone(),
                requesting_package,
                load: load.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("resolving local .bzl load through DICE: {error}"))?;
        let path = path
            .as_ref()
            .as_ref()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let load_label = bzl_source_label(&workspace, path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = transaction
            .compute(&BzlModuleEvalKey {
                workspace,
                path: path.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("evaluating local .bzl load through DICE: {error}"))?;
        let module = module
            .as_ref()
            .as_ref()
            .map_err(|error| anyhow::anyhow!(load_error(&load_label, error).to_string()))?;
        Ok(EvaluatedBzlModule {
            path: module.path.clone(),
            loads: module.loads.clone(),
            manifest: module.manifest.clone(),
        })
    }

    /// Evaluate a package BUILD file with the currently supported Bazel globals
    /// and the same DICE-backed local `.bzl` graph used by [`Self::evaluate_load`].
    pub async fn evaluate_package(
        &self,
        transaction: &mut DiceTransaction,
        package: impl AsRef<Path>,
    ) -> anyhow::Result<LegacyLoadedPackage> {
        let package = package.as_ref().to_path_buf();
        self.ensure_package(&package)?;
        let workspace = self.workspace.clone();
        let package = transaction
            .compute(&PackageLoadKey { workspace, package })
            .await
            .map_err(|error| anyhow::anyhow!("loading BUILD package through DICE: {error}"))?;
        package
            .as_ref()
            .as_ref()
            .map(Clone::clone)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    /// Return the active BUILD companion for `package` using only its injected
    /// direct-directory observation. This is deliberately parse-independent:
    /// a broken companion BUILD file is still discoverable and no package load
    /// is computed as a side effect.
    pub async fn discover_build_file_companion(
        &self,
        transaction: &mut DiceTransaction,
        package: impl AsRef<Path>,
    ) -> anyhow::Result<Option<BuildFileCompanion>> {
        let package = package.as_ref().to_path_buf();
        self.ensure_package(&package)?;
        let value = transaction
            .compute(&WorkspaceDirectoryKey {
                workspace: self.workspace.clone(),
                directory: package.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("reading package directory through DICE: {error}"))?;
        companion_from_directory_value(&self.workspace, &package, &value)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn ensure_package(&self, package: &Path) -> anyhow::Result<()> {
        validate_normalized_workspace_package(&self.workspace, package)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

/// Discover a package's active BUILD companion using the existing
/// `WorkspaceDirectoryKey` only. Missing directories and missing BUILD files
/// are not errors for a load-label package, while observation read failures
/// remain errors.
pub async fn discover_build_file_companion(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    package: &Path,
) -> Result<Option<BuildFileCompanion>, LoadingError> {
    validate_normalized_workspace_package(workspace, package)?;
    let value = ctx
        .compute(&WorkspaceDirectoryKey {
            workspace: workspace.to_path_buf(),
            directory: package.to_path_buf(),
        })
        .await
        .map_err(|error| {
            LoadingError::new(format!("reading package directory through DICE: {error}"))
        })?;
    companion_from_directory_value(workspace, package, &value)
}

fn companion_from_directory_value(
    workspace: &Path,
    package: &Path,
    value: &WorkspaceDirectoryValue,
) -> Result<Option<BuildFileCompanion>, LoadingError> {
    let entries = match value {
        WorkspaceDirectoryValue::Present(entries) => entries,
        WorkspaceDirectoryValue::Absent => return Ok(None),
        WorkspaceDirectoryValue::ReadError(error) => {
            return Err(LoadingError::new(format!(
                "reading package directory {}: {error}",
                package.display()
            )));
        }
    };
    let Some(basename) = ["BUILD.bazel", "BUILD"].into_iter().find(|name| {
        entries.iter().any(|entry| {
            entry.name.as_str() == *name
                && matches!(
                    entry.kind,
                    WorkspaceDirectoryEntryKind::RegularFile | WorkspaceDirectoryEntryKind::Symlink
                )
        })
    }) else {
        return Ok(None);
    };
    let path = package.join(basename);
    Ok(Some(BuildFileCompanion {
        label: canonical_root_label(&build_source_label(workspace, &path)?)?,
        path,
    }))
}

#[async_trait]
impl Key for BzlParseKey {
    type Value = LoadResult<ParsedBzl>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let source = match observed_file(ctx, &self.workspace, &self.path).await {
            Ok(source) => source,
            Err(error) => return Arc::new(Err(error)),
        };
        Arc::new((|| {
            let ast = AstModule::parse(
                &starlark_source_name(&self.path)
                    .ok_or_else(|| LoadingError::new("invalid .bzl source path"))?,
                source.as_ref().clone(),
                &Dialect::Bazel,
            )
            .map_err(|error| {
                let module = self
                    .path
                    .strip_prefix(&self.workspace)
                    .unwrap_or(&self.path)
                    .to_string_lossy()
                    .replace('\\', "/");
                LoadingError::new(format!("{error}\ncompilation of module '{module}' failed"))
            })?;
            Ok(ParsedBzl {
                source_digest: digest(&source),
                source: source.as_ref().clone(),
                loads: ast
                    .loads()
                    .into_iter()
                    .map(|load| load.module_id.to_owned())
                    .collect(),
            })
        })())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for LoadLabelResolutionKey {
    type Value = LoadResult<PathBuf>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(resolve_local_load(
            &self.workspace,
            &self.requesting_package,
            &self.load,
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for BzlModuleEvalKey {
    type Value = LoadResult<FrozenBzlModule>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = async {
            let parsed = match ctx
                .compute(&BzlParseKey {
                    workspace: self.workspace.clone(),
                    path: self.path.clone(),
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(parsed) => parsed.clone(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };

            let mut loaded_modules = Vec::with_capacity(parsed.loads.len());
            for load in &parsed.loads {
                let resolved = match ctx
                    .compute(&LoadLabelResolutionKey {
                        workspace: self.workspace.clone(),
                        requesting_package: self
                            .path
                            .parent()
                            .unwrap_or(&self.workspace)
                            .to_path_buf(),
                        load: load.clone(),
                    })
                    .await
                {
                    Ok(value) => match value.as_ref() {
                        Ok(path) => path.clone(),
                        Err(error) => return Arc::new(Err(error.clone())),
                    },
                    Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
                };
                let load_label = match bzl_source_label(&self.workspace, &resolved) {
                    Ok(label) => label,
                    Err(error) => return Arc::new(Err(error)),
                };
                let child = BzlModuleEvalKey {
                    workspace: self.workspace.clone(),
                    path: resolved,
                };
                let cycle_guard = match ctx.cycle_guard::<BzlLoadCycleGuard>() {
                    Ok(guard) => guard,
                    Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
                };
                let result = match cycle_guard {
                    Some(guard) => match guard.guard_this(ctx.compute(&child)).await {
                        Ok(result) => result,
                        Err(cycle) => {
                            let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                            return Arc::new(Err(LoadingError::load_cycle(cycle)));
                        }
                    },
                    None => ctx.compute(&child).await,
                };
                let module = match result {
                    Ok(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => return Arc::new(Err(load_error(&load_label, error))),
                    },
                    Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
                };
                loaded_modules.push((load.clone(), module));
            }

            Arc::new((|| {
                let ast = AstModule::parse(
                    &starlark_source_name(&self.path)
                        .ok_or_else(|| LoadingError::new("invalid .bzl source path"))?,
                    parsed.source.clone(),
                    &Dialect::Bazel,
                )
                .map_err(|error| LoadingError::new(error.to_string()))?;
                let module = Module::new();
                let manifest = BzlLoadManifest::new(
                    bzl_module_identity(&self.workspace, &self.path)?,
                    parsed.source_digest,
                    loaded_modules.iter().map(|(_, module)| module),
                );
                validate_direct_bzl_load_visibilities(
                    manifest.root.label.package(),
                    &loaded_modules,
                )
                .map_err(|error| LoadingError::new(error.to_string()))?;
                let loader = LocalBzlLoader {
                    modules: loaded_modules
                        .iter()
                        .map(|(load, module)| (load.as_str(), module.module.dupe()))
                        .collect(),
                };
                let evaluation_context = BzlEvaluationContext::from_manifest(&manifest);
                let print_capture = capture_events.then(LoadingPrintCapture::default);
                let globals = loading_globals();
                {
                    let mut evaluator = Evaluator::new(&module);
                    evaluator.extra = Some(&evaluation_context);
                    evaluator.set_loader(&loader);
                    if let Some(print_capture) = print_capture.as_ref() {
                        evaluator.set_print_handler(print_capture);
                    }
                    let evaluation = evaluator.eval_module(ast, &globals).map(|_| ());
                    drop(evaluator);
                    event_batch = print_capture.map(LoadingPrintCapture::into_batch);
                    evaluation.map_err(|error| LoadingError::new(error.to_string()))?;
                }
                let module = module
                    .freeze()
                    .map_err(|error| LoadingError::new(format!("{error:?}")))?;
                Ok(FrozenBzlModule {
                    module,
                    path: self.path.clone(),
                    loads: parsed.loads.clone(),
                    retained_bzl_modules: retained_module_closure(&loaded_modules),
                    bzl_load_visibility: evaluation_context.bzl_load_visibility(),
                    manifest,
                })
            })())
        }
        .await;
        if capture_events {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("BzlModuleEvalKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for PackageLoadKey {
    type Value = LoadResult<LegacyLoadedPackage>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = async {
            let root_module_graph_value = match ctx
                .compute(&RootModuleGraphKey {
                    workspace: self.workspace.clone(),
                })
                .await
            {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let root_module_graph = match root_module_graph_value.as_ref() {
                Ok(graph) => graph,
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let _repository_mapping = &root_module_graph.repository_mapping;
            let listing = match ctx
                .compute(&PackageListingKey {
                    workspace: self.workspace.clone(),
                    package: self.package.clone(),
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(listing) => listing.dupe(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let primary_build = self.package.join("BUILD.bazel");
            let fallback_build = self.package.join("BUILD");
            let (build_file, source) =
                match observed_file(ctx, &self.workspace, &primary_build).await {
                    Ok(source) => (primary_build, source),
                    Err(error) if error.is_absent() => {
                        match observed_file(ctx, &self.workspace, &fallback_build).await {
                            Ok(source) => (fallback_build, source),
                            Err(error) if error.is_absent() => {
                                return Arc::new(Err(LoadingError::new(format!(
                                    "no BUILD.bazel or BUILD file in package {}",
                                    self.package.display()
                                ))));
                            }
                            Err(error) => return Arc::new(Err(error)),
                        }
                    }
                    Err(error) => return Arc::new(Err(error)),
                };
            let ast = match AstModule::parse(
                &build_file.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Bazel,
            ) {
                Ok(ast) => ast,
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };

            let mut loaded_modules = Vec::new();
            for load in ast.loads() {
                let load = load.module_id.to_owned();
                let resolved = match ctx
                    .compute(&LoadLabelResolutionKey {
                        workspace: self.workspace.clone(),
                        requesting_package: self.package.clone(),
                        load: load.clone(),
                    })
                    .await
                {
                    Ok(value) => match value.as_ref() {
                        Ok(path) => path.clone(),
                        Err(error) => return Arc::new(Err(error.clone())),
                    },
                    Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
                };
                let load_label = match bzl_source_label(&self.workspace, &resolved) {
                    Ok(label) => label,
                    Err(error) => return Arc::new(Err(error)),
                };
                let module = match ctx
                    .compute(&BzlModuleEvalKey {
                        workspace: self.workspace.clone(),
                        path: resolved,
                    })
                    .await
                {
                    Ok(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => {
                            return Arc::new(Err(load_error(&load_label, error)
                                .with_package_cycle_origin(&self.workspace, &self.package)));
                        }
                    },
                    Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
                };
                loaded_modules.push((load, module));
            }

            Arc::new((|| {
                let ast = AstModule::parse(
                    &build_file.display().to_string(),
                    source.as_ref().clone(),
                    &Dialect::Bazel,
                )
                .map_err(|error| LoadingError::new(error.to_string()))?;
                let package_label = self
                    .package
                    .strip_prefix(&self.workspace)
                    .map_err(|_| {
                        LoadingError::new(format!(
                            "package is outside workspace: {}",
                            self.package.display()
                        ))
                    })?
                    .to_string_lossy()
                    .replace('\\', "/");
                let package_identifier = PackageIdentifier::new(
                    CanonicalRepoName::root(),
                    PackagePath::parse(&package_label).map_err(LoadingError::new)?,
                );
                validate_direct_bzl_load_visibilities(&package_identifier, &loaded_modules)
                    .map_err(|error| LoadingError::new(error.to_string()))?;
                let print_capture = capture_events.then(|| Rc::new(LoadingPrintCapture::default()));
                let recorder = PackageRecorder::new(listing, package_label)
                    .with_print_capture(print_capture.clone());
                let module = Module::new();
                let loader = LocalBzlLoader {
                    modules: loaded_modules
                        .iter()
                        .map(|(load, module)| (load.as_str(), module.module.dupe()))
                        .collect(),
                };
                let globals = build_file_loading_globals();
                {
                    let mut evaluator = Evaluator::new(&module);
                    evaluator.extra = Some(&recorder);
                    evaluator.set_loader(&loader);
                    if let Some(print_capture) = print_capture.as_deref() {
                        evaluator.set_print_handler(print_capture);
                    }
                    let evaluation = evaluator.eval_module(ast, &globals).map(|_| ());
                    drop(evaluator);
                    event_batch = print_capture
                        .as_deref()
                        .map(LoadingPrintCapture::drain_batch);
                    evaluation.map_err(|error| LoadingError::new(error.to_string()))?;
                }
                let direct_load_roots = first_seen_direct_roots(&loaded_modules);
                let retained_bzl_modules = flattened_lifetime_closure(&loaded_modules);
                let reachable_loads = retained_bzl_modules
                    .iter()
                    .map(|entry| entry.identity.clone())
                    .collect::<Vec<_>>();
                let load_fingerprint = package_load_fingerprint(&loaded_modules);
                Ok(recorder.finish_legacy(
                    self.package.clone(),
                    build_file,
                    direct_load_roots.into(),
                    reachable_loads.into(),
                    load_fingerprint,
                    retained_bzl_modules.into(),
                ))
            })())
        }
        .await;
        if capture_events {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("PackageLoadKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

struct LocalBzlLoader<'a> {
    modules: Vec<(&'a str, FrozenModule)>,
}

impl starlark::eval::FileLoader for LocalBzlLoader<'_> {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        self.modules
            .iter()
            .find(|(load, _)| *load == path)
            .map(|(_, module)| module.dupe())
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "local .bzl loader does not know module `{path}`"
                ))
            })
    }
}

fn resolve_local_load(
    workspace: &Path,
    requesting_package: &Path,
    load: &str,
) -> Result<PathBuf, LoadingError> {
    let label = if let Some(relative) = load.strip_prefix(':') {
        let relative_label =
            LoadLabel::parse(&format!("//:{relative}")).map_err(LoadingError::new)?;
        let path = requesting_package.join(relative_label.label().target().as_str());
        return ensure_within_workspace(workspace, path, load);
    } else {
        LoadLabel::parse(load).map_err(LoadingError::new)?
    };
    if !label.label().repo().is_root() {
        return Err(LoadingError::new(format!(
            "external repository load is not available in the local Stage 4 loader: {load}"
        )));
    }
    let package = label.label().package().as_str();
    let path = workspace
        .join(package)
        .join(label.label().target().as_str());
    ensure_within_workspace(workspace, path, load)
}

fn bzl_source_label(workspace: &Path, path: &Path) -> Result<String, LoadingError> {
    let relative = path.strip_prefix(workspace).map_err(|_| {
        LoadingError::new(format!(
            ".bzl source is outside workspace {}: {}",
            workspace.display(),
            path.display()
        ))
    })?;
    let target = relative
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            LoadingError::new(format!("invalid .bzl source path: {}", path.display()))
        })?;
    let package = relative
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");
    Ok(format!("//{package}:{target}"))
}

fn build_source_label(workspace: &Path, path: &Path) -> Result<String, LoadingError> {
    bzl_source_label(workspace, path)
}

fn bzl_module_identity(workspace: &Path, path: &Path) -> Result<BzlModuleIdentity, LoadingError> {
    Ok(BzlModuleIdentity {
        label: canonical_root_label(&bzl_source_label(workspace, path)?)?,
        workspace_path: path.to_path_buf(),
        repository_mapping: Arc::from([]),
    })
}

fn validate_normalized_workspace_package(
    workspace: &Path,
    package: &Path,
) -> Result<(), LoadingError> {
    if !workspace.is_absolute()
        || !package.is_absolute()
        || !package.starts_with(workspace)
        || workspace.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || has_cur_or_parent_component(workspace)
        || package.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || has_cur_or_parent_component(package)
    {
        return Err(LoadingError::new(format!(
            "package must be a normalized absolute path inside workspace {}: {}",
            workspace.display(),
            package.display()
        )));
    }
    Ok(())
}

fn has_cur_or_parent_component(path: &Path) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .split(|byte| *byte == b'/')
        .any(|component| matches!(component, b"." | b".."))
}

fn canonical_root_label(label: &str) -> Result<CanonicalLabel, LoadingError> {
    CanonicalLabel::parse(&format!("@@{label}")).map_err(LoadingError::new)
}

fn ensure_within_workspace(
    workspace: &Path,
    path: PathBuf,
    load: &str,
) -> Result<PathBuf, LoadingError> {
    if !path.starts_with(workspace) {
        return Err(LoadingError::new(format!(
            "load label escapes workspace: {load}"
        )));
    }
    Ok(path)
}

fn digest(source: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.finalize().into()
}

fn manifest_fingerprint(
    source_digest: [u8; 32],
    direct_modules: impl IntoIterator<Item = impl std::borrow::Borrow<FrozenBzlModule>>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-v2-bzl-load-manifest\0");
    hasher.update(source_digest);
    for module in direct_modules {
        let module = module.borrow();
        fingerprint_identity(&mut hasher, &module.manifest.root);
        hasher.update(module.manifest.fingerprint);
    }
    hasher.finalize().into()
}

fn package_load_fingerprint<'a>(direct_loads: &'a [(String, FrozenBzlModule)]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-v2-package-load-manifest\0");
    let mut seen_labels = SmallSet::with_capacity(direct_loads.len());
    for (_, module) in direct_loads {
        if seen_labels.insert(module.manifest.root.label.clone()) {
            fingerprint_identity(&mut hasher, &module.manifest.root);
            hasher.update(module.manifest.fingerprint);
        }
    }
    hasher.finalize().into()
}

fn fingerprint_identity(hasher: &mut Sha256, identity: &BzlModuleIdentity) {
    let label = identity.label.to_string();
    let label = label.as_bytes();
    let path = identity.workspace_path.as_os_str().as_encoded_bytes();
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update((path.len() as u64).to_be_bytes());
    hasher.update(path);
    hasher.update((identity.repository_mapping.len() as u64).to_be_bytes());
    for (apparent, canonical) in identity.repository_mapping.iter() {
        let apparent = apparent.as_str().as_bytes();
        let canonical = canonical.as_str().as_bytes();
        hasher.update((apparent.len() as u64).to_be_bytes());
        hasher.update(apparent);
        hasher.update((canonical.len() as u64).to_be_bytes());
        hasher.update(canonical);
    }
}

fn first_seen_direct_roots(loaded_modules: &[(String, FrozenBzlModule)]) -> Vec<BzlModuleIdentity> {
    let mut seen_labels = SmallSet::with_capacity(loaded_modules.len());
    loaded_modules
        .iter()
        .filter_map(|(_, module)| {
            seen_labels
                .insert(module.manifest.root.label.clone())
                .then(|| module.manifest.root.clone())
        })
        .collect()
}

fn retained_module_closure(
    loaded_modules: &[(String, FrozenBzlModule)],
) -> Arc<[FrozenBzlLifetimeEntry]> {
    flattened_lifetime_closure(loaded_modules).into()
}

fn flattened_lifetime_closure(
    loaded_modules: &[(String, FrozenBzlModule)],
) -> Vec<FrozenBzlLifetimeEntry> {
    let mut seen_labels = SmallSet::new();
    let mut entries = Vec::new();
    for (_, loaded_module) in loaded_modules {
        for (identity, module) in loaded_module.lifetime_modules() {
            if seen_labels.insert(identity.label.clone()) {
                entries.push(FrozenBzlLifetimeEntry {
                    identity: identity.clone(),
                    module,
                });
            }
        }
    }
    entries
}

#[cfg(all(test, unix))]
#[path = "host_package_attempt_tests.rs"]
mod host_package_attempt_tests;

#[cfg(all(test, unix))]
#[path = "host_package_load_tests.rs"]
mod host_package_load_tests;

#[cfg(test)]
mod module_extension_definition_loading_tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use bv::RepositoryMaterializationResult as MaterializationResult;
    use bv::RepositoryMaterializationSuccess as MaterializationSuccess;
    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_bzlmod_v2 as bv;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RootModuleCommandPolicyKey;
    use slug_bzlmod_v2::RootModuleEnvironmentPolicyKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochError;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::module_extension::HostPureModuleExtensionInvocationError;
    use crate::module_extension::HostPureModuleExtensionInvocationsError;
    use crate::module_extension::HostPureModuleExtensionInvocationsKey;
    use crate::module_extension::test_support::InvocationConsumerKey;
    use crate::module_extension::test_support::InvokePreparedKey;

    const WORKSPACE: &str = "/extension-definition-loading";

    #[test]
    fn builtin_external_repository_promotion_gate_is_exactly_one_apparent_load() {
        let loads = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        for excluded in [
            loads(&[]),
            loads(&[":local.bzl"]),
            loads(&["@@rules_cc+//:defs.bzl"]),
            loads(&["@//:defs.bzl"]),
            loads(&["@rules_cc//:defs.bzl", ":other.bzl"]),
        ] {
            assert_eq!(admitted_builtin_external_repository_load(&excluded), None);
        }
        let admitted = loads(&["@rules_cc//cc/toolchains:toolchain_config_utils.bzl"]);
        assert_eq!(
            admitted_builtin_external_repository_load(&admitted),
            Some("@rules_cc//cc/toolchains:toolchain_config_utils.bzl")
        );
    }

    #[test]
    fn pre_host_package_key_cannot_publish_complete_metadata() {
        fn assert_legacy_value<K: Key<Value = LoadResult<LegacyLoadedPackage>>>() {}
        fn assert_allocative<T: allocative::Allocative>() {}
        assert_legacy_value::<PackageLoadKey>();
        assert_allocative::<crate::PackageEvaluation>();
        assert_allocative::<LegacyLoadedPackage>();
        assert_allocative::<LoadedPackage>();
        assert_eq!(
            std::mem::size_of::<LegacyLoadedPackage>(),
            std::mem::size_of::<crate::PackageEvaluation>()
        );
        assert_eq!(
            std::mem::size_of::<LoadedPackage>(),
            std::mem::size_of::<crate::PackageEvaluation>()
                + std::mem::size_of::<std::sync::Arc<slug_build_api_v2::RunfilesPackageMetadata>>()
        );
    }

    #[derive(Debug, Clone)]
    struct BzlActivation {
        label: CanonicalLabel,
        kind: ActivationKind,
        batch: Option<EventBatch>,
        observed: bool,
    }

    #[derive(Debug, Clone)]
    struct BzlActivationRow {
        key: String,
        family: BzlTrackerFamily,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Debug, Clone)]
    struct BzlDependencyRow {
        key: String,
        family: BzlTrackerFamily,
        dependencies: Vec<String>,
        dependency_families: Vec<BzlTrackerFamily>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BzlTrackerFamily {
        Other,
        BzlmodCommandPolicy,
        BzlmodEnvironmentPolicy,
    }

    fn tracker_family(key: &DynKey) -> BzlTrackerFamily {
        if key.downcast_ref::<RootModuleCommandPolicyKey>().is_some() {
            BzlTrackerFamily::BzlmodCommandPolicy
        } else if key
            .downcast_ref::<RootModuleEnvironmentPolicyKey>()
            .is_some()
        {
            BzlTrackerFamily::BzlmodEnvironmentPolicy
        } else {
            BzlTrackerFamily::Other
        }
    }
    #[derive(Default)]
    struct BzlEventTracker {
        events: Mutex<Vec<BzlActivation>>,
        activation_rows: Mutex<Vec<BzlActivationRow>>,
        dependency_rows: Mutex<Vec<BzlDependencyRow>>,
        legacy_modules: AtomicUsize,
        observed_modules: AtomicUsize,
        legacy_sources: AtomicUsize,
        observed_sources: AtomicUsize,
    }

    impl BzlEventTracker {
        fn take(&self) -> Vec<BzlActivation> {
            std::mem::take(&mut *self.events.lock().unwrap())
        }

        fn take_rows(&self) -> (Vec<BzlActivationRow>, Vec<BzlDependencyRow>) {
            (
                std::mem::take(&mut *self.activation_rows.lock().unwrap()),
                std::mem::take(&mut *self.dependency_rows.lock().unwrap()),
            )
        }

        fn assert_reused_loaded_bzl_result_arcs(
            rows: &[BzlActivationRow],
            before: &ObservedLoadedHandles,
            after: &ObservedLoadedHandles,
        ) {
            for (name, previous, current) in [
                ("ext.bzl", &before.ext, &after.ext),
                ("child.bzl", &before.child, &after.child),
                ("other.bzl", &before.other, &after.other),
            ] {
                let key = HostBzlModuleObservationKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    observed_test_label(name),
                )
                .to_string();
                if rows
                    .iter()
                    .any(|row| row.key == key && row.kind == ActivationKind::Reused)
                {
                    assert!(Arc::ptr_eq(previous, current));
                }
            }
        }

        fn take_empty_observed_batches(&self, only: Option<&CanonicalLabel>) -> bool {
            let activations = self.take();
            only.is_none_or(|name| {
                matches!(
                    activations.as_slice(),
                    [activation] if activation.observed && &activation.label == name
                )
            }) && activations.into_iter().all(|activation| {
                !activation.observed
                    || matches!(activation.batch, Some(batch) if batch.events().is_empty())
            })
        }
    }

    impl ActivationTracker for BzlEventTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            let dependencies = dependencies
                .map(|dependency| (dependency.to_string(), tracker_family(dependency)))
                .collect::<Vec<_>>();
            self.dependency_rows.lock().unwrap().push(BzlDependencyRow {
                key: key.to_string(),
                family: tracker_family(key),
                dependencies: dependencies.iter().map(|(key, _)| key.clone()).collect(),
                dependency_families: dependencies.iter().map(|(_, family)| *family).collect(),
            });
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let batch = || {
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe)
            };
            self.activation_rows.lock().unwrap().push(BzlActivationRow {
                key: key.to_string(),
                family: tracker_family(key),
                kind: activation.kind(),
                batch: batch(),
            });
            if let Some(key) = key.downcast_ref::<HostBzlModuleEvalKey>() {
                self.legacy_modules.fetch_add(1, Ordering::SeqCst);
                self.events.lock().unwrap().push(BzlActivation {
                    label: key.label.canonical_label(),
                    kind: activation.kind(),
                    batch: batch(),
                    observed: false,
                });
            } else if let Some(key) = key.downcast_ref::<HostBzlModuleObservationKey>() {
                self.observed_modules.fetch_add(1, Ordering::SeqCst);
                self.events.lock().unwrap().push(BzlActivation {
                    label: key.label.canonical_label(),
                    kind: activation.kind(),
                    batch: batch(),
                    observed: true,
                });
            } else if key.downcast_ref::<RootPackageSourceKey>().is_some() {
                self.legacy_sources.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<RootPackageSourceObservationKey>()
                .is_some()
            {
                self.observed_sources.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<HostPureModuleExtensionInvocationsKey>()
                .is_some()
            {
                self.events.lock().unwrap().push(BzlActivation {
                    label: CanonicalLabel::parse("@@//:module_extension_invocation").unwrap(),
                    kind: activation.kind(),
                    batch: batch(),
                    observed: false,
                });
            }
        }
    }

    fn host_bzl_user_data(
        detector: Arc<dyn dice::UserCycleDetector>,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> UserComputationData {
        let mut data = UserComputationData {
            cycle_detector: Some(detector),
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data
    }

    async fn host_bzl_transaction(
        dice: &Arc<Dice>,
        detector: Arc<dyn dice::UserCycleDetector>,
        tracker: Arc<BzlEventTracker>,
    ) -> DiceTransaction {
        dice.updater_with_data(host_bzl_user_data(detector, Some(tracker)))
            .commit()
            .await
    }

    async fn compute(
        dice: &Arc<Dice>,
        extension_source: &str,
    ) -> HostLoadedModuleExtensionDefinitionsOutcome {
        compute_case(
            dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n",
            extension_source,
            "def implementation(ctx):\n    pass\n",
            true,
            None,
        )
        .await
    }

    async fn compute_case(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        child_source: &str,
        child_present: bool,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> HostLoadedModuleExtensionDefinitionsOutcome {
        case_transaction(
            dice,
            module_source,
            extension_source,
            child_source,
            child_present,
            tracker,
        )
        .await
        .compute(&HostLoadedModuleExtensionDefinitionsKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        ))
        .await
        .unwrap()
    }

    async fn compute_prepared_case(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> HostPreparedModuleExtensionInputsOutcome {
        case_transaction(dice, module_source, extension_source, "", true, tracker)
            .await
            .compute(&HostPreparedModuleExtensionInputsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_invocation_case(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> crate::module_extension::HostPureModuleExtensionInvocationsOutcome {
        case_transaction(dice, module_source, extension_source, "", true, tracker)
            .await
            .compute(&HostPureModuleExtensionInvocationsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn case_transaction(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        child_source: &str,
        child_present: bool,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> DiceTransaction {
        case_transaction_with_other(
            dice,
            module_source,
            extension_source,
            child_source,
            "def implementation(ctx):\n    pass\nother=module_extension(implementation=implementation)\n",
            child_present,
            tracker,
        )
        .await
    }

    #[tokio::test]
    async fn host_bzl_context_is_recursive_and_structural_for_legacy_and_observed_keys() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let label = HostRootBzlLabel::new(
            PackagePath::root(),
            RootPackageBzlTarget::parse("ext.bzl").unwrap(),
        );
        let mut transaction = case_transaction(
            &dice,
            "module(name='bazel_tools')\n",
            "load('//:child.bzl', 'version')\ncaptured = version\n",
            "version = native.bazel_version\n",
            true,
            None,
        )
        .await;

        let build_key = HostBzlModuleEvalKey::new(workspace.dupe(), label.clone());
        let bzlmod_key = HostBzlModuleEvalKey::new_bzlmod(workspace.dupe(), label.clone());
        let build = transaction.compute(&build_key).await.unwrap();
        let bzlmod = transaction.compute(&bzlmod_key).await.unwrap();
        let restored = transaction.compute(&build_key).await.unwrap();
        assert!(HostBzlModuleEvalKey::equality(&build, &restored));
        assert!(matches!(build, SourcePreparationOutcome::Complete(value) if value.is_err()));
        let SourcePreparationOutcome::Complete(value) = bzlmod else {
            panic!("Bzlmod load unexpectedly requested preparation")
        };
        assert_eq!(
            value
                .as_ref()
                .as_ref()
                .unwrap()
                .module
                .get("captured")
                .unwrap()
                .unpack_str(),
            Some("9.2.0")
        );

        let build_key = HostBzlModuleObservationKey::new(workspace.dupe(), label.clone());
        let bzlmod_key = HostBzlModuleObservationKey::new_bzlmod(workspace, label);
        assert_ne!(build_key, bzlmod_key);
        assert_ne!(build_key.cycle_identity(), bzlmod_key.cycle_identity());
        let build = transaction.compute(&build_key).await.unwrap();
        let bzlmod = transaction.compute(&bzlmod_key).await.unwrap();
        let restored = transaction.compute(&build_key).await.unwrap();
        assert!(HostBzlModuleObservationKey::equality(&build, &restored));
        assert!(matches!(
            build,
            SourcePreparationOutcome::Complete(Ok(value)) if value.result().is_err()
        ));
        let SourcePreparationOutcome::Complete(Ok(value)) = bzlmod else {
            panic!("observed Bzlmod load did not complete")
        };
        assert_eq!(
            value
                .result()
                .as_ref()
                .unwrap()
                .module
                .get("captured")
                .unwrap()
                .unpack_str(),
            Some("9.2.0")
        );
    }

    #[tokio::test]
    async fn external_bzl_context_is_recursive_for_root_and_mapped_canonical_children() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = case_transaction(
            &dice,
            "# external context proof\nmodule(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\n",
            "",
            "",
            true,
            None,
        )
        .await;
        let route_key = slug_bzlmod_v2::RootRepositoryRouteKey::new(
            workspace.dupe(),
            ApparentRepoName::new("dep").unwrap(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(route) =
            transaction.compute(&route_key).await.unwrap()
        else {
            panic!("direct-local route unexpectedly requested preparation")
        };
        let route = route.as_ref().as_ref().unwrap().clone();
        let label = |name| {
            RepositoryBzlLabel::new(
                PackagePath::root(),
                RootPackageBzlTarget::parse(name).unwrap(),
            )
            .unwrap()
        };
        let build_key = ExternalBzlModuleEvalKey::new(route.clone(), label("parent.bzl"));
        let bzlmod_key = ExternalBzlModuleEvalKey::new_bzlmod(route, label("parent.bzl"));
        let build = transaction.compute(&build_key).await.unwrap();
        let bzlmod = transaction.compute(&bzlmod_key).await.unwrap();
        let restored = transaction.compute(&build_key).await.unwrap();
        assert!(ExternalBzlModuleEvalKey::equality(&build, &restored));
        assert!(matches!(build, SourcePreparationOutcome::Complete(value) if value.is_err()));
        assert!(matches!(
            &bzlmod,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().as_ref().unwrap().module.get("captured").unwrap().unpack_str()
                    == Some("9.2.0")
        ));
        let canonical_key = HostCanonicalRepositoryLoadRouteKey::new(
            workspace,
            CanonicalRepoName::new("dep+").unwrap(),
        );
        let SourcePreparationOutcome::Complete(canonical) =
            transaction.compute(&canonical_key).await.unwrap()
        else {
            panic!("canonical route unexpectedly requested preparation")
        };
        let input = canonical.as_ref().as_ref().unwrap().input().clone();
        let build_key = ExternalBzlModuleObservationKey::new_canonical(
            input.clone(),
            label("mapped_parent.bzl"),
        );
        let bzlmod_key = ExternalBzlModuleObservationKey::new_canonical_bzlmod(
            input,
            label("mapped_parent.bzl"),
        );
        let build = transaction.compute(&build_key).await.unwrap();
        let bzlmod = transaction.compute(&bzlmod_key).await.unwrap();
        let restored = transaction.compute(&build_key).await.unwrap();
        assert!(ExternalBzlModuleObservationKey::equality(&build, &restored));
        assert!(matches!(
            build,
            SourcePreparationOutcome::Complete(Ok(value)) if value.result().is_err()
        ));
        assert!(matches!(
            &bzlmod,
            SourcePreparationOutcome::Complete(Ok(value))
                if value.result().as_ref().as_ref().unwrap().module.get("captured").unwrap()
                    .unpack_str() == Some("9.2.0")
        ));
    }

    async fn case_transaction_with_other(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        child_source: &str,
        other_source: &str,
        child_present: bool,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let dep_sources = [
            ("dep/MODULE.bazel", "module(name='dep',version='1.0')\n"),
            (
                "dep/parent.bzl",
                "load(':child.bzl','version')\ncaptured=version\n",
            ),
            (
                "dep/mapped_parent.bzl",
                "load('@dep//:child.bzl','version')\ncaptured=version\n",
            ),
            ("dep/child.bzl", "version=native.bazel_version\n"),
            ("dep/BUILD.bazel", ""),
        ];
        let mut updater = dice.updater_with_data(host_bzl_user_data(
            crate::cycle_detector::bzl_load_cycle_detector(),
            tracker,
        ));
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter(
                        [
                            (
                                workspace.as_path().join("MODULE.bazel"),
                                WorkspaceFileValue::Present(Arc::new(module_source.to_owned())),
                            ),
                            (
                                workspace.as_path().join("ext.bzl"),
                                WorkspaceFileValue::Present(Arc::new(extension_source.to_owned())),
                            ),
                            (
                                workspace.as_path().join("child.bzl"),
                                if child_present {
                                    WorkspaceFileValue::Present(Arc::new(child_source.to_owned()))
                                } else {
                                    WorkspaceFileValue::Absent
                                },
                            ),
                            (
                                workspace.as_path().join("other.bzl"),
                                WorkspaceFileValue::Present(Arc::new(other_source.to_owned())),
                            ),
                            (
                                workspace.as_path().join("BUILD.bazel"),
                                WorkspaceFileValue::Present(Arc::new(String::new())),
                            ),
                        ]
                        .into_iter()
                        .chain(dep_sources.into_iter().map(
                            |(name, source)| {
                                (
                                    workspace.as_path().join(name),
                                    WorkspaceFileValue::Present(Arc::new(source.to_owned())),
                                )
                            },
                        )),
                    )),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        let mut attributes = SmallMap::new();
        attributes.insert(
            "path".into(),
            bv::OverrideAttributeValue::String("dep".into()),
        );
        let request = Arc::new(bv::RepositoryMaterializationRequest {
            id: bv::RepositoryMaterializationRequestId {
                workspace: workspace.dupe(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: bv::RepoSpec {
                rule_id: bv::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(attributes),
            },
            kind: bv::RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(format!("{WORKSPACE}/dep")).unwrap(),
            },
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.dupe(),
                    [bv::RepositoryMaterializationEpochEntry {
                        request,
                        result: MaterializationResult::Success(MaterializationSuccess::Local),
                    }]
                    .into_iter()
                    .filter(|_| module_source.contains("external context proof")),
                )
                .unwrap(),
            )])
            .unwrap();
        let path_epoch = PathObservationEpoch::new(
            [
                ("/", 1),
                (WORKSPACE, 2),
                ("/extension-definition-loading/dep", 25),
            ]
            .into_iter()
            .map(|(path, stamp)| {
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::Directory,
                        stamp,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                )
            })
            .chain(
                [
                    "REPO.bazel",
                    ".bazelignore",
                    "BUILD",
                    "MODULE.bazel.lock",
                    "dep/REPO.bazel",
                    "dep/.bazelignore",
                    "dep/BUILD",
                    "dep/MODULE.bazel.lock",
                ]
                .into_iter()
                .map(|name| {
                    (
                        PathObservationDemand::new(
                            PathObservationNamespace::Host,
                            NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                            PathObservationOperation::Lstat,
                        ),
                        PathObservationResult::Lstat(PathOperationResult::Missing),
                    )
                }),
            )
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/BUILD.bazel")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    20,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            )))
            .chain(
                ["MODULE.bazel", "ext.bzl", "child.bzl", "other.bzl"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| {
                        (
                            PathObservationDemand::new(
                                PathObservationNamespace::Host,
                                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                                PathObservationOperation::Lstat,
                            ),
                            PathObservationResult::Lstat(
                                if name == "child.bzl" && !child_present {
                                    PathOperationResult::Missing
                                } else {
                                    PathOperationResult::Present(PathLstat::new(
                                        PathNodeKind::RegularFile,
                                        index as i64 + 30,
                                        1,
                                        1,
                                        1,
                                        0o644,
                                    ))
                                },
                            ),
                        )
                    }),
            )
            .chain(
                dep_sources
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, _))| {
                        (
                            PathObservationDemand::new(
                                PathObservationNamespace::Host,
                                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                                PathObservationOperation::Lstat,
                            ),
                            PathObservationResult::Lstat(PathOperationResult::Present(
                                PathLstat::new(
                                    PathNodeKind::RegularFile,
                                    index as i64 + 40,
                                    1,
                                    1,
                                    1,
                                    0o644,
                                ),
                            )),
                        )
                    }),
            )
            .chain(
                [
                    ("MODULE.bazel", module_source.as_bytes()),
                    ("ext.bzl", extension_source.as_bytes()),
                    ("child.bzl", child_source.as_bytes()),
                    ("other.bzl", other_source.as_bytes()),
                ]
                .into_iter()
                .map(|(name, bytes)| {
                    (
                        PathObservationDemand::new(
                            PathObservationNamespace::Host,
                            NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                            PathObservationOperation::FileBytes,
                        ),
                        PathObservationResult::FileBytes(
                            if name == "child.bzl" && !child_present {
                                PathOperationResult::Missing
                            } else {
                                PathOperationResult::Present(Arc::from(bytes))
                            },
                        ),
                    )
                }),
            )
            .chain(dep_sources.into_iter().map(|(name, source)| {
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                        PathObservationOperation::FileBytes,
                    ),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        source.as_bytes(),
                    ))),
                )
            })),
        )
        .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, path_epoch)])
            .unwrap();
        updater.commit().await
    }

    fn source(environment: &str, facts: i32) -> String {
        format!(
            "print('extension')\n\
             load('//:child.bzl','implementation')\n\
             tag=tag_class(attrs={{'value':attr.string(mandatory=True)}})\n\
             ext=module_extension(implementation=implementation,tag_classes={{'tag':tag}},environ=['{environment}'],facts_version={facts})\n"
        )
    }

    #[tokio::test]
    async fn real_definition_loading_changes_reuses_restores_and_types_errors() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let a = compute(&dice, &source("A", 1)).await;
        let warm = compute(&dice, &source("A", 1)).await;
        assert!(
            HostLoadedModuleExtensionDefinitionsKey::validity(&a),
            "{a:?}"
        );
        assert!(HostLoadedModuleExtensionDefinitionsKey::equality(&a, &warm));
        let SourcePreparationOutcome::Complete(value) = &a else {
            panic!("definition loading must complete")
        };
        let value = value.as_ref().as_ref().unwrap();
        assert_eq!(value.requests.parts().1.len(), 1);
        assert_eq!(value.definitions[0].definition.environment.as_ref(), ["A"]);
        assert_eq!(value.definitions[0].manifest.direct_children.len(), 1);
        let b = compute(&dice, &source("B", 2)).await;
        assert!(!HostLoadedModuleExtensionDefinitionsKey::equality(&a, &b));
        let restored = compute(&dice, &source("A", 1)).await;
        assert!(HostLoadedModuleExtensionDefinitionsKey::equality(
            &a, &restored
        ));
        let wrong = compute(&dice, "ext=1\n").await;
        assert!(matches!(
            wrong,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostLoadedModuleExtensionDefinitionsError::Request {
                        error: HostLoadedModuleExtensionDefinitionError::WrongKind { .. },
                        ..
                    })
                )
        ));
    }

    #[tokio::test]
    async fn module_extension_selection_uses_assigned_globals_at_either_visibility() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let implementation = "def implementation(ctx):\n    pass\n";
        for (requested, extension) in [
            (
                "_private",
                format!(
                    "{implementation}_private=module_extension(implementation=implementation)\n"
                ),
            ),
            (
                "public_alias",
                format!(
                    "{implementation}_private=module_extension(implementation=implementation)\npublic_alias=_private\n"
                ),
            ),
        ] {
            let outcome = compute_case(
                &dice,
                &format!(
                    "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','{requested}')\n"
                ),
                &extension,
                "",
                true,
                None,
            )
            .await;
            assert!(
                matches!(&outcome, SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()),
                "assigned extension `{requested}` must load: {outcome:?}"
            );
        }

        let reexported = compute_case(
            &dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','reexported')\n",
            "load('//:child.bzl','child')\nreexported=child\n",
            &format!("{implementation}child=module_extension(implementation=implementation)\n"),
            true,
            None,
        )
        .await;
        assert!(
            matches!(&reexported, SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()),
            "assigned reexport must load: {reexported:?}"
        );

        for requested in ["child", "_loaded"] {
            let load = if requested == "child" {
                "load('//:child.bzl','child')\n"
            } else {
                "load('//:child.bzl',_loaded='child')\n"
            };
            let outcome = compute_case(
                &dice,
                &format!(
                    "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','{requested}')\n"
                ),
                load,
                &format!("{implementation}child=module_extension(implementation=implementation)\n"),
                true,
                None,
            )
            .await;
            assert!(
                matches!(
                    &outcome,
                    SourcePreparationOutcome::Complete(value)
                        if matches!(
                            value.as_ref(),
                            Err(HostLoadedModuleExtensionDefinitionsError::Request {
                                error: HostLoadedModuleExtensionDefinitionError::Export { .. },
                                ..
                            })
                        )
                ),
                "raw load `{requested}` must not be a selectable module global: {outcome:?}"
            );
        }
    }

    #[tokio::test]
    async fn real_aggregate_orders_requests_and_stops_before_bzl_on_predecessor_terminal() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let absent = compute_case(
            &dice,
            "module(name='bazel_tools')\n",
            &source("A", 1),
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        let SourcePreparationOutcome::Complete(absent) = absent else {
            panic!("absence is complete")
        };
        assert!(absent.as_ref().as_ref().unwrap().definitions.is_empty());
        assert!(tracker.take().iter().all(|activation| {
            !activation.observed
                || activation.kind == ActivationKind::Reused && activation.batch.is_none()
        }));

        let module = "module(name='bazel_tools')\n\
            b=use_extension('//:other.bzl','other')\n\
            a=use_extension('//:ext.bzl','ext')\n";
        let ordered = compute_case(
            &dice,
            module,
            &source("A", 1),
            "def implementation(ctx):\n    pass\n",
            true,
            None,
        )
        .await;
        let SourcePreparationOutcome::Complete(ordered) = ordered else {
            panic!("multiple requests complete")
        };
        let labels = ordered
            .as_ref()
            .as_ref()
            .unwrap()
            .definitions
            .iter()
            .map(|definition| definition.request.parts().0.target().as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["other.bzl", "ext.bzl"]);

        let need = compute_case(
            &dice,
            "module(name='bazel_tools')\n\
             bazel_dep(name='dep',version='1.0')\n\
             local_path_override(module_name='dep',path='dep')\n\
             e=use_extension('//:ext.bzl','ext')\n",
            &source("A", 1),
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostLoadedModuleExtensionDefinitionsKey::validity(&need));
        assert!(!HostLoadedModuleExtensionDefinitionsKey::equality(
            &need, &need
        ));
        assert!(tracker.take().is_empty());
        let error = compute_case(
            &dice,
            "module(",
            &source("A", 1),
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(
            error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostLoadedModuleExtensionDefinitionsError::Requests(_))
                )
        ));
        assert!(tracker.take().is_empty());
    }

    #[tokio::test]
    async fn real_aggregate_retains_error_context_child_identity_and_event_replay() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let wrong = |repo: &str| {
            format!(
                "module(name='bazel_tools',repo_name='{repo}')\n\
                 e=use_extension('//:ext.bzl','ext')\n"
            )
        };
        let wrong_a = compute_case(&dice, &wrong("a"), "ext=1\n", "", true, None).await;
        let wrong_b = compute_case(&dice, &wrong("b"), "ext=1\n", "", true, None).await;
        let wrong_restored = compute_case(&dice, &wrong("a"), "ext=1\n", "", true, None).await;
        assert!(!HostLoadedModuleExtensionDefinitionsKey::equality(
            &wrong_a, &wrong_b
        ));
        assert!(HostLoadedModuleExtensionDefinitionsKey::equality(
            &wrong_a,
            &wrong_restored
        ));

        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let child_a = "print('child')\ndef implementation(ctx):\n    pass\n";
        let child_b = "print('child')\nx=1\ndef implementation(ctx):\n    pass\n";
        let tracker = Arc::new(BzlEventTracker::default());
        let a = compute_case(
            &dice,
            module,
            &source("A", 1),
            child_a,
            true,
            Some(tracker.clone()),
        )
        .await;
        let first_events = tracker.take();
        let context_changed = compute_case(
            &dice,
            "module(name='bazel_tools',repo_name='replay')\n\
             e=use_extension('//:ext.bzl','ext')\n",
            &source("A", 1),
            child_a,
            true,
            Some(tracker.clone()),
        )
        .await;
        let replay_events = tracker.take();
        assert!(!HostLoadedModuleExtensionDefinitionsKey::equality(
            &a,
            &context_changed
        ));
        assert!(first_events.iter().any(|event| {
            event.label.target().as_str() == "child.bzl"
                && event.kind == ActivationKind::Evaluated
                && event.batch.is_some()
        }));
        assert!(replay_events.iter().any(|event| {
            event.label.target().as_str() == "ext.bzl" && event.kind == ActivationKind::Reused
        }));
        let changed = compute_case(&dice, module, &source("A", 1), child_b, true, None).await;
        let restored = compute_case(&dice, module, &source("A", 1), child_a, true, None).await;
        assert!(!HostLoadedModuleExtensionDefinitionsKey::equality(
            &a, &changed
        ));
        assert!(HostLoadedModuleExtensionDefinitionsKey::equality(
            &a, &restored
        ));

        for (child, present) in [("def broken(\n", true), ("", false)] {
            let outcome = compute_case(&dice, module, &source("A", 1), child, present, None).await;
            assert!(matches!(
                outcome,
                SourcePreparationOutcome::Complete(value)
                    if matches!(
                        value.as_ref(),
                        Err(HostLoadedModuleExtensionDefinitionsError::Request {
                            error: HostLoadedModuleExtensionDefinitionError::Bzl {
                                error: HostBzlModuleError::Child { .. },
                                ..
                            },
                            ..
                        })
                    )
            ));
        }
    }

    fn prepared_source(default: &str) -> String {
        format!(
            "print('extension')\n\
             def implementation(ctx):\n    pass\n\
             tag=tag_class(attrs={{\n\
               'text':attr.string(default='{default}'),\n\
               'flag':attr.bool(default=False),\n\
               'count':attr.int(default=0),\n\
               'target':attr.label(),\n\
             }})\n\
             ext=module_extension(implementation=implementation,tag_classes={{'tag':tag}})\n"
        )
    }

    fn prepared_module(text: Option<&str>) -> String {
        let text = text
            .map(|text| format!(",text='{text}'"))
            .unwrap_or_default();
        format!(
            "module(name='bazel_tools')\n\
             e=use_extension('//:ext.bzl','ext')\n\
             e.tag(flag=True,count=7,target='//:item'{text})\n\
             e.tag(flag=False,count=8,target='//:second',text='second')\n"
        )
    }

    #[tokio::test]
    async fn real_prepared_inputs_order_type_default_restore_and_event_boundary() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let a = compute_prepared_case(
            &dice,
            &prepared_module(Some("A")),
            &prepared_source("default-a"),
            Some(tracker.clone()),
        )
        .await;
        let warm = compute_prepared_case(
            &dice,
            &prepared_module(Some("A")),
            &prepared_source("default-a"),
            Some(tracker.clone()),
        )
        .await;
        assert!(HostPreparedModuleExtensionInputsKey::validity(&a), "{a:?}");
        assert!(HostPreparedModuleExtensionInputsKey::equality(&a, &warm));
        let SourcePreparationOutcome::Complete(value) = &a else {
            panic!("prepared input must complete")
        };
        let value = value.as_ref().as_ref().unwrap();
        assert_eq!(value.inputs.len(), 1);
        let tags = &value.inputs[0].tags;
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].module_index, 0);
        assert_eq!(tags[0].tag_index, 0);
        assert_eq!(tags[1].tag_index, 1);
        assert_eq!(tags[0].attributes[0].0, "text");
        assert_eq!(
            tags[0].attributes[0].1,
            crate::attrs::CoercedAttributeValue::String("A".into())
        );
        assert!(matches!(
            &tags[0].attributes[3].1,
            crate::attrs::CoercedAttributeValue::Label(label)
                if label == &CanonicalLabel::parse("@@//:item").unwrap()
        ));
        let activations = tracker.take();
        assert!(activations.iter().any(|event| {
            event.label.target().as_str() == "ext.bzl"
                && event.kind == ActivationKind::Evaluated
                && event.batch.is_some()
        }));
        let changed = compute_prepared_case(
            &dice,
            &prepared_module(Some("B")),
            &prepared_source("default-a"),
            Some(tracker.clone()),
        )
        .await;
        assert!(tracker.take().iter().any(|event| {
            event.label.target().as_str() == "ext.bzl" && event.kind == ActivationKind::Reused
        }));
        let restored = compute_prepared_case(
            &dice,
            &prepared_module(Some("A")),
            &prepared_source("default-a"),
            None,
        )
        .await;
        assert!(!HostPreparedModuleExtensionInputsKey::equality(
            &a, &changed
        ));
        assert!(HostPreparedModuleExtensionInputsKey::equality(
            &a, &restored
        ));

        let dev_module = prepared_module(Some("A")).replace(
            "use_extension('//:ext.bzl','ext')",
            "use_extension('//:ext.bzl','ext',dev_dependency=True)",
        );
        let dev =
            compute_prepared_case(&dice, &dev_module, &prepared_source("default-a"), None).await;
        let shifted = compute_prepared_case(
            &dice,
            &format!("\n{}", prepared_module(Some("A"))),
            &prepared_source("default-a"),
            None,
        )
        .await;
        assert!(!HostPreparedModuleExtensionInputsKey::equality(&a, &dev));
        assert!(!HostPreparedModuleExtensionInputsKey::equality(
            &a, &shifted
        ));
        let SourcePreparationOutcome::Complete(dev) = dev else {
            panic!("dev input completes")
        };
        assert!(dev.as_ref().as_ref().unwrap().inputs[0].tags[0].dev_dependency);
        let SourcePreparationOutcome::Complete(shifted) = shifted else {
            panic!("shifted input completes")
        };
        assert_ne!(
            value.inputs[0].tags[0].location,
            shifted.as_ref().as_ref().unwrap().inputs[0].tags[0].location
        );

        let default_a = compute_prepared_case(
            &dice,
            &prepared_module(None),
            &prepared_source("default-a"),
            None,
        )
        .await;
        let default_b = compute_prepared_case(
            &dice,
            &prepared_module(None),
            &prepared_source("default-b"),
            None,
        )
        .await;
        let default_restored = compute_prepared_case(
            &dice,
            &prepared_module(None),
            &prepared_source("default-a"),
            None,
        )
        .await;
        assert!(!HostPreparedModuleExtensionInputsKey::equality(
            &default_a, &default_b
        ));
        assert!(HostPreparedModuleExtensionInputsKey::equality(
            &default_a,
            &default_restored
        ));
    }

    #[tokio::test]
    async fn real_prepared_inputs_preserve_raw_first_and_contextual_errors() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let raw_error = compute_prepared_case(
            &dice,
            "module(",
            &prepared_source("default"),
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(
            raw_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostPreparedModuleExtensionInputsError::Raw(_)))
        ));
        assert!(tracker.take().is_empty());

        let raw_need = compute_prepared_case(
            &dice,
            "module(name='bazel_tools')\n\
             bazel_dep(name='dep',version='1.0')\n\
             local_path_override(module_name='dep',path='dep')\n\
             e=use_extension('//:ext.bzl','ext')\n",
            &prepared_source("default"),
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(raw_need, SourcePreparationOutcome::Need(_)));
        assert!(!HostPreparedModuleExtensionInputsKey::validity(&raw_need));
        assert!(!HostPreparedModuleExtensionInputsKey::equality(
            &raw_need, &raw_need
        ));
        assert!(tracker.take().is_empty());

        let definition_error =
            compute_prepared_case(&dice, &prepared_module(Some("A")), "ext=1\n", None).await;
        assert!(matches!(
            definition_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostPreparedModuleExtensionInputsError::Definitions { .. })
                )
        ));
        let unknown_tag = compute_prepared_case(
            &dice,
            &prepared_module(Some("A")),
            "def implementation(ctx):\n    pass\next=module_extension(implementation=implementation)\n",
            None,
        )
        .await;
        assert!(matches!(
            unknown_tag,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostPreparedModuleExtensionInputsError::AfterInputs {
                        request: Some(_),
                        error: HostPreparedModuleExtensionInputError::UnknownTagClass(_),
                        ..
                    })
                )
        ));
        let unused_collection_schema = compute_prepared_case(
            &dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n",
            "def implementation(ctx):\n    pass\n\
             tag=tag_class(attrs={'values':attr.string_list()})\n\
             ext=module_extension(implementation=implementation,tag_classes={'tag':tag})\n",
            None,
        )
        .await;
        assert!(matches!(
            unused_collection_schema,
            SourcePreparationOutcome::Complete(value)
                if value.as_ref().is_ok()
        ));
    }

    #[tokio::test]
    async fn observed_prepared_identity_and_finisher_algebra() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe());
        let other = HostPreparedModuleExtensionInputsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let mut left_hash = DefaultHasher::new();
        let mut right_hash = DefaultHasher::new();
        key.hash(&mut left_hash);
        other.hash(&mut right_hash);
        assert_eq!(
            key.to_string(),
            "observed-host-prepared-module-extension-inputs:\"/extension-definition-loading\""
        );
        assert_ne!(key, other);
        assert_ne!(left_hash.finish(), right_hash.finish());

        let module = prepared_module(Some("A"));
        let extension = prepared_source("default");
        let mut transaction = case_transaction(&dice, &module, &extension, "", true, None).await;
        let raw_child = transaction
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(raw_child)) = raw_child else {
            panic!("raw observation must complete")
        };
        let raw = Arc::new(raw_child.result().as_ref().as_ref().unwrap().clone());
        let definitions_child = transaction
            .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(definitions_child)) = definitions_child else {
            panic!("definition observation must complete")
        };
        let definition_result = definitions_child.result().dupe();

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/observed-prepared").unwrap(),
            PathObservationOperation::Lstat,
        );
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let current = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let duplicate = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let (_, merged) = finish_prepared_module_extension_definitions(
            &raw,
            current.dupe(),
            SourcePreparationOutcome::Complete(Ok((definition_result.dupe(), duplicate))),
        )
        .unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));

        let conflicting = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644),
            ))),
        )])
        .unwrap();
        let merge = finish_prepared_module_extension_definitions(
            &raw,
            current.dupe(),
            SourcePreparationOutcome::Complete(Ok((definition_result.dupe(), conflicting.dupe()))),
        )
        .unwrap_err();
        let outer: <HostPreparedModuleExtensionInputsObservationKey as Key>::Value = match &merge {
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(
                    HostPreparedModuleExtensionInputsObservationError(error.dupe()),
                ))
            }
            _ => panic!("prepared merge must be a typed outer"),
        };
        assert!(HostPreparedModuleExtensionInputsObservationKey::validity(
            &outer
        ));
        assert!(HostPreparedModuleExtensionInputsObservationKey::equality(
            &outer, &outer
        ));
        assert!(matches!(merge, SourcePreparationOutcome::Complete(Err(
            PreparedModuleExtensionInputsObservationError::Merge { raw: actual, .. }
        )) if Arc::ptr_eq(&actual, &raw)));

        let definitions = definition_result.as_ref().as_ref().unwrap();
        let request = definitions.definitions[0].request.clone();
        let requests = definitions.requests.dupe();
        let conflict = union_host_observations(&current, &conflicting).unwrap_err();
        let operation =
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::FileBytes,
            });
        for error in [conflict, operation] {
            let outer = finish_prepared_module_extension_definitions(
                &raw,
                current.dupe(),
                SourcePreparationOutcome::Complete(Err(
                    LoadedModuleExtensionDefinitionsObservationError::Request {
                        requests: requests.dupe(),
                        request: request.clone(),
                        stage: LoadedModuleExtensionDefinitionsObservationStage::Bzl,
                        error,
                    },
                )),
            )
            .unwrap_err();
            assert!(matches!(outer, SourcePreparationOutcome::Complete(Err(
                PreparedModuleExtensionInputsObservationError::Definitions {
                    raw: actual,
                    error: LoadedModuleExtensionDefinitionsObservationError::Request {
                        stage: LoadedModuleExtensionDefinitionsObservationStage::Bzl,
                        ..
                    },
                }
            )) if Arc::ptr_eq(&actual, &raw)));
        }

        let parent = transaction.compute(&key).await.unwrap();
        assert!(HostPreparedModuleExtensionInputsObservationKey::validity(
            &parent
        ));
        assert!(HostPreparedModuleExtensionInputsObservationKey::equality(
            &parent, &parent
        ));
        let SourcePreparationOutcome::Complete(Ok(parent)) = &parent else {
            panic!("prepared observation must complete")
        };
        assert!(parent.result().as_ref().is_ok());
        assert!(!parent.observations().observations().is_empty());
        for (case_module, case_extension, expected) in [
            ("module(".to_owned(), extension.clone(), "raw"),
            (module.clone(), "ext=1\n".to_owned(), "definitions"),
            (
                module.clone(),
                "def implementation(ctx):\n    pass\next=module_extension(implementation=implementation)\n".to_owned(),
                "local",
            ),
        ] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy = compute_prepared_case(
                &case_dice,
                &case_module,
                &case_extension,
                None,
            )
            .await;
            let mut observed_tx =
                case_transaction(&case_dice, &case_module, &case_extension, "", true, None).await;
            let observed = observed_tx.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("{expected} legacy result must complete")
            };
            let SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
                panic!("{expected} observed result must retain a carrier")
            };
            assert_eq!(legacy.as_ref(), observed.result().as_ref());
            assert!(match (expected, observed.result().as_ref()) {
                ("raw", Err(HostPreparedModuleExtensionInputsError::Raw(_))) => true,
                ("definitions", Err(HostPreparedModuleExtensionInputsError::Definitions { .. })) => true,
                ("local", Err(HostPreparedModuleExtensionInputsError::AfterInputs { .. })) => true,
                _ => false,
            });
        }
        let source = include_str!("bzl_module.rs");
        assert!(source.contains("PreparedModuleExtensionInputsObservationError::Raw(error)"));
        assert!(source.contains("PreparedModuleExtensionInputsObservationError::Definitions"));
    }

    #[tokio::test]
    async fn observed_prepared_real_order_terminals_events_and_parity() {
        let module = prepared_module(Some("A"));
        let extension = prepared_source("default");
        let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy = compute_prepared_case(&legacy_dice, &module, &extension, None).await;
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe());
        let mut transaction =
            case_transaction(&dice, &module, &extension, "", true, Some(tracker.clone())).await;
        let observed = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy prepared inputs must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(observed_value)) = &observed else {
            panic!("observed prepared inputs must complete")
        };
        assert_eq!(legacy.as_ref(), observed_value.result().as_ref());
        let raw = transaction
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(raw)) = raw else {
            panic!("raw carrier must complete")
        };
        let definitions = transaction
            .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(definitions)) = definitions else {
            panic!("definitions carrier must complete")
        };
        for epoch in [raw.observations(), definitions.observations()] {
            for (demand, result) in epoch.observations() {
                assert_eq!(
                    observed_value.observations().get(demand).unwrap().as_ref(),
                    result.as_ref()
                );
            }
        }
        let (rows, dependencies) = tracker.take_rows();
        let parent = dependencies
            .iter()
            .find(|row| row.key == key.to_string())
            .unwrap();
        assert_eq!(
            parent.dependencies,
            [
                HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe())
                    .to_string(),
                HostLoadedModuleExtensionDefinitionsObservationKey::new(workspace.dupe())
                    .to_string(),
            ]
        );
        assert!(rows.iter().any(|row| {
            row.key == key.to_string()
                && row.kind == ActivationKind::Evaluated
                && row.batch.is_none()
        }));
        let batches = rows
            .iter()
            .filter(|row| row.batch.is_some())
            .collect::<Vec<_>>();
        assert!(
            matches!(batches.as_slice(), [root, repo, bzl]
            if root.key.starts_with("bzlmod-observed-host-root-module-file:")
                && repo.key.starts_with("bzlmod-observed-host-repo-file:")
                && [root, repo].iter().all(|row|
                    row.kind == ActivationKind::Evaluated
                        && matches!(row.batch.as_ref().map(EventBatch::events), Some([])))
                && bzl.key.ends_with("//:ext.bzl")
                && bzl.kind == ActivationKind::Evaluated
                && matches!(bzl.batch.as_ref().map(EventBatch::events),
                    Some([slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }])
                        if text == "extension")),
            "{batches:?}"
        );

        tracker.take();
        tracker.take_rows();
        let mut warm_tx =
            case_transaction(&dice, &module, &extension, "", true, Some(tracker.clone())).await;
        let warm = warm_tx.compute(&key).await.unwrap();
        assert!(HostPreparedModuleExtensionInputsObservationKey::equality(
            &observed, &warm
        ));
        assert!(tracker.take().is_empty());
        let (warm_rows, _) = tracker.take_rows();
        let warm_parent = warm_rows
            .iter()
            .filter(|row| row.key == key.to_string())
            .collect::<Vec<_>>();
        assert!(matches!(warm_parent.as_slice(), [row]
            if row.kind == ActivationKind::Reused && row.batch.is_none()));
        assert!(warm_rows.iter().all(|row| row.batch.is_none()));

        let mut changed_tx = case_transaction(
            &dice,
            &prepared_module(Some("B")),
            &extension,
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        let changed = changed_tx.compute(&key).await.unwrap();
        assert!(!HostPreparedModuleExtensionInputsObservationKey::equality(
            &observed, &changed
        ));
        let changed_raw = changed_tx
            .compute(
                &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(changed_raw)) = changed_raw else {
            panic!("changed raw child must complete")
        };
        assert_ne!(raw.result(), changed_raw.result());
        let (changed_rows, _) = tracker.take_rows();
        let reused = changed_rows
            .iter()
            .filter(|row| row.key.starts_with("observed-bzlmod-host-bzl-module:"))
            .collect::<Vec<_>>();
        assert!(matches!(reused.as_slice(), [row]
            if row.key.ends_with("//:ext.bzl")
                && row.kind == ActivationKind::Reused
                && row.batch.is_none()));

        tracker.take();
        tracker.take_rows();
        let mut raw_error_tx = case_transaction(
            &dice,
            "module(",
            &extension,
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        let raw_error = raw_error_tx.compute(&key).await.unwrap();
        assert!(
            matches!(raw_error, SourcePreparationOutcome::Complete(Ok(value))
            if matches!(value.result().as_ref(), Err(HostPreparedModuleExtensionInputsError::Raw(_))))
        );
        assert!(tracker.take().is_empty());
        let (_, raw_dependencies) = tracker.take_rows();
        let parent = raw_dependencies
            .iter()
            .find(|row| row.key == key.to_string())
            .unwrap();
        assert_eq!(parent.dependencies.len(), 1);

        let need_module = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n";
        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut need_tx =
            case_transaction(&need_dice, need_module, &extension, "", true, None).await;
        let need = need_tx.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostPreparedModuleExtensionInputsObservationKey::validity(
            &need
        ));
        assert!(!HostPreparedModuleExtensionInputsObservationKey::equality(
            &need, &need
        ));
    }

    #[tokio::test]
    async fn observed_prepared_lifecycle_cancellation_and_nonactivation() {
        macro_rules! carrier {
            ($value:expr) => {
                match $value {
                    SourcePreparationOutcome::Complete(Ok(value)) => value,
                    value => panic!("expected observation carrier: {value:?}"),
                }
            };
        }
        macro_rules! snapshot {
            ($transaction:ident, $workspace:ident, $key:ident) => {{
                let global = $transaction
                    .compute(&PathObservationEpochKey)
                    .await
                    .unwrap();
                let parent = carrier!($transaction.compute(&$key).await.unwrap());
                let raw = carrier!(
                    $transaction
                        .compute(
                            &HostSelectedExtensionEvaluationInputRequestsObservationKey::new(
                                $workspace.dupe(),
                            ),
                        )
                        .await
                        .unwrap()
                );
                let definitions = carrier!(
                    $transaction
                        .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                            $workspace.dupe()
                        ),)
                        .await
                        .unwrap()
                );
                for epoch in [
                    parent.observations(),
                    raw.observations(),
                    definitions.observations(),
                ] {
                    for (demand, result) in epoch.observations() {
                        assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
                    }
                }
                (parent.dupe(), raw.dupe(), definitions.dupe())
            }};
        }
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe());
        let root_a = prepared_module(None);
        let root_b = root_a.replace("//:item", "//:changed");
        let ext_a = prepared_source("default-a");
        let ext_b = prepared_source("default-b");

        let mut first_tx =
            case_transaction(&dice, &root_a, &ext_a, "", true, Some(tracker.clone())).await;
        let first = snapshot!(first_tx, workspace, key);
        let retained = first.clone();

        let mut raw_b_tx =
            case_transaction(&dice, &root_b, &ext_a, "", true, Some(tracker.clone())).await;
        let raw_b = snapshot!(raw_b_tx, workspace, key);
        assert_ne!(first.0.result(), raw_b.0.result());
        assert_ne!(first.1.result(), raw_b.1.result());
        assert_ne!(first.2.result(), raw_b.2.result());

        let mut raw_a_tx =
            case_transaction(&dice, &root_a, &ext_a, "", true, Some(tracker.clone())).await;
        let raw_a = snapshot!(raw_a_tx, workspace, key);
        assert_eq!(first.0.result(), raw_a.0.result());
        assert_eq!(first.1.result(), raw_a.1.result());
        assert_eq!(first.2.result(), raw_a.2.result());

        let mut definitions_b_tx =
            case_transaction(&dice, &root_a, &ext_b, "", true, Some(tracker.clone())).await;
        let definitions_b = snapshot!(definitions_b_tx, workspace, key);
        assert_ne!(first.0.result(), definitions_b.0.result());
        assert_eq!(first.1.result(), definitions_b.1.result());
        assert_ne!(first.2.result(), definitions_b.2.result());

        let mut definitions_a_tx =
            case_transaction(&dice, &root_a, &ext_a, "", true, Some(tracker.clone())).await;
        let definitions_a = snapshot!(definitions_a_tx, workspace, key);
        assert_eq!(first.0.result(), definitions_a.0.result());
        assert_eq!(first.1.result(), definitions_a.1.result());
        assert_eq!(first.2.result(), definitions_a.2.result());
        assert!(Arc::ptr_eq(first.0.result(), retained.0.result()));
        assert!(Arc::ptr_eq(first.1.result(), retained.1.result()));
        assert!(Arc::ptr_eq(first.2.result(), retained.2.result()));
        assert_eq!(first.0.observations(), retained.0.observations());
        assert_eq!(first.1.observations(), retained.1.observations());
        assert_eq!(first.2.observations(), retained.2.observations());

        let (rows, dependencies) = tracker.take_rows();
        for active in rows
            .iter()
            .map(|row| row.key.as_str())
            .chain(dependencies.iter().flat_map(|row| {
                std::iter::once(row.key.as_str()).chain(row.dependencies.iter().map(String::as_str))
            }))
        {
            assert!(
                ![
                    "host-selected-extension-evaluation-input-requests:",
                    "host-loaded-module-extension-definitions:",
                    "host-prepared-module-extension-inputs:",
                    "host-pure-module-extension-invocations:",
                    "host-instantiated-module-extension-repositories:",
                    "host-validated-module-extension-repositories:",
                    "host-root-repository-mapping:",
                    "host-canonical-selected-module-definition:",
                    "host-generated-repository-definition:",
                    "slug-command:",
                ]
                .iter()
                .any(|prefix| active.starts_with(prefix)),
                "unexpected activation: {active}"
            );
        }

        tracker.take();
        tracker.take_rows();
        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut cancelled = case_transaction(
            &cancel_dice,
            &root_a,
            &ext_a,
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(tracker.take().is_empty());
        let (cancelled_rows, cancelled_dependencies) = tracker.take_rows();
        assert!(cancelled_rows.is_empty() && cancelled_dependencies.is_empty());
        let mut recovered_tx = case_transaction(
            &cancel_dice,
            &root_a,
            &ext_a,
            "",
            true,
            Some(tracker.clone()),
        )
        .await;
        let recovered = snapshot!(recovered_tx, workspace, key);
        assert_eq!(first.0.result(), recovered.0.result());
        assert_eq!(first.1.result(), recovered.1.result());
        assert_eq!(first.2.result(), recovered.2.result());
        let source = include_str!("bzl_module.rs");
        let start = source
            .find("type HostPreparedModuleExtensionInputsOutcome")
            .unwrap();
        let end = source.find("type ExternalBzlModuleCarrier").unwrap();
        let slice = &source[start..end];
        for upper in [
            "HostPureModuleExtensionInvocationsKey",
            "HostInstantiatedModuleExtensionRepositoriesKey",
            "HostValidatedModuleExtensionRepositoriesKey",
            "HostRootRepositoryMappingKey",
            "HostCanonicalSelectedModuleDefinitionKey",
            "HostGeneratedRepositoryDefinitionKey",
            "slug-command:",
        ] {
            assert!(!slice.contains(upper), "unexpected upper owner: {upper}");
        }
    }

    #[tokio::test]
    async fn real_pure_invocation_publishes_prints_and_reuses_semantics() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\ne.tag(text='A',target='//:item')\n";
        let extension = "def implementation(ctx):\n    m=ctx.modules[0]\n    t=m.tags.tag[0]\n    print('%s|%s|%s|%s|%s|%s' % (m.name,m.version,m.is_root,t.text,t.target,ctx.is_dev_dependency(t)))\ntag=tag_class(attrs={'text':attr.string(),'target':attr.label()})\next=module_extension(implementation=implementation,tag_classes={'tag':tag})\n";
        let first = compute_invocation_case(&dice, module, extension, Some(tracker.clone())).await;
        assert!(matches!(first, SourcePreparationOutcome::Complete(ref value) if value.is_ok()));
        let first_events = tracker.take();
        assert!(first_events.iter().any(|event| {
            event.label.target().as_str() == "module_extension_invocation"
                && event
                    .batch
                    .as_ref()
                    .is_some_and(|batch| matches!(batch.events(), [slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }] if text.contains("@@//:item")))
        }));
        let warm = compute_invocation_case(&dice, module, extension, Some(tracker.clone())).await;
        assert!(HostPureModuleExtensionInvocationsKey::equality(
            &first, &warm
        ));
        tracker.take();
        let mut transaction =
            case_transaction(&dice, module, extension, "", true, Some(tracker.clone())).await;
        let replay = transaction
            .compute(&InvocationConsumerKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                id: 1,
            })
            .await
            .unwrap();
        assert!(HostPureModuleExtensionInvocationsKey::equality(
            &first, &replay
        ));
        let activations = tracker.take();
        assert!(
            activations.iter().any(|event| {
                event.label.target().as_str() == "module_extension_invocation"
                    && event.kind == ActivationKind::Reused
                    && event.batch.is_none()
            }),
            "{activations:#?}"
        );
    }

    #[tokio::test]
    async fn invocation_preflights_every_request_before_running_user_code() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let prepared_need = compute_invocation_case(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n",
            "def implementation(ctx):\n    fail('must not run')\next=module_extension(implementation=implementation)\n",
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(prepared_need, SourcePreparationOutcome::Need(_)));
        assert!(!HostPureModuleExtensionInvocationsKey::validity(
            &prepared_need
        ));
        assert!(!HostPureModuleExtensionInvocationsKey::equality(
            &prepared_need,
            &prepared_need
        ));
        assert!(tracker.take().iter().all(|event| {
            event.label.target().as_str() == "module_extension_invocation" && event.batch.is_none()
        }));

        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','ext')\nb=use_extension('//:child.bzl','other')\n";
        let extension = "def implementation(ctx):\n    print('first')\next=module_extension(implementation=implementation)\n";
        let child = "def implementation(ctx):\n    print('second')\nother=module_extension(implementation=implementation)\n";
        let mut initial = case_transaction(&dice, module, extension, child, true, None).await;
        let prepared = initial
            .compute(&HostPreparedModuleExtensionInputsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let prepared = match prepared {
            SourcePreparationOutcome::Complete(value) => {
                Arc::new(value.as_ref().as_ref().unwrap().clone())
            }
            SourcePreparationOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
        };

        let mut missing = case_transaction(&dice, module, extension, child, false, None).await;
        let missing = missing
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared: prepared.clone(),
                id: 1,
            })
            .await
            .unwrap();
        assert!(matches!(
            missing.outcome,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Bzl(_),
                    ..
                }))
        ));
        assert!(missing.prints.is_empty());

        let mut restored = case_transaction(&dice, module, extension, child, true, None).await;
        let restored = restored
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared,
                id: 2,
            })
            .await
            .unwrap();
        assert!(matches!(
            restored.outcome,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        assert_eq!(restored.prints.as_ref(), ["first", "second"]);
    }

    #[tokio::test]
    async fn invocation_preserves_definition_terminals_and_loaded_callable_abi() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        for source in ["ext=1\n", "def broken(\n"] {
            let tracker = Arc::new(BzlEventTracker::default());
            let outcome =
                compute_invocation_case(&dice, module, source, Some(tracker.clone())).await;
            assert!(matches!(
                outcome,
                SourcePreparationOutcome::Complete(value)
                    if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::Prepared(_)))
            ));
            assert!(tracker.take().iter().all(|event| {
                event.label.target().as_str() != "module_extension_invocation"
                    || event.batch.is_none()
            }));
        }

        let tracker = Arc::new(BzlEventTracker::default());
        let mut transaction = case_transaction(
            &dice,
            module,
            "load('//:child.bzl','captured')\next=module_extension(implementation=captured)\n",
            "def captured(ctx):\n    return ctx.execute(['forbidden'])\n",
            true,
            Some(tracker.clone()),
        )
        .await;
        let outcome = transaction
            .compute(&HostPureModuleExtensionInvocationsKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Invocation(_),
                    ..
                }))
        ));
        let invocation = tracker
            .take()
            .into_iter()
            .find(|event| event.label.target().as_str() == "module_extension_invocation")
            .unwrap();
        assert!(
            invocation
                .batch
                .is_some_and(|batch| batch.events().is_empty())
        );
    }

    #[tokio::test]
    async fn invocation_rejects_factors_results_and_preserves_print_before_throw() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        for options in [
            "environ=['X']",
            "os_dependent=True",
            "arch_dependent=True",
            "facts_version=1",
        ] {
            let factors = compute_invocation_case(
                &dice,
                module,
                &format!(
                    "def implementation(ctx):\n    fail('must not run')\next=module_extension(implementation=implementation,{options})\n"
                ),
                None,
            )
            .await;
            assert!(matches!(
                factors,
                SourcePreparationOutcome::Complete(value)
                    if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                        error: HostPureModuleExtensionInvocationError::UnsupportedFactors,
                        ..
                    }))
            ));
        }

        let wrong = compute_invocation_case(
            &dice,
            module,
            "def implementation(ctx):\n    return 1\next=module_extension(implementation=implementation)\n",
            None,
        )
        .await;
        assert!(matches!(
            wrong,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Result(_),
                    ..
                }))
        ));

        let tracker = Arc::new(BzlEventTracker::default());
        let thrown = compute_invocation_case(
            &dice,
            module,
            "def implementation(ctx):\n    print('before')\n    fail('boom')\next=module_extension(implementation=implementation)\n",
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(
            thrown,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Invocation(_),
                    ..
                }))
        ));
        let invocation = tracker
            .take()
            .into_iter()
            .find(|event| event.label.target().as_str() == "module_extension_invocation")
            .unwrap();
        let events = invocation.batch.unwrap();
        assert_eq!(events.events().len(), 1);
        assert!(matches!(
            &events.events()[0],
            slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. } if text == "before"
        ));
    }

    #[tokio::test]
    async fn invocation_identity_and_reacquisition_drift_are_structural() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module_a =
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\ne.tag(text='A')\n";
        let module_b =
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\ne.tag(text='B')\n";
        let source_a = "def implementation(ctx):\n    print('A')\ntag=tag_class(attrs={'text':attr.string()})\next=module_extension(implementation=implementation,tag_classes={'tag':tag})\n";
        let source_b = "def implementation(ctx):\n    print('B')\ntag=tag_class(attrs={'text':attr.string()})\next=module_extension(implementation=implementation,tag_classes={'tag':tag})\n";
        let a = compute_invocation_case(&dice, module_a, source_a, None).await;
        let tag_changed = compute_invocation_case(&dice, module_b, source_a, None).await;
        let tag_restored = compute_invocation_case(&dice, module_a, source_a, None).await;
        let callable_changed = compute_invocation_case(&dice, module_a, source_b, None).await;
        let callable_restored = compute_invocation_case(&dice, module_a, source_a, None).await;
        assert!(!HostPureModuleExtensionInvocationsKey::equality(
            &a,
            &tag_changed
        ));
        assert!(HostPureModuleExtensionInvocationsKey::equality(
            &a,
            &tag_restored
        ));
        assert!(!HostPureModuleExtensionInvocationsKey::equality(
            &a,
            &callable_changed
        ));
        assert!(HostPureModuleExtensionInvocationsKey::equality(
            &a,
            &callable_restored
        ));

        let prepared = compute_prepared_case(&dice, module_a, source_a, None).await;
        let prepared = match prepared {
            SourcePreparationOutcome::Complete(value) => {
                Arc::new(value.as_ref().as_ref().unwrap().clone())
            }
            SourcePreparationOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
        };
        let mut changed = case_transaction(&dice, module_a, source_b, "", true, None).await;
        let changed = changed
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared: prepared.clone(),
                id: 20,
            })
            .await
            .unwrap();
        assert!(matches!(
            changed.outcome,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Drift(_),
                    ..
                }))
        ));
        assert!(changed.prints.is_empty());
        let mut restored = case_transaction(&dice, module_a, source_a, "", true, None).await;
        let restored = restored
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared: prepared.clone(),
                id: 21,
            })
            .await
            .unwrap();
        assert!(matches!(
            restored.outcome,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        assert_eq!(restored.prints.as_ref(), ["A"]);

        let mut altered = prepared.as_ref().clone();
        let mut aggregate = altered.definitions.as_ref().clone();
        let mut definitions = aggregate.definitions.to_vec();
        definitions[0].definition.tag_classes = Arc::from([]);
        aggregate.definitions = definitions.into();
        altered.definitions = Arc::new(aggregate);
        let mut transaction = case_transaction(&dice, module_a, source_a, "", true, None).await;
        let definition_drift = transaction
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared: Arc::new(altered),
                id: 22,
            })
            .await
            .unwrap();
        assert!(matches!(
            definition_drift.outcome,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Drift(_),
                    ..
                }))
        ));
        assert!(definition_drift.prints.is_empty());

        let wrong_kind = "ext=1\n";
        let mut transaction = case_transaction(&dice, module_a, wrong_kind, "", true, None).await;
        let request = prepared.inputs[0].input.parts().0;
        let label = request.parts().0;
        let target = RootPackageBzlTarget::parse(label.target().as_str()).unwrap();
        let module = transaction
            .compute(&HostBzlModuleEvalKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                HostRootBzlLabel::new(label.package().package().clone(), target),
            ))
            .await
            .unwrap();
        let manifest = match module {
            SourcePreparationOutcome::Complete(value) => {
                value.as_ref().as_ref().unwrap().manifest.clone()
            }
            SourcePreparationOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
        };
        let mut altered = prepared.as_ref().clone();
        let mut aggregate = altered.definitions.as_ref().clone();
        let mut definitions = aggregate.definitions.to_vec();
        definitions[0].manifest = manifest;
        aggregate.definitions = definitions.into();
        altered.definitions = Arc::new(aggregate);
        let export_drift = transaction
            .compute(&InvokePreparedKey {
                workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                prepared: Arc::new(altered),
                id: 23,
            })
            .await
            .unwrap();
        assert!(matches!(
            export_drift.outcome,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
                    error: HostPureModuleExtensionInvocationError::Drift(_),
                    ..
                }))
        ));
        assert!(export_drift.prints.is_empty());
    }
    fn observed_test_label(name: &str) -> HostRootBzlLabel {
        HostRootBzlLabel::new(
            PackagePath::parse("").unwrap(),
            RootPackageBzlTarget::parse(name).unwrap(),
        )
    }

    async fn compute_observed_module(
        transaction: &mut DiceTransaction,
        name: &str,
    ) -> <HostBzlModuleObservationKey as Key>::Value {
        transaction
            .compute(&HostBzlModuleObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                observed_test_label(name),
            ))
            .await
            .unwrap()
    }

    fn observed_module(
        outcome: &<HostBzlModuleObservationKey as Key>::Value,
    ) -> &ObservedHostBzlModule {
        let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
            panic!("observed Host bzl module did not complete with a carrier: {outcome:?}");
        };
        value
    }

    fn assert_observed_frontier(
        value: &ObservedHostBzlModule,
        epoch: &PathObservationEpoch,
        required_files: &[&str],
        excluded_files: &[&str],
    ) {
        for (demand, result) in value.observations().observations() {
            let expected = epoch
                .get(demand)
                .unwrap_or_else(|| panic!("missing {demand:?}"));
            assert!(Arc::ptr_eq(result, expected), "changed Arc for {demand:?}");
        }
        let has_file = |name: &str| {
            value.observations().observations().keys().any(|demand| {
                demand.operation() == PathObservationOperation::FileBytes
                    && demand.path().as_path().ends_with(name)
            })
        };
        assert!(required_files.iter().all(|name| has_file(name)));
        assert!(excluded_files.iter().all(|name| !has_file(name)));
    }

    async fn transaction_with_epoch(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: Arc<BzlEventTracker>,
        force_freeze: bool,
    ) -> DiceTransaction {
        let mut data = host_bzl_user_data(
            crate::cycle_detector::bzl_load_cycle_detector(),
            Some(tracker.clone()),
        );
        if force_freeze {
            data.data.set(ForceHostBzlFreezeFailure);
        }
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater.commit().await
    }

    #[tokio::test]
    async fn observed_bzl_module_retains_recursive_arcs_events_and_a_b_a_identity() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let parent = "print('parent')\nload('//:child.bzl','child')\nvalue=child\n";
        let child_a = "print('child-a')\nchild=1\n";
        let child_b = "print('child-b')\nchild=2\n";
        let mut first_transaction =
            case_transaction(&dice, "", parent, child_a, true, Some(tracker.clone())).await;
        let epoch = first_transaction
            .compute(&PathObservationEpochKey)
            .await
            .unwrap();
        let first = compute_observed_module(&mut first_transaction, "ext.bzl").await;
        assert!(HostBzlModuleObservationKey::validity(&first));
        assert!(HostBzlModuleObservationKey::equality(&first, &first));
        let value = observed_module(&first);
        assert!(value.result().is_ok());
        assert_observed_frontier(value, &epoch, &["ext.bzl", "child.bzl"], &[]);
        assert_eq!(tracker.legacy_modules.load(Ordering::SeqCst), 0);
        assert_eq!(tracker.legacy_sources.load(Ordering::SeqCst), 0);
        assert_eq!(tracker.observed_modules.load(Ordering::SeqCst), 2);
        assert_eq!(tracker.observed_sources.load(Ordering::SeqCst), 2);
        let activations = tracker.take();
        assert!(activations.iter().all(|activation| {
            activation.observed
                && activation
                    .batch
                    .as_ref()
                    .is_some_and(|batch| !batch.events().is_empty())
        }));
        let mut warm_transaction =
            case_transaction(&dice, "", parent, child_a, true, Some(tracker.clone())).await;
        let warm = compute_observed_module(&mut warm_transaction, "ext.bzl").await;
        assert!(HostBzlModuleObservationKey::equality(&first, &warm));
        assert!(tracker.take().iter().all(|activation| {
            !activation.observed
                || activation.kind == ActivationKind::Reused && activation.batch.is_none()
        }));
        let mut changed_transaction =
            case_transaction(&dice, "", parent, child_b, true, None).await;
        let changed = compute_observed_module(&mut changed_transaction, "ext.bzl").await;
        assert!(!HostBzlModuleObservationKey::equality(&first, &changed));
        let mut restored_transaction =
            case_transaction(&dice, "", parent, child_a, true, None).await;
        let restored = compute_observed_module(&mut restored_transaction, "ext.bzl").await;
        assert!(HostBzlModuleObservationKey::equality(&first, &restored));
    }

    #[tokio::test]
    async fn observed_bzl_module_preserves_decisive_terminals_need_and_outer_polarity() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let mut seed = case_transaction(&dice, "", "x=1\n", "", true, None).await;
        let epoch = seed.compute(&PathObservationEpochKey).await.unwrap();

        let mut need_transaction =
            transaction_with_epoch(&dice, PathObservationEpoch::empty(), tracker.clone(), false)
                .await;
        let need = compute_observed_module(&mut need_transaction, "ext.bzl").await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostBzlModuleObservationKey::validity(&need));
        assert!(!HostBzlModuleObservationKey::equality(&need, &need));
        assert!(
            tracker
                .take()
                .iter()
                .all(|activation| activation.batch.is_none())
        );

        let sources = [
            ("parse", "def broken(\n", "", true),
            (
                "load-label",
                "load('@outside//:child.bzl','child')\n",
                "",
                true,
            ),
            (
                "child",
                "load('//:child.bzl','child')\nx=child\n",
                "",
                false,
            ),
            ("evaluation", "print('before')\nfail('boom')\n", "", true),
        ];
        for (name, source, child, child_present) in sources {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(BzlEventTracker::default());
            let mut transaction = case_transaction(
                &case_dice,
                "",
                source,
                child,
                child_present,
                Some(tracker.clone()),
            )
            .await;
            let injected = transaction.compute(&PathObservationEpochKey).await.unwrap();
            let outcome = compute_observed_module(&mut transaction, "ext.bzl").await;
            let value = observed_module(&outcome);
            assert_observed_frontier(
                value,
                &injected,
                &["ext.bzl"],
                if name == "child" { &[] } else { &["child.bzl"] },
            );
            if name == "child" {
                assert!(
                    value
                        .observations()
                        .observations()
                        .keys()
                        .any(|demand| demand.path().as_path().ends_with("child.bzl"))
                );
            }
            assert!(matches!(
                (name, value.result()),
                ("parse", Err(HostBzlModuleError::Parse { .. }))
                    | ("load-label", Err(HostBzlModuleError::LoadLabel { .. }))
                    | ("child", Err(HostBzlModuleError::Child { .. }))
                    | ("evaluation", Err(HostBzlModuleError::Evaluation(_)))
            ));
            assert_eq!(tracker.legacy_modules.load(Ordering::SeqCst), 0);
            assert_eq!(tracker.legacy_sources.load(Ordering::SeqCst), 0);
            assert!(
                tracker
                    .take()
                    .iter()
                    .all(|activation| !activation.observed || activation.batch.is_some())
            );
        }

        let bytes = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let bad = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::<[u8]>::from([0xff])),
        ));
        let invalid_epoch = PathObservationEpoch::from_shared(epoch.observations().iter().map(
            |(demand, result)| {
                (
                    demand.dupe(),
                    if demand == &bytes {
                        bad.dupe()
                    } else {
                        result.dupe()
                    },
                )
            },
        ))
        .unwrap();
        let terminal_label = observed_test_label("ext.bzl").canonical_label();
        let terminal_tracker = Arc::new(BzlEventTracker::default());
        let mut input_tx =
            transaction_with_epoch(&dice, invalid_epoch.dupe(), terminal_tracker.clone(), false)
                .await;
        let input = compute_observed_module(&mut input_tx, "ext.bzl").await;
        let input = observed_module(&input);
        assert!(matches!(input.result(), Err(HostBzlModuleError::Input(_))));
        assert!(Arc::ptr_eq(input.observations().get(&bytes).unwrap(), &bad));
        assert!(terminal_tracker.take_empty_observed_batches(Some(&terminal_label)));
        let mut freeze_tx =
            transaction_with_epoch(&dice, epoch.dupe(), terminal_tracker.clone(), true).await;
        let freeze = compute_observed_module(&mut freeze_tx, "ext.bzl").await;
        let freeze = observed_module(&freeze);
        assert!(matches!(
            freeze.result(),
            Err(HostBzlModuleError::Freeze { .. })
        ));
        assert_observed_frontier(freeze, &epoch, &["ext.bzl"], &["child.bzl"]);
        assert!(terminal_tracker.take_empty_observed_batches(Some(&terminal_label)));
        let source_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut source_tx = case_transaction(&source_dice, "", "", "", false, None).await;
        let source_epoch = source_tx.compute(&PathObservationEpochKey).await.unwrap();
        let source = compute_observed_module(&mut source_tx, "child.bzl").await;
        let source = observed_module(&source);
        assert!(matches!(
            source.result(),
            Err(HostBzlModuleError::Source(_))
        ));
        assert_observed_frontier(source, &source_epoch, &[], &["ext.bzl"]);

        let error = PathObservationEpoch::from_shared([(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/mismatch").unwrap(),
                PathObservationOperation::Lstat,
            ),
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Missing,
            )),
        )])
        .unwrap_err()
        .into();
        let outer: <HostBzlModuleObservationKey as Key>::Value =
            SourcePreparationOutcome::Complete(Err(error));
        assert!(HostBzlModuleObservationKey::validity(&outer));
        assert!(HostBzlModuleObservationKey::equality(&outer, &outer));
    }

    #[tokio::test]
    async fn observed_bzl_cycles_retain_only_cycle_keys_then_parent_prefix() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let parent = "load('//:child.bzl','child')\na=child\n";
        let child = "load('//:other.bzl','other')\nchild=other\n";
        let other = "load('//:child.bzl','child')\nother=child\n";

        let mut parent_transaction = case_transaction_with_other(
            &dice,
            "",
            parent,
            child,
            other,
            true,
            Some(tracker.clone()),
        )
        .await;
        let epoch = parent_transaction
            .compute(&PathObservationEpochKey)
            .await
            .unwrap();
        let parent_value = compute_observed_module(&mut parent_transaction, "ext.bzl").await;
        let parent_value = observed_module(&parent_value);
        assert!(matches!(
            parent_value.result(),
            Err(HostBzlModuleError::Child { error, .. }) if error.cycle().is_some()
        ));
        assert_observed_frontier(
            parent_value,
            &epoch,
            &["ext.bzl", "child.bzl", "other.bzl"],
            &[],
        );

        for (current, required) in [
            ("child.bzl", ["child.bzl", "other.bzl"]),
            ("other.bzl", ["other.bzl", "child.bzl"]),
        ] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let mut transaction =
                case_transaction_with_other(&case_dice, "", parent, child, other, true, None).await;
            let current_epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
            let value = compute_observed_module(&mut transaction, current).await;
            let value = observed_module(&value);
            assert!(value.result().as_ref().unwrap_err().cycle().is_some());
            assert_observed_frontier(value, &current_epoch, &required, &["ext.bzl"]);
        }

        assert_eq!(tracker.legacy_modules.load(Ordering::SeqCst), 0);
        assert_eq!(tracker.legacy_sources.load(Ordering::SeqCst), 0);
        assert!(tracker.take_empty_observed_batches(None));

        assert!(!BzlLoadCyclePoisonKey::validity(&()));
        let family_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let self_source = "load('//:ext.bzl','x')\nx=1\n";
        let mut seed = case_transaction(&family_dice, "", self_source, "", true, None).await;
        let self_epoch = seed.compute(&PathObservationEpochKey).await.unwrap();
        drop(seed);
        let detector = crate::cycle_detector::bzl_load_cycle_detector();
        let family_tracker = Arc::new(BzlEventTracker::default());
        let mut legacy_tx =
            host_bzl_transaction(&family_dice, detector.clone(), family_tracker.clone()).await;
        let mut observed_tx =
            host_bzl_transaction(&family_dice, detector, family_tracker.clone()).await;
        let label = observed_test_label("ext.bzl");
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let (legacy, observed) = tokio::join!(
            legacy_tx.compute(&HostBzlModuleEvalKey::new(workspace.dupe(), label.clone())),
            observed_tx.compute(&HostBzlModuleObservationKey::new(workspace, label)),
        );
        drop(legacy_tx);
        drop(observed_tx);
        assert!(
            matches!(legacy.unwrap(), SourcePreparationOutcome::Complete(value) if matches!(value.as_ref(), Err(HostBzlModuleError::Cycle(_))))
        );
        let observed = observed.unwrap();
        let self_value = observed_module(&observed);
        assert!(self_value.result().as_ref().unwrap_err().cycle().is_some());
        assert_observed_frontier(self_value, &self_epoch, &["ext.bzl"], &["child.bzl"]);
        assert!(family_tracker.legacy_modules.load(Ordering::SeqCst) > 0);
        assert!(family_tracker.observed_modules.load(Ordering::SeqCst) > 0);
        family_tracker.take();
        let module_activations = family_tracker.observed_modules.load(Ordering::SeqCst);
        let source_activations = family_tracker.observed_sources.load(Ordering::SeqCst);
        let mut repeated = host_bzl_transaction(
            &family_dice,
            crate::cycle_detector::bzl_load_cycle_detector(),
            family_tracker.clone(),
        )
        .await;
        let repeated = compute_observed_module(&mut repeated, "ext.bzl").await;
        assert!(HostBzlModuleObservationKey::equality(&observed, &repeated));
        assert!(family_tracker.observed_modules.load(Ordering::SeqCst) > module_activations);
        assert!(family_tracker.observed_sources.load(Ordering::SeqCst) > source_activations);
        let activations = family_tracker.take();
        let evaluated = activations
            .iter()
            .any(|a| a.kind == ActivationKind::Evaluated);
        assert!(evaluated);
    }

    #[tokio::test]
    async fn observed_bzl_cancellation_publishes_no_parent_and_recovers() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let source = "load('//:ext.bzl','x')\nx=1\n";
        let mut cancelled =
            case_transaction(&dice, "", source, "", true, Some(tracker.clone())).await;
        let key = HostBzlModuleObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            observed_test_label("ext.bzl"),
        );
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(tracker.take().is_empty());
        let mut successor = case_transaction(&dice, "", source, "", true, Some(tracker)).await;
        let recovered = compute_observed_module(&mut successor, "ext.bzl").await;
        assert!(matches!(
            observed_module(&recovered).result(),
            Err(error) if error.cycle().is_some()
        ));
    }
    async fn compute_observed_loaded(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        child_source: &str,
        child_present: bool,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> <HostLoadedModuleExtensionDefinitionsObservationKey as Key>::Value {
        case_transaction(
            dice,
            module_source,
            extension_source,
            child_source,
            child_present,
            tracker,
        )
        .await
        .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        ))
        .await
        .unwrap()
    }

    fn observed_loaded(
        value: &<HostLoadedModuleExtensionDefinitionsObservationKey as Key>::Value,
    ) -> &ObservedHostLoadedModuleExtensionDefinitions {
        let SourcePreparationOutcome::Complete(Ok(value)) = value else {
            panic!("observed loaded definitions must complete with a carrier: {value:?}");
        };
        value
    }

    #[derive(Clone)]
    struct ObservedLoadedHandles {
        parent: LoadedModuleExtensionDefinitionsResult,
        parent_epoch: PathObservationEpoch,
        request: Arc<
            Result<
                HostSelectedExtensionDefinitionLoadRequests,
                HostSelectedExtensionDefinitionLoadRequestsError,
            >,
        >,
        request_epoch: PathObservationEpoch,
        ext: Arc<Result<FrozenBzlModule, HostBzlModuleError>>,
        ext_epoch: PathObservationEpoch,
        child: Arc<Result<FrozenBzlModule, HostBzlModuleError>>,
        child_epoch: PathObservationEpoch,
        other: Arc<Result<FrozenBzlModule, HostBzlModuleError>>,
        other_epoch: PathObservationEpoch,
        projection: Arc<[(BzlLoadManifest, ModuleExtensionDefinitionProjection)]>,
        global_epoch: PathObservationEpoch,
    }

    async fn observed_loaded_handles(
        dice: &Arc<Dice>,
        module: &str,
        ext: &str,
        child: &str,
        other: &str,
        tracker: Option<Arc<BzlEventTracker>>,
    ) -> ObservedLoadedHandles {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction =
            case_transaction_with_other(dice, module, ext, child, other, true, tracker).await;
        let global_epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let parent = observed_loaded(
            &transaction
                .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                    workspace.dupe(),
                ))
                .await
                .unwrap(),
        )
        .dupe();
        let request = transaction
            .compute(
                &HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe()),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(request)) = request else {
            panic!("observed definition requests must complete: {request:?}");
        };
        let ext = compute_observed_module(&mut transaction, "ext.bzl").await;
        let ext_value = observed_module(&ext).dupe();
        let child = compute_observed_module(&mut transaction, "child.bzl").await;
        let child_value = observed_module(&child).dupe();
        let other = compute_observed_module(&mut transaction, "other.bzl").await;
        let other_value = observed_module(&other).dupe();
        let projection = Arc::from(
            parent
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .definitions
                .iter()
                .map(|definition| (definition.manifest.clone(), definition.definition.clone()))
                .collect::<Vec<_>>(),
        );
        ObservedLoadedHandles {
            parent: parent.result().dupe(),
            parent_epoch: parent.observations().dupe(),
            request: request.result().dupe(),
            request_epoch: request.observations().dupe(),
            ext: ext_value.result.dupe(),
            ext_epoch: ext_value.observations().dupe(),
            child: child_value.result.dupe(),
            child_epoch: child_value.observations().dupe(),
            other: other_value.result.dupe(),
            other_epoch: other_value.observations().dupe(),
            projection,
            global_epoch,
        }
    }

    fn assert_held_observed_loaded_handles(
        handles: &ObservedLoadedHandles,
        retained: &ObservedLoadedHandles,
    ) {
        assert!(handles.parent.as_ref().is_ok() && retained.parent.as_ref().is_ok());
        assert!(handles.request.as_ref().is_ok() && retained.request.as_ref().is_ok());
        assert!(handles.ext.as_ref().is_ok() && retained.ext.as_ref().is_ok());
        assert!(handles.child.as_ref().is_ok() && retained.child.as_ref().is_ok());
        assert!(handles.other.as_ref().is_ok() && retained.other.as_ref().is_ok());
        assert!(Arc::ptr_eq(&handles.parent, &retained.parent));
        assert!(Arc::ptr_eq(&handles.request, &retained.request));
        assert!(Arc::ptr_eq(&handles.ext, &retained.ext));
        assert!(Arc::ptr_eq(&handles.child, &retained.child));
        assert!(Arc::ptr_eq(&handles.other, &retained.other));
        assert_eq!(
            handles.projection.as_ref(),
            handles
                .parent
                .as_ref()
                .as_ref()
                .unwrap()
                .definitions
                .iter()
                .map(|definition| (definition.manifest.clone(), definition.definition.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(handles.parent_epoch, retained.parent_epoch);
        assert_eq!(handles.request_epoch, retained.request_epoch);
        assert_eq!(handles.ext_epoch, retained.ext_epoch);
        assert_eq!(handles.child_epoch, retained.child_epoch);
        assert_eq!(handles.other_epoch, retained.other_epoch);
    }

    fn assert_transaction_epoch_subsets(handles: &ObservedLoadedHandles) {
        for epoch in [
            &handles.parent_epoch,
            &handles.request_epoch,
            &handles.ext_epoch,
            &handles.child_epoch,
            &handles.other_epoch,
        ] {
            for (demand, result) in epoch.observations() {
                assert_eq!(
                    result.as_ref(),
                    handles.global_epoch.get(demand).unwrap().as_ref()
                );
            }
        }
    }

    async fn assert_loaded_warm(
        dice: &Arc<Dice>,
        root: &str,
        ext: &str,
        child: &str,
        other: &str,
        tracker: &Arc<BzlEventTracker>,
        recovered: &ObservedLoadedHandles,
    ) {
        tracker.take();
        tracker.take_rows();
        let warm_parent =
            compute_observed_loaded(dice, root, ext, child, true, Some(tracker.clone())).await;
        assert_eq!(
            recovered.parent.as_ref(),
            observed_loaded(&warm_parent).result().as_ref()
        );
        assert!(tracker.take().is_empty());
        let (parent_rows, _) = tracker.take_rows();
        assert!(
            !parent_rows
                .iter()
                .any(|row| row.key.starts_with("observed-bzlmod-host-bzl-module:"))
        );
        let warm =
            observed_loaded_handles(dice, root, ext, child, other, Some(tracker.clone())).await;
        assert_eq!(recovered.parent, warm.parent);
        assert_eq!(recovered.request, warm.request);
        assert_eq!(recovered.ext, warm.ext);
        assert_eq!(recovered.child, warm.child);
        assert_eq!(recovered.other, warm.other);
        assert_eq!(recovered.projection, warm.projection);
        assert_transaction_epoch_subsets(&warm);
        assert!(tracker.take().iter().all(|activation| {
            activation.observed
                && activation.kind == ActivationKind::Reused
                && activation.batch.is_none()
        }));
        let (activations, dependencies) = tracker.take_rows();
        assert!(
            activations
                .iter()
                .all(|row| row.kind == ActivationKind::Reused && row.batch.is_none())
        );
        assert!(dependencies.is_empty());
        BzlEventTracker::assert_reused_loaded_bzl_result_arcs(&activations, recovered, &warm);
    }

    fn loaded_test_source(name: &str) -> String {
        format!(
            "{}def implementation(ctx):\n    pass\next=module_extension(implementation=implementation,environ=['{name}'])\n",
            if name.is_empty() {
                String::new()
            } else {
                format!("print('{name}')\n")
            }
        )
    }

    async fn assert_observed_bzl_batches(
        key: &HostLoadedModuleExtensionDefinitionsObservationKey,
        labels: &[&str; 3],
        module: impl Fn(&[&str], Option<&str>) -> String,
        rows: &[BzlActivationRow],
        dependencies: &[BzlDependencyRow],
    ) {
        let admitted = |key: &str, family: BzlTrackerFamily| {
            matches!(family, BzlTrackerFamily::BzlmodCommandPolicy | BzlTrackerFamily::BzlmodEnvironmentPolicy)
            || "root-module-command-policy: root-module-environment-policy: root-module-lockfile-mode: visible-lockfile: host-visible-lockfile:".split_whitespace().any(|prefix| key.starts_with(prefix))
            || !"host-selected-extension-definition-load-requests: bzlmod-host-bzl-module: host-loaded-module-extension-definitions: host-prepared-module-extension-inputs: host-pure-module-extension-invocations: host-instantiated-module-extension-repositories: host-validated-module-extension-repositories: host-root-repository-mapping: host-canonical-selected-module-definition: host-generated-repository-definition: slug-command:".split_whitespace().any(|prefix| key.starts_with(prefix))
        };
        let bzl = rows
            .iter()
            .filter(|row| row.key.starts_with("observed-bzlmod-host-bzl-module:"))
            .collect::<Vec<_>>();
        assert_eq!(bzl.len(), labels.len(), "{bzl:?}");
        for ((row, label), print) in bzl
            .iter()
            .zip(labels)
            .zip([Some("A"), Some("B"), Some("C")])
        {
            assert!(
                row.key.ends_with(&format!("//:{label}")) && row.kind == ActivationKind::Evaluated,
                "{row:?}"
            );
            assert!(matches!(
                row.batch.as_ref().map(EventBatch::events),
                Some([slug_events_v2::EvaluationEvent::StarlarkPrint { text: actual, .. }])
                    if actual == print.unwrap()
            ));
        }
        let parent = dependencies
            .iter()
            .find(|row| row.key == key.to_string())
            .unwrap();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string()
                    || row
                        .key
                        .starts_with("observed-host-selected-extension-definition-load-requests:"))
                .all(|row| row.batch.is_none())
        );
        assert!(!rows.iter().any(|row| {
            row.key
                .starts_with("host-selected-extension-definition-load-requests:")
                || row.key.starts_with("bzlmod-host-bzl-module:")
        }));
        assert!(
            parent.dependencies[0]
                .starts_with("observed-host-selected-extension-definition-load-requests")
        );
        assert_eq!(
            parent
                .dependencies
                .iter()
                .filter(|row| row.starts_with("observed-bzlmod-host-bzl-module"))
                .map(|row| row.rsplit(':').next().unwrap())
                .collect::<Vec<_>>(),
            labels
        );
        assert!(rows.iter().all(|row| admitted(&row.key, row.family)));
        assert!(dependencies.iter().all(|row| {
            admitted(&row.key, row.family)
                && row
                    .dependencies
                    .iter()
                    .zip(&row.dependency_families)
                    .all(|(key, family)| admitted(key, *family))
        }));
        for position in 0..3 {
            for (kind, source) in [
                ("Bzl", "print('before')\nfail('boom')\n".to_owned()),
                ("Export", loaded_test_source("")),
                ("WrongKind", "ext=1\n".to_owned()),
            ] {
                let order = [
                    [labels[0], labels[1], labels[2]],
                    [labels[2], labels[1], labels[0]],
                    [labels[1], labels[0], labels[2]],
                ][position];
                let mut inputs = [
                    loaded_test_source(""),
                    loaded_test_source(""),
                    loaded_test_source(""),
                ];
                inputs[position] = source;
                let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
                let tracker = Arc::new(BzlEventTracker::default());
                let mut transaction = case_transaction_with_other(
                    &dice,
                    &module(&order, (kind == "Export").then_some(labels[position])),
                    &inputs[0],
                    &inputs[2],
                    &inputs[1],
                    true,
                    Some(tracker.clone()),
                )
                .await;
                let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
                let outcome = transaction.compute(key).await.unwrap();
                let result = observed_loaded(&outcome);
                let error = result.result().as_ref().as_ref().unwrap_err();
                let HostLoadedModuleExtensionDefinitionsError::Request { request, .. } = error
                else {
                    panic!("wrong terminal: {error:?}")
                };
                assert_eq!(request.parts().0.target().as_str(), labels[position]);
                match (kind, error) {
                    (
                        "Bzl",
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            error: HostLoadedModuleExtensionDefinitionError::Bzl { label, .. },
                            ..
                        },
                    ) => assert_eq!(label.target().as_str(), labels[position]),
                    (
                        "Export",
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            error:
                                HostLoadedModuleExtensionDefinitionError::Export { label, name, .. },
                            ..
                        },
                    ) => assert_eq!(
                        (label.target().as_str(), name.as_str()),
                        (labels[position], "missing")
                    ),
                    (
                        "WrongKind",
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            error:
                                HostLoadedModuleExtensionDefinitionError::WrongKind { label, name },
                            ..
                        },
                    ) => assert_eq!(
                        (label.target().as_str(), name.as_str()),
                        (labels[position], "ext")
                    ),
                    _ => panic!("wrong terminal: {error:?}"),
                }
                for (expected, targets) in
                    [(true, &order[..=position]), (false, &order[position + 1..])]
                {
                    assert!(targets.iter().all(|label| {
                        result.observations().observations().keys().any(|demand| {
                            demand.operation() == PathObservationOperation::FileBytes
                                && demand.path().as_path().ends_with(label)
                        }) == expected
                    }));
                }
                assert!(result.observations().observations().iter().all(
                    |(demand, observation)| Arc::ptr_eq(observation, epoch.get(demand).unwrap())
                ));
                let (rows, dependencies) = tracker.take_rows();
                let bzl = rows
                    .iter()
                    .filter(|row| row.key.starts_with("observed-bzlmod-host-bzl-module:"))
                    .collect::<Vec<_>>();
                assert_eq!(bzl.len(), position + 1);
                for (index, row) in bzl.iter().enumerate() {
                    assert!(
                        row.key.ends_with(&format!("//:{}", order[index]))
                            && row.kind == ActivationKind::Evaluated
                    );
                    if kind == "Bzl" && index == position {
                        assert!(
                            matches!(row.batch.as_ref().map(EventBatch::events), Some([slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }]) if text == "before")
                        );
                    } else {
                        assert!(
                            row.batch
                                .as_ref()
                                .is_none_or(|batch| batch.events().is_empty())
                        );
                    }
                }
                assert!(
                    rows.iter()
                        .filter(|row| row.key == key.to_string()
                            || row.key.starts_with(
                                "observed-host-selected-extension-definition-load-requests:"
                            ))
                        .all(|row| row.batch.is_none())
                );
                assert!(rows.iter().all(|row| admitted(&row.key, row.family)));
                assert!(dependencies.iter().all(|row| {
                    admitted(&row.key, row.family)
                        && row
                            .dependencies
                            .iter()
                            .zip(&row.dependency_families)
                            .all(|(key, family)| admitted(key, *family))
                }));
            }
        }
    }

    async fn compute_observed_loaded_with_other(
        dice: &Arc<Dice>,
        module: &str,
        ext: &str,
        child: &str,
        other: &str,
        tracker: Arc<BzlEventTracker>,
    ) -> <HostLoadedModuleExtensionDefinitionsObservationKey as Key>::Value {
        case_transaction_with_other(dice, module, ext, child, other, true, Some(tracker))
            .await
            .compute(&HostLoadedModuleExtensionDefinitionsObservationKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn assert_loaded_definition_production_slice_has_no_upper_keys() {
        let source = include_str!("bzl_module.rs");
        let start = source
            .find("type HostLoadedModuleExtensionDefinitionsOutcome")
            .unwrap();
        let end = source
            .find("pub(crate) struct HostPreparedModuleExtensionInputsKey")
            .unwrap();
        let slice = &source[start..end];
        for key in [
            "HostPreparedModuleExtensionInputsKey",
            "HostPureModuleExtensionInvocationsKey",
            "HostInstantiatedModuleExtensionRepositoriesKey",
            "HostValidatedModuleExtensionRepositoriesKey",
            "HostRootRepositoryMappingKey",
            "HostCanonicalSelectedModuleDefinitionKey",
            "HostGeneratedRepositoryDefinitionKey",
        ] {
            assert!(!slice.contains(key), "unexpected upper key: {key}");
        }
    }

    #[tokio::test]
    async fn observed_loaded_real_order_terminals_events_and_parity() {
        let labels = ["ext.bzl", "other.bzl", "child.bzl"];
        let module = |order: &[&str], missing: Option<&str>| {
            format!(
                "module(name='bazel_tools')\n{}",
                order
                    .iter()
                    .enumerate()
                    .map(|(i, label)| format!(
                        "e{i}=use_extension('//:{label}','{}')\n",
                        if missing == Some(*label) {
                            "missing"
                        } else {
                            "ext"
                        }
                    ))
                    .collect::<String>()
            )
        };
        let sources = [
            loaded_test_source("A"),
            loaded_test_source("B"),
            loaded_test_source("C"),
        ];
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostLoadedModuleExtensionDefinitionsObservationKey::new(workspace.dupe());
        assert_eq!(
            key.to_string(),
            "observed-host-loaded-module-extension-definitions:\"/extension-definition-loading\""
        );
        assert_ne!(
            key,
            HostLoadedModuleExtensionDefinitionsObservationKey::new(
                NormalizedAbsolutePath::new("/other").unwrap()
            )
        );

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let request_module = module(&labels, None);
        let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let legacy_tracker = Arc::new(BzlEventTracker::default());
        let mut legacy_transaction = case_transaction_with_other(
            &legacy_dice,
            &request_module,
            &sources[0],
            &sources[2],
            &sources[1],
            true,
            Some(legacy_tracker.clone()),
        )
        .await;
        let legacy = legacy_transaction
            .compute(&HostLoadedModuleExtensionDefinitionsKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy definition loading did not complete")
        };
        let (legacy_rows, _) = legacy_tracker.take_rows();
        assert!(legacy_rows.iter().any(|row| {
            row.key
                .starts_with("host-selected-extension-definition-load-requests:")
        }));
        assert_eq!(
            legacy_rows
                .iter()
                .filter(|row| row.key.starts_with("bzlmod-host-bzl-module:"))
                .count(),
            3
        );
        assert!(!legacy_rows.iter().any(|row| {
            row.key
                .starts_with("observed-host-selected-extension-definition-load-requests:")
                || row.key.starts_with("observed-bzlmod-host-bzl-module:")
        }));
        let mut first_transaction = case_transaction_with_other(
            &dice,
            &request_module,
            &sources[0],
            &sources[2],
            &sources[1],
            true,
            Some(tracker.clone()),
        )
        .await;
        let epoch = first_transaction
            .compute(&PathObservationEpochKey)
            .await
            .unwrap();
        let first = first_transaction.compute(&key).await.unwrap();
        assert!(
            HostLoadedModuleExtensionDefinitionsObservationKey::validity(&first),
            "{first:?}"
        );
        let first_value = observed_loaded(&first);
        assert_eq!(legacy.as_ref(), first_value.result().as_ref());
        let definitions = first_value.result().as_ref().as_ref().unwrap();
        assert_eq!(definitions.definitions.len(), 3);
        assert_eq!(
            definitions
                .definitions
                .iter()
                .map(|definition| definition.request.parts().0.target().as_str())
                .collect::<Vec<_>>(),
            labels
        );
        assert!(
            definitions
                .definitions
                .iter()
                .enumerate()
                .all(|(index, definition)| definition.manifest
                    == legacy.as_ref().as_ref().unwrap().definitions[index].manifest
                    && definition.definition
                        == legacy.as_ref().as_ref().unwrap().definitions[index].definition)
        );
        for (demand, result) in first_value.observations().observations() {
            assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
        }
        assert_eq!(tracker.legacy_modules.load(Ordering::SeqCst), 0);
        assert_eq!(tracker.legacy_sources.load(Ordering::SeqCst), 0);
        let (fresh_rows, fresh_dependencies) = tracker.take_rows();
        assert!(
            fresh_rows
                .iter()
                .any(|row| row.family == BzlTrackerFamily::BzlmodCommandPolicy)
                || fresh_dependencies.iter().any(|row| {
                    row.family == BzlTrackerFamily::BzlmodCommandPolicy
                        || row
                            .dependency_families
                            .contains(&BzlTrackerFamily::BzlmodCommandPolicy)
                })
        );
        assert_observed_bzl_batches(&key, &labels, module, &fresh_rows, &fresh_dependencies).await;

        let warm = compute_observed_loaded_with_other(
            &dice,
            &request_module,
            &sources[0],
            &sources[2],
            &sources[1],
            tracker.clone(),
        )
        .await;
        assert!(HostLoadedModuleExtensionDefinitionsObservationKey::equality(&first, &warm));
        let (warm_rows, _) = tracker.take_rows();
        assert!(
            !warm_rows
                .iter()
                .any(|row| row.key.starts_with("observed-bzlmod-host-bzl-module"))
        );

        let changed = compute_observed_loaded_with_other(
            &dice,
            &request_module,
            &loaded_test_source("changed"),
            &sources[2],
            &sources[1],
            tracker.clone(),
        )
        .await;
        let (changed_rows, _) = tracker.take_rows();
        assert!(
            changed_rows
                .iter()
                .any(|row| row.key.ends_with("//:other.bzl")
                    && row.kind == ActivationKind::Reused
                    && row.batch.is_none())
        );
        assert!(
            !changed_rows
                .iter()
                .any(|row| row.key.ends_with("//:other.bzl")
                    && row.kind == ActivationKind::Evaluated)
        );
        let restored = compute_observed_loaded_with_other(
            &dice,
            &request_module,
            &sources[0],
            &sources[2],
            &sources[1],
            tracker.clone(),
        )
        .await;
        assert!(!HostLoadedModuleExtensionDefinitionsObservationKey::equality(&first, &changed));
        assert!(HostLoadedModuleExtensionDefinitionsObservationKey::equality(&first, &restored));
    }
    #[tokio::test]
    async fn observed_loaded_identity_and_finisher_algebra() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let extension = source("finisher", 1);
        let key = HostLoadedModuleExtensionDefinitionsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let other = HostLoadedModuleExtensionDefinitionsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let mut left_hash = DefaultHasher::new();
        let mut right_hash = DefaultHasher::new();
        key.hash(&mut left_hash);
        other.hash(&mut right_hash);
        assert_eq!(
            key.to_string(),
            "observed-host-loaded-module-extension-definitions:\"/extension-definition-loading\""
        );
        assert_ne!(key, other);
        assert_ne!(left_hash.finish(), right_hash.finish());
        let parent = compute_observed_loaded(
            &dice,
            module,
            &extension,
            "def implementation(ctx):\n    pass\n",
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(HostLoadedModuleExtensionDefinitionsObservationKey::validity(&parent));
        assert!(HostLoadedModuleExtensionDefinitionsObservationKey::equality(&parent, &parent));
        let (activations, dependencies) = tracker.take_rows();
        let parent_dependencies = dependencies
            .iter()
            .find(|row| row.key == key.to_string())
            .unwrap();
        assert!(parent_dependencies.dependencies.iter().any(|dependency| {
            dependency.starts_with("observed-host-selected-extension-definition-load-requests")
        }));
        assert!(
            parent_dependencies
                .dependencies
                .iter()
                .any(|dependency| dependency.starts_with("observed-bzlmod-host-bzl-module"))
        );
        assert!(activations.iter().any(|row| {
            row.key == key.to_string()
                && row.batch.is_none()
                && row.kind == ActivationKind::Evaluated
        }));
        let source = include_str!("bzl_module.rs");
        assert!(source.contains("Ok((observed.result().dupe(), observed.observations().dupe()))"));
        assert!(
            source.contains("LoadedModuleExtensionDefinitionsObservationError::Requests(error)")
        );
        let loaded = observed_loaded(&parent);
        let requests = loaded.result().as_ref().as_ref().unwrap().requests.dupe();
        let request_context = requests.parts().1[0].clone();
        let mut transaction = case_transaction(
            &dice,
            module,
            &extension,
            "def implementation(ctx):\n    pass\n",
            true,
            None,
        )
        .await;
        let child = compute_observed_module(&mut transaction, "ext.bzl").await;
        let child = observed_module(&child);
        let carrier = child.result.dupe();
        let incoming = child.observations().dupe();
        let (_, prior) = finish_loaded_extension_definition_observed_child(
            &requests,
            &request_context,
            PathObservationEpoch::empty(),
            SourcePreparationOutcome::Complete(Ok((
                DefinitionBzlModuleCarrier::Root(carrier.dupe()),
                incoming,
            ))),
        )
        .unwrap();
        let (_, current) = finish_loaded_extension_definition_observed_child(
            &requests,
            &request_context,
            prior.dupe(),
            SourcePreparationOutcome::Complete(Ok((
                DefinitionBzlModuleCarrier::Root(carrier.dupe()),
                prior.dupe(),
            ))),
        )
        .unwrap();
        assert_eq!(current, prior);
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/observed-loaded").unwrap(),
            PathObservationOperation::Lstat,
        );
        let first = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let request = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let duplicate = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let (_, duplicate) = finish_loaded_extension_definition_observed_child(
            &requests,
            &request_context,
            request.dupe(),
            SourcePreparationOutcome::Complete(Ok((
                DefinitionBzlModuleCarrier::Root(carrier.dupe()),
                duplicate,
            ))),
        )
        .unwrap();
        assert!(Arc::ptr_eq(duplicate.get(&demand).unwrap(), &first));

        let conflicting = PathObservationEpoch::from_shared([(
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644),
            ))),
        )])
        .unwrap();
        let merge = finish_loaded_extension_definition_observed_child(
            &requests,
            &request_context,
            request.dupe(),
            SourcePreparationOutcome::Complete(Ok((
                DefinitionBzlModuleCarrier::Root(carrier.dupe()),
                conflicting.dupe(),
            ))),
        )
        .unwrap_err();
        assert!(matches!(
            merge,
            SourcePreparationOutcome::Complete(Err(
                LoadedModuleExtensionDefinitionsObservationError::Request {
                    requests: actual,
                    request: actual_request,
                    stage: LoadedModuleExtensionDefinitionsObservationStage::Merge,
                    ..
                }
            )) if Arc::ptr_eq(&actual, &requests) && actual_request == request_context
        ));

        let conflict = union_host_observations(&request, &conflicting).unwrap_err();
        let operation =
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::FileBytes,
            });
        for error in [conflict, operation] {
            let outer = finish_loaded_extension_definition_observed_child(
                &requests,
                &request_context,
                PathObservationEpoch::empty(),
                SourcePreparationOutcome::Complete(Err(error)),
            )
            .unwrap_err();
            assert!(outer.is_complete());
            assert!(outer.complete_eq(&outer));
            assert!(matches!(
                outer,
                SourcePreparationOutcome::Complete(Err(
                    LoadedModuleExtensionDefinitionsObservationError::Request {
                        requests: actual,
                        request: actual_request,
                        stage: LoadedModuleExtensionDefinitionsObservationStage::Bzl,
                        ..
                    }
                )) if Arc::ptr_eq(&actual, &requests) && actual_request == request_context
            ));
        }
        let request_error =
            compute_observed_loaded(&dice, "module(", &extension, "", true, None).await;
        assert!(HostLoadedModuleExtensionDefinitionsObservationKey::validity(&request_error));
        assert!(
            HostLoadedModuleExtensionDefinitionsObservationKey::equality(
                &request_error,
                &request_error
            )
        );
        assert!(matches!(
            request_error,
            SourcePreparationOutcome::Complete(Ok(value))
                if matches!(value.result().as_ref(), Err(HostLoadedModuleExtensionDefinitionsError::Requests(_)))
        ));
        let need = compute_observed_loaded(
            &dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n",
            &extension,
            "",
            true,
            None,
        )
        .await;
        assert!(!HostLoadedModuleExtensionDefinitionsObservationKey::validity(&need));
        assert!(!HostLoadedModuleExtensionDefinitionsObservationKey::equality(&need, &need));
    }
    #[tokio::test]
    async fn observed_loaded_lifecycle_cancellation_and_nonactivation() {
        assert_loaded_definition_production_slice_has_no_upper_keys();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BzlEventTracker::default());
        let root = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\no=use_extension('//:other.bzl','other')\n";
        let reordered = "module(name='bazel_tools')\no=use_extension('//:other.bzl','other')\ne=use_extension('//:ext.bzl','ext')\n";
        let alternate = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','alternate')\no=use_extension('//:other.bzl','other')\n";
        let ext_a = "load('//:child.bzl','child')\ndef implementation(ctx):\n    pass\next=module_extension(implementation=implementation,environ=[child])\nalternate=module_extension(implementation=implementation,environ=['alternate'])\n";
        let ext_b = "load('//:child.bzl','child')\ndef implementation(ctx):\n    pass\next=module_extension(implementation=implementation,environ=['direct'])\nalternate=module_extension(implementation=implementation,environ=['alternate'])\n";
        let child_a = "child='child-a'\n";
        let child_b = "child='child-b'\n";
        let other = "def implementation(ctx):\n    pass\nother=module_extension(implementation=implementation)\n";
        let first =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        assert_transaction_epoch_subsets(&first);
        let retained = first.clone();
        let request_b = observed_loaded_handles(
            &dice,
            reordered,
            ext_a,
            child_a,
            other,
            Some(tracker.clone()),
        )
        .await;
        let request_a =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        assert_transaction_epoch_subsets(&request_b);
        assert_transaction_epoch_subsets(&request_a);
        assert_ne!(first.parent, request_b.parent);
        assert_eq!(first.parent, request_a.parent);
        assert_ne!(first.request, request_b.request);
        assert_eq!(first.ext, request_b.ext);
        assert_eq!(first.child, request_b.child);
        assert_eq!(first.other, request_b.other);
        assert_held_observed_loaded_handles(&first, &retained);

        let direct_b =
            observed_loaded_handles(&dice, root, ext_b, child_a, other, Some(tracker.clone()))
                .await;
        let direct_a =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        assert_transaction_epoch_subsets(&direct_b);
        assert_transaction_epoch_subsets(&direct_a);
        assert_ne!(first.parent, direct_b.parent);
        assert_eq!(first.parent, direct_a.parent);
        assert_ne!(first.ext, direct_b.ext);
        assert_ne!(first.ext_epoch, direct_b.ext_epoch);
        assert_eq!(first.request, direct_b.request);
        assert_eq!(first.child, direct_b.child);
        assert_eq!(first.other, direct_b.other);
        assert_held_observed_loaded_handles(&first, &retained);

        let recursive_b =
            observed_loaded_handles(&dice, root, ext_a, child_b, other, Some(tracker.clone()))
                .await;
        let recursive_a =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        assert_transaction_epoch_subsets(&recursive_b);
        assert_transaction_epoch_subsets(&recursive_a);
        assert_ne!(first.parent, recursive_b.parent);
        assert_eq!(first.parent, recursive_a.parent);
        assert_ne!(first.ext, recursive_b.ext);
        assert_ne!(first.ext_epoch, recursive_b.ext_epoch);
        assert_ne!(first.child, recursive_b.child);
        assert_ne!(first.child_epoch, recursive_b.child_epoch);
        assert_eq!(first.request, recursive_b.request);
        assert_eq!(first.other, recursive_b.other);
        assert_held_observed_loaded_handles(&first, &retained);

        let export_b = observed_loaded_handles(
            &dice,
            alternate,
            ext_a,
            child_a,
            other,
            Some(tracker.clone()),
        )
        .await;
        let export_a =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        assert_transaction_epoch_subsets(&export_b);
        assert_transaction_epoch_subsets(&export_a);
        assert_ne!(first.parent, export_b.parent);
        assert_ne!(first.projection, export_b.projection);
        assert_eq!(first.parent, export_a.parent);
        assert_ne!(first.request, export_b.request);
        assert_eq!(first.ext, export_b.ext);
        assert_eq!(first.child, export_b.child);
        assert_eq!(first.other, export_b.other);
        assert_held_observed_loaded_handles(&first, &retained);

        let (activations, dependencies) = tracker.take_rows();
        let parent = HostLoadedModuleExtensionDefinitionsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        )
        .to_string();
        let parent_row = dependencies.iter().find(|row| row.key == parent).unwrap();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        assert_eq!(
            parent_row.dependencies,
            [
                HostSelectedExtensionDefinitionLoadRequestsObservationKey::new(workspace.dupe())
                    .to_string(),
                HostBzlModuleObservationKey::new_bzlmod(
                    workspace.dupe(),
                    observed_test_label("ext.bzl"),
                )
                .to_string(),
                HostBzlModuleObservationKey::new_bzlmod(
                    workspace,
                    observed_test_label("other.bzl"),
                )
                .to_string(),
            ]
        );
        for key in
            activations
                .iter()
                .map(|row| row.key.as_str())
                .chain(dependencies.iter().flat_map(|row| {
                    std::iter::once(row.key.as_str())
                        .chain(row.dependencies.iter().map(String::as_str))
                }))
        {
            assert!(
                ![
                    "host-selected-extension-definition-load-requests:",
                    "host-bzl-module:",
                    "host-loaded-module-extension-definitions:",
                    "host-prepared-module-extension-inputs:",
                    "host-pure-module-extension-invocations:",
                    "host-instantiated-module-extension-repositories:",
                    "host-validated-module-extension-repositories:",
                    "host-root-repository-mapping:",
                    "host-canonical-selected-module-definition:",
                    "host-generated-repository-definition:",
                    "slug-command:",
                ]
                .iter()
                .any(|prefix| key.starts_with(prefix)),
                "unexpected activation: {key}"
            );
        }
        tracker.take();
        let key = HostLoadedModuleExtensionDefinitionsObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let mut cancelled = case_transaction_with_other(
            &dice,
            root,
            ext_a,
            child_a,
            other,
            true,
            Some(tracker.clone()),
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(tracker.take().is_empty());
        let (activations, dependencies) = tracker.take_rows();
        assert!(activations.is_empty() && dependencies.is_empty());
        let recovered =
            observed_loaded_handles(&dice, root, ext_a, child_a, other, Some(tracker.clone()))
                .await;
        let clean_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let clean = observed_loaded_handles(&clean_dice, root, ext_a, child_a, other, None).await;
        assert_eq!(recovered.parent, clean.parent);
        assert_eq!(recovered.request, clean.request);
        assert_eq!(recovered.ext, clean.ext);
        assert_eq!(recovered.child, clean.child);
        assert_eq!(recovered.other, clean.other);
        assert_eq!(recovered.projection, clean.projection);
        assert_transaction_epoch_subsets(&recovered);
        assert_transaction_epoch_subsets(&clean);
        let recovered_retained = recovered.clone();
        assert_held_observed_loaded_handles(&recovered, &recovered_retained);
        assert_loaded_warm(&dice, root, ext_a, child_a, other, &tracker, &recovered).await;
    }

    #[test]
    fn selected_external_dispatch_uses_each_resolved_child_route() {
        let source = include_str!("bzl_module.rs");
        let production = &source[..source
            .find("mod module_extension_definition_loading_tests")
            .unwrap()];
        let definition_start = production
            .find("async fn loaded_extension_definition_bzl")
            .unwrap();
        let definition_end = production[definition_start..]
            .find("fn finish_loaded_extension_definition_observed_child")
            .unwrap()
            + definition_start;
        let definition = &production[definition_start..definition_end];
        assert!(definition.contains("match request.source()"));
        assert!(definition.contains("RootRepositoryRoute::for_selected_extension_definition"));
        assert!(definition.contains("ExternalBzlModuleEvalKey::new_bzlmod"));
        assert!(definition.contains("ExternalBzlModuleObservationKey::new_bzlmod"));
        let resolver = &production[production
            .find("fn resolve_external_bzl_load_label")
            .unwrap()..];
        assert!(resolver.contains("selected_bzl_load_route(label.repo())"));
        let children = &production[production
            .find("async fn compute_external_bzl_children")
            .unwrap()..];
        let parse = children.find("parse_with_string_encoding").unwrap();
        let promote = children.find("compute_external_bzl_effective").unwrap();
        assert!(parse < promote);
        assert!(children.contains("resolved.label.canonical_label(&resolved.route)"));
        assert!(children.contains("compute_external_bzl_child("));
        assert!(children.contains("key.context"));
    }
}

#[cfg(all(test, windows))]
mod external_bzl_windows_tests {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::external_source_name;

    #[test]
    fn external_bzl_module_non_unicode_parser_path_is_a_typed_encoding_input() {
        let raw = OsString::from_wide(&[b'C'.into(), b':'.into(), b'\\'.into(), 0xd800]);
        let path = NormalizedAbsolutePath::new(PathBuf::from(raw)).unwrap();
        assert!(external_source_name(&path).is_err());
    }
}
