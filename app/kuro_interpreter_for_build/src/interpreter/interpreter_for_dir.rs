/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implements the core skylark interpreter. This encodes the primitive
//! operations of converting file content to ASTs and evaluating import and
//! build files.

use std::cell::OnceCell;
use std::cell::RefCell;
use std::sync::Arc;

use allocative::Allocative;
use dice::CancellationContext;
use dupe::Dupe;
use gazebo::prelude::*;
use kuro_common::legacy_configs::configs::LegacyBuckConfig;
use kuro_common::legacy_configs::key::BuckconfigKeyRef;
use kuro_common::package_listing::listing::PackageListing;
use kuro_core::build_file_path::BuildFilePath;
use kuro_core::bxl::BxlFilePath;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_event_observer::humanized::HumanizedBytes;
use kuro_events::dispatch::get_dispatcher;
use kuro_interpreter::factory::BuckStarlarkModule;
use kuro_interpreter::factory::FinishedStarlarkEvaluation;
use kuro_interpreter::factory::StarlarkEvaluatorProvider;
use kuro_interpreter::file_loader::InterpreterFileLoader;
use kuro_interpreter::file_loader::LoadResolver;
use kuro_interpreter::file_loader::LoadedModules;
use kuro_interpreter::file_type::StarlarkFileType;
use kuro_interpreter::import_paths::ImplicitImportPaths;
use kuro_interpreter::package_imports::ImplicitImport;
use kuro_interpreter::parse_import::RelativeImports;
use kuro_interpreter::parse_import::parse_import;
use kuro_interpreter::paths::module::OwnedStarlarkModulePath;
use kuro_interpreter::paths::module::StarlarkModulePath;
use kuro_interpreter::paths::package::PackageFilePath;
use kuro_interpreter::paths::path::OwnedStarlarkPath;
use kuro_interpreter::paths::path::StarlarkPath;
use kuro_interpreter::prelude_path::PreludePath;
use kuro_interpreter::print_handler::EventDispatcherPrintHandler;
use kuro_interpreter::soft_error::KuroStarlarkSoftErrorHandler;
use kuro_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use kuro_node::nodes::eval_result::EvaluationResult;
use kuro_node::nodes::eval_result::EvaluationResultWithStats;
use kuro_node::super_package::SuperPackage;
use kuro_util::per_thread_instruction_counter::PerThreadInstructionCounter;
use starlark::codemap::FileSpan;
use starlark::environment::FrozenModule;
use starlark::syntax::AstModule;
use starlark::values::OwnedFrozenRef;
use starlark::values::any_complex::StarlarkAnyComplex;
use starlark::values::dict::DictRef;

use crate::interpreter::buckconfig::BuckConfigsViewForStarlark;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;
use crate::interpreter::bzl_eval_ctx::BzlEvalCtx;
use crate::interpreter::cell_info::InterpreterCellInfo;
use crate::interpreter::extra_value::InterpreterExtraValue;
use crate::interpreter::global_interpreter_state::GlobalInterpreterState;
use crate::interpreter::module_internals::ModuleInternals;
use crate::interpreter::package_file_extra::FrozenPackageFileExtra;
use crate::super_package::eval_ctx::PackageFileEvalCtx;

const DEFAULT_STARLARK_MEMORY_USAGE_LIMIT: u64 = 2 * (1 << 30);

#[derive(Debug, kuro_error::Error)]
#[error("Tabs are not allowed in Buck files: `{0}`")]
#[kuro(input)]
struct StarlarkTabsError(OwnedStarlarkPath);

#[derive(Debug, kuro_error::Error)]
enum StarlarkPeakMemoryError {
    #[error(
        "Starlark peak memory usage for {0} is {1} which exceeds the limit {2}! Please reduce memory usage to prevent OOMs. See {3} for debugging tips."
    )]
    #[kuro(input)]
    ExceedsThreshold(BuildFilePath, HumanizedBytes, HumanizedBytes, String),
}

/// A ParseData includes the parsed AST and a list of the imported files.
///
/// The imports are under a separate Arc so that that can be shared with
/// the evaluation result (which needs the imports but no longer needs the AST).
pub struct ParseData(
    pub AstModule,
    pub Arc<Vec<(Option<FileSpan>, OwnedStarlarkModulePath)>>,
);

