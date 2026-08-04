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
use slug_bzlmod_v2::RepositorySourceFileError;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorError;
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::RootPackageBzlTargetError;
use slug_bzlmod_v2::RootPackageSource;
use slug_bzlmod_v2::RootPackageSourceError;
use slug_bzlmod_v2::RootPackageSourceKey;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
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
use crate::package::HostGlobAttemptControl;
use crate::package::HostGlobAttemptError;
use crate::package::LoadedPackage;
use crate::package::PackageRecorder;
use crate::package::PackageTargetKind;
use crate::package::loading_globals;
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
    module: FrozenModule,
    path: PathBuf,
    loads: Vec<String>,
    manifest: BzlLoadManifest,
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
async fn evaluate_host_package_attempts(
    ctx: &mut DiceComputations<'_>,
    input: HostPackageAttemptInput<'_>,
) -> HostPackageAttemptOutcome {
    let mut prepared = Arc::new(SmallMap::new());
    loop {
        // The synchronous attempt returns only compact terminal state or one
        // request, so no evaluator/module/recorder borrow can cross this await.
        match evaluate_host_package_attempt(&input, prepared.dupe()) {
            HostPackageAttemptStep::Terminal(terminal) => {
                return SourcePreparationOutcome::Complete(Arc::new(terminal));
            }
            HostPackageAttemptStep::Pending {
                request,
                event_batch,
            } => {
                let outcome = match compute_host_glob_request(
                    ctx,
                    input.workspace.dupe(),
                    input.logical_package_root.dupe(),
                    input.package.clone(),
                    request.dupe(),
                )
                .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Arc::new(
                            HostPackageAttemptTerminal {
                                result: Err(HostPackageAttemptError::Input(error)),
                                event_batch,
                            },
                        ));
                    }
                };
                match outcome {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(value) => {
                        let replaced = Arc::make_mut(&mut prepared).insert(request, value);
                        debug_assert!(replaced.is_none());
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRootBzlLabel {
    package: PackagePath,
    target: RootPackageBzlTarget,
}

impl HostRootBzlLabel {
    fn new(package: PackagePath, target: RootPackageBzlTarget) -> Self {
        Self { package, target }
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
            Self::Parse { label, message } => write!(f, "parsing {label}: {message}"),
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

impl HostBzlModuleEvalKey {
    fn new(workspace: NormalizedAbsolutePath, label: HostRootBzlLabel) -> Self {
        Self { workspace, label }
    }
}

impl fmt::Display for HostBzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-bzl-module:{}:{}", self.workspace, self.label)
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RepositoryPackageLoadErrorInner {
    Source {
        path: PathBuf,
        error: RepositorySourceFileError,
    },
    SourceCompute {
        path: PathBuf,
        message: Arc<str>,
    },
    MissingBuild {
        canonical_repo: CompactString,
        package: PackagePath,
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
}

impl fmt::Display for RepositoryPackageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            RepositoryPackageLoadErrorInner::Source { path, error } => {
                write!(
                    f,
                    "reading external repository source {}: {error:?}",
                    path.display()
                )
            }
            RepositoryPackageLoadErrorInner::SourceCompute { path, message } => {
                write!(
                    f,
                    "computing external repository source {}: {message}",
                    path.display()
                )
            }
            RepositoryPackageLoadErrorInner::MissingBuild {
                canonical_repo,
                package,
            } => write!(
                f,
                "no such package '@@{canonical_repo}//{package}': BUILD file not found in directory '{package}' of external repository @@{canonical_repo}. Add a BUILD file to a directory to mark it as a package."
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

impl std::error::Error for RepositoryPackageLoadError {}

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

impl fmt::Display for RootPackageLoadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-package-load:{}//{}", self.workspace, self.package)
    }
}

#[track_caller]
fn host_dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host loading DICE invariant failed: {error:?}"))
}

fn host_bzl_complete(
    result: Result<FrozenBzlModule, HostBzlModuleError>,
) -> SourcePreparationOutcome<Arc<Result<FrozenBzlModule, HostBzlModuleError>>> {
    SourcePreparationOutcome::Complete(Arc::new(result))
}

#[async_trait]
impl Key for HostBzlModuleEvalKey {
    type Value = SourcePreparationOutcome<Arc<Result<FrozenBzlModule, HostBzlModuleError>>>;

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
            let source = match host_dice_invariant(
                ctx.compute(&RootPackageSourceKey::for_bzl(
                    self.workspace.dupe(),
                    self.label.package.clone(),
                    self.label.target.dupe(),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(source) => match source.as_ref() {
                    Ok(source) => source.dupe(),
                    Err(error) => {
                        return host_bzl_complete(Err(HostBzlModuleError::Source(error.clone())));
                    }
                },
            };
            let source_text = match host_source_text(&source) {
                Ok(source) => source,
                Err(error) => return host_bzl_complete(Err(HostBzlModuleError::Input(error))),
            };
            let source_name = match host_source_name(&source) {
                Ok(name) => name,
                Err(error) => return host_bzl_complete(Err(HostBzlModuleError::Input(error))),
            };
            let ast = match AstModule::parse_with_string_encoding(
                &source_name,
                source_text.as_ref().clone(),
                &Dialect::Standard,
                StringEncoding::BazelInternal,
            ) {
                Ok(ast) => ast,
                Err(error) => {
                    return host_bzl_complete(Err(HostBzlModuleError::Parse {
                        label: self.label.clone(),
                        message: Arc::from(error.to_string()),
                    }));
                }
            };
            let loads = ast
                .loads()
                .into_iter()
                .map(|load| load.module_id.to_owned())
                .collect::<Vec<_>>();
            let mut loaded_modules = Vec::with_capacity(loads.len());
            for load in &loads {
                let label = match resolve_host_load_label(&self.label.package, load) {
                    Ok(label) => label,
                    Err(error) => {
                        return host_bzl_complete(Err(HostBzlModuleError::LoadLabel {
                            source: self.label.clone(),
                            error,
                        }));
                    }
                };
                let child = HostBzlModuleEvalKey::new(self.workspace.dupe(), label.clone());
                let guard = host_dice_invariant(ctx.cycle_guard::<HostBzlLoadCycleGuard>())
                    .expect("Host bzl loading requires the request cycle detector");
                let child_value = match guard.guard_this(ctx.compute(&child)).await {
                    Ok(result) => host_dice_invariant(result),
                    Err(cycle) => {
                        let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                        return host_bzl_complete(Err(HostBzlModuleError::Cycle(cycle)));
                    }
                };
                let module = match child_value {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => {
                            return host_bzl_complete(Err(HostBzlModuleError::Child {
                                load: Arc::from(load.as_str()),
                                label,
                                error: Arc::new(error.clone()),
                            }));
                        }
                    },
                };
                loaded_modules.push((load.clone(), module));
            }

            let module = Module::new();
            let manifest = BzlLoadManifest::new(
                BzlModuleIdentity {
                    label: self.label.canonical_label(),
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
            let evaluation_context = BzlEvaluationContext::new(self.label.to_string());
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
                    return host_bzl_complete(Err(HostBzlModuleError::Evaluation(
                        LoadingError::new(error.to_string()),
                    )));
                }
            }
            let module = match module.freeze() {
                Ok(module) => module,
                Err(error) => {
                    return host_bzl_complete(Err(HostBzlModuleError::Freeze {
                        label: self.label.clone(),
                        message: Arc::from(format!("{error:?}")),
                    }));
                }
            };
            host_bzl_complete(Ok(FrozenBzlModule {
                module,
                path: source.logical_path().as_path().to_path_buf(),
                loads,
                retained_bzl_modules: retained_module_closure(&loaded_modules),
                manifest,
            }))
        }
        .await;
        if capture_events && value.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("HostBzlModuleEvalKey stores one local Complete event batch");
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

fn root_package_complete(
    result: Result<LoadedPackage, RootPackageLoadError>,
) -> SourcePreparationOutcome<Arc<Result<LoadedPackage, RootPackageLoadError>>> {
    SourcePreparationOutcome::Complete(Arc::new(result))
}

#[async_trait]
impl Key for RootPackageLoadKey {
    type Value = SourcePreparationOutcome<Arc<Result<LoadedPackage, RootPackageLoadError>>>;

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
            match host_dice_invariant(
                ctx.compute(&RootModuleLoadingAnchorKey::new(self.workspace.dupe()))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(anchor) => {
                    if let Err(error) = anchor.as_ref() {
                        return root_package_complete(Err(RootPackageLoadError::new(
                            RootPackageLoadErrorInner::RootModule(error.clone()),
                        )));
                    }
                }
            }
            let source = match host_dice_invariant(
                ctx.compute(&RootPackageSourceKey::for_build(
                    self.workspace.dupe(),
                    self.package.clone(),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(source) => match source.as_ref() {
                    Ok(source) => source.dupe(),
                    Err(error) => {
                        return root_package_complete(Err(RootPackageLoadError::new(
                            RootPackageLoadErrorInner::Source(error.clone()),
                        )));
                    }
                },
            };
            let source_text = match host_source_text(&source) {
                Ok(source) => source,
                Err(error) => {
                    return root_package_complete(Err(RootPackageLoadError::new(
                        RootPackageLoadErrorInner::Input(error),
                    )));
                }
            };
            let source_name = match host_source_name(&source) {
                Ok(name) => name,
                Err(error) => {
                    return root_package_complete(Err(RootPackageLoadError::new(
                        RootPackageLoadErrorInner::Input(error),
                    )));
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
                    return root_package_complete(Err(RootPackageLoadError::new(
                        RootPackageLoadErrorInner::Parse {
                            package: self.package.clone(),
                            message: Arc::from(error.to_string()),
                        },
                    )));
                }
            };
            let mut loaded_modules = Vec::new();
            for load in ast.loads() {
                let load = load.module_id.to_owned();
                let label = match resolve_host_load_label(&self.package, &load) {
                    Ok(label) => label,
                    Err(error) => {
                        return root_package_complete(Err(RootPackageLoadError::new(
                            RootPackageLoadErrorInner::LoadLabel {
                                package: self.package.clone(),
                                error,
                            },
                        )));
                    }
                };
                let child = HostBzlModuleEvalKey::new(self.workspace.dupe(), label.clone());
                let child_value = host_dice_invariant(ctx.compute(&child).await);
                let module = match child_value {
                    SourcePreparationOutcome::Need(need) => {
                        return SourcePreparationOutcome::Need(need);
                    }
                    SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                        Ok(module) => module.clone(),
                        Err(error) => {
                            let build_name: String = source
                                .relative_path()
                                .iter()
                                .copied()
                                .map(char::from)
                                .collect();
                            let inner = RootPackageLoadErrorInner::Bzl {
                                origin: Arc::from(if self.package.as_str().is_empty() {
                                    build_name
                                } else {
                                    format!("{}/{build_name}", self.package)
                                }),
                                load: Arc::from(load),
                                label,
                                error: Arc::new(error.clone()),
                            };
                            return root_package_complete(Err(RootPackageLoadError::new(inner)));
                        }
                    },
                };
                loaded_modules.push((load, module));
            }
            let package_dir = source.package_root().as_path().join(self.package.as_str());
            let attempts = evaluate_host_package_attempts(
                ctx,
                HostPackageAttemptInput {
                    workspace: self.workspace.dupe(),
                    logical_package_root: source.package_root().dupe(),
                    package: self.package.clone(),
                    package_dir,
                    build_file: source.logical_path().as_path().to_path_buf(),
                    source: source_text,
                    package_label: CompactString::new(self.package.as_str()),
                    loaded_modules: &loaded_modules,
                    capture_events,
                },
            )
            .await;
            match attempts {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(terminal) => {
                    event_batch = Some(terminal.event_batch.clone());
                    root_package_complete(terminal.result.clone().map_err(|error| {
                        RootPackageLoadError::new(RootPackageLoadErrorInner::Attempt(error))
                    }))
                }
            }
        }
        .await;
        if capture_events && value.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("HostPackageLoadKey stores one local Complete event batch");
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
            .filter(|(schema, _)| schema.dependency_reachable())
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
            let primary = PathBuf::from(self.package.as_str()).join("BUILD.bazel");
            let fallback = PathBuf::from(self.package.as_str()).join("BUILD");
            let primary_path = primary;
            let primary_value = ctx
                .compute(&HostRepositorySourceFileKey::new(
                    self.route.clone(),
                    primary_path.clone(),
                ))
                .await;
            let source = match primary_value {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Present { bytes, .. },
                ))) => (primary_path, bytes),
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Absent,
                ))) => {
                    let fallback_path = fallback;
                    let fallback_value = ctx
                        .compute(&HostRepositorySourceFileKey::new(
                            self.route.clone(),
                            fallback_path.clone(),
                        ))
                        .await;
                    match fallback_value {
                        Ok(SourcePreparationOutcome::Need(need)) => {
                            return SourcePreparationOutcome::Need(need);
                        }
                        Ok(SourcePreparationOutcome::Complete(Ok(
                            HostRepositorySourceFileValue::Present { bytes, .. },
                        ))) => (fallback_path, bytes),
                        Ok(SourcePreparationOutcome::Complete(Ok(
                            HostRepositorySourceFileValue::Absent,
                        ))) => {
                            return repository_package_complete(Err(
                                RepositoryPackageLoadError::new(
                                    RepositoryPackageLoadErrorInner::MissingBuild {
                                        canonical_repo: CompactString::new(
                                            self.route.canonical_repo().as_str(),
                                        ),
                                        package: self.package.clone(),
                                    },
                                ),
                            ));
                        }
                        Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                            return repository_package_complete(Err(
                                RepositoryPackageLoadError::new(
                                    RepositoryPackageLoadErrorInner::Source {
                                        path: fallback_path,
                                        error,
                                    },
                                ),
                            ));
                        }
                        Err(error) => {
                            return repository_package_complete(Err(
                                RepositoryPackageLoadError::new(
                                    RepositoryPackageLoadErrorInner::SourceCompute {
                                        path: fallback_path,
                                        message: Arc::from(error.to_string()),
                                    },
                                ),
                            ));
                        }
                    }
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::Source {
                            path: primary_path,
                            error,
                        },
                    )));
                }
                Err(error) => {
                    return repository_package_complete(Err(RepositoryPackageLoadError::new(
                        RepositoryPackageLoadErrorInner::SourceCompute {
                            path: primary_path,
                            message: Arc::from(error.to_string()),
                        },
                    )));
                }
            };
            let (relative_build_file, bytes) = source;
            let source = match std::str::from_utf8(bytes.as_ref()) {
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
