/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Interpreter related Dice calculations

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use futures::future::BoxFuture;
use itertools::Itertools;
use slug_common::dice::cells::HasCellResolver;
use slug_common::package_listing::dice::DicePackageListingResolver;
use slug_core::build_file_path::BuildFilePath;
use slug_core::bzl::ImportPath;
use slug_core::cells::build_file_cell::BuildFileCell;
use slug_core::package::PackageLabel;
use slug_events::dispatch::async_record_root_spans;
use slug_events::span::SpanId;
use slug_interpreter::file_loader::LoadedModule;
use slug_interpreter::file_loader::ModuleDeps;
use slug_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use slug_interpreter::load_module::InterpreterCalculationImpl;
use slug_interpreter::paths::module::OwnedStarlarkModulePath;
use slug_interpreter::paths::module::StarlarkModulePath;
use slug_interpreter::paths::package::PackageFilePath;
use slug_interpreter::paths::path::OwnedStarlarkPath;
use slug_interpreter::paths::path::StarlarkPath;
use slug_node::metadata::key::MetadataKey;
use slug_node::nodes::eval_result::EvaluationResult;
use slug_node::nodes::frontend::TARGET_GRAPH_CALCULATION_IMPL;
use slug_node::nodes::frontend::TargetGraphCalculation;
use slug_node::nodes::frontend::TargetGraphCalculationImpl;
use slug_node::package_values_calculation::PACKAGE_VALUES_CALCULATION;
use slug_node::package_values_calculation::PackageValuesCalculation;
use slug_util::time_span::TimeSpan;
use once_cell::sync::Lazy;
use smallvec::SmallVec;
use starlark::environment::Globals;
use starlark_map::small_map::SmallMap;
use tokio::sync::Semaphore;

use crate::interpreter::dice_calculation_delegate::HasCalculationDelegate;
use crate::interpreter::dice_calculation_delegate::testing::EvalImportKey;
use crate::interpreter::global_interpreter_state::HasGlobalInterpreterState;
use crate::interpreter::package_file_calculation::EvalPackageFile;

// Key for 'InterpreterCalculation::get_interpreter_results'
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
pub struct InterpreterResultsKey(pub PackageLabel);

static INTERPRETER_RESULTS_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static INTERPRETER_RESULTS_QUEUED: AtomicUsize = AtomicUsize::new(0);
static INTERPRETER_RESULTS_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static INTERPRETER_RESULTS_MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static INTERPRETER_RESULTS_MAX_QUEUED: AtomicUsize = AtomicUsize::new(0);
static EVAL_IMPORT_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static EVAL_IMPORT_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static EVAL_IMPORT_MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);

static PACKAGE_EVALUATION_SEMAPHORE: Lazy<Semaphore> =
    Lazy::new(|| Semaphore::new(package_evaluation_concurrency_limit()));

fn canonicalize_bzlmod_module_path(
    path: StarlarkModulePath<'_>,
) -> slug_error::Result<OwnedStarlarkModulePath> {
    fn import_path(path: &ImportPath) -> slug_error::Result<ImportPath> {
        ImportPath::new_with_build_file_cells(
            path.path().clone(),
            BuildFileCell::new(path.path().cell()),
        )
    }

    Ok(match path {
        StarlarkModulePath::LoadFile(path) => OwnedStarlarkModulePath::LoadFile(import_path(path)?),
        StarlarkModulePath::JsonFile(path) => OwnedStarlarkModulePath::JsonFile(import_path(path)?),
        StarlarkModulePath::TomlFile(path) => OwnedStarlarkModulePath::TomlFile(import_path(path)?),
        StarlarkModulePath::BxlFile(path) => OwnedStarlarkModulePath::BxlFile(path.clone()),
    })
}

fn package_evaluation_concurrency_limit() -> usize {
    // Bazel's --loading_phase_threads=auto is host-resource based. Use the same
    // shape here so DICE cannot fan package/build-file evaluation out to every
    // discovered external package at once.
    slug_util::threads::available_parallelism().max(1)
}