pub type ParseResult = Result<ParseData, kuro_error::Error>;

impl ParseData {
    fn new(
        ast: AstModule,
        implicit_imports: Vec<OwnedStarlarkModulePath>,
        resolver: &dyn LoadResolver,
    ) -> kuro_error::Result<Self> {
        let mut loads = implicit_imports.into_map(|x| (None, x));
        for x in ast.loads() {
            let path = resolver
                .resolve_load(x.module_id, Some(&x.span))
                .with_buck_error_context(|| {
                    format!(
                        "Error loading `load` of `{}` from `{}`",
                        x.module_id, x.span
                    )
                })?;
            loads.push((Some(x.span), path));
        }
        Ok(Self(ast, Arc::new(loads)))
    }

    pub fn ast(&self) -> &AstModule {
        &self.0
    }

    pub fn imports(&self) -> &Arc<Vec<(Option<FileSpan>, OwnedStarlarkModulePath)>> {
        &self.1
    }
}

pub fn get_starlark_warning_link() -> &'static str {
    if kuro_core::is_open_source() {
        "https://kuro.build/docs/users/faq/starlark_peak_mem"
    } else {
        "https://fburl.com/starlark_peak_mem_warning"
    }
}
/// Interpreter for build files.
///
/// The Interpreter is responsible for parsing files to an AST and then
/// evaluating that AST. The Interpreter doesn't maintain state or cache results
/// of parsing or loading imports.
#[derive(Allocative)]
pub(crate) struct InterpreterForDir {
    /// Non-cell-specific information.
    global_state: Arc<GlobalInterpreterState>,
    /// Cell-specific alias resolver.
    cell_info: InterpreterCellInfo,
    /// Log GC.
    verbose_gc: bool,
    /// When true, rule function creates a node with no attributes.
    /// (Which won't work correctly, but useful for profiling of starlark).
    ignore_attrs_for_profiling: bool,
    /// Implicit imports. These are only used for build files (e.g. `BUCK`),
    /// not for `bzl` or other files, because we only have implicit imports for build files.
    implicit_import_paths: Arc<ImplicitImportPaths>,
    /// Enable relative imports for the current dir
    current_dir_with_allowed_relative_dirs: Arc<CellPathWithAllowedRelativeDir>,
    /// Optional package directory for Bazel-compatible `:subdir/file.bzl` resolution.
    /// In Bazel, `:target` resolves relative to the nearest enclosing BUILD file's package,
    /// not the .bzl file's directory. This field stores that enclosing package directory.
    package_dir: Option<CellPath>,
    /// Autoload path for @rules_cc//cc:defs.bzl in Bazel mode (no prelude).
    /// When set, cc_library/cc_binary/cc_test from rules_cc will be automatically
    /// imported into BUILD file environments, overriding native stubs.
    rules_cc_autoload: Option<OwnedStarlarkModulePath>,
    /// Plan 28: autoload path for `@kuro_builtins//:exports.bzl`. Public
    /// symbols from this bundled module are imported into every BUILD
    /// and `.bzl` env regardless of prelude or workspace configuration.
    bazel_builtins_autoload: Option<OwnedStarlarkModulePath>,
}

struct InterpreterLoadResolver {
    config: Arc<InterpreterForDir>,
    loader_file_type: StarlarkFileType,
    build_file_cell: BuildFileCell,
}

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum LoadResolutionError {
    #[error(
        "Cannot load `{0}`. Bxl loads are not allowed from within this context. bxl files can only be loaded from other bxl files."
    )]
    BxlLoadNotAllowed(CellPath),
    #[error("The `load` at {location} of `{got}` should use the canonical name `{wanted}`")]
    WrongCell {
        got: CellPath,
        wanted: CellPath,
        location: String,
    },
}

