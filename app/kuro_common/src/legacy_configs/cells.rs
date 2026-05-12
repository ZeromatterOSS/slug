/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use dice::DiceComputations;
use dupe::Dupe;
use kuro_bzlmod::ModuleCache;
use kuro_bzlmod::ModuleSource;
use kuro_bzlmod::MvsResolver;
use kuro_bzlmod::ResolvedGraph;
use kuro_bzlmod::parse_module_bazel;
use kuro_bzlmod::resolve_local_modules;
use kuro_bzlmod::types::ParsedModuleFile;
use kuro_bzlmod::types::TagValue;
use kuro_core::cells::CellAliasResolver;
use kuro_core::cells::CellResolver;
use kuro_core::cells::alias::NonEmptyCellAlias;
use kuro_core::cells::cell_root_path::CellRootPath;
use kuro_core::cells::cell_root_path::CellRootPathBuf;
use kuro_core::cells::external::BzlmodCellSetup;
use kuro_core::cells::external::ExtensionRepoCellSetup;
use kuro_core::cells::external::ExternalCellOrigin;
use kuro_core::cells::external::GitCellSetup;
use kuro_core::cells::external::GitObjectFormat;
use kuro_core::cells::name::CellName;
use kuro_core::fs::project::ProjectRoot;
use kuro_core::fs::project_rel_path::ProjectRelativePath;
use kuro_error::BuckErrorContext;
use kuro_fs::fs_util;
use kuro_fs::paths::RelativePath;

use crate::dice::cells::HasCellResolver;
use crate::dice::data::HasIoProvider;
use crate::external_cells::EXTERNAL_CELLS_IMPL;
use crate::legacy_configs::aggregator::CellsAggregator;
use crate::legacy_configs::args::ResolvedLegacyConfigArg;
use crate::legacy_configs::args::resolve_config_args;
use crate::legacy_configs::args::to_proto_config_args;
use crate::legacy_configs::cells_symlinks::cleanup_stale_symlinks;
use crate::legacy_configs::cells_symlinks::ensure_symlink;
use crate::legacy_configs::configs::LegacyBuckConfig;
use crate::legacy_configs::dice::HasInjectedLegacyConfigs;
use crate::legacy_configs::key::BuckconfigKeyRef;

/// Bundled toolchain labels auto-injected when `rules_python` is in the
/// module graph but the root module didn't register a py3 toolchain.
///
/// Ordering matters: `host_toolchain` provides the default py3 runtime; the
/// launcher_maker stub satisfies rules_python 1.9+'s mandatory
/// launcher_maker_toolchain_type (only actually invoked on Windows, but
/// resolution must succeed on Linux/macOS too).
///
/// Grep for this constant to find every place that implicitly assumes the
/// bundled `local_config_python` cell is registered.
const BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS: &[&str] = &[
    "@local_config_python//:host_toolchain",
    "@local_config_python//:host_launcher_maker_toolchain",
];

/// The module name used by the canonical rules_python Bazel module. Matched
/// against `ParsedModuleFile::module.name` (the declared `module(name = ...)`
/// value), not against cell names.
const RULES_PYTHON_MODULE_NAME: &str = "rules_python";

/// Sentinel substring used to detect whether a user-registered toolchain
/// label already targets the bundled `@local_config_python` cell. Any label
/// containing this substring means we should not auto-inject duplicates.
const LOCAL_CONFIG_PYTHON_CELL: &str = "local_config_python";

/// True iff `parsed_modules` contains the canonical rules_python module.
fn module_depends_on_rules_python(parsed_modules: &[(String, ParsedModuleFile)]) -> bool {
    parsed_modules
        .iter()
        .any(|(name, _)| name == RULES_PYTHON_MODULE_NAME)
}

/// True iff any toolchain label already references the bundled
/// `@local_config_python` cell (meaning the user has already wired up
/// bundled rules_python toolchains and we should skip auto-injection).
fn toolchains_include_bundled_python(toolchains: &[kuro_bzlmod::RegisteredToolchain]) -> bool {
    toolchains
        .iter()
        .any(|tc| tc.label.contains(LOCAL_CONFIG_PYTHON_CELL))
}

/// Buckconfigs can partially be loaded from within dice. However, some parts of what makes up the
/// buckconfig comes from outside the buildgraph, and this type represents those parts.
///
/// Q1=B: no .buckconfig files are parsed; only CLI -c flag overrides are stored here.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct ExternalBuckconfigData {
    // The result of processing command-line config args (e.g. --config key=value)
    args: Vec<ResolvedLegacyConfigArg>,
}

impl ExternalBuckconfigData {
    pub fn testing_default() -> Self {
        Self { args: Vec::new() }
    }

    pub fn filter_values<F>(self, filter: F) -> Self
    where
        F: Fn(&BuckconfigKeyRef) -> bool,
    {
        Self {
            args: self
                .args
                .into_iter()
                .filter(|arg| match arg {
                    ResolvedLegacyConfigArg::Flag(flag) => {
                        flag.cell.is_some()
                            || filter(&BuckconfigKeyRef {
                                section: &flag.section,
                                property: &flag.key,
                            })
                    }
                    _ => true,
                })
                .collect(),
        }
    }

    /// Serialize CLI config overrides for DICE invalidation tracking.
    ///
    /// Q1=B: .buckconfig file components are no longer collected; only CLI -c args
    /// are serialized. The `project_root` parameter is retained for API compatibility.
    pub async fn get_buckconfig_components(
        &self,
        _project_root: &ProjectRoot,
    ) -> Vec<kuro_data::BuckconfigComponent> {
        to_proto_config_args(&self.args)
    }
}

/// Used for creating a CellResolver in a buckv1-compatible way based on values
/// in .buckconfig in each cell.
///
/// We'll traverse the structure of the `[cells]` sections starting from
/// the root .buckconfig. All aliases found in the root config will also be
/// available in all other cells (v1 provides that same behavior).
///
/// We don't (currently) enforce that all aliases appear in the root config, but
/// unlike v1, our cells implementation works just fine if that isn't the case.
pub struct BuckConfigBasedCells {
    pub cell_resolver: CellResolver,
    pub root_config: LegacyBuckConfig,
    pub external_data: ExternalBuckconfigData,
    /// True when MODULE.bazel is present - all cell resolution is done via bzlmod.
    /// Per-cell .buckconfig [repository_aliases] sections are ignored in this mode.
    pub is_bzlmod: bool,
}

/// Result of bzlmod dependency resolution.
struct BzlmodResolutionResult {
    /// Root module name from MODULE.bazel `module(name = "...")`.
    /// Used as the root cell name (falls back to `_main` if empty).
    root_module_name: String,
    /// Cells to register: (name, path, optional setup for remote modules)
    cells: Vec<(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)>,
    /// Extension-generated cells: (name, path, setup for extension repos)
    /// These are created by module extensions (e.g., pip.parse(), go_deps)
    /// and are populated from lockfile cache during resolution.
    extension_cells: Vec<(CellName, CellRootPathBuf, ExtensionRepoCellSetup)>,
    /// Cell aliases to register: (alias_name, target_cell_name)
    /// These come from repo_name parameters in bazel_dep()
    aliases: Vec<(NonEmptyCellAlias, CellName)>,
}

