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
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
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
use slug_bzlmod_v2::HostRepositorySourceFileKey;
use slug_bzlmod_v2::HostRepositorySourceFileValue;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequests;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsError;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsKey;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInput;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequests;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsError;
use slug_bzlmod_v2::HostSelectedExtensionEvaluationInputRequestsKey;
use slug_bzlmod_v2::LogicalSpan;
use slug_bzlmod_v2::RepositoryPackageSourceError;
use slug_bzlmod_v2::RepositoryPackageSourceKey;
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
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
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

use crate::cycle_detector::BzlLoadCycle;
use crate::cycle_detector::BzlLoadCycleGuard;
use crate::cycle_detector::ExternalBzlLoadCycle;
use crate::cycle_detector::ExternalBzlLoadCycleGuard;
use crate::cycle_detector::HostBzlLoadCycle;
use crate::cycle_detector::HostBzlLoadCycleGuard;
use crate::glob::PackageListing;
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
use crate::package::LoadedPackage;
use crate::package::ModuleExtensionDefinitionProjection;
use crate::package::ModuleExtensionTagCoercionError;
use crate::package::PackageRecorder;
use crate::package::PackageTargetKind;
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

#[derive(Default)]
struct LoadingPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl LoadingPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
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
/// normalized absolute path DICE evaluated.  The value intentionally carries
/// no evaluator handle, so it can be used at semantic equality boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlModuleIdentity {
    pub label: CanonicalLabel,
    pub workspace_path: PathBuf,
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
        self.manifest == other.manifest
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