fn record_max_active(max: &AtomicUsize, active: usize) {
    let mut current = max.load(Ordering::Relaxed);
    while active > current {
        match max.compare_exchange_weak(current, active, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

struct TargetGraphCalculationInstance;

pub(crate) fn init_target_graph_calculation_impl() {
    TARGET_GRAPH_CALCULATION_IMPL.init(&TargetGraphCalculationInstance);
}

#[async_trait]
impl Key for InterpreterResultsKey {
    type Value = slug_error::Result<Arc<EvaluationResult>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        let memory_checkpoints = slug_util::memory_checkpoint::enabled();
        let queued = INTERPRETER_RESULTS_QUEUED.fetch_add(1, Ordering::Relaxed) + 1;
        record_max_active(&INTERPRETER_RESULTS_MAX_QUEUED, queued);
        let wait_started = Instant::now();
        let _permit = PACKAGE_EVALUATION_SEMAPHORE.acquire().await.unwrap();
        let wait_us = wait_started.elapsed().as_micros().min(usize::MAX as u128) as usize;
        let queued = INTERPRETER_RESULTS_QUEUED.fetch_sub(1, Ordering::Relaxed) - 1;
        let active = INTERPRETER_RESULTS_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
        record_max_active(&INTERPRETER_RESULTS_MAX_ACTIVE, active);
        let ((time_span, result), spans) = async_record_root_spans(
            ctx.get_interpreter_results_uncached(self.0.dupe(), cancellation),
        )
        .await;
        let active = INTERPRETER_RESULTS_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
        let completed = INTERPRETER_RESULTS_COMPLETED.fetch_add(1, Ordering::Relaxed) + 1;

        let dep_packages = result.as_ref().ok().map(|result| {
            result
                .targets()
                .values()
                .flat_map(|target| target.deps().map(|target| target.pkg()))
                .filter(|dep_pkg| dep_pkg != &self.0)
                .unique()
                .collect::<Vec<_>>()
        });

        if memory_checkpoints {
            let (targets, imports, target_name_bytes, ok) = match &result {
                Ok(result) => (
                    result.targets().len(),
                    result.imports().len(),
                    result
                        .targets()
                        .keys()
                        .map(|name| name.as_str().len())
                        .sum::<usize>(),
                    1,
                ),
                Err(_) => (0, 0, 0, 0),
            };
            slug_util::memory_checkpoint::checkpoint(
                "interpreter_results_key",
                [
                    ("active", active),
                    ("completed", completed),
                    (
                        "max_active",
                        INTERPRETER_RESULTS_MAX_ACTIVE.load(Ordering::Relaxed),
                    ),
                    ("queued", queued),
                    (
                        "max_queued",
                        INTERPRETER_RESULTS_MAX_QUEUED.load(Ordering::Relaxed),
                    ),
                    ("concurrency_limit", package_evaluation_concurrency_limit()),
                    ("wait_us", wait_us),
                    ("ok", ok),
                    ("targets", targets),
                    ("imports", imports),
                    (
                        "dep_packages",
                        dep_packages.as_ref().map_or(0, |packages| packages.len()),
                    ),
                    ("target_name_bytes", target_name_bytes),
                    (
                        "package_path_len",
                        self.0.as_cell_path().path().as_str().len(),
                    ),
                ],
            );
        }

        ctx.store_evaluation_data(InterpreterResultsKeyActivationData {
            time_span,
            dep_packages,
            spans,
        })?;

        result
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // TODO consider if we want to impl eq for this
        false
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[async_trait]
impl TargetGraphCalculationImpl for TargetGraphCalculationInstance {
    async fn get_interpreter_results_uncached(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
        cancellation: &CancellationContext,
    ) -> (TimeSpan, slug_error::Result<Arc<EvaluationResult>>) {
        match ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await
        {
            Ok(mut interpreter) => {
                interpreter
                    .eval_build_file(package.dupe(), cancellation)
                    .await
            }
            Err(e) => (TimeSpan::empty_now(), Err(e.into())),
        }
    }

    fn get_interpreter_results<'a>(
        &self,
        ctx: &'a mut DiceComputations,
        package: PackageLabel,
    ) -> BoxFuture<'a, slug_error::Result<Arc<EvaluationResult>>> {
        ctx.compute(&InterpreterResultsKey(package.dupe()))
            .map(|v| v?.map_err(slug_error::Error::from))
            .boxed()
    }
}

struct InterpreterCalculationInstance;
struct PackageValuesCalculationInstance;

pub(crate) fn init_interpreter_calculation_impl() {
    INTERPRETER_CALCULATION_IMPL.init(&InterpreterCalculationInstance);
    PACKAGE_VALUES_CALCULATION.init(&PackageValuesCalculationInstance);
}

#[async_trait]
impl Key for EvalImportKey {
    type Value = slug_error::Result<LoadedModule>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        let memory_checkpoints = slug_util::memory_checkpoint::enabled();
        let active = EVAL_IMPORT_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
        record_max_active(&EVAL_IMPORT_MAX_ACTIVE, active);
        let starlark_path = self.0.borrow();
        // We cannot just use the inner default delegate's eval_import
        // because that wouldn't delegate back to us for inner eval_import calls.
        let result = async {
            ctx.get_interpreter_calculator(OwnedStarlarkPath::new(starlark_path.starlark_path()))
                .await?
                .eval_module_uncached(starlark_path, cancellation)
                .await
        }
        .await;
        let active = EVAL_IMPORT_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
        let completed = EVAL_IMPORT_COMPLETED.fetch_add(1, Ordering::Relaxed) + 1;
        if memory_checkpoints {
            let (ok, direct_imports, import_path_bytes) = match &result {
                Ok(module) => (1, module.import_count(), module.import_path_bytes()),
                Err(_) => (0, 0, 0),
            };
            let cross_cell = match self.0.borrow() {
                StarlarkModulePath::LoadFile(path)
                | StarlarkModulePath::JsonFile(path)
                | StarlarkModulePath::TomlFile(path) => {
                    usize::from(path.build_file_cell().name() != path.path().cell())
                }
                StarlarkModulePath::BxlFile(_) => 0,
            };
            if completed <= 10 || completed % 1000 == 0 || direct_imports >= 50 || ok == 0 {
                slug_util::memory_checkpoint::checkpoint(
                    "eval_import_key",
                    [
                        ("active", active),
                        ("completed", completed),
                        ("max_active", EVAL_IMPORT_MAX_ACTIVE.load(Ordering::Relaxed)),
                        ("ok", ok),
                        ("direct_imports", direct_imports),
                        ("import_path_bytes", import_path_bytes),
                        ("module_path_len", self.0.path().path().as_str().len()),
                        ("cross_cell", cross_cell),
                    ],
                );
            }
        }
        Ok(result?)
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // While it is technically possible to compare the modules
        // at least for simple modules (like modules defining only string constants),
        // practically it is too hard to make it work correctly for every case.
        false
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[async_trait]
impl InterpreterCalculationImpl for InterpreterCalculationInstance {
    async fn get_loaded_module(
        &self,
        ctx: &mut DiceComputations<'_>,
        starlark_path: StarlarkModulePath<'_>,
    ) -> slug_error::Result<LoadedModule> {
        let key = if ctx.is_bzlmod().await? {
            canonicalize_bzlmod_module_path(starlark_path)?
        } else {
            OwnedStarlarkModulePath::new(starlark_path)
        };
        ctx.compute(&EvalImportKey(key)).await?
    }

    async fn get_module_deps(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> slug_error::Result<ModuleDeps> {
        let build_file_name = DicePackageListingResolver(ctx)
            .resolve_package_listing(package.dupe())
            .await?
            .buildfile()
            .to_owned();

        let mut calc = ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await?;

        let (_module, module_deps) = calc
            .prepare_eval(StarlarkPath::BuildFile(&BuildFilePath::new(
                package.dupe(),
                build_file_name,
            )))
            .await?;

        Ok(module_deps)
    }

    async fn get_package_file_deps(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> slug_error::Result<Option<(PackageFilePath, Vec<ImportPath>)>> {
        // These aren't cached on the DICE graph, since in normal evaluation there aren't that many, and we can cache at a higher level.
        // Therefore we re-parse the file, if it exists.
        // Fortunately, there are only a small number (currently a few hundred)
        let mut interpreter = ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await?;
        let x = interpreter.prepare_package_file_eval(package).await?;
        let Some((package_file_path, _module, deps)) = x else {
            return Ok(None);
        };
        Ok(Some((
            package_file_path,
            deps.get_loaded_modules().imports().cloned().collect(),
        )))
    }

    async fn global_env(&self, ctx: &mut DiceComputations<'_>) -> slug_error::Result<Globals> {
        Ok(ctx.get_global_interpreter_state().await?.globals().dupe())
    }
}

#[async_trait]
impl PackageValuesCalculation for PackageValuesCalculationInstance {
    async fn package_values(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> slug_error::Result<SmallMap<MetadataKey, serde_json::Value>> {
        ctx.eval_package_file(package)
            .await?
            .package_values()
            .package_values_json()
    }
}

pub struct InterpreterResultsKeyActivationData {
    /// TimeSpan of just the starlark evaluation of the build file.
    pub time_span: TimeSpan,
    pub dep_packages: Option<Vec<PackageLabel>>,
    pub spans: SmallVec<[SpanId; 1]>,
}