impl LoadResolver for InterpreterLoadResolver {
    fn resolve_load(
        &self,
        path: &str,
        location: Option<&FileSpan>,
    ) -> kuro_error::Result<OwnedStarlarkModulePath> {
        let relative_import_option = RelativeImports::Allow {
            current_dir_with_allowed_relative: &self.config.current_dir_with_allowed_relative_dirs,
            package_dir: self.config.package_dir.as_ref(),
        };
        let path = parse_import(
            &self.config.cell_info.cell_alias_resolver(),
            relative_import_option,
            path,
        )?;

        // check for bxl files first before checking for prelude.
        // All bxl imports are parsed the same regardless of prelude or not.
        if path.path().extension() == Some("bxl") {
            match self.loader_file_type {
                StarlarkFileType::Bzl
                | StarlarkFileType::Buck
                | StarlarkFileType::Package
                | StarlarkFileType::Json
                | StarlarkFileType::Toml => {
                    return Err(LoadResolutionError::BxlLoadNotAllowed(path).into());
                }
                StarlarkFileType::Bxl => {
                    return Ok(OwnedStarlarkModulePath::BxlFile(BxlFilePath::new(path)?));
                }
            }
        }

        // If you load the same .bzl file twice via different aliases (e.g. fbcode//kuro/prelude/foo.bzl and prelude.bzl)
        // then anything doing pointer equality (t-sets, provider identities) will go wrong.
        let project_path = self
            .config
            .global_state
            .cell_resolver
            .resolve_path(path.as_ref())?;
        let reformed_path = self
            .config
            .global_state
            .cell_resolver
            .get_cell_path(&project_path);
        if reformed_path.cell() != path.cell() {
            // We actually call resolve_load twice for each loadable - once with all load's up front,
            // then again on each one when we are loading. The second time we don't have a location,
            // so just omit the soft_error that time. Once it is a real error, we should real error on either.
            if let Some(location) = location {
                return Err(LoadResolutionError::WrongCell {
                    got: path,
                    wanted: reformed_path,
                    location: location.to_string(),
                }
                .into());
            }
        }

        // If importing from the prelude, then do not let that inherit the configuration. This
        // ensures that if you define a UDR outside of the prelude's cell, it gets the same prelude
        // as using the exported rules from the prelude would. This matters notably for identity
        // checks in t-sets, which would fail if we had > 1 copy of the prelude.
        if let Some(prelude_import) = self.config.global_state.configuror.prelude_import() {
            if prelude_import.is_prelude_path(&path) {
                if path.path().extension() == Some("json") {
                    return Ok(OwnedStarlarkModulePath::JsonFile(
                        ImportPath::new_same_cell(path)?,
                    ));
                } else {
                    return Ok(OwnedStarlarkModulePath::LoadFile(
                        ImportPath::new_same_cell(path)?,
                    ));
                }
            }
        }
        let import_path = ImportPath::new_with_build_file_cells(path, self.build_file_cell)?;
        Ok(match import_path.path().path().extension() {
            Some("json") => OwnedStarlarkModulePath::JsonFile(import_path),
            Some("toml") => OwnedStarlarkModulePath::TomlFile(import_path),
            _ => OwnedStarlarkModulePath::LoadFile(import_path),
        })
    }
}

struct EvalResult {
    additional: PerFileTypeContext,
    starlark_peak_allocated_byte_limit: OnceCell<Option<u64>>,
    is_profiling_enabled: bool,
    cpu_instruction_count: Option<u64>,
}

impl InterpreterForDir {
    fn verbose_gc() -> kuro_error::Result<bool> {
        match std::env::var_os("BUCK2_STARLARK_VERBOSE_GC") {
            Some(val) => Ok(!val.is_empty()),
            None => Ok(false),
        }
    }

    fn is_ignore_attrs_for_profiling() -> kuro_error::Result<bool> {
        // If unsure, feel free to break this code or just delete it.
        // It is intended only for profiling of very specific use cases.
        let ignore_attrs_for_profiling = match std::env::var_os("BUCK2_IGNORE_ATTRS_FOR_PROFILING")
        {
            Some(val) => !val.is_empty(),
            None => false,
        };
        if ignore_attrs_for_profiling {
            // This messages is printed in each run once per cell.
            // Somewhat inconvenient, but it is safe.
            eprintln!("Ignoring rule attributes");
        }
        Ok(ignore_attrs_for_profiling)
    }