impl BuckConfigBasedCells {
    /// In the client and one place in the daemon, we need access to the alias resolver for the cwd
    /// in some places where we don't have normal dice access
    ///
    /// This function reads buckconfigs to compute an appropriate cell alias resolver to make that
    /// possible.
    pub async fn get_cell_alias_resolver_for_cwd_fast(
        &self,
        _project_fs: &ProjectRoot,
        cwd: &ProjectRelativePath,
    ) -> kuro_error::Result<CellAliasResolver> {
        let cell_name = self.cell_resolver.find(cwd);

        // Q1=B: no .buckconfig files are read; alias resolution is CLI-flag-only.
        // In bzlmod mode (the only supported mode), all cell aliases come from MODULE.bazel,
        // not from per-cell .buckconfig [repository_aliases] sections.
        let config = LegacyBuckConfig::from_resolved_flags(&self.external_data.args);
        let cell_aliases: Box<dyn Iterator<Item = (NonEmptyCellAlias, NonEmptyCellAlias)>> =
            if self.is_bzlmod {
                Box::new(std::iter::empty())
            } else {
                Box::new(BuckConfigBasedCells::get_cell_aliases_from_config(&config)?)
            };

        CellAliasResolver::new_for_non_root_cell(
            cell_name,
            self.cell_resolver.root_cell_cell_alias_resolver(),
            cell_aliases,
        )
    }