#[allow(dead_code)]
fn evaluate_host_package_attempt(
    input: &HostPackageAttemptInput<'_>,
    prepared: Arc<SmallMap<HostGlobLoadingRequest, HostGlobPrepared>>,
) -> HostPackageAttemptStep {
    let ast = match AstModule::parse_with_string_encoding(
        &input.build_file.display().to_string(),
        input.source.as_ref().clone(),
        &Dialect::Standard,
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
    let recorder = PackageRecorder::new_host(prepared, input.package_label.clone());
    let module = Module::new();
    let loader = LocalBzlLoader {
        modules: input
            .loaded_modules
            .iter()
            .map(|(load, module)| (load.as_str(), module.module.dupe()))
            .collect(),
    };
    let print_capture = input.capture_events.then(LoadingPrintCapture::default);
    let globals = loading_globals();
    let evaluation = {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&recorder);
        evaluator.set_loader(&loader);
        if let Some(print_capture) = print_capture.as_ref() {
            evaluator.set_print_handler(print_capture);
        }
        evaluator.eval_module(ast, &globals).map(|_| ())
    };
    let event_batch = print_capture
        .map(LoadingPrintCapture::into_batch)
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
) -> HostPackageAttemptDriverOutcome {
    let mut prepared = Arc::new(SmallMap::new());
    let mut observations = PathObservationEpoch::empty();
    loop {
        // The synchronous attempt returns only compact terminal state or one
        // request, so no evaluator/module/recorder borrow can cross this await.
        match evaluate_host_package_attempt(&input, prepared.dupe()) {
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
    match evaluate_host_package_attempts_driver(ctx, input, HostPackageLoadMode::Legacy).await {
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
struct RepositoryBzlLabel {
    package: PackagePath,
    target: RootPackageBzlTarget,
}

impl RepositoryBzlLabel {
    fn new(
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

    fn canonical_label(&self, route: &RootRepositoryRoute) -> CanonicalLabel {
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
        Ok(source
            .logical_path()
            .as_path()
            .as_os_str()
            .as_bytes()
            .iter()
            .copied()
            .map(char::from)
            .collect())
    }
    #[cfg(not(unix))]
    {
        source
            .logical_path()
            .as_path()
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| HostSourceInputError::UnsupportedPathEncoding {
                logical_path: source.logical_path().dupe(),
            })
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostBzlModuleEvalKey {
    workspace: NormalizedAbsolutePath,
    label: HostRootBzlLabel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostBzlCycleIdentity {
    workspace: NormalizedAbsolutePath,
    label: HostRootBzlLabel,
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
        Self { workspace, label }
    }

    pub(crate) fn cycle_identity(&self) -> HostBzlCycleIdentity {
        HostBzlCycleIdentity {
            workspace: self.workspace.dupe(),
            label: self.label.clone(),
        }
    }
}

impl fmt::Display for HostBzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-bzl-module:{}:{}", self.workspace, self.label)
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
}

impl HostBzlModuleObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self { workspace, label }
    }

    pub(crate) fn cycle_identity(&self) -> HostBzlCycleIdentity {
        HostBzlCycleIdentity {
            workspace: self.workspace.dupe(),
            label: self.label.clone(),
        }
    }
}

impl fmt::Display for HostBzlModuleObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observed-host-bzl-module:{}:{}",
            self.workspace, self.label
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ExternalBzlModuleEvalKey {
    route: RootRepositoryRoute,
    label: RepositoryBzlLabel,
}

impl ExternalBzlModuleEvalKey {
    fn new(route: RootRepositoryRoute, label: RepositoryBzlLabel) -> Self {
        Self { route, label }
    }

    fn canonical_label(&self) -> CanonicalLabel {
        self.label.canonical_label(&self.route)
    }
}

impl fmt::Display for ExternalBzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external-bzl-module:{}", self.canonical_label())
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
    fn cycle(&self) -> Option<&ExternalBzlLoadCycle> {
        match self {
            Self::Cycle(cycle) => Some(cycle),
            Self::Child { error, .. } => error.cycle(),
            Self::SourceCompute { .. }
            | Self::Source { .. }
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
    Bzl {
        origin: Arc<str>,
        load: Arc<str>,
        label: HostRootBzlLabel,
        error: Arc<HostBzlModuleError>,
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
            RootPackageLoadErrorInner::Attempt(error) => write!(f, "{error:?}"),
        }
    }
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
pub(crate) struct RootPackageLoadObservationKey(RootPackageLoadKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed key.
pub(crate) struct ObservedRootPackageLoad {
    result: Arc<Result<LoadedPackage, RootPackageLoadError>>,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedRootPackageLoad {
    pub(crate) fn result(&self) -> &Arc<Result<LoadedPackage, RootPackageLoadError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
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
    GlobUnsupported {
        canonical_repo: CompactString,
        package: PackagePath,
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
            RepositoryPackageLoadErrorInner::GlobUnsupported {
                canonical_repo,
                package,
            } => write!(
                f,
                "external repository BUILD globs are deferred: @@{canonical_repo}//{package}"
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
pub struct RepositoryPackageLoadKey {
    route: RootRepositoryRoute,
    package: PackagePath,
}

impl RepositoryPackageLoadKey {
    pub fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self { route, package }
    }
}

impl std::hash::Hash for RepositoryPackageLoadKey {
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
            self.route.canonical_repo(),
            self.package
        )
    }
}

impl RootPackageLoadKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

#[allow(dead_code)]
impl RootPackageLoadObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
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
        &Dialect::Standard,
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
                let child = HostBzlModuleEvalKey::new(workspace.dupe(), child_label.clone());
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
                let child = HostBzlModuleObservationKey::new(workspace.dupe(), child_label.clone());
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
        },
        digest(source_text.as_str()),
        loaded_modules.iter().map(|(_, module)| module),
    );
    let loader = LocalBzlLoader {
        modules: loaded_modules
            .iter()
            .map(|(load, module)| (load.as_str(), module.module.dupe()))
            .collect(),
    };
    let evaluation_context = BzlEvaluationContext::new(label.to_string());
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

#[allow(dead_code)]
fn loaded_extension_definitions_complete(
    value: Result<HostLoadedModuleExtensionDefinitions, HostLoadedModuleExtensionDefinitionsError>,
) -> HostLoadedModuleExtensionDefinitionsOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostLoadedModuleExtensionDefinitionsKey {
    type Value = HostLoadedModuleExtensionDefinitionsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let requests = match ctx
            .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return loaded_extension_definitions_complete(Err(
                        HostLoadedModuleExtensionDefinitionsError::Requests(error.clone()),
                    ));
                }
            },
            Err(error) => {
                return loaded_extension_definitions_complete(Err(
                    HostLoadedModuleExtensionDefinitionsError::RequestsCompute(
                        error.to_string().into(),
                    ),
                ));
            }
        };

        let mut definitions = Vec::new();
        for request in requests.parts().1 {
            let (label, export, _, _) = request.parts();
            let target = match RootPackageBzlTarget::parse(label.target().as_str()) {
                Ok(target) => target,
                Err(error) => {
                    return loaded_extension_definitions_complete(Err(
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            requests: requests.clone(),
                            request: request.clone(),
                            error: HostLoadedModuleExtensionDefinitionError::Label {
                                label: label.clone(),
                                message: error.to_string().into(),
                            },
                        },
                    ));
                }
            };
            let host_label = HostRootBzlLabel::new(label.package().package().clone(), target);
            let module = match host_dice_invariant(
                ctx.compute(&HostBzlModuleEvalKey::new(
                    self.workspace.dupe(),
                    host_label,
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                    Ok(module) => module.clone(),
                    Err(error) => {
                        return loaded_extension_definitions_complete(Err(
                            HostLoadedModuleExtensionDefinitionsError::Request {
                                requests: requests.clone(),
                                request: request.clone(),
                                error: HostLoadedModuleExtensionDefinitionError::Bzl {
                                    label: label.clone(),
                                    error: error.clone(),
                                },
                            },
                        ));
                    }
                },
            };
            let exported = match module.module.get(export) {
                Ok(value) => value,
                Err(error) => {
                    return loaded_extension_definitions_complete(Err(
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            requests: requests.clone(),
                            request: request.clone(),
                            error: HostLoadedModuleExtensionDefinitionError::Export {
                                label: label.clone(),
                                name: export.into(),
                                message: error.to_string().into(),
                            },
                        },
                    ));
                }
            };
            let exported = match exported.downcast::<FrozenModuleExtensionDefinition>() {
                Ok(value) => value,
                Err(_) => {
                    return loaded_extension_definitions_complete(Err(
                        HostLoadedModuleExtensionDefinitionsError::Request {
                            requests: requests.clone(),
                            request: request.clone(),
                            error: HostLoadedModuleExtensionDefinitionError::WrongKind {
                                label: label.clone(),
                                name: export.into(),
                            },
                        },
                    ));
                }
            };
            definitions.push(HostLoadedModuleExtensionDefinition {
                request: request.clone(),
                manifest: module.manifest.clone(),
                definition: exported.projection(),
            });
        }
        loaded_extension_definitions_complete(Ok(HostLoadedModuleExtensionDefinitions {
            requests,
            definitions: definitions.into(),
        }))
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

fn prepared_module_extension_inputs_complete(
    value: Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>,
) -> HostPreparedModuleExtensionInputsOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostPreparedModuleExtensionInputsKey {
    type Value = HostPreparedModuleExtensionInputsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let raw = match ctx
            .compute(&HostSelectedExtensionEvaluationInputRequestsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return prepared_module_extension_inputs_complete(Err(
                        HostPreparedModuleExtensionInputsError::Raw(error.clone()),
                    ));
                }
            },
            Err(error) => {
                return prepared_module_extension_inputs_complete(Err(
                    HostPreparedModuleExtensionInputsError::RawCompute(error.to_string().into()),
                ));
            }
        };
        let definitions = match ctx
            .compute(&HostLoadedModuleExtensionDefinitionsKey::new(
                self.workspace.dupe(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return prepared_module_extension_inputs_complete(Err(
                        HostPreparedModuleExtensionInputsError::Definitions {
                            raw,
                            error: Ok(error.clone()),
                        },
                    ));
                }
            },
            Err(error) => {
                return prepared_module_extension_inputs_complete(Err(
                    HostPreparedModuleExtensionInputsError::Definitions {
                        raw,
                        error: Err(error.to_string().into()),
                    },
                ));
            }
        };
        prepared_module_extension_inputs_complete(prepare_module_extension_inputs(raw, definitions))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn external_bzl_complete(
    result: Result<FrozenBzlModule, ExternalBzlModuleError>,
) -> SourcePreparationOutcome<Arc<Result<FrozenBzlModule, ExternalBzlModuleError>>> {
    SourcePreparationOutcome::Complete(Arc::new(result))
}