    //, configuror: Arc<dyn InterpreterConfigurer>
    pub(crate) fn new(
        cell_info: InterpreterCellInfo,
        global_state: Arc<GlobalInterpreterState>,
        implicit_import_paths: Arc<ImplicitImportPaths>,
        current_dir_with_allowed_relative_dirs: Arc<CellPathWithAllowedRelativeDir>,
        package_dir: Option<CellPath>,
    ) -> kuro_error::Result<Self> {
        // In Bazel mode (no prelude), auto-load rules_cc for BUILD files so that
        // native cc_library/cc_binary/cc_test calls use rules_cc's Starlark
        // implementations instead of empty native stubs.
        let rules_cc_autoload = if global_state.configuror.prelude_import().is_none() {
            cell_info
                .cell_alias_resolver()
                .resolve("rules_cc")
                .ok()
                .and_then(|_| {
                    parse_import(
                        cell_info.cell_alias_resolver(),
                        RelativeImports::Disallow,
                        "@rules_cc//cc:defs.bzl",
                    )
                    .ok()
                    .and_then(|cell_path| {
                        ImportPath::new_with_build_file_cells(cell_path, cell_info.name())
                            .ok()
                            .map(OwnedStarlarkModulePath::LoadFile)
                    })
                })
        } else {
            None
        };

        // Plan 28: resolve the bundled `@kuro_builtins//:exports.bzl` once
        // per InterpreterForDir. The cell is auto-registered by
        // `kuro_common::legacy_configs::cells` for every bzlmod project,
        // so the alias resolution below succeeds in every Bazel-mode
        // workspace. For legacy non-bzlmod projects without
        // `[external_cells] kuro_builtins = bundled` in `.buckconfig`,
        // resolution returns None and the autoload is a no-op.
        let bazel_builtins_autoload = cell_info
            .cell_alias_resolver()
            .resolve("kuro_builtins")
            .ok()
            .and_then(|_| {
                parse_import(
                    cell_info.cell_alias_resolver(),
                    RelativeImports::Disallow,
                    "@kuro_builtins//:exports.bzl",
                )
                .ok()
                .and_then(|cell_path| {
                    ImportPath::new_with_build_file_cells(cell_path, cell_info.name())
                        .ok()
                        .map(OwnedStarlarkModulePath::LoadFile)
                })
            });

        Ok(Self {
            global_state,
            cell_info,
            verbose_gc: Self::verbose_gc()?,
            ignore_attrs_for_profiling: Self::is_ignore_attrs_for_profiling()?,
            implicit_import_paths,
            current_dir_with_allowed_relative_dirs,
            package_dir,
            rules_cc_autoload,
            bazel_builtins_autoload,
        })
    }