    pub async fn parse_with_config_args(
        project_fs: &ProjectRoot,
        config_args: &[kuro_cli_proto::ConfigOverride],
    ) -> kuro_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(config_args, Some(project_fs))
            .await
            .buck_error_context("Parsing cells")
    }

    /// Testing entry point: equivalent to `parse_with_config_args` with no project root.
    pub async fn testing_parse(
        config_args: &[kuro_cli_proto::ConfigOverride],
    ) -> kuro_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(config_args, None)
            .await
            .buck_error_context("Parsing cells")
    }

    async fn parse_with_file_ops_and_options_inner(
        config_args: &[kuro_cli_proto::ConfigOverride],
        project_fs: Option<&ProjectRoot>,
    ) -> kuro_error::Result<Self> {
        // Q1=B: only CLI -c flag args are processed; no file I/O.
        let processed_config_args = resolve_config_args(config_args).await?;

        let root_path = CellRootPathBuf::new(ProjectRelativePath::empty().to_owned());

        // Q1=B: build root_config purely from CLI -c overrides; no .buckconfig files are read.
        let root_config = LegacyBuckConfig::from_overrides_only(config_args)?;

        let mut cell_definitions = Vec::new();
        let mut bzlmod_external_cells: Vec<(CellName, BzlmodCellSetup)> = Vec::new();
        let mut bzlmod_extension_cells: Vec<(CellName, ExtensionRepoCellSetup)> = Vec::new();
        let mut bzlmod_bundled_cells: Vec<CellName> = Vec::new();
        let mut has_module_bazel = false;

        // ===== Bzlmod Integration =====
        // When MODULE.bazel exists, ALL cell definitions come from bzlmod resolution.
        // The root cell name is derived from module(name = "...") in MODULE.bazel.
        // .buckconfig [cells], [cell_aliases], and [external_cells] sections are skipped.
        let mut bzlmod_aliases: Vec<(NonEmptyCellAlias, CellName)> = Vec::new();
        if let Some(project_fs) = project_fs {
            if let Some(bzlmod_result) = Self::resolve_bzlmod_dependencies(project_fs).await? {
                has_module_bazel = true;

                // Root cell comes from MODULE.bazel module(name = "...")
                let root_cell_name = CellName::unchecked_new(&bzlmod_result.root_module_name)?;
                cell_definitions.push((root_cell_name, root_path.clone()));
                tracing::info!(
                    "Root cell '{}' defined from MODULE.bazel",
                    bzlmod_result.root_module_name
                );

                for (name, path, maybe_setup) in bzlmod_result.cells {
                    if !cell_definitions.iter().any(|(n, _)| *n == name) {
                        cell_definitions.push((name, path));
                        tracing::info!("Added bzlmod cell: {}", name);

                        if let Some(setup) = maybe_setup {
                            bzlmod_external_cells.push((name, setup));
                        }
                    }
                }

                for (name, path, setup) in bzlmod_result.extension_cells {
                    if !cell_definitions.iter().any(|(n, _)| *n == name) {
                        cell_definitions.push((name, path));
                        tracing::info!("Added extension repo cell: {}", name);
                        bzlmod_extension_cells.push((name, setup));
                    }
                }

                bzlmod_aliases = bzlmod_result.aliases;

                // Auto-register @bazel_tools for bzlmod projects
                let bazel_tools_name = CellName::unchecked_new("bazel_tools")?;
                if !cell_definitions.iter().any(|(n, _)| *n == bazel_tools_name) {
                    let bazel_tools_path =
                        CellRootPathBuf::new(ProjectRelativePath::new("bazel_tools")?.to_owned());
                    cell_definitions.push((bazel_tools_name, bazel_tools_path));
                    bzlmod_bundled_cells.push(bazel_tools_name);
                    tracing::info!("Auto-registered bundled cell: bazel_tools");
                }

                // Auto-register @local_config_platform for bzlmod projects
                let lcp_name = CellName::unchecked_new("local_config_platform")?;
                if !cell_definitions.iter().any(|(n, _)| *n == lcp_name) {
                    let lcp_path = CellRootPathBuf::new(
                        ProjectRelativePath::new("local_config_platform")?.to_owned(),
                    );
                    cell_definitions.push((lcp_name, lcp_path));
                    bzlmod_bundled_cells.push(lcp_name);
                    tracing::info!("Auto-registered bundled cell: local_config_platform");
                }

                // Plan 28: auto-register @kuro_builtins for bzlmod projects.
                // The cell ships exports.bzl whose public symbols are
                // injected into every BUILD/.bzl by `bazel_builtins_autoload`.
                let kb_name = CellName::unchecked_new("kuro_builtins")?;
                if !cell_definitions.iter().any(|(n, _)| *n == kb_name) {
                    let kb_path =
                        CellRootPathBuf::new(ProjectRelativePath::new("kuro_builtins")?.to_owned());
                    cell_definitions.push((kb_name, kb_path));
                    bzlmod_bundled_cells.push(kb_name);
                    tracing::info!("Auto-registered bundled cell: kuro_builtins");
                }

                // Auto-register @local_config_python for bzlmod projects that
                // depend on rules_python. The bundled cell provides a host
                // py_runtime + py_runtime_pair + toolchain() target so
                // rules_python's py_library/py_binary analysis finds a
                // py3_runtime when the user's MODULE.bazel hasn't registered
                // its own Python toolchain (common for projects that use
                // small py_binary helpers, e.g. @llvm-project//clang:clang).
                let lcpy_name = CellName::unchecked_new("local_config_python")?;
                if !cell_definitions.iter().any(|(n, _)| *n == lcpy_name) {
                    let lcpy_path = CellRootPathBuf::new(
                        ProjectRelativePath::new("local_config_python")?.to_owned(),
                    );
                    cell_definitions.push((lcpy_name, lcpy_path));
                    bzlmod_bundled_cells.push(lcpy_name);
                    tracing::info!("Auto-registered bundled cell: local_config_python");
                }
            }
        }

        // Legacy .buckconfig cell definitions - only used when MODULE.bazel is NOT present
        if !has_module_bazel {
            let repositories = root_config
                .get_section("cells")
                .or_else(|| root_config.get_section("repositories"));
            if let Some(repositories) = repositories {
                for (alias, alias_path) in repositories.iter() {
                    let alias_path = CellRootPathBuf::new(
                        root_path
                            .as_project_relative_path()
                            .join_normalized(RelativePath::new(alias_path.as_str()))
                            .with_buck_error_context(|| {
                                format!(
                                    "expected alias path to be a relative path, but found `{}` for `{}`",
                                    alias_path.as_str(),
                                    alias,
                                )
                            })?,
                    );
                    let name = CellName::unchecked_new(alias)?;
                    cell_definitions.push((name, alias_path));
                }
            }
        }
        // ===== End Bzlmod Integration =====

        // Build root aliases:
        // - When MODULE.bazel exists: only bzlmod aliases (skip .buckconfig [cell_aliases])
        // - When no MODULE.bazel: merge .buckconfig aliases with bzlmod aliases
        let mut root_aliases: HashMap<NonEmptyCellAlias, NonEmptyCellAlias> = if has_module_bazel {
            HashMap::new()
        } else {
            Self::get_cell_aliases_from_config(&root_config)?.collect()
        };
        for (alias, target) in bzlmod_aliases {
            let target_alias = NonEmptyCellAlias::new(target.as_str().to_owned())?;
            if root_aliases.contains_key(&alias) {
                continue;
            }
            if cell_definitions
                .iter()
                .any(|(n, _)| n.as_str() == alias.as_str())
            {
                tracing::debug!(
                    "Skipping bzlmod alias '{}' -> '{}': conflicts with cell definition",
                    alias,
                    target
                );
                continue;
            }
            tracing::info!("Adding bzlmod repo_name alias: {} -> {}", alias, target);
            kuro_core::cells::register_dynamic_extension_cell_alias(
                alias.as_str().to_owned(),
                target.as_str().to_owned(),
            );
            root_aliases.insert(alias, target_alias);
        }

        let mut aggregator = CellsAggregator::new(cell_definitions, root_aliases.clone())?;

        // Mark remote bzlmod modules as external cells
        for (name, setup) in bzlmod_external_cells {
            aggregator.mark_external_cell(name, ExternalCellOrigin::Bzlmod(setup))?;
        }

        // Mark bundled cells added for bzlmod projects
        for name in bzlmod_bundled_cells {
            aggregator.mark_external_cell(name, ExternalCellOrigin::Bundled(name))?;
        }

        // Mark extension-generated cells
        for (name, setup) in bzlmod_extension_cells {
            aggregator.mark_external_cell(name, ExternalCellOrigin::ExtensionRepo(setup))?;
        }

        // Legacy .buckconfig [external_cells] - only used when MODULE.bazel is NOT present
        if !has_module_bazel {
            if let Some(external_cells) = root_config.get_section("external_cells") {
                for (alias, origin) in external_cells.iter() {
                    if origin.as_str() == "disabled" {
                        continue;
                    }
                    let alias = NonEmptyCellAlias::new(alias.to_owned())?;
                    let name = aggregator.resolve_root_alias(alias)?;
                    let origin =
                        Self::parse_external_cell_origin(name, origin.as_str(), &root_config)?;
                    if let ExternalCellOrigin::Bundled(name) = origin {
                        if let Ok(imp) = EXTERNAL_CELLS_IMPL.get() {
                            imp.check_bundled_cell_exists(name)?;
                        }
                    }
                    aggregator.mark_external_cell(name, origin)?;
                }
            }
        }

        let cell_resolver = aggregator.make_cell_resolver()?;

        Ok(Self {
            cell_resolver,
            root_config,
            external_data: ExternalBuckconfigData {
                args: processed_config_args,
            },
            is_bzlmod: has_module_bazel,
        })
    }

    /// Resolve bzlmod dependencies from MODULE.bazel if it exists.
    ///
    /// This function:
    /// 1. Checks if MODULE.bazel exists in the project root
    /// 2. Parses it for module() and bazel_dep() directives
    /// 3. Resolves local_path_override() to local cells
    /// 4. Fetches remote dependencies from BCR and extracts them
    /// Resolve bzlmod dependencies from MODULE.bazel.
    ///
    /// Returns cells to register and aliases from repo_name parameters.
    async fn resolve_bzlmod_dependencies(
        project_root: &ProjectRoot,
    ) -> kuro_error::Result<Option<BzlmodResolutionResult>> {
        let module_bazel_rel = ProjectRelativePath::new("MODULE.bazel")?;
        let module_bazel_path = project_root.resolve(module_bazel_rel);

        // Check if MODULE.bazel exists
        if !fs_util::try_exists(&module_bazel_path)? {
            return Ok(None);
        }

        tracing::info!("Found MODULE.bazel, resolving bzlmod dependencies");

        // Parse MODULE.bazel
        let parsed = match parse_module_bazel(module_bazel_path.as_path()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to parse MODULE.bazel: {}", e);
                return Ok(None);
            }
        };

        let mut cells = Vec::new();
        let mut aliases = Vec::new();
        let workspace_root = project_root.root().as_path();
        let mut resolved_graph_for_aliases = None;

        // Resolve local path overrides first
        let local_modules = resolve_local_modules(&parsed.module.overrides, workspace_root)?;
        for (name, resolved) in local_modules.iter() {
            let cell_name = CellName::unchecked_new(name)?;
            let cell_path =
                CellRootPathBuf::new(ProjectRelativePath::new(&resolved.relative_path)?.to_owned());
            // Local modules don't need BzlmodCellSetup - they use LocalPath external origin
            // which is handled separately if needed
            cells.push((cell_name, cell_path, None));
            tracing::info!(
                "Resolved local module: {} -> {}",
                name,
                resolved.relative_path
            );
        }

        // Resolve ALL dependencies (including transitive) using MVS algorithm
        if !parsed.module.bazel_deps.is_empty() {
            tracing::info!(
                "Running MVS resolution for {} direct dependencies",
                parsed.module.bazel_deps.len()
            );

            // Propagate resolver-level errors: a failure here means the
            // bzlmod resolver itself is broken (e.g. cache dir inaccessible,
            // BCR unreachable, MVS couldn't converge). This is distinct from
            // "no MODULE.bazel" (handled above) or "module has no deps"
            // (parsed.module.bazel_deps.is_empty() branch). Callers need to
            // see the difference so they don't silently build against a
            // truncated cell graph.
            let cache = ModuleCache::new().with_buck_error_context(|| {
                format!(
                    "Failed to initialize bzlmod module cache while resolving MODULE.bazel for root \
                     module '{}'",
                    parsed.module.name
                )
            })?;
            let mut resolver = MvsResolver::new(cache).await.with_buck_error_context(|| {
                format!(
                    "Failed to create MVS resolver while resolving MODULE.bazel for root module '{}'",
                    parsed.module.name
                )
            })?;
            let mut resolved_graph = resolver
                .resolve(&parsed.module, workspace_root)
                .await
                .with_buck_error_context(|| {
                    format!(
                        "MVS resolution failed for root module '{}' ({} direct dependencies)",
                        parsed.module.name,
                        parsed.module.bazel_deps.len()
                    )
                })?;

            tracing::info!(
                "MVS resolved {} total modules (including transitive)",
                resolved_graph.modules.len()
            );
            resolved_graph_for_aliases = Some(resolved_graph.clone());

            // Fetch sources for all resolved modules (downloads and extracts).
            // Keep as a warning: partial fetch failures (e.g. one registry URL
            // down) shouldn't block the whole build — cells for modules whose
            // sources did fetch remain usable, and unresolved cells will surface
            // a concrete "path does not exist" error at cell-access time.
            if let Err(e) = resolver.fetch_sources(&mut resolved_graph).await {
                tracing::warn!(
                    "Failed to fetch some module sources for root module '{}': {}",
                    parsed.module.name,
                    e
                );
            }

            // Build a set of local override names to skip
            let local_override_names: std::collections::HashSet<_> = parsed
                .module
                .overrides
                .iter()
                .filter_map(|o| match o {
                    kuro_bzlmod::types::Override::LocalPath(local) => {
                        Some(local.module_name.clone())
                    }
                    _ => None,
                })
                .collect();

            // Create symlinks from bazel-external/ to cached sources
            // This enables external tools and build actions to access files directly
            let external_base_dir = project_root.root().as_path().join("bazel-external");
            // Also create symlinks in buck-out/v2/external_cells/bzlmod/ for
            // build action source resolution (recreated after kuro clean)
            let buck_out_external_cells_dir = project_root
                .root()
                .as_path()
                .join("buck-out/v2/external_cells/bzlmod");
            // Track valid symlink names for stale cleanup
            let mut valid_symlink_names = std::collections::HashSet::new();
            for (module_name, module_info) in &resolved_graph.modules {
                // Skip root module and local overrides
                if module_name == &parsed.module.name || local_override_names.contains(module_name)
                {
                    continue;
                }

                // Only create symlinks for modules with cached source paths
                if let Some(source_path) = &module_info.source_path {
                    let entry_name = format!("{}+{}", module_name, module_info.version);
                    valid_symlink_names.insert(entry_name.clone());
                    let link_path = external_base_dir.join(&entry_name);

                    match ensure_symlink(&link_path, source_path) {
                        Ok(()) => {
                            tracing::debug!(
                                "Created symlink: {:?} -> {:?}",
                                link_path,
                                source_path
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to create symlink for {}@{}: {}",
                                module_name,
                                module_info.version,
                                e
                            );
                        }
                    }

                    // Also create buck-out/v2/external_cells/bzlmod/ symlink
                    // so that build action command lines can reference source
                    // files at their resolved paths (re-created after clean)
                    let buck_out_link = buck_out_external_cells_dir.join(&entry_name);
                    if let Err(e) = ensure_symlink(&buck_out_link, source_path) {
                        tracing::warn!(
                            "Failed to create external_cells symlink for {}@{}: {}",
                            module_name,
                            module_info.version,
                            e
                        );
                    }
                }
            }

            // Remove stale symlinks from previous resolutions
            // (e.g., modules removed from MODULE.bazel or version changes)
            cleanup_stale_symlinks(&external_base_dir, &valid_symlink_names);
            cleanup_stale_symlinks(&buck_out_external_cells_dir, &valid_symlink_names);

            // Register ALL resolved modules as cells. Sort the map by
            // module name first — HashMap/FxHashMap iteration order is
            // insertion-order-dependent under hashbrown (SwissTable), and
            // the upstream `selected` is a default-hashed HashMap, so
            // iteration here would otherwise vary across invocations and
            // flip first-wins dedup downstream (Plan 21.2).
            let mut sorted_modules: Vec<_> = resolved_graph.modules.iter().collect();
            sorted_modules.sort_by(|a, b| a.0.cmp(b.0));
            for (module_name, module_info) in sorted_modules {
                // Skip the root module and local overrides
                if module_name == &parsed.module.name || local_override_names.contains(module_name)
                {
                    continue;
                }

                let cell_name = CellName::unchecked_new(module_name)?;

                // Determine the cell path and setup based on source type
                match &module_info.source {
                    ModuleSource::Registry { url } => {
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Create a project-relative path for this external module
                        let external_path =
                            format!("bazel-external/{}+{}", module_name, module_info.version);
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );

                        tracing::info!(
                            "Registered module: {}@{} -> {} (external path: {})",
                            module_name,
                            module_info.version,
                            source_path_str,
                            external_path
                        );

                        let setup = kuro_core::cells::external::BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(url.as_str()),
                            source_path: Arc::from(source_path_str.as_str()),
                        };

                        cells.push((cell_name, cell_path, Some(setup)));
                    }
                    ModuleSource::LocalPath { path } => {
                        // Local path modules from overrides are handled separately
                        let cell_path =
                            CellRootPathBuf::new(ProjectRelativePath::new(path)?.to_owned());
                        cells.push((cell_name, cell_path, None));
                        tracing::info!("Registered local module: {} -> {}", module_name, path);
                    }
                    ModuleSource::Git { remote, commit, .. } => {
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let external_path =
                            format!("bazel-external/{}+{}", module_name, module_info.version);
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );

                        // Git modules use Bzlmod setup with empty registry URL
                        let setup = kuro_core::cells::external::BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(format!("git+{}", remote).as_str()),
                            source_path: Arc::from(source_path_str.as_str()),
                        };

                        cells.push((cell_name, cell_path, Some(setup)));
                        tracing::info!(
                            "Registered git module: {}@{} -> {} (commit: {})",
                            module_name,
                            module_info.version,
                            external_path,
                            commit
                        );
                    }
                    ModuleSource::Archive { urls, .. } => {
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let external_path =
                            format!("bazel-external/{}+{}", module_name, module_info.version);
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );

                        // Use first URL as the registry URL
                        let url = urls.first().map(|u| u.as_str()).unwrap_or("archive");
                        let setup = kuro_core::cells::external::BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(url),
                            source_path: Arc::from(source_path_str.as_str()),
                        };

                        cells.push((cell_name, cell_path, Some(setup)));
                        tracing::info!(
                            "Registered archive module: {}@{} -> {}",
                            module_name,
                            module_info.version,
                            external_path
                        );
                    }
                }
            }

            if let Some(repo_name) = &parsed.module.repo_name {
                if repo_name != &parsed.module.name {
                    let alias_name = NonEmptyCellAlias::new(repo_name.clone())?;
                    let cell_name = CellName::unchecked_new(&parsed.module.name)?;
                    tracing::info!(
                        "Creating root module self repo_name alias: {} -> {}",
                        repo_name,
                        parsed.module.name
                    );
                    aliases.push((alias_name, cell_name));
                }
            }

            // Handle apparent repository names from the root module's direct deps.
            // In Bazel, every bazel_dep is visible in the declaring module under
            // repo_name if specified, otherwise under name. Kuro's cell identity
            // can differ from that apparent name (for example when a selected
            // module is represented by a disambiguated cell), so register the
            // apparent name as an alias whenever it is not already the cell name.
            for dep in &parsed.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                if let Some(target_name) =
                    selected_bzlmod_cell_name_for_dep(&cells, &dep.name, &resolved_graph)
                {
                    if apparent_name != target_name {
                        let cell_name = CellName::unchecked_new(target_name)?;
                        let alias_name = NonEmptyCellAlias::new(apparent_name.to_owned())?;
                        tracing::info!(
                            "Creating root bazel_dep apparent alias: {} -> {}",
                            apparent_name,
                            target_name
                        );
                        aliases.push((alias_name, cell_name));
                    }
                }
            }

            // Populate the global module version registry
            // so module_version() builtin returns the correct version
            {
                let mut version_map = std::collections::HashMap::new();
                // Add root module
                version_map.insert(
                    parsed.module.name.clone(),
                    parsed.module.version.to_string(),
                );
                // Add all resolved external modules
                for (name, info) in &resolved_graph.modules {
                    version_map.insert(name.clone(), info.version.clone());
                }
                kuro_bzlmod::set_module_versions(version_map);
            }

            // Handle repo_name aliases from transitive deps
            // Parse each resolved module's MODULE.bazel to extract repo_name aliases
            Self::collect_transitive_repo_aliases(
                &resolved_graph,
                &parsed.module.name,
                &mut aliases,
            )
            .await;
        }

        // Build parsed_modules list for extension resolution
        let mut parsed_modules: Vec<(String, ParsedModuleFile)> = Vec::new();
        parsed_modules.push((parsed.module.name.clone(), parsed.clone()));
        for (cell_name, _cell_path, setup) in &cells {
            if let Some(bzlmod_setup) = setup {
                let module_bazel_path = std::path::PathBuf::from(bzlmod_setup.source_path.as_ref())
                    .join("MODULE.bazel");
                if module_bazel_path.exists() {
                    if let Ok(dep_parsed) = parse_module_bazel(&module_bazel_path) {
                        // Use the module's declared name for aggregation, not the cell name
                        // (which includes version suffix like "bazel_features+1.42.0").
                        // This ensures extension IDs are consistent: "//private:ext.bzl" from
                        // bazel_features resolves to "@bazel_features//private:ext.bzl", matching
                        // what other modules use when referencing this extension.
                        let module_key = if dep_parsed.module.name.is_empty() {
                            cell_name.as_str().to_string()
                        } else {
                            dep_parsed.module.name.clone()
                        };
                        parsed_modules.push((module_key, dep_parsed));
                    }
                }
            }
        }

        if let Some(resolved_graph) = &resolved_graph_for_aliases {
            for (_module_name, parsed_mod) in &parsed_modules {
                for dep in &parsed_mod.module.bazel_deps {
                    let apparent_name = dep.apparent_name();
                    if aliases
                        .iter()
                        .any(|(alias, _)| alias.as_str() == apparent_name)
                    {
                        continue;
                    }
                    let Some(target_name) =
                        selected_bzlmod_cell_name_for_dep(&cells, &dep.name, resolved_graph)
                    else {
                        continue;
                    };
                    if apparent_name == target_name {
                        continue;
                    }
                    let alias_name = NonEmptyCellAlias::new(apparent_name.to_owned())?;
                    let cell_name = CellName::unchecked_new(target_name)?;
                    tracing::info!(
                        "Creating bazel_dep apparent alias from module graph: {} -> {}",
                        apparent_name,
                        target_name
                    );
                    aliases.push((alias_name, cell_name));
                }
            }
        }

        // Pre-compute extension repo cells from use_repo() declarations alone.
        // This is the Bazel 9.0-compatible approach: canonical names are deterministic
        // from MODULE.bazel topology, no extension execution or lockfile needed.
        let root_module_name = if parsed.module.name.is_empty() {
            "_main"
        } else {
            &parsed.module.name
        };
        let mut module_extensions: std::collections::HashMap<
            String,
            Vec<kuro_bzlmod::types::ExtensionUsage>,
        > = std::collections::HashMap::new();
        for (module_name, parsed_mod) in &parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated =
            kuro_bzlmod::aggregate_extensions_with_root(&module_extensions, Some(root_module_name));
        let (mut pre_computed_cells, pre_computed_aliases) =
            kuro_bzlmod::pre_compute_extension_repo_cells(&parsed_modules, root_module_name)?;

        // Augment with extension-internal spokes recorded in MODULE.bazel.lock.
        // The use_repo()-driven pass above only registers repos the project
        // explicitly imports (e.g. the `crates` hub), not the spokes the hub's
        // generated BUILD.bazel references via `@crates__<name>//`. Without this
        // pass, warm builds with a populated cache fail with `unknown cell name`
        // because the only path that registers spokes (`get_file_ops_delegate`'s
        // post-extension-eval loop) is gated on the hub's `.kuro_repo_complete`
        // marker.
        if let Some(lockfile) = kuro_bzlmod::cached_lockfile(project_root.root().as_path()) {
            let extra = kuro_bzlmod::pre_compute_extension_repo_cells_from_lockfile(
                &lockfile,
                &aggregated,
                root_module_name,
                &mut pre_computed_cells,
                project_root.root().as_path(),
            );
            // Mirror lockfile-seeded cells into the dynamic-extension-cell
            // registry so `resolve_label_to_path` (used by `rctx.path`)
            // finds them before any materialization has run on disk. The
            // pending cells flow through the static aggregator further
            // down; this registration is just the parallel dynamic mapping.
            for cell in &extra {
                kuro_core::cells::register_dynamic_extension_cell(
                    cell.canonical_name.clone(),
                    cell.path.clone(),
                );
                if cell.internal_name != cell.canonical_name {
                    kuro_core::cells::register_dynamic_extension_cell(
                        cell.internal_name.clone(),
                        cell.path.clone(),
                    );
                }
            }
            pre_computed_cells.extend(extra);
        }
        kuro_util::memory_checkpoint::checkpoint(
            "legacy_cells_bzlmod_precomputed_repos",
            [
                ("parsed_modules", parsed_modules.len()),
                ("precomputed_cells", pre_computed_cells.len()),
                ("precomputed_aliases", pre_computed_aliases.len()),
            ],
        );

        // Aggregate extension usages from all modules and store globally.
        // This data is needed by DICE when extension repos are lazily executed.
        kuro_bzlmod::set_extension_aggregations(
            aggregated,
            root_module_name.to_owned(),
            project_root.root().to_path_buf(),
        );

        // Collect toolchain and execution platform registrations from all modules.
        // Priority order: root module first, then BFS order of dep graph.
        // parsed_modules is already in BFS order (root first from resolution).
        // dev_dependency items from non-root modules are skipped (Bazel 9.0 behavior).
        {
            let mut all_toolchains: Vec<kuro_bzlmod::RegisteredToolchain> = Vec::new();
            let mut all_exec_platforms = Vec::new();
            for (module_name, parsed_mod) in &parsed_modules {
                let is_root = module_name == root_module_name
                    || module_name == "_main"
                    || parsed_mod.module.name == root_module_name;
                let repo_mapping =
                    kuro_bzlmod::BzlmodRepoMapping::for_module(parsed_mod, root_module_name);
                for item in &parsed_mod.registered_toolchains {
                    if item.dev_dependency && !is_root {
                        tracing::debug!(
                            "Skipping dev_dependency toolchain '{}' from non-root module '{}'",
                            item.label,
                            module_name
                        );
                        continue;
                    }
                    let label = repo_mapping.canonicalize_label_to_storage_string(&item.label);
                    all_toolchains.push(kuro_bzlmod::RegisteredToolchain {
                        module: module_name.clone(),
                        label,
                        is_root,
                    });
                }
                for item in &parsed_mod.registered_execution_platforms {
                    if item.dev_dependency && !is_root {
                        tracing::debug!(
                            "Skipping dev_dependency execution platform '{}' from non-root module '{}'",
                            item.label,
                            module_name
                        );
                        continue;
                    }
                    all_exec_platforms
                        .push(repo_mapping.canonicalize_label_to_storage_string(&item.label));
                }
            }
            // If the module graph depends on rules_python but never registers
            // a py3 toolchain, auto-inject BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS
            // at lowest priority so ctx.toolchains[@rules_python//python:toolchain_type]
            // resolves to a host py_runtime. Users can override by registering
            // their own toolchain earlier in MODULE.bazel.
            //
            // WHY string match on the module name: ParsedModuleFile currently has no
            // typed "is rules_python" flag, and adding one would require threading a
            // new field through kuro_bzlmod::types + the MVS resolver + every caller
            // that constructs ParsedModuleFile — well out of scope for an error-
            // handling fix. The constants below keep the magic strings grep-able so a
            // future typed flag can replace them in one place.
            if module_depends_on_rules_python(&parsed_modules)
                && !toolchains_include_bundled_python(&all_toolchains)
            {
                for label in BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS {
                    all_toolchains.push(kuro_bzlmod::RegisteredToolchain {
                        module: RULES_PYTHON_MODULE_NAME.to_owned(),
                        label: (*label).to_owned(),
                        // Auto-injected bundled toolchains must always be
                        // eagerly loaded — they back the bundled
                        // `@local_config_python` cell that callers expect to
                        // be available without bzlmod fetch.
                        is_root: true,
                    });
                }
                tracing::info!(
                    "Auto-registered bundled rules_python toolchains (rules_python in deps): {:?}",
                    BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS
                );
            }

            tracing::info!(
                "Collected {} toolchain registration(s) and {} execution platform registration(s)",
                all_toolchains.len(),
                all_exec_platforms.len()
            );
            kuro_bzlmod::set_registered_toolchains(all_toolchains.clone());
            kuro_bzlmod::set_registered_execution_platforms(all_exec_platforms);

            // Ensure toolchain repos referenced in register_toolchains() exist.
            // Extract repo names from label patterns and check if the repo directories
            // are present. Extension repos that haven't materialized will be triggered
            // when their ExtensionRepoCellSetup is first accessed during analysis.
            // Here we just log which repos are pending to aid debugging.
            let project_root_path = project_root.root().to_path_buf();
            let bazel_ext_dir = project_root_path.join("bazel-external");
            let mut repos_needing_materialization = Vec::new();
            for tc in &all_toolchains {
                let tc_label = &tc.label;
                if let Some(repo_name) = extract_repo_name_from_label(tc_label) {
                    // Diagnostic/materialization bookkeeping only: label
                    // resolution itself goes through the typed resolvers.
                    // This scan checks whether a registered toolchain's repo
                    // already has a materialized legacy/module-version
                    // directory so we can log pending repos.
                    let has_dir = if bazel_ext_dir.is_dir() {
                        std::fs::read_dir(&bazel_ext_dir)
                            .ok()
                            .map(|entries| {
                                entries.flatten().any(|e| {
                                    let name = e.file_name();
                                    let s = name.to_string_lossy();
                                    // Match: exact name, "name+version", or "ext+name+name"
                                    s.as_ref() == repo_name
                                        || s.starts_with(&format!("{}+", repo_name))
                                        || s.ends_with(&format!("+{}", repo_name))
                                })
                            })
                            .unwrap_or(false)
                    } else {
                        false
                    };
                    if !has_dir {
                        repos_needing_materialization.push(repo_name.to_owned());
                    }
                }
            }
            if !repos_needing_materialization.is_empty() {
                tracing::info!(
                    "{} toolchain repo(s) pending materialization: {:?}",
                    repos_needing_materialization.len(),
                    repos_needing_materialization
                );
            }
        }

        // Set project root for dynamic cell filesystem scanning
        kuro_core::cells::set_dynamic_project_root(project_root.root().to_path_buf());

        // Convert pre-computed cells to the format expected by
        // BzlmodResolutionResult. Bazel's identity for extension-generated
        // repositories is the canonical repo name; apparent names from
        // use_repo() are repository-mapping entries that point at that identity.
        let mut ext_cells = Vec::new();
        for cell in pre_computed_cells {
            let cell_name = CellName::unchecked_new(&cell.canonical_name)?;
            let cell_path = CellRootPathBuf::new(ProjectRelativePath::new(&cell.path)?.to_owned());
            let setup = ExtensionRepoCellSetup {
                canonical_name: Arc::from(cell.canonical_name.as_str()),
                extension_id: Arc::from(cell.extension_id.as_str()),
                internal_name: Arc::from(cell.internal_name.as_str()),
                spec_hash: Arc::from(cell.spec_hash.as_str()),
                repo_spec_json: Arc::from(cell.repo_spec_json.as_str()),
                materialized: false,
            };
            // repository_ctx.execute/path(Label) resolves through the dynamic
            // extension-cell registry, not the static CellResolver being built
            // here. Mirror every precomputed extension repo there so
            // use_repo_rule tools (for example @toml2json_linux_amd64) have a
            // canonical filesystem path before lazy materialization runs.
            kuro_core::cells::register_dynamic_extension_cell_with_setup(
                cell.canonical_name.clone(),
                cell.path.clone(),
                setup.clone(),
            );
            if cell.internal_name != cell.canonical_name {
                kuro_core::cells::register_dynamic_extension_cell_with_setup(
                    cell.internal_name.clone(),
                    cell.path.clone(),
                    setup.clone(),
                );
            }
            if !cell.repo_spec_json.is_empty() {
                match serde_json::from_str::<kuro_bzlmod::RepoSpec>(&cell.repo_spec_json) {
                    Ok(repo_spec) => {
                        let registration = kuro_bzlmod::SpokeRegistration {
                            extension_id: Arc::from(cell.extension_id.as_str()),
                            repo_spec: Arc::new(repo_spec),
                            project_root: Arc::new(project_root.root().to_path_buf()),
                        };
                        kuro_bzlmod::register_spoke(
                            cell.canonical_name.clone(),
                            registration.clone(),
                        );
                        if cell.internal_name != cell.canonical_name {
                            kuro_bzlmod::register_spoke(cell.internal_name.clone(), registration);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse precomputed RepoSpec for '{}': {}",
                            cell.canonical_name,
                            e
                        );
                    }
                }
            }
            ext_cells.push((cell_name, cell_path, setup));
        }

        // Build a set of existing cell names (from bzlmod deps + synthetic repos)
        // to avoid creating aliases that conflict with cell names.
        let existing_cell_names: std::collections::HashSet<&str> =
            cells.iter().map(|(name, _, _)| name.as_str()).collect();

        // Convert pre-computed aliases. Apparent names from use_repo() are
        // module-scoped in Bazel; Kuro still has a global alias table, so keep
        // this to direct apparent names to canonical cells without inventing a
        // second cell identity.
        let mut ext_aliases = Vec::new();
        for alias in pre_computed_aliases {
            if existing_cell_names.contains(alias.apparent_name.as_str()) {
                tracing::debug!(
                    "Skipping alias '{}' -> '{}': cell already exists (synthetic repo)",
                    alias.apparent_name,
                    alias.canonical_name
                );
                continue;
            }
            let apparent_name = NonEmptyCellAlias::new(alias.apparent_name)?;
            let canonical_name = CellName::unchecked_new(&alias.canonical_name)?;
            if let Some((owner_module, _, _)) =
                kuro_bzlmod::parse_canonical_name(canonical_name.as_str())
            {
                kuro_core::cells::register_scoped_bzlmod_repo_alias(
                    owner_module.to_owned(),
                    apparent_name.as_str().to_owned(),
                    canonical_name.as_str().to_owned(),
                );
            }
            kuro_core::cells::register_dynamic_extension_cell_alias(
                apparent_name.as_str().to_owned(),
                canonical_name.as_str().to_owned(),
            );
            ext_aliases.push((apparent_name, canonical_name));
        }

        // Add extension aliases to the main aliases list
        aliases.extend(ext_aliases);

        // Process use_repo_rule() invocations from MODULE.bazel files.
        // These are direct repo rule calls like http_file(name="toml2json_linux_amd64", ...).
        // They need to be materialized eagerly and registered as cells.
        {
            let project_root_path = project_root.root().to_path_buf();
            for (_module_name, parsed_mod) in &parsed_modules {
                let module_name = if parsed_mod.module.name.is_empty() {
                    "_main"
                } else {
                    &parsed_mod.module.name
                };
                tracing::info!(
                    "Module '{}' has {} repo_rule_invocations",
                    module_name,
                    parsed_mod.repo_rule_invocations.len()
                );
                for invocation in &parsed_mod.repo_rule_invocations {
                    if ext_cells
                        .iter()
                        .any(|(_, _, setup)| setup.internal_name.as_ref() == invocation.name)
                    {
                        continue;
                    }

                    let cell_name_str = invocation.name.clone();
                    let cell_path_str = format!("bazel-external/{}", cell_name_str);

                    // Skip if already registered
                    if existing_cell_names.contains(cell_name_str.as_str()) {
                        continue;
                    }

                    let rule_name = invocation
                        .rule_source
                        .split('%')
                        .last()
                        .unwrap_or("unknown");

                    // Check if this is a custom Starlark rule (has .bzl source)
                    let is_custom_rule = !kuro_bzlmod::is_builtin_repo_rule(rule_name);

                    if is_custom_rule {
                        // Register as extension cell for lazy DICE-based Starlark execution.
                        // In Bazel, use_repo_rule() is syntactic sugar for an implicit extension.
                        let extension_id = invocation.rule_source.clone();
                        let mut repo_spec =
                            kuro_bzlmod::RepoSpec::new(invocation.rule_source.clone());
                        for (k, v) in &invocation.attrs {
                            repo_spec
                                .attributes
                                .insert(k.clone(), tag_value_to_attr_value(v));
                        }
                        let repo_spec_json = serde_json::to_string(&repo_spec).unwrap_or_default();

                        if let Ok(cell_name) = CellName::unchecked_new(&cell_name_str) {
                            if let Ok(cell_path) = ProjectRelativePath::new(&cell_path_str)
                                .map(|p| CellRootPathBuf::new(p.to_owned()))
                            {
                                let setup = ExtensionRepoCellSetup {
                                    canonical_name: Arc::from(cell_name_str.as_str()),
                                    extension_id: Arc::from(extension_id.as_str()),
                                    internal_name: Arc::from(cell_name_str.as_str()),
                                    spec_hash: Arc::from(""),
                                    repo_spec_json: Arc::from(repo_spec_json.as_str()),
                                    materialized: false,
                                };
                                ext_cells.push((cell_name, cell_path, setup));
                                tracing::info!(
                                    "Registered custom repo rule '{}' as extension cell for lazy execution",
                                    cell_name_str
                                );
                            }
                        }
                        continue;
                    }

                    // Convert TagValue attrs to RepositoryInvocation attrs for the executor
                    let mut inv = kuro_bzlmod::RepositoryInvocation::new(
                        invocation.name.clone(),
                        rule_name.to_owned(),
                    );
                    inv.rule_source = Some(invocation.rule_source.clone());
                    for (k, v) in &invocation.attrs {
                        inv.attrs.insert(k.clone(), tag_value_to_repo_attr(v));
                    }

                    // Materialize the repo
                    match kuro_bzlmod::execute_repository_rule(&inv, &project_root_path) {
                        Ok(_result) => {
                            tracing::info!(
                                "Materialized MODULE.bazel repo '{}' from '{}'",
                                cell_name_str,
                                module_name
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to materialize MODULE.bazel repo '{}': {}",
                                cell_name_str,
                                e
                            );
                            continue;
                        }
                    }

                    // Register as a cell
                    if let Ok(cell_name) = CellName::unchecked_new(&cell_name_str) {
                        if let Ok(cell_path) = ProjectRelativePath::new(&cell_path_str)
                            .map(|p| CellRootPathBuf::new(p.to_owned()))
                        {
                            cells.push((cell_name, cell_path, None));
                        }
                    }
                }
            }
        }

        let root_module_name = if parsed.module.name.is_empty() {
            "_main".to_owned()
        } else {
            parsed.module.name.clone()
        };

        // Create external/ symlinks for all non-root cells so that action commands
        // using artifact paths like `external/<cell>/...` can find source files.
        {
            let cell_pairs: Vec<(String, String)> =
                cells
                    .iter()
                    .map(|(name, path, _)| (name.as_str().to_owned(), path.as_str().to_owned()))
                    .chain(ext_cells.iter().map(|(name, path, _)| {
                        (name.as_str().to_owned(), path.as_str().to_owned())
                    }))
                    .collect();
            kuro_core::cells::ensure_external_symlinks_for_cells(&cell_pairs);
            // Also create symlinks for apparent names (aliases)
            // These map short names to the same paths as their canonical cells.
            for (alias, canonical) in &aliases {
                let alias_str = alias.as_str();
                // Find the cell path for this canonical name
                if let Some((_, path, _)) = cells
                    .iter()
                    .find(|(name, _, _)| name.as_str() == canonical.as_str())
                {
                    kuro_core::cells::ensure_external_symlink(alias_str, path.as_str());
                } else if let Some((_, path, _)) = ext_cells
                    .iter()
                    .find(|(name, _, _)| name.as_str() == canonical.as_str())
                {
                    kuro_core::cells::ensure_external_symlink(alias_str, path.as_str());
                }
            }
        }

        Ok(Some(BzlmodResolutionResult {
            root_module_name,
            cells,
            extension_cells: ext_cells,
            aliases,
        }))
    }

    /// Collect repo_name aliases from transitive dependencies.
    ///
    /// This function parses the MODULE.bazel of each resolved module to extract
    /// bazel_dep declarations with repo_name parameters, creating global aliases
    /// so that transitive dependencies can be accessed by their aliased names.
    ///
    /// Note: In Bazel, repo_name aliases are scoped to the declaring module.
    /// This implementation creates global aliases for simplicity. A future
    /// enhancement could implement proper scoping.
    async fn collect_transitive_repo_aliases(
        resolved_graph: &ResolvedGraph,
        root_module_name: &str,
        aliases: &mut Vec<(NonEmptyCellAlias, CellName)>,
    ) {
        for (module_name, module_info) in &resolved_graph.modules {
            // Skip root module (already handled)
            if module_name == root_module_name {
                continue;
            }

            // Get the source path where MODULE.bazel is located
            let source_path = match &module_info.source_path {
                Some(path) => path.clone(),
                None => continue,
            };

            let module_bazel_path = source_path.join("MODULE.bazel");
            if !module_bazel_path.exists() {
                continue;
            }

            // Parse the transitive module's MODULE.bazel
            match parse_module_bazel(&module_bazel_path) {
                Ok(dep_parsed) => {
                    // Extract repo_name aliases from this module's dependencies.
                    // Skip dev_dependency deps and deps not in the resolved graph -
                    // they won't have a corresponding cell.
                    for dep in &dep_parsed.module.bazel_deps {
                        if dep.dev_dependency {
                            continue;
                        }
                        if let Some(repo_name) = &dep.repo_name {
                            if repo_name != &dep.name {
                                // Only create alias if the target module is in the resolved graph
                                if !resolved_graph.modules.contains_key(&dep.name) {
                                    tracing::debug!(
                                        "Skipping transitive repo_name alias: {} -> {} (from {}): target not in resolved graph",
                                        repo_name,
                                        dep.name,
                                        module_name
                                    );
                                    continue;
                                }
                                // Create alias: repo_name -> dep.name
                                match (
                                    NonEmptyCellAlias::new(repo_name.clone()),
                                    CellName::unchecked_new(&dep.name),
                                ) {
                                    (Ok(alias_name), Ok(cell_name)) => {
                                        // Check if this alias already exists
                                        let already_exists =
                                            aliases.iter().any(|(a, _)| a == &alias_name);
                                        if !already_exists {
                                            tracing::info!(
                                                "Creating transitive repo_name alias: {} -> {} (from {})",
                                                repo_name,
                                                dep.name,
                                                module_name
                                            );
                                            aliases.push((alias_name, cell_name));
                                        }
                                    }
                                    _ => {
                                        tracing::debug!(
                                            "Failed to create alias {} -> {} from {}",
                                            repo_name,
                                            dep.name,
                                            module_name
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to parse MODULE.bazel for {}: {}", module_name, e);
                }
            }
        }
    }

    pub(crate) fn get_cell_aliases_from_config(
        config: &LegacyBuckConfig,
    ) -> kuro_error::Result<impl Iterator<Item = (NonEmptyCellAlias, NonEmptyCellAlias)> + use<>>
    {
        let mut aliases = Vec::new();
        if let Some(section) = config
            .get_section("cell_aliases")
            .or_else(|| config.get_section("repository_aliases"))
        {
            for (alias, destination) in section.iter() {
                let alias = NonEmptyCellAlias::new(alias.to_owned())?;
                let destination = NonEmptyCellAlias::new(destination.as_str().to_owned())?;
                aliases.push((alias, destination));
            }
        }
        Ok(aliases.into_iter())
    }

    pub(crate) async fn parse_single_cell_with_dice(
        ctx: &mut DiceComputations<'_>,
        _cell_path: &CellRootPath,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        let external_data = ctx.get_injected_external_buckconfig_data().await?;
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(&external_data.args))
    }

    pub async fn parse_single_cell(
        &self,
        _cell: CellName,
        _project_fs: &ProjectRoot,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(
            &self.external_data.args,
        ))
    }

    pub(crate) async fn parse_single_cell_with_file_ops(
        &self,
        _cell: CellName,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(
            &self.external_data.args,
        ))
    }

    fn parse_external_cell_origin(
        cell: CellName,
        value: &str,
        config: &LegacyBuckConfig,
    ) -> kuro_error::Result<ExternalCellOrigin> {
        #[derive(kuro_error::Error, Debug)]
        #[kuro(tag = Input)]
        enum ExternalCellOriginParseError {
            #[error("Unknown external cell origin `{0}`")]
            Unknown(String),
            #[error("Missing buckconfig `{0}.{1}` for external cell configuration")]
            MissingConfiguration(String, String),
        }

        let get_config = |section: &str, property: &str| {
            config
                .get(crate::legacy_configs::key::BuckconfigKeyRef { section, property })
                .ok_or_else(|| {
                    ExternalCellOriginParseError::MissingConfiguration(
                        section.to_owned(),
                        property.to_owned(),
                    )
                })
        };

        if value == "bundled" {
            Ok(ExternalCellOrigin::Bundled(cell))
        } else if value == "git" {
            let section = &format!("external_cell_{}", cell.as_str());
            let commit = get_config(section, "commit_hash")?;
            let object_format = match get_config(section, "object_format") {
                Ok(s) => {
                    let object_format = GitObjectFormat::from_str(s)?;
                    object_format.check(commit)?;
                    Option::Some(GitObjectFormat::from_str(s)?)
                }
                Err(_) => {
                    // We pretend that the object format is SHA1 for this check only;
                    // We do not use it when interacting with Git.
                    GitObjectFormat::Sha1.check(commit)?;
                    Option::None
                }
            };
            Ok(ExternalCellOrigin::Git(GitCellSetup {
                git_origin: get_config(section, "git_origin")?.into(),
                commit: Arc::from(commit),
                object_format,
            }))
        } else {
            Err(ExternalCellOriginParseError::Unknown(value.to_owned()).into())
        }
    }
}

fn selected_bzlmod_cell_name_for_dep<'a>(
    cells: &'a [(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    dep_name: &str,
    resolved_graph: &kuro_bzlmod::ResolvedGraph,
) -> Option<&'a str> {
    if let Some((name, _, _)) = cells.iter().find(|(name, _, _)| name.as_str() == dep_name) {
        return Some(name.as_str());
    }

    let selected_version = resolved_graph.selected_versions.get(dep_name)?;
    let versioned_name = format!("{}+{}", dep_name, selected_version);
    if let Some((name, _, _)) = cells
        .iter()
        .find(|(name, _, _)| name.as_str() == versioned_name)
    {
        return Some(name.as_str());
    }

    None
}