fn external_source_name(logical_path: &NormalizedAbsolutePath) -> Result<String, ()> {
    #[cfg(unix)]
    {
        Ok(logical_path
            .as_path()
            .as_os_str()
            .as_bytes()
            .iter()
            .copied()
            .map(char::from)
            .collect())
    }
    #[cfg(not(unix))]
    {
        logical_path.as_path().to_str().map(str::to_owned).ok_or(())
    }
}

#[async_trait]
impl Key for ExternalBzlModuleEvalKey {
    type Value = SourcePreparationOutcome<Arc<Result<FrozenBzlModule, ExternalBzlModuleError>>>;

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
            let canonical_label = self.canonical_label();
            let source_path = self.label.repository_relative_path();
            let source = match ctx
                .compute(&HostRepositorySourceFileKey::new(
                    self.route.clone(),
                    source_path,
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Present {
                        bytes,
                        logical_path,
                    },
                ))) => (bytes, logical_path),
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Absent,
                ))) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Absent {
                        label: canonical_label,
                    }));
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Source {
                        label: canonical_label,
                        error,
                    }));
                }
                Err(error) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::SourceCompute {
                        label: canonical_label,
                        message: Arc::from(error.to_string()),
                    }));
                }
            };
            let (bytes, logical_path) = source;
            let source_text = match String::from_utf8(bytes.to_vec()) {
                Ok(source) => Arc::new(source),
                Err(_) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Encoding {
                        label: canonical_label,
                    }));
                }
            };
            let source_name = match external_source_name(&logical_path) {
                Ok(source_name) => source_name,
                Err(()) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Encoding {
                        label: canonical_label,
                    }));
                }
            };
            let ast = match AstModule::parse_with_string_encoding(
                &source_name,
                source_text.as_ref().clone(),
                &Dialect::Standard,
                StringEncoding::BazelInternal,
            ) {
                Ok(ast) => ast,
                Err(error) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Parse {
                        label: canonical_label,
                        message: Arc::from(error.to_string()),
                    }));
                }
            };
            let loads = ast
                .loads()
                .into_iter()
                .map(|load| load.module_id.to_owned())
                .collect::<Vec<_>>();
            // Validate the complete direct-load set before any child source key
            // can be requested. This keeps rejected route/package forms outside
            // the external source graph.
            let resolved_loads = match loads
                .iter()
                .map(|load| {
                    resolve_external_load_label(&self.label.package, load)
                        .map(|label| (load.clone(), label))
                })
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(loads) => loads,
                Err(error) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::LoadLabel {
                        source: canonical_label,
                        error,
                    }));
                }
            };

            let mut loaded_modules = Vec::with_capacity(resolved_loads.len());
            for (raw_load, label) in resolved_loads {
                let child = ExternalBzlModuleEvalKey::new(self.route.clone(), label);
                let child_label = child.canonical_label();
                let guard = ctx
                    .cycle_guard::<ExternalBzlLoadCycleGuard>()
                    .unwrap_or_else(|error| {
                        panic!("external Bzl cycle-guard invariant failed: {error:?}")
                    })
                    .expect("external Bzl loading requires the request cycle detector");
                let child_value = match guard.guard_this(ctx.compute(&child)).await {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => {
                        return external_bzl_complete(Err(ExternalBzlModuleError::Child {
                            raw_load: Arc::from(raw_load.as_str()),
                            canonical_label: child_label.clone(),
                            error: Arc::new(ExternalBzlModuleError::SourceCompute {
                                label: child_label,
                                message: Arc::from(error.to_string()),
                            }),
                        }));
                    }
                    Err(cycle) => {
                        let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                        return external_bzl_complete(Err(ExternalBzlModuleError::Cycle(cycle)));
                    }
                };
                let module = match child_value {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => {
                            return external_bzl_complete(Err(ExternalBzlModuleError::Child {
                                raw_load: Arc::from(raw_load.as_str()),
                                canonical_label: child_label,
                                error: Arc::new(error.clone()),
                            }));
                        }
                    },
                };
                loaded_modules.push((raw_load, module));
            }

            let module = Module::new();
            let manifest = BzlLoadManifest::new(
                BzlModuleIdentity {
                    label: canonical_label.clone(),
                    workspace_path: logical_path.as_path().to_path_buf(),
                },
                digest(source_text.as_str()),
                loaded_modules.iter().map(|(_, module)| module),
            );
            let loader = LocalBzlLoader {
                modules: loaded_modules
                    .iter()
                    .map(|(load, module)| (load.as_str(), module.module.dupe()))
                    .collect(),
            };
            let evaluation_context = BzlEvaluationContext::new(canonical_label.to_string());
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
                if let Err(error) = evaluation {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Evaluation {
                        label: canonical_label,
                        message: Arc::from(error.to_string()),
                    }));
                }
            }
            let module = match module.freeze() {
                Ok(module) => module,
                Err(error) => {
                    return external_bzl_complete(Err(ExternalBzlModuleError::Freeze {
                        label: canonical_label,
                        message: Arc::from(format!("{error:?}")),
                    }));
                }
            };
            external_bzl_complete(Ok(FrozenBzlModule {
                module,
                path: logical_path.as_path().to_path_buf(),
                loads,
                retained_bzl_modules: retained_module_closure(&loaded_modules),
                manifest,
            }))
        }
        .await;
        if capture_events && value.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ExternalBzlModuleEvalKey stores one local Complete event batch");
        }
        value
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
        &Dialect::Standard,
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
        let label = match resolve_host_load_label(&key.package, &load) {
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
        let child = match mode {
            HostPackageLoadMode::Legacy => {
                let child = HostBzlModuleEvalKey::new(key.workspace.dupe(), label.clone());
                host_dice_invariant(ctx.compute(&child).await)
                    .map(|result| Ok((result.as_ref().clone(), PathObservationEpoch::empty())))
            }
            HostPackageLoadMode::Observed => {
                let child = HostBzlModuleObservationKey::new(key.workspace.dupe(), label.clone());
                host_dice_invariant(ctx.compute(&child).await).map(|value| {
                    value
                        .map(|observed| (observed.result().clone(), observed.observations().dupe()))
                })
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
        observations = match merge_root_package_observations(mode, observations, &incoming) {
            Ok(observations) => observations,
            Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
        };
        let module = match child {
            Ok(module) => module,
            Err(error) => {
                let build_name: String = source
                    .relative_path()
                    .iter()
                    .copied()
                    .map(char::from)
                    .collect();
                let inner = RootPackageLoadErrorInner::Bzl {
                    origin: Arc::from(if key.package.as_str().is_empty() {
                        build_name
                    } else {
                        format!("{}/{build_name}", key.package)
                    }),
                    load: Arc::from(load),
                    label,
                    error: Arc::new(error),
                };
                return root_package_driver_complete(
                    Err(RootPackageLoadError::new(inner)),
                    observations,
                );
            }
        };
        loaded_modules.push((load, module));
    }
    let package_dir = source.package_root().as_path().join(key.package.as_str());
    let attempts = evaluate_host_package_attempts_driver(
        ctx,
        HostPackageAttemptInput {
            workspace: key.workspace.dupe(),
            logical_package_root: source.package_root().dupe(),
            package: key.package.clone(),
            package_dir,
            build_file: source.logical_path().as_path().to_path_buf(),
            source: source_text,
            package_label: CompactString::new(key.package.as_str()),
            loaded_modules: &loaded_modules,
            capture_events,
        },
        mode,
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

fn repository_package_complete(
    result: Result<LoadedPackage, RepositoryPackageLoadError>,
) -> SourcePreparationOutcome<Arc<Result<LoadedPackage, RepositoryPackageLoadError>>> {
    SourcePreparationOutcome::Complete(Arc::new(result))
}

fn loaded_external_target_kind(kind: &PackageTargetKind) -> Option<&'static str> {
    match kind {
        PackageTargetKind::ExportedFile | PackageTargetKind::Filegroup { .. } => None,
        PackageTargetKind::Alias { .. } => Some("alias"),
        PackageTargetKind::ConfigSetting { .. } => Some("config_setting"),
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

#[async_trait]
impl Key for RepositoryPackageLoadKey {
    type Value = SourcePreparationOutcome<Arc<Result<LoadedPackage, RepositoryPackageLoadError>>>;

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
            let package =
                PackageIdentifier::new(self.route.canonical_repo().clone(), self.package.clone());
            let source_key = RepositoryPackageSourceKey::new(self.route.clone(), package)
                .expect("repository package load route and package agree");
            let source = match ctx.compute(&source_key).await {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                    Ok(source) => source.dupe(),
                    Err(error) => {
                        return repository_package_complete(Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::Source {
                                error: error.clone(),
                            },
                        )));
                    }
                },
                Err(error) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::SourceCompute {
                            canonical_repo: CompactString::new(
                                self.route.canonical_repo().as_str(),
                            ),
                            package: self.package.clone(),
                            message: Arc::from(error.to_string()),
                        },
                    )));
                }
            };
            let relative_build_file =
                PathBuf::from(self.package.as_str()).join(source.build_file_name());
            let source = match std::str::from_utf8(source.bytes().as_ref()) {
                Ok(source) => Arc::new(source.to_owned()),
                Err(_) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::Encoding {
                            path: relative_build_file,
                        },
                    )));
                }
            };
            let canonical_repo = CompactString::new(self.route.canonical_repo().as_str());
            let logical_package_dir = PathBuf::from("<output_base>")
                .join("external")
                .join(canonical_repo.as_str())
                .join(self.package.as_str());
            let logical_build_file = logical_package_dir.join(
                relative_build_file
                    .file_name()
                    .expect("BUILD candidate has a basename"),
            );
            let ast = match AstModule::parse_with_string_encoding(
                &logical_build_file.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Standard,
                StringEncoding::BazelInternal,
            ) {
                Ok(ast) => ast,
                Err(error) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::Parse {
                            canonical_repo,
                            package: self.package.clone(),
                            message: Arc::from(error.to_string()),
                        },
                    )));
                }
            };
            let loads = ast
                .loads()
                .into_iter()
                .map(|load| load.module_id.to_owned())
                .collect::<Vec<_>>();
            let resolved_loads = match loads
                .iter()
                .map(|load| {
                    resolve_external_load_label(&self.package, load)
                        .map(|label| (load.clone(), label))
                })
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(loads) => loads,
                Err(error) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::LoadLabel {
                            canonical_repo,
                            package: self.package.clone(),
                            error,
                        },
                    )));
                }
            };
            let build_basename = relative_build_file
                .file_name()
                .expect("BUILD candidate has a basename")
                .to_string_lossy();
            let build_origin: Arc<str> = Arc::from(format!(
                "@@{canonical_repo}//{}/{}",
                self.package, build_basename
            ));
            let mut loaded_modules = Vec::with_capacity(resolved_loads.len());
            for (raw_load, label) in resolved_loads {
                let child = ExternalBzlModuleEvalKey::new(self.route.clone(), label);
                let canonical_label = child.canonical_label();
                let child_value = match ctx.compute(&child).await {
                    Ok(value) => value,
                    Err(error) => {
                        return repository_package_complete(Err(RepositoryPackageLoadError::new(
                            RepositoryPackageLoadErrorInner::Bzl {
                                origin: build_origin.clone(),
                                raw_load: Arc::from(raw_load.as_str()),
                                canonical_label: canonical_label.clone(),
                                error: Arc::new(ExternalBzlModuleError::SourceCompute {
                                    label: canonical_label,
                                    message: Arc::from(error.to_string()),
                                }),
                            },
                        )));
                    }
                };
                let module = match child_value {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => {
                            return repository_package_complete(Err(
                                RepositoryPackageLoadError::new(
                                    RepositoryPackageLoadErrorInner::Bzl {
                                        origin: build_origin.clone(),
                                        raw_load: Arc::from(raw_load.as_str()),
                                        canonical_label,
                                        error: Arc::new(error.clone()),
                                    },
                                ),
                            ));
                        }
                    },
                };
                loaded_modules.push((raw_load, module));
            }
            let input = HostPackageAttemptInput {
                workspace: self.route.workspace().dupe(),
                logical_package_root: self.route.workspace().dupe(),
                package: self.package.clone(),
                package_dir: logical_package_dir,
                build_file: logical_build_file,
                source,
                package_label: CompactString::new(self.package.as_str()),
                loaded_modules: &loaded_modules,
                capture_events,
            };
            match evaluate_host_package_attempt(&input, Arc::new(SmallMap::new())) {
                HostPackageAttemptStep::Pending {
                    event_batch: batch, ..
                } => {
                    event_batch = Some(batch);
                    repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::GlobUnsupported {
                            canonical_repo,
                            package: self.package.clone(),
                        },
                    )))
                }
                HostPackageAttemptStep::Terminal(terminal) => {
                    event_batch = Some(terminal.event_batch);
                    let result = terminal.result.map_err(|error| {
                        RepositoryPackageLoadError::new(RepositoryPackageLoadErrorInner::Attempt(
                            error,
                        ))
                    });
                    let result = result.and_then(|loaded| {
                        if loads.is_empty() {
                            return Ok(loaded);
                        }
                        if let Some((target, reason)) =
                            loaded_external_starlark_rule_reason(&loaded.targets)
                        {
                            return Err(RepositoryPackageLoadError::new(
                                RepositoryPackageLoadErrorInner::LoadedStarlarkRule {
                                    canonical_repo,
                                    package: self.package.clone(),
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
                            return Ok(loaded);
                        }
                        if let Some((target, kind)) = loaded.targets.iter().find_map(|target| {
                            loaded_external_target_kind(&target.kind)
                                .map(|kind| (target.name.as_str(), kind))
                        }) {
                            return Err(RepositoryPackageLoadError::new(
                                RepositoryPackageLoadErrorInner::LoadedTargetKind {
                                    canonical_repo,
                                    package: self.package.clone(),
                                    target: Arc::from(target),
                                    kind: Arc::from(kind),
                                },
                            ));
                        }
                        Ok(loaded)
                    });
                    repository_package_complete(result)
                }
            }
        }
        .await;
        if capture_events && value.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("RepositoryPackageLoadKey stores one local Complete event batch");
        }
        value
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
    ) -> anyhow::Result<LoadedPackage> {
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
                &self.path.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Standard,
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
                    &self.path.display().to_string(),
                    parsed.source.clone(),
                    &Dialect::Standard,
                )
                .map_err(|error| LoadingError::new(error.to_string()))?;
                let module = Module::new();
                let manifest = BzlLoadManifest::new(
                    bzl_module_identity(&self.workspace, &self.path)?,
                    parsed.source_digest,
                    loaded_modules.iter().map(|(_, module)| module),
                );
                let loader = LocalBzlLoader {
                    modules: loaded_modules
                        .iter()
                        .map(|(load, module)| (load.as_str(), module.module.dupe()))
                        .collect(),
                };
                let evaluation_context = BzlEvaluationContext::new(
                    bzl_source_label(&self.workspace, &self.path)
                        .map_err(|error| LoadingError::new(error.to_string()))?,
                );
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
    type Value = LoadResult<LoadedPackage>;

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
                &Dialect::Standard,
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
                    &Dialect::Standard,
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
                let recorder = PackageRecorder::new(listing, package_label);
                let module = Module::new();
                let loader = LocalBzlLoader {
                    modules: loaded_modules
                        .iter()
                        .map(|(load, module)| (load.as_str(), module.module.dupe()))
                        .collect(),
                };
                let print_capture = capture_events.then(LoadingPrintCapture::default);
                let globals = loading_globals();
                {
                    let mut evaluator = Evaluator::new(&module);
                    evaluator.extra = Some(&recorder);
                    evaluator.set_loader(&loader);
                    if let Some(print_capture) = print_capture.as_ref() {
                        evaluator.set_print_handler(print_capture);
                    }
                    let evaluation = evaluator.eval_module(ast, &globals).map(|_| ());
                    drop(evaluator);
                    event_batch = print_capture.map(LoadingPrintCapture::into_batch);
                    evaluation.map_err(|error| LoadingError::new(error.to_string()))?;
                }
                let direct_load_roots = first_seen_direct_roots(&loaded_modules);
                let retained_bzl_modules = flattened_lifetime_closure(&loaded_modules);
                let reachable_loads = retained_bzl_modules
                    .iter()
                    .map(|entry| entry.identity.clone())
                    .collect::<Vec<_>>();
                let load_fingerprint = package_load_fingerprint(&loaded_modules);
                Ok(recorder.finish(
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
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
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

    #[derive(Debug, Clone)]
    struct BzlActivation {
        label: CanonicalLabel,
        kind: ActivationKind,
        batch: Option<EventBatch>,
        observed: bool,
    }

    #[derive(Default)]
    struct BzlEventTracker {
        events: Mutex<Vec<BzlActivation>>,
        legacy_modules: AtomicUsize,
        observed_modules: AtomicUsize,
        legacy_sources: AtomicUsize,
        observed_sources: AtomicUsize,
    }

    impl BzlEventTracker {
        fn take(&self) -> Vec<BzlActivation> {
            std::mem::take(&mut *self.events.lock().unwrap())
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
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
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
                    files: Arc::new(SortedMap::from_iter([
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
                    ])),
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
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let path_epoch = PathObservationEpoch::new(
            ["/", WORKSPACE]
                .into_iter()
                .enumerate()
                .map(|(index, path)| {
                    (
                        PathObservationDemand::new(
                            PathObservationNamespace::Host,
                            NormalizedAbsolutePath::new(path).unwrap(),
                            PathObservationOperation::Lstat,
                        ),
                        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::Directory,
                            index as i64 + 1,
                            1,
                            1,
                            1,
                            0o755,
                        ))),
                    )
                })
                .chain(
                    ["REPO.bazel", ".bazelignore", "BUILD"]
                        .into_iter()
                        .map(|name| {
                            (
                                PathObservationDemand::new(
                                    PathObservationNamespace::Host,
                                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}"))
                                        .unwrap(),
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
                    ["ext.bzl", "child.bzl", "other.bzl"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, name)| {
                            (
                                PathObservationDemand::new(
                                    PathObservationNamespace::Host,
                                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}"))
                                        .unwrap(),
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
                    [
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
                ),
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
        assert!(tracker.take().is_empty());

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
        let unused_deferred_schema = compute_prepared_case(
            &dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n",
            "def implementation(ctx):\n    pass\n\
             tag=tag_class(attrs={'values':attr.string_list()})\n\
             ext=module_extension(implementation=implementation,tag_classes={'tag':tag})\n",
            None,
        )
        .await;
        assert!(matches!(
            unused_deferred_schema,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostPreparedModuleExtensionInputsError::AfterInputs {
                        error: HostPreparedModuleExtensionInputError::Attribute(_),
                        ..
                    })
                )
        ));
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
            Some(tracker),
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