    fn create_env(
        &self,
        env: BuckStarlarkModule,
        starlark_path: StarlarkPath<'_>,
        loaded_modules: &LoadedModules,
    ) -> kuro_error::Result<BuckStarlarkModule> {
        if let Some(prelude_import) = self.prelude_import(starlark_path) {
            let prelude_env = loaded_modules
                .map
                .get(&StarlarkModulePath::LoadFile(prelude_import.import_path()))
                .with_internal_error(|| {
                    format!("Should've had an env for the prelude import `{prelude_import}`",)
                })?;
            env.import_public_symbols(prelude_env.env());
            if let StarlarkPath::BuildFile(_) = starlark_path {
                for (name, value) in prelude_env.extra_globals_from_prelude_for_buck_files()? {
                    env.set(name, value.to_value());
                }
            }
        }

        // Plan 28: inject names from `@kuro_builtins//:exports.bzl`'s
        // `exported_toplevels` dict into the consuming env.
        // Skipped inside the kuro_builtins cell itself (an exports module
        // can't import from itself). The matching `parse()` arm pushed
        // this path into `implicit_imports`, so DICE has already loaded
        // the module by the time we get here.
        //
        // Phase 28.3 reads from the explicit `exported_toplevels` dict
        // rather than `import_public_symbols`. Visibility-control lives
        // in the bundled `exports.bzl`, not in the interpreter — adding
        // a top-level name is an explicit per-symbol decision.
        if let Some(OwnedStarlarkModulePath::LoadFile(builtins_path)) =
            &self.bazel_builtins_autoload
        {
            let import_cell = starlark_path.path().cell();
            if import_cell != builtins_path.path().cell() {
                if let Some(builtins_env) = loaded_modules
                    .map
                    .get(&StarlarkModulePath::LoadFile(builtins_path))
                {
                    let frozen = builtins_env.env();
                    let exports_value = frozen
                        .get_option("exported_toplevels")
                        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))?;
                    if let Some(exports_value) = exports_value {
                        let value = exports_value.owned_value(env.frozen_heap());
                        let dict = DictRef::from_value(value).with_internal_error(|| {
                            format!(
                                "@kuro_builtins exports.bzl `exported_toplevels` must be a \
                                 dict, got: {}",
                                value.get_type()
                            )
                        })?;
                        for (k, v) in dict.iter() {
                            let name = k.unpack_str().with_internal_error(|| {
                                format!(
                                    "@kuro_builtins exports.bzl `exported_toplevels` keys must \
                                     be strings, got: {}",
                                    k.get_type()
                                )
                            })?;
                            env.set(name, v);
                        }
                    }
                }
            }
        }

        env.set_extra_value_no_overwrite(env.heap().alloc_complex(StarlarkAnyComplex {
            value: InterpreterExtraValue::default(),
        }))
        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Interpreter))?;

        Ok(env)
    }

    // The environment for evaluating a build file contains additional information
    // to support the extra things available in that context. For example, rule
    // functions can be invoked when evaluating a build file, the package (cell
    // + path) is available. It also includes the implicit root include and
    // implicit package include.
    fn create_build_env(
        &self,
        env: BuckStarlarkModule,
        build_file: &BuildFilePath,
        package_listing: &PackageListing,
        super_package: SuperPackage,
        package_boundary_exception: bool,
        loaded_modules: &LoadedModules,
    ) -> kuro_error::Result<(BuckStarlarkModule, ModuleInternals)> {
        let internals = self.global_state.configuror.new_extra_context(
            &self.cell_info,
            build_file.clone(),
            package_listing.dupe(),
            super_package,
            package_boundary_exception,
            loaded_modules,
            self.package_import(build_file),
            self.current_dir_with_allowed_relative_dirs
                .as_ref()
                .to_owned(),
        )?;
        let env = self.create_env(env, StarlarkPath::BuildFile(build_file), loaded_modules)?;

        if let Some(root_import) = self.root_import() {
            let root_env = loaded_modules
                .map
                .get(&StarlarkModulePath::LoadFile(&root_import))
                .with_internal_error(|| {
                    format!("Should've had an env for the root import `{root_import}`",)
                })?
                .env();
            env.import_public_symbols(root_env);
        }

        // Autoload rules_cc symbols in Bazel mode (no prelude).
        // This makes cc_library/cc_binary/cc_test from rules_cc override native stubs,
        // matching Bazel's autoloading behavior for BUILD files.
        if let Some(OwnedStarlarkModulePath::LoadFile(ref import_path)) = self.rules_cc_autoload {
            if let Some(rules_cc_module) = loaded_modules
                .map
                .get(&StarlarkModulePath::LoadFile(import_path))
            {
                env.import_public_symbols(rules_cc_module.env());
            }
        }

        Ok((env, internals))
    }

    fn load_resolver(
        self: &Arc<Self>,
        current_file_path: StarlarkPath<'_>,
    ) -> InterpreterLoadResolver {
        InterpreterLoadResolver {
            config: self.dupe(),
            loader_file_type: current_file_path.file_type(),
            build_file_cell: current_file_path.build_file_cell(),
        }
    }

    fn package_import(&self, build_file_import: &BuildFilePath) -> Option<&Arc<ImplicitImport>> {
        self.implicit_import_paths
            .package_imports
            .get(build_file_import.package())
    }

    fn root_import(&self) -> Option<ImportPath> {
        self.implicit_import_paths.root_import.clone()
    }

    fn prelude_import(&self, import: StarlarkPath) -> Option<&PreludePath> {
        let prelude_import = self.global_state.configuror.prelude_import();
        if let Some(prelude_import) = prelude_import {
            let import_path = import.path();

            match import {
                StarlarkPath::BuildFile(_)
                | StarlarkPath::PackageFile(_)
                | StarlarkPath::BxlFile(_) => return Some(prelude_import),
                StarlarkPath::LoadFile(_) => {
                    if !prelude_import.is_prelude_path(&import_path) {
                        return Some(prelude_import);
                    }
                }
                StarlarkPath::JsonFile(_) | StarlarkPath::TomlFile(_) => return None,
            }
        }

        None
    }

    /// Parses skylark code to an AST.
    pub(crate) fn parse(
        self: &Arc<Self>,
        import: StarlarkPath,
        content: String,
    ) -> kuro_error::Result<ParseResult> {
        // Buck2 prohibits tabs in .bzl files, but Bazel allows them.
        // Log a warning instead of erroring for Bazel compatibility.
        if content.contains('\t') {
            tracing::warn!(
                "Tabs found in Starlark file: `{}` (allowed for Bazel compat)",
                OwnedStarlarkPath::new(import)
            );
        }

        let project_relative_path = self
            .global_state
            .cell_resolver
            .resolve_path(import.path().as_ref().as_ref())?;

        let disable_starlark_types = self.global_state.disable_starlark_types;
        let ast = match AstModule::parse(
            project_relative_path.as_str(),
            content,
            &import.file_type().dialect(disable_starlark_types),
        ) {
            Ok(ast) => ast,
            Err(e) => {
                return Ok(Err(kuro_error::Error::from(e).context(format!(
                    "Error parsing: `{}`",
                    OwnedStarlarkPath::new(import)
                ))));
            }
        };
        let mut implicit_imports = Vec::new();
        if let Some(i) = self.prelude_import(import) {
            implicit_imports.push(OwnedStarlarkModulePath::LoadFile(i.import_path().clone()));
        }
        // Plan 28: autoload @kuro_builtins for both BUILD and .bzl paths,
        // EXCEPT inside the kuro_builtins cell itself (a bundled .bzl
        // can't load itself). Fires for every cell and every file kind.
        if let Some(OwnedStarlarkModulePath::LoadFile(ref builtins_path)) =
            self.bazel_builtins_autoload
        {
            let import_cell = import.path().cell();
            if import_cell != builtins_path.path().cell() {
                implicit_imports.push(OwnedStarlarkModulePath::LoadFile(builtins_path.clone()));
            }
        }
        if let StarlarkPath::BuildFile(build_file) = import {
            if let Some(i) = self.package_import(build_file) {
                implicit_imports.push(OwnedStarlarkModulePath::LoadFile(i.import().clone()));
            }
            if let Some(i) = self.root_import() {
                implicit_imports.push(OwnedStarlarkModulePath::LoadFile(i));
            }
            // Autoload rules_cc in Bazel mode (no prelude)
            if let Some(ref rules_cc_path) = self.rules_cc_autoload {
                implicit_imports.push(rules_cc_path.clone());
            }
        }
        ParseData::new(ast, implicit_imports, &self.load_resolver(import)).map(Ok)
    }

    pub(crate) fn resolve_path(
        self: &Arc<Self>,
        import: StarlarkPath<'_>,
        import_string: &str,
    ) -> kuro_error::Result<OwnedStarlarkModulePath> {
        self.load_resolver(import).resolve_load(import_string, None)
    }

    fn eval(
        self: &Arc<Self>,
        env: &BuckStarlarkModule,
        ast: AstModule,
        buckconfigs: &mut dyn BuckConfigsViewForStarlark,
        loaded_modules: LoadedModules,
        extra_context: PerFileTypeContext,
        eval_provider: StarlarkEvaluatorProvider,
        unstable_typecheck: bool,
        cancellation: &CancellationContext,
    ) -> kuro_error::Result<(FinishedStarlarkEvaluation, EvalResult)> {
        let import = extra_context.starlark_path();
        let globals = self.global_state.globals();
        let file_loader =
            InterpreterFileLoader::new(loaded_modules, Arc::new(self.load_resolver(import)));
        let host_info = self.global_state.configuror.host_info();
        let extra = BuildContext::new(
            &self.cell_info,
            buckconfigs,
            host_info,
            extra_context,
            self.ignore_attrs_for_profiling,
        );

        let print = EventDispatcherPrintHandler(get_dispatcher());
        let (finished_eval, (cpu_instruction_count, is_profiling_enabled)) = eval_provider
            .with_evaluator(
                env,
                cancellation.into(),
                |eval, is_profiling_enabled_by_provider| {
                    eval.enable_static_typechecking(unstable_typecheck);
                    eval.set_print_handler(&print);
                    eval.set_soft_error_handler(&KuroStarlarkSoftErrorHandler);
                    eval.set_loader(&file_loader);
                    eval.extra = Some(&extra);
                    if self.verbose_gc {
                        eval.verbose_gc();
                    }

                    // Ignore error if failed to initialize instruction counter.
                    let instruction_counter: Option<PerThreadInstructionCounter> =
                        PerThreadInstructionCounter::init().ok().unwrap_or_default();

                    match eval.eval_module(ast, globals) {
                        Ok(_) => {
                            let cpu_instruction_count =
                                instruction_counter.and_then(|c| c.collect().ok());
                            Ok((cpu_instruction_count, is_profiling_enabled_by_provider))
                        }
                        Err(p) => Err(p.into()),
                    }
                },
            )?;
        Ok((
            finished_eval,
            EvalResult {
                additional: extra.additional,
                is_profiling_enabled,
                starlark_peak_allocated_byte_limit: extra.starlark_peak_allocated_byte_limit,
                cpu_instruction_count,
            },
        ))
    }

    /// Evaluates the AST for a parsed module. Loaded modules must contain the loaded
    /// environment for all (transitive) required imports.
    /// Returns the FrozenModule for the module.
    pub(crate) fn eval_module(
        self: &Arc<Self>,
        starlark_path: StarlarkModulePath<'_>,
        buckconfigs: &mut dyn BuckConfigsViewForStarlark,
        ast: AstModule,
        loaded_modules: LoadedModules,
        eval_provider: StarlarkEvaluatorProvider,
        cancellation: &CancellationContext,
    ) -> kuro_error::Result<FrozenModule> {
        BuckStarlarkModule::with_profiling(|env| {
            let env = self.create_env(env, starlark_path.into(), &loaded_modules)?;
            let extra_context = match starlark_path {
                StarlarkModulePath::LoadFile(bzl) => PerFileTypeContext::Bzl(BzlEvalCtx {
                    bzl_path: bzl.clone(),
                }),
                StarlarkModulePath::BxlFile(bxl) => PerFileTypeContext::Bxl(bxl.clone()),
                StarlarkModulePath::JsonFile(j) => PerFileTypeContext::Json(j.clone()),
                StarlarkModulePath::TomlFile(t) => PerFileTypeContext::Toml(t.clone()),
            };
            let typecheck = self.global_state.unstable_typecheck
                || matches!(starlark_path, StarlarkModulePath::BxlFile(..))
                || match self.global_state.configuror.prelude_import() {
                    Some(prelude_import) => {
                        prelude_import.prelude_cell()
                            == self.cell_info.cell_alias_resolver().resolve_self()
                    }
                    None => false,
                };
            let (finished_eval, _) = self.eval(
                &env,
                ast,
                buckconfigs,
                loaded_modules,
                extra_context,
                eval_provider,
                typecheck,
                cancellation,
            )?;
            let (token, frozen, _) = finished_eval.freeze_and_finish(env)?;

            Ok((token, frozen))
        })
    }

    pub(crate) fn eval_package_file(
        self: &Arc<Self>,
        package_file_path: &PackageFilePath,
        ast: AstModule,
        parent: SuperPackage,
        buckconfigs: &mut dyn BuckConfigsViewForStarlark,
        loaded_modules: LoadedModules,
        eval_provider: StarlarkEvaluatorProvider,
        cancellation: &CancellationContext,
    ) -> kuro_error::Result<SuperPackage> {
        BuckStarlarkModule::with_profiling(|env| {
            let env = self.create_env(
                env,
                StarlarkPath::PackageFile(package_file_path),
                &loaded_modules,
            )?;

            let extra_context = PerFileTypeContext::Package(PackageFileEvalCtx {
                path: package_file_path.clone(),
                parent,
                visibility: RefCell::new(None),
                test_config_unification_rollout: RefCell::new(None),
            });

            let (finished_eval, eval_result) = self.eval(
                &env,
                ast,
                buckconfigs,
                loaded_modules,
                extra_context,
                eval_provider,
                false,
                cancellation,
            )?;

            let per_file_context = eval_result.additional;

            let (token, extra): (_, Option<OwnedFrozenRef<FrozenPackageFileExtra>>) =
                if InterpreterExtraValue::get(&env)?
                    .package_extra
                    .get()
                    .is_some()
                {
                    // Only freeze if there's something to freeze, otherwise we will needlessly freeze
                    // globals. TODO(nga): add API to only freeze extra.
                    let (token, frozen, _) = finished_eval.freeze_and_finish(env)?;
                    (token, FrozenPackageFileExtra::get(&frozen)?)
                } else {
                    let (token, _) = finished_eval.finish(None)?;
                    (token, None)
                };

            let package_file_eval_ctx = per_file_context.into_package_file()?;

            Ok((token, package_file_eval_ctx.build_super_package(extra)?))
        })
    }

    /// Evaluates the AST for a parsed build file. Loaded modules must contain the
    /// loaded environment for all (transitive) required imports.
    /// Returns the result of evaluation.
    pub(crate) fn eval_build_file(
        self: &Arc<Self>,
        build_file: &BuildFilePath,
        buckconfigs: &mut dyn BuckConfigsViewForStarlark,
        listing: PackageListing,
        super_package: SuperPackage,
        package_boundary_exception: bool,
        ast: AstModule,
        loaded_modules: LoadedModules,
        eval_provider: StarlarkEvaluatorProvider,
        unstable_typecheck: bool,
        cancellation: &CancellationContext,
    ) -> kuro_error::Result<(
        Option<Arc<StarlarkProfileDataAndStats>>,
        EvaluationResultWithStats,
    )> {
        BuckStarlarkModule::with_profiling(|env| {
            let (env, internals) = self.create_build_env(
                env,
                build_file,
                &listing,
                super_package,
                package_boundary_exception,
                &loaded_modules,
            )?;
            let buckconfig_key = BuckconfigKeyRef {
                section: "kuro",
                property: "check_starlark_peak_memory",
            };
            let starlark_peak_mem_config_enabled = LegacyBuckConfig::parse_value(
                buckconfig_key,
                buckconfigs
                    .read_root_cell_config(buckconfig_key)?
                    .as_deref(),
            )?
            .unwrap_or(false);

            let (finished_eval, eval_result) = self.eval(
                &env,
                ast,
                buckconfigs,
                loaded_modules,
                PerFileTypeContext::Build(internals),
                eval_provider,
                unstable_typecheck,
                cancellation,
            )?;

            let internals = eval_result.additional.into_build()?;
            let starlark_peak_allocated_bytes = env.heap().peak_allocated_bytes() as u64;
            let starlark_peak_mem_check_enabled =
                !eval_result.is_profiling_enabled && starlark_peak_mem_config_enabled;
            let starlark_mem_limit = eval_result
                .starlark_peak_allocated_byte_limit
                .get()
                .and_then(|limit| *limit)
                .unwrap_or(DEFAULT_STARLARK_MEMORY_USAGE_LIMIT);

            if starlark_peak_mem_check_enabled && starlark_peak_allocated_bytes > starlark_mem_limit
            {
                Err(StarlarkPeakMemoryError::ExceedsThreshold(
                    build_file.to_owned(),
                    HumanizedBytes::fixed_width(starlark_peak_allocated_bytes),
                    HumanizedBytes::fixed_width(starlark_mem_limit),
                    get_starlark_warning_link().to_owned(),
                )
                .into())
            } else {
                let (token, profile_data) = finished_eval.finish(None)?;

                Ok((
                    token,
                    (
                        profile_data,
                        EvaluationResultWithStats {
                            result: EvaluationResult::from(internals),
                            starlark_peak_allocated_bytes,
                            cpu_instruction_count: eval_result.cpu_instruction_count,
                        },
                    ),
                ))
            }
        })
    }
}