/// Extract the repo name from a toolchain/platform label.
/// E.g., "@local_config_cc_toolchains//:all" → "local_config_cc_toolchains"
///       "//cc/private/toolchain/test:default_test_runner_toolchain" → None (relative)
fn extract_repo_name_from_label(label: &str) -> Option<String> {
    let parsed = kuro_bzlmod::canonicalize_label_with_package_context(label, "", "", None)?;
    let repo = parsed.repo().as_str();
    if repo.is_empty() {
        None
    } else {
        Some(repo.to_owned())
    }
}

/// Convert a TagValue to a RepoSpec AttrValue (for extension cell repo specs).
fn tag_value_to_attr_value(tv: &TagValue) -> kuro_bzlmod::repository_invocations::AttrValue {
    use kuro_bzlmod::repository_invocations::AttrValue;
    match tv {
        TagValue::String(s) => {
            if s.starts_with("//") || s.starts_with("@") || s.starts_with(":") {
                AttrValue::Label(s.clone())
            } else {
                AttrValue::String(s.clone())
            }
        }
        TagValue::Int(i) => AttrValue::Int(*i),
        TagValue::Bool(b) => AttrValue::Bool(*b),
        TagValue::None => AttrValue::None,
        TagValue::Label(s) => AttrValue::Label(s.clone()),
        TagValue::List(items) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|v| match v {
                    TagValue::String(s) | TagValue::Label(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            AttrValue::StringList(strings)
        }
        TagValue::Dict(entries) => {
            let map: indexmap::IndexMap<String, AttrValue> = entries
                .iter()
                .map(|(k, v)| (k.clone(), tag_value_to_attr_value(v)))
                .collect();
            AttrValue::Dict(map)
        }
    }
}

fn tag_value_to_repo_attr(tv: &TagValue) -> kuro_bzlmod::RepoAttrValue {
    match tv {
        TagValue::String(s) => {
            if s.starts_with("//") || s.starts_with("@") || s.starts_with(":") {
                kuro_bzlmod::RepoAttrValue::Label(s.clone())
            } else {
                kuro_bzlmod::RepoAttrValue::String(s.clone())
            }
        }
        TagValue::Int(i) => kuro_bzlmod::RepoAttrValue::Int(*i),
        TagValue::Bool(b) => kuro_bzlmod::RepoAttrValue::Bool(*b),
        TagValue::None => kuro_bzlmod::RepoAttrValue::None,
        TagValue::Label(s) => kuro_bzlmod::RepoAttrValue::Label(s.clone()),
        TagValue::List(items) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|v| match v {
                    TagValue::String(s) | TagValue::Label(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            kuro_bzlmod::RepoAttrValue::StringList(strings)
        }
        TagValue::Dict(entries) => {
            let map: indexmap::IndexMap<String, kuro_bzlmod::RepoAttrValue> = entries
                .iter()
                .map(|(k, v)| (k.clone(), tag_value_to_repo_attr(v)))
                .collect();
            kuro_bzlmod::RepoAttrValue::Dict(map)
        }
    }
}
