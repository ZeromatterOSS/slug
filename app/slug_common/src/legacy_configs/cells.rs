/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use base64::Engine;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::Key;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod::BzlmodEventKind;
use slug_bzlmod::ModuleCache;
use slug_bzlmod::ModuleSource;
use slug_bzlmod::MvsResolver;
use slug_bzlmod::parse_module_bazel;
use slug_bzlmod::record_bzlmod_event;
use slug_bzlmod::resolve_local_modules;
use slug_bzlmod::types::ParsedModuleFile;
use slug_core::cells::CellAliasResolver;
use slug_core::cells::CellResolver;
use slug_core::cells::alias::NonEmptyCellAlias;
use slug_core::cells::cell_root_path::CellRootPath;
use slug_core::cells::cell_root_path::CellRootPathBuf;
use slug_core::cells::external::BzlmodCellSetup;
use slug_core::cells::external::ExtensionRepoCellSetup;
use slug_core::cells::external::ExternalCellOrigin;
use slug_core::cells::external::GitCellSetup;
use slug_core::cells::external::GitObjectFormat;
use slug_core::cells::name::CellName;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_error::BuckErrorContext;
use slug_fs::fs_util;
use slug_fs::paths::RelativePath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;
use slug_fs::paths::abs_path::AbsPath;

use crate::external_cells::EXTERNAL_CELLS_IMPL;
use crate::file_ops::dice::DiceFileComputations;
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

const BZLMOD_ALWAYS_BUNDLED_CELLS: &[&str] = &[
    "bazel_tools",
    "local_config_platform",
    "slug_builtins",
    "local_config_python",
];

fn repo_env_json(repo_env: &BTreeMap<String, String>) -> Arc<str> {
    Arc::from(
        serde_json::to_string(repo_env)
            .unwrap_or_else(|_| "{}".to_owned())
            .as_str(),
    )
}

fn validate_precomputed_repo_spec(canonical_name: &str, repo_spec_json: &str) {
    if repo_spec_json.is_empty() {
        return;
    }
    match serde_json::from_str::<slug_bzlmod::RepoSpec>(repo_spec_json) {
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to parse precomputed RepoSpec for '{}': {}",
                canonical_name,
                e
            );
        }
    }
}

fn runtime_extension_setup_from_cell_graph(
    cell: &slug_bzlmod::BzlmodCellGraphExtensionCell,
) -> ExtensionRepoCellSetup {
    ExtensionRepoCellSetup {
        canonical_name: Arc::from(cell.canonical_name.as_str()),
        extension_id: Arc::from(cell.extension_id.as_str()),
        internal_name: Arc::from(cell.internal_name.as_str()),
        spec_hash: Arc::from(cell.spec_hash.as_str()),
        repo_spec_json: Arc::from(cell.repo_spec_json.as_str()),
        repo_env_json: Arc::from(cell.repo_env_json.as_str()),
        materialized: cell.materialized,
    }
}

fn module_setup_from_cell_graph(
    cell: &slug_bzlmod::BzlmodCellGraphCell,
) -> Option<BzlmodCellSetup> {
    cell.module_setup.as_ref().map(|setup| BzlmodCellSetup {
        module_name: Arc::from(setup.module_name.as_str()),
        version: Arc::from(setup.version.as_str()),
        registry_url: Arc::from(setup.registry_url.as_str()),
        source_path: Arc::from(setup.source_path.as_str()),
    })
}

fn cell_root_path_from_cell_graph(path: &str) -> slug_error::Result<CellRootPathBuf> {
    Ok(CellRootPathBuf::new(
        ProjectRelativePath::new(path)?.to_owned(),
    ))
}

fn runtime_cell_install_snapshot(
    cell_graph: &slug_bzlmod::BzlmodCellGraphValue,
) -> slug_core::cells::BzlmodRuntimeCellInstallSnapshot {
    let mut snapshot = slug_core::cells::BzlmodRuntimeCellInstallSnapshot::default();
    snapshot
        .extension_cells
        .extend(cell_graph.extension_cells.iter().map(|cell| {
            validate_precomputed_repo_spec(&cell.canonical_name, &cell.repo_spec_json);
            slug_core::cells::BzlmodRuntimeExtensionCell {
                canonical_name: cell.canonical_name.clone(),
                internal_name: cell.internal_name.clone(),
                path: cell.path.clone(),
                setup: runtime_extension_setup_from_cell_graph(cell),
                registration: if cell.lazy {
                    slug_core::cells::BzlmodRuntimeExtensionCellRegistration::Lazy
                } else {
                    slug_core::cells::BzlmodRuntimeExtensionCellRegistration::Eager
                },
            }
        }));
    snapshot
        .scoped_aliases
        .extend(cell_graph.scoped_aliases.iter().map(|alias| {
            slug_core::cells::BzlmodRuntimeScopedRepoAlias {
                owner_module: alias.owner_module.clone(),
                apparent_name: alias.apparent_name.clone(),
                target_name: alias.target_name.clone(),
            }
        }));
    snapshot
        .dynamic_aliases
        .extend(cell_graph.dynamic_aliases.iter().map(|alias| {
            slug_core::cells::BzlmodRuntimeDynamicAlias {
                apparent_name: alias.apparent_name.clone(),
                canonical_name: alias.canonical_name.clone(),
            }
        }));
    snapshot
}

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

fn repo_mapping_snapshot_for_modules(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
) -> slug_bzlmod::RepoMappingSnapshot {
    let mut snapshot = slug_bzlmod::RepoMappingSnapshot::new();
    for (module_name, parsed_mod) in parsed_modules {
        let mapping = slug_bzlmod::BzlmodRepoMapping::for_module(parsed_mod, root_module_name)
            .entries_as_strings();
        if module_name == root_module_name {
            snapshot.insert(String::new(), mapping.clone());
        }
        snapshot.insert(module_name.clone(), mapping);
    }
    snapshot
}

fn repo_mapping_overrides_for_root(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
) -> slug_bzlmod::RepoMappingOverrides {
    let mut overrides = slug_bzlmod::RepoMappingOverrides::new();
    if ignore_dev_dependency {
        return overrides;
    }
    for (cell_name, parsed_mod) in parsed_modules {
        let module_name = if parsed_mod.module.name.is_empty() {
            root_module_name
        } else {
            &parsed_mod.module.name
        };
        let is_root = cell_name == root_module_name
            || cell_name == "_main"
            || parsed_mod.module.name == root_module_name;
        if !is_root {
            continue;
        }

        for usage in &parsed_mod.extension_usages {
            if usage.repo_overrides.is_empty() && usage.injected_repos.is_empty() {
                continue;
            }
            let ext_id = slug_bzlmod::canonical_extension_id(
                &usage.extension_bzl_file,
                &usage.extension_name,
                module_name,
            );
            let entry = overrides.entry(ext_id).or_default();
            for (generated_name, replacement_repo) in &usage.repo_overrides {
                entry.insert(generated_name.clone(), replacement_repo.clone());
            }
            for (injected_name, source_repo) in &usage.injected_repos {
                entry.insert(injected_name.clone(), source_repo.clone());
            }
        }
    }
    overrides
}

fn add_extension_repo_mapping_rows_from_cells(
    snapshot: &mut slug_bzlmod::RepoMappingSnapshot,
    cells: &[slug_bzlmod::PendingRepoCell],
    root_module_name: &str,
    repo_mapping_overrides: &slug_bzlmod::RepoMappingOverrides,
) {
    let mut by_extension: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for cell in cells {
        by_extension
            .entry(cell.extension_id.clone())
            .or_default()
            .push((cell.internal_name.clone(), cell.canonical_name.clone()));
    }

    for (extension_id, generated_repos) in by_extension {
        let overrides = repo_mapping_overrides.get(&extension_id);
        if !slug_bzlmod::add_extension_generated_repo_mappings(
            snapshot,
            &extension_id,
            root_module_name,
            generated_repos,
            overrides,
        ) {
            tracing::debug!(
                "Skipping extension repo mapping rows for '{}': owner module mapping is unavailable",
                extension_id
            );
        }
    }
}

fn add_scoped_repo_aliases_from_mapping_snapshot(
    aliases: &mut Vec<BzlmodScopedRepoAlias>,
    snapshot: &slug_bzlmod::RepoMappingSnapshot,
) {
    for (source_repo, mappings) in snapshot {
        if source_repo.is_empty() {
            continue;
        }
        for (apparent_name, target_name) in mappings {
            aliases.push(BzlmodScopedRepoAlias {
                owner_module: source_repo.clone(),
                apparent_name: apparent_name.clone(),
                target_name: target_name.clone(),
            });
        }
    }
}

fn add_scoped_repo_aliases_from_root_overrides(
    aliases: &mut Vec<BzlmodScopedRepoAlias>,
    repo_mapping_overrides: &slug_bzlmod::RepoMappingOverrides,
    root_module_name: &str,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
) {
    for (extension_id, overrides) in repo_mapping_overrides {
        let owner_module = slug_bzlmod::extract_owning_module(extension_id, root_module_name);
        for (generated_name, replacement_repo) in overrides {
            let target_name = resolved_graph
                .and_then(|graph| selected_bzlmod_cell_name_for_dep(cells, replacement_repo, graph))
                .unwrap_or(replacement_repo.as_str())
                .to_owned();
            aliases.push(BzlmodScopedRepoAlias {
                owner_module: owner_module.clone(),
                apparent_name: generated_name.clone(),
                target_name: target_name.clone(),
            });
            if let Some(owner_without_separator) = owner_module.strip_suffix('+') {
                aliases.push(BzlmodScopedRepoAlias {
                    owner_module: owner_without_separator.to_owned(),
                    apparent_name: generated_name.clone(),
                    target_name,
                });
            }
        }
    }
}

fn collect_bzlmod_registered_items(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
) -> (Vec<slug_bzlmod::RegisteredToolchain>, Vec<String>) {
    let mut all_toolchains = Vec::new();
    let mut all_exec_platforms = Vec::new();
    for (module_name, parsed_mod) in parsed_modules {
        let is_root = module_name == root_module_name
            || module_name == "_main"
            || parsed_mod.module.name == root_module_name;
        let repo_mapping = slug_bzlmod::BzlmodRepoMapping::for_module(parsed_mod, root_module_name);
        for item in &parsed_mod.registered_toolchains {
            if item.dev_dependency && (!is_root || ignore_dev_dependency) {
                tracing::debug!(
                    "Skipping dev_dependency toolchain '{}' from module '{}'",
                    item.label,
                    module_name
                );
                continue;
            }
            let label = repo_mapping.canonicalize_label_to_storage_string(&item.label);
            all_toolchains.push(slug_bzlmod::RegisteredToolchain {
                module: module_name.clone(),
                label,
                is_root,
            });
        }
        for item in &parsed_mod.registered_execution_platforms {
            if item.dev_dependency && (!is_root || ignore_dev_dependency) {
                tracing::debug!(
                    "Skipping dev_dependency execution platform '{}' from module '{}'",
                    item.label,
                    module_name
                );
                continue;
            }
            all_exec_platforms.push(repo_mapping.canonicalize_label_to_storage_string(&item.label));
        }
    }
    (all_toolchains, all_exec_platforms)
}

/// True iff any toolchain label already references the bundled
/// `@local_config_python` cell (meaning the user has already wired up
/// bundled rules_python toolchains and we should skip auto-injection).
fn toolchains_include_bundled_python(toolchains: &[slug_bzlmod::RegisteredToolchain]) -> bool {
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
    ) -> Vec<slug_data::BuckconfigComponent> {
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
    /// Bzlmod facts from MODULE.bazel resolution that must participate in DICE
    /// invalidation for this command.
    pub bzlmod_session_data: slug_bzlmod::BzlmodSessionData,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodExternalModuleSymlink {
    entry_name: String,
    source_path: PathBuf,
}

fn bazel_canonical_module_repo_name(module_name: &str, version: &str) -> String {
    if module_name.contains('+') {
        module_name.to_owned()
    } else if version.is_empty() {
        format!("{module_name}+")
    } else {
        // Bazel 9 canonical module repository names keep an empty version
        // segment for the selected module repo (`@@llvm+//...`), even when the
        // selected version is recorded separately in the lockfile.
        format!("{module_name}+")
    }
}

fn local_override_cell_path_and_symlink(
    project_root: &ProjectRoot,
    project_root_abs: &AbsNormPathBuf,
    module_name: &str,
    module_version: &str,
    override_path: &str,
) -> slug_error::Result<(CellRootPathBuf, Option<BzlmodExternalModuleSymlink>)> {
    let module_dir = resolve_local_override_module_dir(project_root_abs, override_path)?;
    if let Some(project_path) =
        project_relative_path_for_abs_path(project_root, module_dir.as_path())
    {
        return Ok((CellRootPathBuf::new(project_path), None));
    }

    let canonical_repo = bazel_canonical_module_repo_name(module_name, module_version);
    let external_path = format!("bazel-external/{canonical_repo}");
    let source_path = module_dir
        .as_path()
        .canonicalize()
        .unwrap_or_else(|_| module_dir.as_path().to_path_buf());
    Ok((
        CellRootPathBuf::new(ProjectRelativePath::new(&external_path)?.to_owned()),
        Some(BzlmodExternalModuleSymlink {
            entry_name: canonical_repo,
            source_path,
        }),
    ))
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodPendingRepoCell {
    canonical_name: String,
    extension_id: String,
    internal_name: String,
    spec_hash: String,
    repo_spec_json: String,
    path: String,
}

impl BzlmodPendingRepoCell {
    fn from_pending(cell: &slug_bzlmod::PendingRepoCell) -> Self {
        Self {
            canonical_name: cell.canonical_name.clone(),
            extension_id: cell.extension_id.clone(),
            internal_name: cell.internal_name.clone(),
            spec_hash: cell.spec_hash.clone(),
            repo_spec_json: cell.repo_spec_json.clone(),
            path: cell.path.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodScopedRepoAlias {
    owner_module: String,
    apparent_name: String,
    target_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodDynamicAlias {
    apparent_name: String,
    canonical_name: String,
}

fn dynamic_alias_for_generated_override(
    alias: &slug_bzlmod::RepoAlias,
    target_name: &str,
) -> Option<BzlmodDynamicAlias> {
    if alias.declaring_module.is_none()
        && alias.apparent_name != target_name
        && slug_bzlmod::parse_canonical_name(&alias.apparent_name).is_some()
    {
        Some(BzlmodDynamicAlias {
            apparent_name: alias.apparent_name.clone(),
            canonical_name: target_name.to_owned(),
        })
    } else {
        None
    }
}

fn replay_bzlmod_runtime_state(
    cell_graph: &slug_bzlmod::BzlmodCellGraphValue,
    project_root: &ProjectRoot,
) {
    slug_core::cells::reset_dynamic_bzlmod_state_for_project_root(
        project_root.root().to_path_buf(),
    );

    let external_base_dir = project_root.root().as_path().join("bazel-external");
    let buck_out_external_cells_dir = project_root
        .root()
        .as_path()
        .join("buck-out/v2/external_cells/bzlmod");
    let mut valid_symlink_names = std::collections::HashSet::new();
    for symlink in cell_graph.module_symlinks.iter() {
        valid_symlink_names.insert(symlink.entry_name.clone());
        let link_path = external_base_dir.join(&symlink.entry_name);
        if let Err(e) = ensure_symlink(&link_path, &symlink.source_path) {
            tracing::warn!(
                "Failed to create symlink for bzlmod module '{}': {}",
                symlink.entry_name,
                e
            );
        }
        let buck_out_link = buck_out_external_cells_dir.join(&symlink.entry_name);
        if let Err(e) = ensure_symlink(&buck_out_link, &symlink.source_path) {
            tracing::warn!(
                "Failed to create external_cells symlink for bzlmod module '{}': {}",
                symlink.entry_name,
                e
            );
        }
    }
    cleanup_stale_symlinks(&external_base_dir, &valid_symlink_names);
    cleanup_stale_symlinks(&buck_out_external_cells_dir, &valid_symlink_names);

    let runtime_cell_snapshot = runtime_cell_install_snapshot(cell_graph);
    slug_core::cells::install_bzlmod_runtime_cell_snapshot(&runtime_cell_snapshot);

    let cell_pairs: Vec<(String, String)> = cell_graph
        .cells
        .iter()
        .map(|cell| (cell.name.clone(), cell.path.clone()))
        .chain(
            cell_graph
                .extension_cells
                .iter()
                .filter(|cell| !cell.lazy)
                .map(|cell| (cell.canonical_name.clone(), cell.path.clone())),
        )
        .collect();
    slug_core::cells::ensure_external_symlinks_for_cells(&cell_pairs);
    for alias in cell_graph.root_aliases.iter() {
        let alias_str = alias.apparent_name.as_str();
        if let Some(cell) = cell_graph
            .cells
            .iter()
            .find(|cell| cell.name == alias.target_name)
        {
            slug_core::cells::ensure_external_symlink(alias_str, cell.path.as_str());
        } else if let Some(cell) = cell_graph
            .extension_cells
            .iter()
            .filter(|cell| !cell.lazy)
            .find(|cell| cell.canonical_name == alias.target_name)
        {
            slug_core::cells::ensure_external_symlink(alias_str, cell.path.as_str());
        }
    }
    slug_core::cells::repair_external_symlink_targets(project_root.root().as_path());
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct BzlmodResolutionOptions {
    lockfile_mode: slug_bzlmod::LockfileMode,
    ignore_dev_dependency: bool,
    allow_yanked_versions_env: Option<String>,
    allow_yanked_versions_flags: Vec<String>,
    hidden_lockfile_path: Option<PathBuf>,
    repo_env: BTreeMap<String, String>,
    repo_env_digest: String,
}

const BZLMOD_BAZEL_RELEASE_ID: &str = "bazel-9.0.1";
const BZLMOD_STARLARK_SEMANTICS_DIGEST: &str = "slug-bazel9-starlark-semantics-v1";
const BZLMOD_DEFAULT_REGISTRY_CONFIG_DIGEST: &str = "default-registry-config";
const BZLMOD_DEFAULT_REPOSITORY_CACHE_CONFIG_DIGEST: &str = "default-repository-cache-config";
const BZLMOD_DEFAULT_NETWORK_POLICY_DIGEST: &str = "default-network-policy";
const BZLMOD_DEFAULT_NONSTRICT_REPO_ENV_DIGEST: &str = "empty-nonstrict-repo-env";
const BZLMOD_DEFAULT_BAZEL_COMPATIBILITY_POLICY_DIGEST: &str = "default-bazel-compatibility-policy";

impl BzlmodResolutionOptions {
    fn from_config(root_config: &LegacyBuckConfig) -> slug_error::Result<Self> {
        let bzlmod_section = root_config.get_section("bzlmod");
        let repo_env = bzlmod_section
            .and_then(|section| section.get("repo_env_json"))
            .map(|value| serde_json::from_str::<BTreeMap<String, String>>(value.as_str()))
            .transpose()
            .map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Invalid bzlmod.repo_env_json value: {}",
                    e
                )
            })?
            .unwrap_or_default();
        let repo_env_digest = slug_bzlmod::repo_env_policy_digest(&repo_env);
        Ok(Self {
            lockfile_mode: BuckConfigBasedCells::bzlmod_lockfile_mode_from_config(root_config)?,
            ignore_dev_dependency: bzlmod_section
                .and_then(|section| section.get("ignore_dev_dependency"))
                .map(|value| parse_bzlmod_bool("ignore_dev_dependency", value.as_str()))
                .transpose()?
                .unwrap_or(false),
            allow_yanked_versions_env: bzlmod_section
                .and_then(|section| section.get("allow_yanked_versions_env"))
                .map(|value| value.as_str().to_owned()),
            allow_yanked_versions_flags: bzlmod_section
                .and_then(|section| section.get("allow_yanked_versions"))
                .map(|value| vec![value.as_str().to_owned()])
                .unwrap_or_default(),
            hidden_lockfile_path: bzlmod_section
                .and_then(|section| section.get("hidden_lockfile_path"))
                .map(|value| PathBuf::from(value.as_str())),
            repo_env,
            repo_env_digest,
        })
    }

    fn policy_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", self.lockfile_mode).as_bytes());
        hasher.update([0]);
        hasher.update([u8::from(self.ignore_dev_dependency)]);
        hasher.update([0]);
        if let Some(value) = &self.allow_yanked_versions_env {
            hasher.update(value.as_bytes());
        }
        hasher.update([0]);
        for value in &self.allow_yanked_versions_flags {
            hasher.update(value.as_bytes());
            hasher.update([0]);
        }
        hasher.update(self.repo_env_digest.as_bytes());
        hasher.update([0]);
        if let Some(value) = &self.hidden_lockfile_path {
            hasher.update(value.to_string_lossy().as_bytes());
        }
        hasher.update([0]);
        hex::encode(hasher.finalize())
    }

    fn command_policy_key(
        &self,
        workspace_id: slug_bzlmod::WorkspaceId,
    ) -> slug_bzlmod::BzlmodCommandPolicyKey {
        slug_bzlmod::BzlmodCommandPolicyKey {
            workspace_id,
            bazel_release_id: Arc::from(BZLMOD_BAZEL_RELEASE_ID),
            starlark_semantics_digest: Arc::from(BZLMOD_STARLARK_SEMANTICS_DIGEST),
            bzlmod_flags_digest: Arc::from(self.policy_digest().as_str()),
            lockfile_mode: Arc::from(format!("{:?}", self.lockfile_mode).as_str()),
            registry_config_digest: Arc::from(BZLMOD_DEFAULT_REGISTRY_CONFIG_DIGEST),
            repository_cache_config_digest: Arc::from(
                BZLMOD_DEFAULT_REPOSITORY_CACHE_CONFIG_DIGEST,
            ),
            network_policy_digest: Arc::from(BZLMOD_DEFAULT_NETWORK_POLICY_DIGEST),
            repo_env_digest: Arc::from(self.repo_env_digest.as_str()),
            nonstrict_repo_env_digest: Arc::from(BZLMOD_DEFAULT_NONSTRICT_REPO_ENV_DIGEST),
            ignore_dev_dependency: self.ignore_dev_dependency,
            allow_yanked_versions_digest: Arc::from(
                allow_yanked_versions_digest(
                    self.allow_yanked_versions_env.as_deref(),
                    &self.allow_yanked_versions_flags,
                )
                .as_str(),
            ),
            bazel_compatibility_policy_digest: Arc::from(
                BZLMOD_DEFAULT_BAZEL_COMPATIBILITY_POLICY_DIGEST,
            ),
            isolated_extension_usages: false,
        }
    }
}

fn parse_bzlmod_bool(key: &str, value: &str) -> slug_error::Result<bool> {
    match value {
        "1" | "true" | "True" | "yes" => Ok(true),
        "0" | "false" | "False" | "no" | "" => Ok(false),
        _ => Err(slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Invalid bzlmod.{key} value `{value}`; expected true or false"
        )),
    }
}

fn allow_yanked_versions_digest(from_env: Option<&str>, from_flags: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"allow-yanked-versions-policy-v1");
    hasher.update([0]);
    if let Some(value) = from_env {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update([0]);
    for value in from_flags {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("TrackedRootModuleFileKey({})", project_root.display())]
struct TrackedRootModuleFileKey {
    project_root: AbsNormPathBuf,
}

#[async_trait]
impl Key for TrackedRootModuleFileKey {
    type Value = slug_error::Result<Arc<slug_bzlmod::RootModuleFileValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let root_path = Arc::new(self.project_root.as_path().join("MODULE.bazel"));
        let root_project_path = ProjectRelativePath::new("MODULE.bazel")?;
        let Some(content) =
            DiceFileComputations::read_project_file_if_exists(ctx, root_project_path).await?
        else {
            return Ok(Arc::new(slug_bzlmod::RootModuleFileValue {
                path: root_path,
                input_digest: None,
                input_count: 0,
                parsed: None,
            }));
        };

        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        let parsed_with_inputs = parse_module_with_tracked_project_includes(
            ctx,
            &project_fs,
            root_path.as_ref(),
            content,
        )
        .await?;
        let input_digest = slug_bzlmod::module_file_inputs_digest(&parsed_with_inputs.inputs);
        let input_count = parsed_with_inputs.inputs.len();

        Ok(Arc::new(slug_bzlmod::RootModuleFileValue {
            path: root_path,
            input_digest: Some(input_digest),
            input_count,
            parsed: Some(parsed_with_inputs.parsed),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.path == y.path && x.input_digest == y.input_digest,
            _ => false,
        }
    }
}

async fn parse_module_with_tracked_project_includes(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    module_path: &Path,
    module_content: String,
) -> slug_error::Result<slug_bzlmod::ParsedModuleFileWithInputs> {
    let module_root = module_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let mut session = slug_bzlmod::ModuleFileParseSession::new(module_root.clone());
    let module_digest = slug_bzlmod::lockfile::compute_sha256_hex(module_content.as_bytes());
    let include_labels = session.eval_segment(module_path, &module_content, module_digest)?;
    let mut pending = Vec::new();
    push_pending_include_labels(&mut pending, include_labels, Vec::new());

    while let Some((label, ancestors)) = pending.pop() {
        let include_path = slug_bzlmod::include_label_to_path(&module_root, &label)?;
        let canonical = include_path
            .canonicalize()
            .unwrap_or_else(|_| include_path.clone());
        if ancestors.contains(&canonical) {
            return Err(slug_bzlmod::parser::ModuleParseError::IncludeError(format!(
                "cyclic include of {}",
                label
            ))
            .into());
        }

        let (include_read, _tracking) =
            read_bzlmod_file_for_module_inputs(ctx, project_fs, &include_path).await?;
        let Some((include_content, include_digest)) = include_read else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to read included MODULE.bazel segment at {:?}: file not found",
                include_path
            ));
        };
        let nested_labels =
            session.eval_segment(&include_path, &include_content, include_digest)?;
        let mut nested_ancestors = ancestors;
        nested_ancestors.push(canonical);
        push_pending_include_labels(&mut pending, nested_labels, nested_ancestors);
    }

    session.finish()
}

fn parse_module_with_polled_includes(
    module_path: &Path,
    module_content: String,
) -> slug_error::Result<slug_bzlmod::ParsedModuleFileWithInputs> {
    let module_root = module_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let mut session = slug_bzlmod::ModuleFileParseSession::new_silent(module_root.clone());
    let module_digest = slug_bzlmod::lockfile::compute_sha256_hex(module_content.as_bytes());
    let include_labels = session.eval_segment(module_path, &module_content, module_digest)?;
    let mut pending = Vec::new();
    push_pending_include_labels(&mut pending, include_labels, Vec::new());

    while let Some((label, ancestors)) = pending.pop() {
        let include_path = slug_bzlmod::include_label_to_path(&module_root, &label)?;
        let canonical = include_path
            .canonicalize()
            .unwrap_or_else(|_| include_path.clone());
        if ancestors.contains(&canonical) {
            return Err(slug_bzlmod::parser::ModuleParseError::IncludeError(format!(
                "cyclic include of {}",
                label
            ))
            .into());
        }

        let (include_content, include_digest) = read_absolute_text_file_input(&include_path)?;
        let Some((include_content, include_digest)) = include_content.zip(include_digest) else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to read included MODULE.bazel segment at {:?}: file not found",
                include_path
            ));
        };
        let nested_labels =
            session.eval_segment(&include_path, &include_content, include_digest)?;
        let mut nested_ancestors = ancestors;
        nested_ancestors.push(canonical);
        push_pending_include_labels(&mut pending, nested_labels, nested_ancestors);
    }

    session.finish()
}

fn push_pending_include_labels(
    pending: &mut Vec<(String, Vec<PathBuf>)>,
    include_labels: Vec<String>,
    ancestors: Vec<PathBuf>,
) {
    for label in include_labels.into_iter().rev() {
        pending.push((label, ancestors.clone()));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
enum BzlmodFileInputTracking {
    Project,
    Polled,
}

async fn read_bzlmod_file_for_module_inputs(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    path: &Path,
) -> slug_error::Result<(Option<(String, String)>, BzlmodFileInputTracking)> {
    if let Some(project_path) = project_relative_path_for_abs_path(project_fs, path) {
        let Some(content) =
            DiceFileComputations::read_project_file_if_exists(ctx, &project_path).await?
        else {
            return Ok((None, BzlmodFileInputTracking::Project));
        };
        let digest = slug_bzlmod::lockfile::compute_sha256_hex(content.as_bytes());
        return Ok((Some((content, digest)), BzlmodFileInputTracking::Project));
    }

    let (content, digest) = read_absolute_text_file_input(path)?;
    Ok((
        content
            .zip(digest)
            .map(|(content, digest)| (content, digest)),
        BzlmodFileInputTracking::Polled,
    ))
}

fn project_relative_path_for_abs_path(
    project_fs: &ProjectRoot,
    path: &Path,
) -> Option<ProjectRelativePathBuf> {
    let path = AbsPath::new(path).ok()?;
    project_fs.relativize_any(path).ok()
}

async fn read_text_file_for_project_input(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    path: &Path,
) -> slug_error::Result<(Option<String>, BzlmodFileInputTracking)> {
    if let Some(project_path) = project_relative_path_for_abs_path(project_fs, path) {
        return Ok((
            DiceFileComputations::read_project_file_if_exists(ctx, &project_path).await?,
            BzlmodFileInputTracking::Project,
        ));
    }

    let (content, _digest) = read_absolute_text_file_input(path)?;
    Ok((content, BzlmodFileInputTracking::Polled))
}

fn read_absolute_text_file_input(
    path: &Path,
) -> slug_error::Result<(Option<String>, Option<String>)> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(e) => return Err(e.into()),
    };
    let digest = slug_bzlmod::lockfile::compute_sha256_hex(&bytes);
    let content = String::from_utf8(bytes).map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Failed to read MODULE.bazel-like file at {:?} as UTF-8: {}",
            path,
            e
        )
    })?;
    Ok((Some(content), Some(digest)))
}

fn absolute_text_file_digest(path: &Path) -> slug_error::Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(slug_bzlmod::lockfile::compute_sha256_hex(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn text_file_poll_digest(
    project_fs: &ProjectRoot,
    path: &Path,
    domain: &[u8],
) -> slug_error::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update([0]);

    if project_relative_path_for_abs_path(project_fs, path).is_some() {
        hasher.update(b"project-tracked");
        hasher.update([0]);
        return Ok(hex::encode(hasher.finalize()));
    }

    match absolute_text_file_digest(path)? {
        Some(digest) => {
            hasher.update(b"present");
            hasher.update([0]);
            hasher.update(digest.as_bytes());
        }
        None => hasher.update(b"missing"),
    }
    hasher.update([0]);
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Clone, Debug, Display, Allocative)]
#[display(
    "LegacyBzlmodResolutionDiceKey({}, {})",
    project_root.display(),
    resolution_key.command_policy_digest
)]
struct LegacyBzlmodResolutionDiceKey {
    project_root: AbsNormPathBuf,
    resolution_key: slug_bzlmod::BzlmodResolutionKey,
    options: BzlmodResolutionOptions,
    root_module_file: Arc<slug_bzlmod::RootModuleFileValue>,
    visible_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
    hidden_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
    local_override_inputs: Arc<LocalOverrideModuleInputsValue>,
    non_registry_override_inputs: Arc<NonRegistryOverrideModuleInputsValue>,
    registry_file_inputs: Arc<RegistryFileInputsValue>,
    extension_replay_summary_digest: Option<Arc<str>>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("TrackedLockfileContentKey({:?}, {})", kind, path.display())]
struct TrackedLockfileContentKey {
    project_root: AbsNormPathBuf,
    kind: slug_bzlmod::LockfileContentKind,
    path: Arc<PathBuf>,
    poll_digest: String,
}

#[async_trait]
impl Key for TrackedLockfileContentKey {
    type Value = slug_error::Result<Arc<slug_bzlmod::LockfileContentValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        let (content, _tracking) =
            read_text_file_for_project_input(ctx, &project_fs, &self.path).await?;
        // Project-root lockfiles are tracked by DICE file deps. Hidden/output-base
        // lockfiles are outside the project root, so their current bytes are part of
        // this key's poll digest.
        let tracked_by_dice = true;
        let path = self.path.clone();
        let Some(content) = content else {
            return Ok(Arc::new(slug_bzlmod::LockfileContentValue {
                path,
                digest: None,
                tracked_by_dice,
                lockfile: None,
            }));
        };

        record_bzlmod_event(BzlmodEventKind::LockfileRead, path.display().to_string());
        let digest = slug_bzlmod::lockfile::compute_sri_hash(content.as_bytes());
        match slug_bzlmod::lockfile::parse_lockfile_content(&path, &content) {
            Ok(lockfile) => Ok(Arc::new(slug_bzlmod::LockfileContentValue {
                path,
                digest: Some(digest),
                tracked_by_dice,
                lockfile: Some(Arc::new(lockfile)),
            })),
            Err(e) if self.kind == slug_bzlmod::LockfileContentKind::Hidden => {
                tracing::warn!(
                    "Ignoring unreadable hidden lockfile '{}': {}",
                    path.display(),
                    e
                );
                Ok(Arc::new(slug_bzlmod::LockfileContentValue {
                    path,
                    digest: None,
                    tracked_by_dice,
                    lockfile: None,
                }))
            }
            Err(e) => Err(e),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.digest == y.digest && x.path == y.path,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => value.tracked_by_dice,
            Err(_) => false,
        }
    }
}

impl PartialEq for LegacyBzlmodResolutionDiceKey {
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.resolution_key == other.resolution_key
            && bzlmod_resolution_options_policy_eq(&self.options, &other.options)
            && self.root_module_file.path == other.root_module_file.path
            && self.root_module_file.input_digest == other.root_module_file.input_digest
            && lockfile_content_identity_eq(&self.visible_lockfile, &other.visible_lockfile)
            && lockfile_content_identity_eq(&self.hidden_lockfile, &other.hidden_lockfile)
            && self.local_override_inputs.digest == other.local_override_inputs.digest
            && self.non_registry_override_inputs.digest == other.non_registry_override_inputs.digest
            && self.registry_file_inputs.digest == other.registry_file_inputs.digest
            && self.extension_replay_summary_digest == other.extension_replay_summary_digest
    }
}

impl Eq for LegacyBzlmodResolutionDiceKey {}

impl std::hash::Hash for LegacyBzlmodResolutionDiceKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.project_root.hash(state);
        self.resolution_key.hash(state);
        hash_bzlmod_resolution_options_policy(&self.options, state);
        self.root_module_file.path.hash(state);
        self.root_module_file.input_digest.hash(state);
        hash_lockfile_content_identity(&self.visible_lockfile, state);
        hash_lockfile_content_identity(&self.hidden_lockfile, state);
        self.local_override_inputs.digest.hash(state);
        self.non_registry_override_inputs.digest.hash(state);
        self.registry_file_inputs.digest.hash(state);
        self.extension_replay_summary_digest.hash(state);
    }
}

fn lockfile_content_identity_eq(
    left: &Option<Arc<slug_bzlmod::LockfileContentValue>>,
    right: &Option<Arc<slug_bzlmod::LockfileContentValue>>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => match (&left.digest, &right.digest) {
            (Some(_), Some(_)) => left.path == right.path && left.digest == right.digest,
            (None, None) => true,
            _ => false,
        },
        (None, None) => true,
        _ => false,
    }
}

fn hash_lockfile_content_identity<H: std::hash::Hasher>(
    value: &Option<Arc<slug_bzlmod::LockfileContentValue>>,
    state: &mut H,
) {
    match value {
        Some(value) => {
            true.hash(state);
            if value.digest.is_some() {
                value.path.hash(state);
            }
            value.digest.hash(state);
        }
        None => false.hash(state),
    }
}

fn bzlmod_resolution_options_policy_eq(
    left: &BzlmodResolutionOptions,
    right: &BzlmodResolutionOptions,
) -> bool {
    left.lockfile_mode == right.lockfile_mode
        && left.ignore_dev_dependency == right.ignore_dev_dependency
        && left.allow_yanked_versions_env == right.allow_yanked_versions_env
        && left.allow_yanked_versions_flags == right.allow_yanked_versions_flags
        && left.repo_env == right.repo_env
        && left.repo_env_digest == right.repo_env_digest
        && left.hidden_lockfile_path == right.hidden_lockfile_path
}

fn hash_bzlmod_resolution_options_policy<H: std::hash::Hasher>(
    value: &BzlmodResolutionOptions,
    state: &mut H,
) {
    value.lockfile_mode.hash(state);
    value.ignore_dev_dependency.hash(state);
    value.allow_yanked_versions_env.hash(state);
    value.allow_yanked_versions_flags.hash(state);
    value.repo_env.hash(state);
    value.repo_env_digest.hash(state);
    value.hidden_lockfile_path.hash(state);
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("TrackedExtensionBzlDigestKey({}, {})", project_root.display(), extension_id)]
struct TrackedExtensionBzlDigestKey {
    project_root: AbsNormPathBuf,
    extension_id: Arc<str>,
    repo_mappings: Arc<slug_bzlmod::RepoMappingSnapshot>,
}

#[async_trait]
impl Key for TrackedExtensionBzlDigestKey {
    type Value = slug_error::Result<Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        let digest = tracked_extension_bzl_digest_for_project(
            ctx,
            &project_fs,
            &self.extension_id,
            self.repo_mappings.as_ref(),
        )
        .await?;
        Ok(Arc::from(digest.as_str()))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        // This producer now reads project-local implementation files through
        // DICE, but it still shares the transitional literal-load scanner and
        // can see missing-file creations that are not yet a stable child-key
        // cutoff boundary. Recompute it each transaction so replay summary
        // hits are counted only after the digest is refreshed.
        false
    }
}

#[derive(Clone, Debug)]
enum BzlDigestFileInput {
    Present {
        bytes: Vec<u8>,
        text: Option<String>,
    },
    Missing {
        error: String,
    },
    ReadError {
        error: String,
    },
}

impl BzlDigestFileInput {
    fn bytes_for_hash(&self) -> Option<&[u8]> {
        match self {
            Self::Present { bytes, .. } => Some(bytes.as_slice()),
            Self::Missing { .. } | Self::ReadError { .. } => None,
        }
    }

    fn text_for_loads(&self) -> Option<&str> {
        match self {
            Self::Present {
                text: Some(text), ..
            } => Some(text),
            Self::Present { text: None, .. } | Self::Missing { .. } | Self::ReadError { .. } => {
                None
            }
        }
    }

    fn error_for_hash(&self) -> Option<&str> {
        match self {
            Self::Present { .. } => None,
            Self::Missing { error } | Self::ReadError { error } => Some(error.as_str()),
        }
    }
}

async fn tracked_extension_bzl_digest_for_project(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    extension_id: &str,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
) -> slug_error::Result<String> {
    let Some(root_bzl) = slug_bzlmod::extension_bzl_location_under_project(
        extension_id,
        project_fs.root().as_path(),
        Some(repo_mappings),
    ) else {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    };

    if !matches!(
        read_bzl_digest_file_input(ctx, project_fs, &root_bzl.path).await?,
        BzlDigestFileInput::Present { .. }
    ) {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    }

    let mut pending = VecDeque::from([root_bzl]);
    let mut seen_locations = BTreeSet::new();
    let mut file_inputs = BTreeMap::new();
    while let Some(location) = pending.pop_front() {
        let location_key = (
            location.path.clone(),
            location.repo.clone(),
            location.package.clone(),
        );
        if !seen_locations.insert(location_key) {
            continue;
        }

        let input = read_bzl_digest_file_input(ctx, project_fs, &location.path).await?;
        let text = input.text_for_loads().map(str::to_owned);
        file_inputs.insert(location.path.clone(), input);
        let Some(text) = text else {
            continue;
        };

        for load in slug_bzlmod::literal_loads(&location.path, &text) {
            let Some(load_location) = slug_bzlmod::label_bzl_location_under_project(
                &load,
                project_fs.root().as_path(),
                Some(&location),
                Some(repo_mappings),
            ) else {
                continue;
            };
            if load_location.path.starts_with(project_fs.root().as_path()) {
                pending.push_back(load_location);
            }
        }
    }

    if file_inputs.is_empty() {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    }

    Ok(hash_extension_bzl_digest_inputs(
        extension_id,
        project_fs.root().as_path(),
        file_inputs,
    ))
}

async fn read_bzl_digest_file_input(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    path: &Path,
) -> slug_error::Result<BzlDigestFileInput> {
    if let Some(project_path) = project_relative_path_for_abs_path(project_fs, path) {
        return Ok(
            match DiceFileComputations::read_project_file_if_exists(ctx, &project_path).await {
                Ok(Some(content)) => BzlDigestFileInput::Present {
                    bytes: content.as_bytes().to_vec(),
                    text: Some(content),
                },
                Ok(None) => BzlDigestFileInput::Missing {
                    error: std_fs_read_error(path),
                },
                Err(e) => BzlDigestFileInput::ReadError {
                    error: e.to_string(),
                },
            },
        );
    }

    Ok(match std::fs::read(path) {
        Ok(bytes) => BzlDigestFileInput::Present {
            text: String::from_utf8(bytes.clone()).ok(),
            bytes,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BzlDigestFileInput::Missing {
            error: e.to_string(),
        },
        Err(e) => BzlDigestFileInput::ReadError {
            error: e.to_string(),
        },
    })
}

fn std_fs_read_error(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(_) => std::io::Error::from(std::io::ErrorKind::NotFound).to_string(),
        Err(e) => e.to_string(),
    }
}

fn hash_extension_bzl_digest_inputs(
    extension_id: &str,
    project_root: &Path,
    file_inputs: BTreeMap<PathBuf, BzlDigestFileInput>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v2:");
    hasher.update(extension_id.as_bytes());
    hasher.update([0]);
    for (path, input) in file_inputs {
        let rel = path.strip_prefix(project_root).unwrap_or(path.as_path());
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        if let Some(bytes) = input.bytes_for_hash() {
            hasher.update(bytes);
        } else if let Some(error) = input.error_for_hash() {
            hasher.update(b"read_error:");
            hasher.update(error.as_bytes());
        }
        hasher.update([0]);
    }

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

async fn root_extension_replay_summary_digest(
    ctx: &mut DiceComputations<'_>,
    parsed: &ParsedModuleFile,
    project_fs: &ProjectRoot,
    visible_lockfile: Option<&slug_bzlmod::Lockfile>,
    hidden_lockfile: Option<&slug_bzlmod::Lockfile>,
    repo_env: &BTreeMap<String, String>,
    ignore_dev_dependency: bool,
) -> slug_error::Result<Option<String>> {
    if parsed.extension_usages.is_empty()
        || !parsed.module.bazel_deps.is_empty()
        || !parsed.module.overrides.is_empty()
        || !parsed.repo_rule_invocations.is_empty()
    {
        return Ok(None);
    }

    let root_module_name = if parsed.module.name.is_empty() {
        "_main"
    } else {
        &parsed.module.name
    };
    let parsed_modules = vec![(root_module_name.to_owned(), parsed.clone())];
    let mut module_extensions = HashMap::new();
    module_extensions.insert(root_module_name.to_owned(), parsed.extension_usages.clone());
    let aggregated = slug_bzlmod::aggregate_extensions_with_policy(
        &module_extensions,
        Some(root_module_name),
        ignore_dev_dependency,
    );
    if aggregated.is_empty() {
        return Ok(None);
    }

    let repo_mappings = repo_mapping_snapshot_for_modules(&parsed_modules, root_module_name);
    let repo_mappings = Arc::new(repo_mappings);
    let repo_mapping_overrides =
        repo_mapping_overrides_for_root(&parsed_modules, root_module_name, ignore_dev_dependency);

    let mut hasher = Sha256::new();
    hasher.update(b"root-extension-replay-summary-v1");
    hasher.update([0]);
    for (name, value) in repo_env {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    for (extension_id, overrides) in &repo_mapping_overrides {
        hasher.update(extension_id.as_bytes());
        hasher.update([0]);
        for (generated_name, target_name) in overrides {
            hasher.update(generated_name.as_bytes());
            hasher.update([0]);
            hasher.update(target_name.as_bytes());
            hasher.update([0]);
        }
    }

    let mut extension_ids = aggregated.keys().cloned().collect::<Vec<_>>();
    extension_ids.sort();
    for extension_id in extension_ids {
        let Some(extension) = aggregated.get(&extension_id) else {
            return Ok(None);
        };
        let bzl_transitive_digest = ctx
            .compute(&TrackedExtensionBzlDigestKey {
                project_root: AbsNormPathBuf::try_from(project_fs.root().as_path().to_path_buf())?,
                extension_id: Arc::from(extension_id.as_str()),
                repo_mappings: repo_mappings.clone(),
            })
            .await??;
        let usages_digest = slug_bzlmod::compute_extension_input_hash(extension);
        let visible_specs = visible_lockfile.and_then(|lockfile| {
            lockfile.get_extension_cache_for_workspace(
                &extension_id,
                &bzl_transitive_digest,
                &usages_digest,
                Some(project_fs.root().as_path()),
                Some(repo_env),
                Some(repo_mappings.as_ref()),
                Some(root_module_name),
                Some(&repo_mapping_overrides),
            )
        });
        let (source, cached_specs) = if let Some(cached_specs) = visible_specs {
            ("visible", cached_specs)
        } else if let Some(cached_specs) = hidden_lockfile.and_then(|lockfile| {
            lockfile.get_extension_cache_for_workspace(
                &extension_id,
                &bzl_transitive_digest,
                &usages_digest,
                Some(project_fs.root().as_path()),
                Some(repo_env),
                Some(repo_mappings.as_ref()),
                Some(root_module_name),
                Some(&repo_mapping_overrides),
            )
        }) {
            ("hidden", cached_specs)
        } else {
            return Ok(None);
        };

        hasher.update(extension_id.as_bytes());
        hasher.update([0]);
        hasher.update(bzl_transitive_digest.as_bytes());
        hasher.update([0]);
        hasher.update(usages_digest.as_bytes());
        hasher.update([0]);
        hasher.update(source.as_bytes());
        hasher.update([0]);
        let mut repo_names = cached_specs.keys().cloned().collect::<Vec<_>>();
        repo_names.sort();
        for repo_name in repo_names {
            let Some(spec) = cached_specs.get(&repo_name) else {
                return Ok(None);
            };
            hasher.update(repo_name.as_bytes());
            hasher.update([0]);
            hasher.update(spec.compute_hash().as_bytes());
            hasher.update([0]);
        }
    }

    Ok(Some(hex::encode(hasher.finalize())))
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("LocalOverrideModuleInputsKey({})", project_root.display())]
struct LocalOverrideModuleInputsKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, String)>,
    poll_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct LocalOverrideModuleInputsValue {
    digest: String,
    has_bazel_deps: bool,
    has_extension_usages: bool,
    has_repo_rule_invocations: bool,
    has_git_overrides: bool,
    has_untracked_inputs: bool,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRegistryOverrideModuleInputsKey({})", project_root.display())]
struct NonRegistryOverrideModuleInputsKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, PathBuf)>,
    poll_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct NonRegistryOverrideModuleInputsValue {
    digest: String,
    has_inputs: bool,
    has_untracked_inputs: bool,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("RegistryFileInputsKey")]
struct RegistryFileInputsKey {
    project_root: AbsNormPathBuf,
    registry_file_hashes: Vec<(String, String)>,
    poll_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct RegistryFileInputsValue {
    digest: String,
    has_inputs: bool,
    cache_safe: bool,
    has_untracked_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct NonRootModuleFileInput {
    module_key: String,
    module_bazel_path: PathBuf,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRootModuleFilesKey({}, {})", project_root.display(), inputs.len())]
struct NonRootModuleFilesKey {
    project_root: AbsNormPathBuf,
    inputs: Vec<NonRootModuleFileInput>,
    poll_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct NonRootModuleFilesValue {
    digest: String,
    parsed_modules: Vec<(String, ParsedModuleFile)>,
    has_untracked_inputs: bool,
}

fn local_overrides_from_root_module(
    root_module_file: &slug_bzlmod::RootModuleFileValue,
    ignore_dev_dependency: bool,
) -> Vec<(String, String)> {
    root_module_file
        .parsed
        .as_ref()
        .map(|parsed| {
            active_root_overrides(&parsed.module, ignore_dev_dependency)
                .iter()
                .filter_map(|override_| match override_ {
                    slug_bzlmod::types::Override::LocalPath(local) => {
                        Some((local.module_name.clone(), local.path.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn active_root_overrides(
    module: &slug_bzlmod::types::Module,
    ignore_dev_dependency: bool,
) -> Vec<slug_bzlmod::types::Override> {
    if !ignore_dev_dependency {
        return module.overrides.clone();
    }

    let ignored_root_dev_deps: HashSet<_> = module
        .bazel_deps
        .iter()
        .filter(|dep| dep.dev_dependency)
        .map(|dep| dep.name.clone())
        .collect();
    module
        .overrides
        .iter()
        .filter(|override_| match override_ {
            slug_bzlmod::types::Override::LocalPath(local) => {
                !ignored_root_dev_deps.contains(&local.module_name)
            }
            slug_bzlmod::types::Override::Git(git) => {
                !ignored_root_dev_deps.contains(&git.module_name)
            }
            slug_bzlmod::types::Override::Archive(archive) => {
                !ignored_root_dev_deps.contains(&archive.module_name)
            }
            _ => true,
        })
        .cloned()
        .collect()
}

fn non_registry_override_module_dirs_from_root_module(
    root_module_file: &slug_bzlmod::RootModuleFileValue,
    ignore_dev_dependency: bool,
) -> slug_error::Result<Vec<(String, PathBuf)>> {
    let Some(parsed) = &root_module_file.parsed else {
        return Ok(Vec::new());
    };
    let active_overrides = active_root_overrides(&parsed.module, ignore_dev_dependency);
    if !active_overrides.iter().any(|override_| {
        matches!(
            override_,
            slug_bzlmod::types::Override::Git(_) | slug_bzlmod::types::Override::Archive(_)
        )
    }) {
        return Ok(Vec::new());
    }
    let cache = ModuleCache::new()?;
    Ok(active_overrides
        .iter()
        .filter_map(|override_| match override_ {
            slug_bzlmod::types::Override::Git(git) => {
                Some((git.module_name.clone(), cache.git_override_dir(git)))
            }
            slug_bzlmod::types::Override::Archive(archive) => Some((
                archive.module_name.clone(),
                cache.archive_override_dir(archive),
            )),
            _ => None,
        })
        .collect::<Vec<_>>())
}

async fn local_override_module_inputs_digest(
    ctx: &mut DiceComputations<'_>,
    project_root: &AbsNormPathBuf,
    overrides: &[(String, String)],
) -> slug_error::Result<LocalOverrideModuleInputsValue> {
    let project_fs = ProjectRoot::new_unchecked(project_root.clone());
    let mut hasher = Sha256::new();
    hasher.update(b"local-override-module-inputs-v2");
    hasher.update([0]);
    let mut queue = VecDeque::new();
    for (module_name, path) in overrides {
        queue.push_back((module_name.clone(), project_root.clone(), path.clone()));
    }

    let mut visited_module_dirs = HashSet::new();
    let mut has_bazel_deps = false;
    let mut has_extension_usages = false;
    let mut has_repo_rule_invocations = false;
    let mut has_git_overrides = false;
    let has_untracked_inputs = false;

    while let Some((module_name, base, path)) = queue.pop_front() {
        let module_dir = resolve_local_override_module_dir(&base, &path)?;
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => module_dir.clone(),
        };
        hasher.update(module_name.as_bytes());
        hasher.update([0]);
        hasher.update(base.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(normalized_module_dir.to_string_lossy().as_bytes());
        hasher.update([0]);

        if !visited_module_dirs.insert(normalized_module_dir.clone()) {
            hasher.update(b"already-seen");
            hasher.update([0]);
            continue;
        }

        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        let (module_read, _tracking) =
            read_bzlmod_file_for_module_inputs(ctx, &project_fs, &module_bazel_path).await?;
        match module_read {
            Some((content, _content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                let parsed_with_inputs = parse_module_with_tracked_project_includes(
                    ctx,
                    &project_fs,
                    &module_bazel_path,
                    content,
                )
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for local override '{}' at {:?}",
                        module_name, module_bazel_path
                    )
                })?;
                for input in &parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }

                let parsed = parsed_with_inputs.parsed;
                has_bazel_deps |= !parsed.module.bazel_deps.is_empty();
                has_extension_usages |= !parsed.extension_usages.is_empty();
                has_repo_rule_invocations |= !parsed.repo_rule_invocations.is_empty();
                has_git_overrides |= parsed
                    .module
                    .overrides
                    .iter()
                    .any(|override_| matches!(override_, slug_bzlmod::types::Override::Git(_)));

                for override_ in &parsed.module.overrides {
                    if let slug_bzlmod::types::Override::LocalPath(local) = override_ {
                        queue.push_back((
                            local.module_name.clone(),
                            normalized_module_dir.clone(),
                            local.path.clone(),
                        ));
                    }
                }
            }
            None => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
        }
    }

    Ok(LocalOverrideModuleInputsValue {
        digest: hex::encode(hasher.finalize()),
        has_bazel_deps,
        has_extension_usages,
        has_repo_rule_invocations,
        has_git_overrides,
        has_untracked_inputs,
    })
}

fn local_override_inputs_poll_digest(
    project_fs: &ProjectRoot,
    project_root: &AbsNormPathBuf,
    overrides: &[(String, String)],
) -> slug_error::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"local-override-module-inputs-poll-v1");
    hasher.update([0]);

    let mut queue = VecDeque::new();
    for (module_name, path) in overrides {
        queue.push_back((module_name.clone(), project_root.clone(), path.clone()));
    }

    let mut visited_module_dirs = HashSet::new();
    while let Some((module_name, base, path)) = queue.pop_front() {
        let module_dir = resolve_local_override_module_dir(&base, &path)?;
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => module_dir.clone(),
        };

        hasher.update(module_name.as_bytes());
        hasher.update([0]);
        hasher.update(base.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(normalized_module_dir.to_string_lossy().as_bytes());
        hasher.update([0]);

        if project_relative_path_for_abs_path(project_fs, normalized_module_dir.as_path()).is_some()
        {
            hasher.update(b"project-tracked");
            hasher.update([0]);
            continue;
        }

        if !visited_module_dirs.insert(normalized_module_dir.clone()) {
            hasher.update(b"already-seen");
            hasher.update([0]);
            continue;
        }

        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        match read_absolute_text_file_input(&module_bazel_path)? {
            (Some(content), Some(content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);

                let parsed_with_inputs =
                    parse_module_with_polled_includes(&module_bazel_path, content)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse MODULE.bazel for local override '{}' at {:?}",
                                module_name, module_bazel_path
                            )
                        })?;
                for input in &parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }

                for override_ in &parsed_with_inputs.parsed.module.overrides {
                    if let slug_bzlmod::types::Override::LocalPath(local) = override_ {
                        queue.push_back((
                            local.module_name.clone(),
                            normalized_module_dir.clone(),
                            local.path.clone(),
                        ));
                    }
                }
            }
            (None, None) => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
            _ => unreachable!("absolute text file reads return content and digest together"),
        }
    }

    Ok(hex::encode(hasher.finalize()))
}

async fn non_registry_override_module_inputs_digest(
    ctx: &mut DiceComputations<'_>,
    project_root: &AbsNormPathBuf,
    overrides: &[(String, PathBuf)],
) -> slug_error::Result<NonRegistryOverrideModuleInputsValue> {
    let project_fs = ProjectRoot::new_unchecked(project_root.clone());
    let mut hasher = Sha256::new();
    hasher.update(b"non-registry-override-module-inputs-v1");
    hasher.update([0]);

    for (module_name, module_dir) in overrides {
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => AbsNormPathBuf::try_from(normalize_path_lexically(module_dir.clone()))?,
        };
        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        hasher.update(module_name.as_bytes());
        hasher.update([0]);
        hasher.update(normalized_module_dir.to_string_lossy().as_bytes());
        hasher.update([0]);

        let (module_read, _tracking) =
            read_bzlmod_file_for_module_inputs(ctx, &project_fs, &module_bazel_path).await?;
        match module_read {
            Some((content, _content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                let parsed_with_inputs = parse_module_with_tracked_project_includes(
                    ctx,
                    &project_fs,
                    &module_bazel_path,
                    content,
                )
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for non-registry override '{}' at {:?}",
                        module_name, module_bazel_path
                    )
                })?;
                for input in &parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }
            }
            None => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
        }
    }

    Ok(NonRegistryOverrideModuleInputsValue {
        digest: hex::encode(hasher.finalize()),
        has_inputs: !overrides.is_empty(),
        has_untracked_inputs: false,
    })
}

fn non_registry_override_inputs_poll_digest(
    project_fs: &ProjectRoot,
    overrides: &[(String, PathBuf)],
) -> slug_error::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"non-registry-override-module-inputs-poll-v1");
    hasher.update([0]);

    for (module_name, module_dir) in overrides {
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => AbsNormPathBuf::try_from(normalize_path_lexically(module_dir.clone()))?,
        };
        hasher.update(module_name.as_bytes());
        hasher.update([0]);
        hasher.update(normalized_module_dir.to_string_lossy().as_bytes());
        hasher.update([0]);

        if project_relative_path_for_abs_path(project_fs, normalized_module_dir.as_path()).is_some()
        {
            hasher.update(b"project-tracked");
            hasher.update([0]);
            continue;
        }

        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        match read_absolute_text_file_input(&module_bazel_path)? {
            (Some(content), Some(content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs = parse_module_with_polled_includes(
                    &module_bazel_path,
                    content,
                )
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for non-registry override '{}' at {:?}",
                        module_name, module_bazel_path
                    )
                })?;
                for input in &parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }
            }
            (None, None) => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
            _ => unreachable!("absolute text file reads return content and digest together"),
        }
        hasher.update([0]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn non_root_module_file_inputs(
    project_root: &ProjectRoot,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    module_symlinks: &[BzlmodExternalModuleSymlink],
) -> Vec<NonRootModuleFileInput> {
    let symlink_sources: HashMap<_, _> = module_symlinks
        .iter()
        .map(|symlink| (symlink.entry_name.as_str(), symlink.source_path.as_path()))
        .collect();
    cells
        .iter()
        .filter_map(|(cell_name, cell_path, setup)| {
            let module_bazel_path = if let Some(setup) = setup {
                if setup.source_path.is_empty() {
                    return None;
                }
                PathBuf::from(setup.source_path.as_ref()).join("MODULE.bazel")
            } else {
                let project_relative = cell_path.as_project_relative_path().as_str();
                if let Some(source_path) = project_relative
                    .strip_prefix("bazel-external/")
                    .and_then(|entry| symlink_sources.get(entry))
                {
                    source_path.join("MODULE.bazel")
                } else {
                    project_root
                        .root()
                        .as_path()
                        .join(project_relative)
                        .join("MODULE.bazel")
                }
            };
            Some(NonRootModuleFileInput {
                module_key: cell_name.as_str().to_owned(),
                module_bazel_path,
            })
        })
        .collect()
}

fn non_root_module_files_poll_digest(
    project_fs: &ProjectRoot,
    inputs: &[NonRootModuleFileInput],
) -> slug_error::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"non-root-module-files-poll-v1");
    hasher.update([0]);
    for input in inputs {
        hasher.update(input.module_key.as_bytes());
        hasher.update([0]);
        hasher.update(input.module_bazel_path.to_string_lossy().as_bytes());
        hasher.update([0]);

        if project_relative_path_for_abs_path(project_fs, &input.module_bazel_path).is_some() {
            hasher.update(b"project-tracked");
            hasher.update([0]);
            continue;
        }

        match read_absolute_text_file_input(&input.module_bazel_path)? {
            (Some(content), Some(content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs =
                    parse_module_with_polled_includes(&input.module_bazel_path, content)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse non-root MODULE.bazel for '{}' at {:?}",
                                input.module_key, input.module_bazel_path
                            )
                        })?;
                for parsed_input in &parsed_with_inputs.inputs {
                    hasher.update(parsed_input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(parsed_input.digest.as_bytes());
                    hasher.update([0]);
                }
            }
            (None, None) => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
            _ => unreachable!("absolute text file reads return content and digest together"),
        }
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn parse_non_root_module_files(
    ctx: &mut DiceComputations<'_>,
    project_root: &AbsNormPathBuf,
    inputs: &[NonRootModuleFileInput],
) -> slug_error::Result<NonRootModuleFilesValue> {
    let project_fs = ProjectRoot::new_unchecked(project_root.clone());
    let mut hasher = Sha256::new();
    hasher.update(b"non-root-module-files-v1");
    hasher.update([0]);
    let mut parsed_modules = Vec::new();

    for input in inputs {
        hasher.update(input.module_key.as_bytes());
        hasher.update([0]);
        hasher.update(input.module_bazel_path.to_string_lossy().as_bytes());
        hasher.update([0]);

        let (module_read, _tracking) =
            read_bzlmod_file_for_module_inputs(ctx, &project_fs, &input.module_bazel_path).await?;
        let Some((content, _content_digest)) = module_read else {
            hasher.update(b"missing");
            hasher.update([0]);
            continue;
        };

        hasher.update(b"present");
        hasher.update([0]);
        let parsed_with_inputs = parse_module_with_tracked_project_includes(
            ctx,
            &project_fs,
            &input.module_bazel_path,
            content,
        )
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to parse non-root MODULE.bazel for '{}' at {:?}",
                input.module_key, input.module_bazel_path
            )
        })?;
        for parsed_input in &parsed_with_inputs.inputs {
            hasher.update(parsed_input.path.to_string_lossy().as_bytes());
            hasher.update([0]);
            hasher.update(parsed_input.digest.as_bytes());
            hasher.update([0]);
        }

        let parsed = parsed_with_inputs.parsed;
        let module_key = if parsed.module.name.is_empty() {
            input.module_key.clone()
        } else {
            parsed.module.name.clone()
        };
        parsed_modules.push((module_key, parsed));
        hasher.update([0]);
    }

    Ok(NonRootModuleFilesValue {
        digest: hex::encode(hasher.finalize()),
        parsed_modules,
        has_untracked_inputs: false,
    })
}

fn parse_non_root_module_files_direct(
    inputs: &[NonRootModuleFileInput],
) -> slug_error::Result<Vec<(String, ParsedModuleFile)>> {
    let mut parsed_modules = Vec::new();
    for input in inputs {
        if !input.module_bazel_path.exists() {
            continue;
        }
        let parsed =
            parse_module_bazel(&input.module_bazel_path).with_buck_error_context(|| {
                format!(
                    "Failed to parse non-root MODULE.bazel for '{}' at {:?}",
                    input.module_key, input.module_bazel_path
                )
            })?;
        let module_key = if parsed.module.name.is_empty() {
            input.module_key.clone()
        } else {
            parsed.module.name.clone()
        };
        parsed_modules.push((module_key, parsed));
    }
    Ok(parsed_modules)
}

fn resolve_local_override_module_dir(
    base: &AbsNormPathBuf,
    path: &str,
) -> slug_error::Result<AbsNormPathBuf> {
    let path_obj = Path::new(path);
    let joined = if path_obj.is_absolute() {
        path_obj.to_path_buf()
    } else {
        base.as_path().join(path_obj)
    };
    AbsNormPathBuf::try_from(normalize_path_lexically(joined))
}

fn normalize_path_lexically(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[async_trait]
impl Key for LocalOverrideModuleInputsKey {
    type Value = slug_error::Result<Arc<LocalOverrideModuleInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        local_override_module_inputs_digest(ctx, &self.project_root, &self.overrides)
            .await
            .map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_untracked_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
impl Key for NonRegistryOverrideModuleInputsKey {
    type Value = slug_error::Result<Arc<NonRegistryOverrideModuleInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        non_registry_override_module_inputs_digest(ctx, &self.project_root, &self.overrides)
            .await
            .map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_untracked_inputs,
            Err(_) => false,
        }
    }
}

fn cached_registry_file_path(cache: &ModuleCache, url: &str) -> Option<PathBuf> {
    if let Some(registry_url) = url.strip_suffix("/bazel_registry.json") {
        return Some(cache.registry_dir(registry_url).join("bazel_registry.json"));
    }

    let (registry_url, module_path) = url.split_once("/modules/")?;
    let mut parts = module_path.split('/');
    let name = parts.next()?;
    let version = parts.next()?;
    let file_name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    match file_name {
        "MODULE.bazel" => Some(cache.module_bazel_path(registry_url, name, version)),
        "source.json" => Some(cache.source_json_path(registry_url, name, version)),
        _ => None,
    }
}

fn registry_file_inputs_poll_digest(
    registry_file_hashes: &[(String, String)],
) -> slug_error::Result<String> {
    let cache = ModuleCache::new()?;
    let mut hasher = Sha256::new();
    hasher.update(b"registry-file-inputs-poll-v1");
    hasher.update([0]);

    for (url, expected_hash) in registry_file_hashes {
        let Some(path) = cached_registry_file_path(&cache, url) else {
            continue;
        };

        hasher.update(url.as_bytes());
        hasher.update([0]);
        hasher.update(expected_hash.as_bytes());
        hasher.update([0]);
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update([0]);
        match absolute_text_file_digest(&path)? {
            Some(digest) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(digest.as_bytes());
            }
            None => hasher.update(b"missing"),
        }
        hasher.update([0]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[async_trait]
impl Key for RegistryFileInputsKey {
    type Value = slug_error::Result<Arc<RegistryFileInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let mut hasher = Sha256::new();
        hasher.update(b"registry-file-inputs-v1");
        hasher.update([0]);
        if self.registry_file_hashes.is_empty() {
            return Ok(Arc::new(RegistryFileInputsValue {
                digest: hex::encode(hasher.finalize()),
                has_inputs: false,
                cache_safe: true,
                has_untracked_inputs: false,
            }));
        }
        let cache = ModuleCache::new()?;
        let mut cache_safe = true;
        let has_untracked_inputs = false;
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        for (url, expected_hash) in &self.registry_file_hashes {
            hasher.update(url.as_bytes());
            hasher.update([0]);
            hasher.update(expected_hash.as_bytes());
            hasher.update([0]);
            let Some(path) = cached_registry_file_path(&cache, url) else {
                cache_safe = false;
                hasher.update(b"unsupported");
                hasher.update([0]);
                continue;
            };
            hasher.update(path.to_string_lossy().as_bytes());
            hasher.update([0]);
            let (content, _tracking) =
                read_text_file_for_project_input(ctx, &project_fs, &path).await?;
            match content {
                Some(content) => {
                    hasher.update(b"present");
                    hasher.update([0]);
                    let actual_hash = slug_bzlmod::lockfile::compute_sha256_hex(content.as_bytes());
                    if &actual_hash != expected_hash {
                        return Err(slug_error::slug_error!(
                            slug_error::ErrorTag::Input,
                            "Registry file checksum mismatch for {}: expected {}, got {}",
                            url,
                            expected_hash,
                            actual_hash
                        ));
                    }
                    hasher.update(actual_hash);
                }
                None => {
                    hasher.update(b"missing");
                }
            }
            hasher.update([0]);
        }

        Ok(Arc::new(RegistryFileInputsValue {
            digest: hex::encode(hasher.finalize()),
            has_inputs: !self.registry_file_hashes.is_empty(),
            cache_safe,
            has_untracked_inputs,
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => value.cache_safe && !value.has_untracked_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
impl Key for NonRootModuleFilesKey {
    type Value = slug_error::Result<Arc<NonRootModuleFilesValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        parse_non_root_module_files(ctx, &self.project_root, &self.inputs)
            .await
            .map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.digest == y.digest && x.parsed_modules == y.parsed_modules,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_untracked_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
impl Key for LegacyBzlmodResolutionDiceKey {
    type Value = slug_error::Result<Arc<Option<slug_bzlmod::BzlmodSessionData>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        BuckConfigBasedCells::resolve_bzlmod_resolution_from_key(self, ctx).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
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
    ) -> slug_error::Result<CellAliasResolver> {
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
        config_args: &[slug_cli_proto::ConfigOverride],
    ) -> slug_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(config_args, Some(project_fs), None, None, None)
            .await
            .buck_error_context("Parsing cells")
    }

    pub async fn parse_with_config_args_and_root_module(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        root_module_file: Arc<slug_bzlmod::RootModuleFileValue>,
    ) -> slug_error::Result<Self> {
        Self::parse_with_config_args_and_bzlmod_inputs(
            project_fs,
            config_args,
            root_module_file,
            None,
        )
        .await
    }

    pub async fn parse_with_config_args_and_bzlmod_inputs(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        root_module_file: Arc<slug_bzlmod::RootModuleFileValue>,
        visible_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
    ) -> slug_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            Some(root_module_file),
            visible_lockfile,
            None,
        )
        .await
        .buck_error_context("Parsing cells")
    }

    pub async fn parse_with_config_args_and_dice_bzlmod_resolution(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        dice_ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Self> {
        let key = Self::build_dice_bzlmod_resolution_key(project_fs, config_args, dice_ctx).await?;
        let bzlmod_resolution = dice_ctx
            .compute(&key)
            .await?
            .buck_error_context("Computing bzlmod resolution through DICE")?;

        Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            None,
            None,
            Some(bzlmod_resolution),
        )
        .await
        .buck_error_context("Parsing cells")
    }

    pub async fn parse_with_config_args_and_persisted_dice_bzlmod_resolution(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        updater: &mut DiceTransactionUpdater,
    ) -> slug_error::Result<Self> {
        let mut dice_ctx = updater.existing_state().await;
        let key =
            Self::build_dice_bzlmod_resolution_key(project_fs, config_args, &mut dice_ctx).await?;
        let bzlmod_resolution = dice_ctx
            .compute(&key)
            .await?
            .buck_error_context("Computing bzlmod resolution through DICE")?;

        Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            None,
            None,
            Some(bzlmod_resolution),
        )
        .await
        .buck_error_context("Parsing cells")
    }

    async fn build_dice_bzlmod_resolution_key(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        dice_ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<LegacyBzlmodResolutionDiceKey> {
        let root_config = LegacyBuckConfig::from_overrides_only(config_args)?;
        let options = BzlmodResolutionOptions::from_config(&root_config)?;
        let project_root_path = project_fs.root().to_path_buf();
        let workspace_id = slug_bzlmod::WorkspaceId::new(
            project_root_path.clone(),
            project_root_path.join("buck-out/v2"),
        );
        let command_policy = dice_ctx
            .compute(&options.command_policy_key(workspace_id.clone()))
            .await?
            .buck_error_context("Computing bzlmod command policy")?;
        let resolution_key = slug_bzlmod::BzlmodResolutionKey {
            workspace_id: workspace_id.clone(),
            command_policy_digest: command_policy.digest.clone(),
        };
        let root_module_file = dice_ctx
            .compute(&TrackedRootModuleFileKey {
                project_root: AbsNormPathBuf::try_from(project_root_path.clone())?,
            })
            .await?
            .buck_error_context("Computing root MODULE.bazel for bzlmod resolution")?;
        let project_root = AbsNormPathBuf::try_from(project_root_path)?;
        let local_overrides = local_overrides_from_root_module(
            root_module_file.as_ref(),
            options.ignore_dev_dependency,
        );
        let local_override_poll_digest =
            local_override_inputs_poll_digest(project_fs, &project_root, &local_overrides)?;
        let local_override_inputs = dice_ctx
            .compute(&LocalOverrideModuleInputsKey {
                project_root: project_root.clone(),
                overrides: local_overrides,
                poll_digest: local_override_poll_digest,
            })
            .await?
            .buck_error_context(
                "Computing local override MODULE.bazel inputs for bzlmod resolution",
            )?;
        let non_registry_overrides = non_registry_override_module_dirs_from_root_module(
            root_module_file.as_ref(),
            options.ignore_dev_dependency,
        )?;
        let non_registry_override_poll_digest =
            non_registry_override_inputs_poll_digest(project_fs, &non_registry_overrides)?;
        let non_registry_override_inputs = dice_ctx
            .compute(&NonRegistryOverrideModuleInputsKey {
                project_root: project_root.clone(),
                overrides: non_registry_overrides,
                poll_digest: non_registry_override_poll_digest,
            })
            .await?
            .buck_error_context(
                "Computing non-registry override MODULE.bazel inputs for bzlmod resolution",
            )?;
        let visible_lockfile = if root_module_file.parsed.is_some()
            && options.lockfile_mode != slug_bzlmod::LockfileMode::Off
        {
            let visible_path = slug_bzlmod::lockfile_path(project_fs.root().as_path());
            let visible_poll_digest =
                text_file_poll_digest(project_fs, &visible_path, b"lockfile-content-poll-v1")?;
            Some(
                dice_ctx
                    .compute(&TrackedLockfileContentKey {
                        project_root: project_root.clone(),
                        kind: slug_bzlmod::LockfileContentKind::Workspace,
                        path: Arc::new(visible_path),
                        poll_digest: visible_poll_digest,
                    })
                    .await?
                    .buck_error_context(
                        "Computing visible MODULE.bazel.lock for bzlmod resolution",
                    )?,
            )
        } else {
            None
        };
        let hidden_lockfile = if root_module_file.parsed.is_some()
            && options.lockfile_mode != slug_bzlmod::LockfileMode::Off
        {
            if let Some(path) = &options.hidden_lockfile_path {
                let hidden_poll_digest =
                    text_file_poll_digest(project_fs, path, b"lockfile-content-poll-v1")?;
                Some(
                    dice_ctx
                        .compute(&TrackedLockfileContentKey {
                            project_root: project_root.clone(),
                            kind: slug_bzlmod::LockfileContentKind::Hidden,
                            path: Arc::new(path.clone()),
                            poll_digest: hidden_poll_digest,
                        })
                        .await?
                        .buck_error_context(
                            "Computing hidden MODULE.bazel lockfile for bzlmod resolution",
                        )?,
                )
            } else {
                None
            }
        } else {
            None
        };
        let registry_file_hashes = visible_lockfile
            .as_ref()
            .and_then(|value| value.lockfile.as_deref())
            .map(|lockfile| {
                lockfile
                    .registry_file_hashes
                    .iter()
                    .map(|(url, hash)| (url.clone(), hash.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let registry_file_poll_digest = registry_file_inputs_poll_digest(&registry_file_hashes)?;
        let registry_file_inputs = dice_ctx
            .compute(&RegistryFileInputsKey {
                project_root: project_root.clone(),
                registry_file_hashes,
                poll_digest: registry_file_poll_digest,
            })
            .await?
            .buck_error_context("Computing registry file inputs for bzlmod resolution bridge")?;
        let extension_replay_summary_digest = match root_module_file.parsed.as_ref() {
            Some(parsed) => {
                root_extension_replay_summary_digest(
                    dice_ctx,
                    parsed,
                    project_fs,
                    visible_lockfile
                        .as_ref()
                        .and_then(|value| value.lockfile.as_deref()),
                    hidden_lockfile
                        .as_ref()
                        .and_then(|value| value.lockfile.as_deref()),
                    &options.repo_env,
                    options.ignore_dev_dependency,
                )
                .await?
            }
            None => None,
        };
        let key = LegacyBzlmodResolutionDiceKey {
            project_root,
            resolution_key,
            options,
            root_module_file,
            visible_lockfile,
            hidden_lockfile,
            local_override_inputs,
            non_registry_override_inputs,
            registry_file_inputs,
            extension_replay_summary_digest: extension_replay_summary_digest.map(Arc::from),
        };
        Ok(key)
    }

    /// Testing entry point: equivalent to `parse_with_config_args` with no project root.
    pub async fn testing_parse(
        config_args: &[slug_cli_proto::ConfigOverride],
    ) -> slug_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(config_args, None, None, None, None)
            .await
            .buck_error_context("Parsing cells")
    }

    async fn parse_with_file_ops_and_options_inner(
        config_args: &[slug_cli_proto::ConfigOverride],
        project_fs: Option<&ProjectRoot>,
        root_module_file: Option<Arc<slug_bzlmod::RootModuleFileValue>>,
        visible_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
        dice_bzlmod_resolution: Option<Arc<Option<slug_bzlmod::BzlmodSessionData>>>,
    ) -> slug_error::Result<Self> {
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
        // Non-bzlmod parsing still injects empty bzlmod projections for legacy
        // consumers. Use the real project root when one exists, and keep the
        // no-project testing sentinel explicit.
        let mut bzlmod_session_data = slug_bzlmod::BzlmodSessionData::empty_for_project_root(
            project_fs
                .map(|project_fs| project_fs.root().to_path_buf())
                .unwrap_or_default(),
        );
        let mut bzlmod_runtime_cell_snapshot = None;

        // ===== Bzlmod Integration =====
        // When MODULE.bazel exists, ALL cell definitions come from bzlmod resolution.
        // The root cell name is derived from module(name = "...") in MODULE.bazel.
        // .buckconfig [cells], [cell_aliases], and [external_cells] sections are skipped.
        let mut bzlmod_aliases: Vec<(NonEmptyCellAlias, CellName)> = Vec::new();
        if let Some(session_data) = if let Some(dice_bzlmod_resolution) = dice_bzlmod_resolution {
            dice_bzlmod_resolution.as_ref().clone()
        } else if let Some(project_fs) = project_fs {
            let options = BzlmodResolutionOptions::from_config(&root_config)?;
            Self::resolve_bzlmod_dependencies_with_options(
                project_fs,
                &options,
                root_module_file.as_deref(),
                visible_lockfile.clone(),
                None,
                None,
            )
            .await?
        } else {
            None
        } {
            let runtime_cell_snapshot = runtime_cell_install_snapshot(&session_data.cell_graph);
            if let Some(project_fs) = project_fs {
                replay_bzlmod_runtime_state(&session_data.cell_graph, project_fs);
            }
            has_module_bazel = true;
            let cell_graph = &session_data.cell_graph;

            // Root cell comes from MODULE.bazel module(name = "...")
            let root_cell_name = CellName::unchecked_new(&cell_graph.root_module_name)?;
            cell_definitions.push((root_cell_name, root_path.clone()));
            tracing::info!(
                "Root cell '{}' defined from MODULE.bazel",
                cell_graph.root_module_name
            );

            for cell in cell_graph.cells.iter() {
                let name = CellName::unchecked_new(&cell.name)?;
                let path = cell_root_path_from_cell_graph(&cell.path)?;
                if !cell_definitions.iter().any(|(n, _)| *n == name) {
                    cell_definitions.push((name, path));
                    tracing::info!("Added bzlmod cell: {}", name);

                    if let Some(setup) = module_setup_from_cell_graph(cell) {
                        bzlmod_external_cells.push((name, setup));
                    } else if cell.bundled {
                        bzlmod_bundled_cells.push(name);
                    }
                }
            }

            for cell in cell_graph.extension_cells.iter().filter(|cell| !cell.lazy) {
                let name = CellName::unchecked_new(&cell.canonical_name)?;
                let path = cell_root_path_from_cell_graph(&cell.path)?;
                if !cell_definitions.iter().any(|(n, _)| *n == name) {
                    let setup = runtime_extension_setup_from_cell_graph(cell);
                    cell_definitions.push((name, path));
                    tracing::info!("Added extension repo cell: {}", name);
                    bzlmod_extension_cells.push((name, setup));
                }
            }

            for alias in cell_graph.root_aliases.iter() {
                bzlmod_aliases.push((
                    NonEmptyCellAlias::new(alias.apparent_name.clone())?,
                    CellName::unchecked_new(&alias.target_name)?,
                ));
            }
            bzlmod_session_data = session_data;
            bzlmod_runtime_cell_snapshot = Some(runtime_cell_snapshot);
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

        let cell_resolver = if has_module_bazel {
            aggregator
                .make_bzlmod_cell_resolver(bzlmod_runtime_cell_snapshot.unwrap_or_default())?
        } else {
            aggregator.make_cell_resolver()?
        };

        Ok(Self {
            cell_resolver,
            root_config,
            external_data: ExternalBuckconfigData {
                args: processed_config_args,
            },
            is_bzlmod: has_module_bazel,
            bzlmod_session_data,
        })
    }

    async fn resolve_bzlmod_resolution_from_key(
        key: &LegacyBzlmodResolutionDiceKey,
        dice_ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Arc<Option<slug_bzlmod::BzlmodSessionData>>> {
        let root_module_file = key.root_module_file.clone();
        if root_module_file.parsed.is_none() {
            return Ok(Arc::new(None));
        }

        let project_root = ProjectRoot::new_unchecked(key.project_root.clone());
        Self::resolve_bzlmod_dependencies_with_options(
            &project_root,
            &key.options,
            Some(root_module_file.as_ref()),
            key.visible_lockfile.clone(),
            key.hidden_lockfile.clone(),
            Some(dice_ctx),
        )
        .await
        .map(Arc::new)
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
    async fn resolve_bzlmod_dependencies_with_options(
        project_root: &ProjectRoot,
        options: &BzlmodResolutionOptions,
        root_module_file: Option<&slug_bzlmod::RootModuleFileValue>,
        visible_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
        hidden_lockfile: Option<Arc<slug_bzlmod::LockfileContentValue>>,
        mut dice_ctx: Option<&mut DiceComputations<'_>>,
    ) -> slug_error::Result<Option<slug_bzlmod::BzlmodSessionData>> {
        let module_bazel_rel = ProjectRelativePath::new("MODULE.bazel")?;
        let module_bazel_path = project_root.resolve(module_bazel_rel);

        let parsed = if let Some(root_module_file) = root_module_file {
            let Some(parsed) = root_module_file.parsed.clone() else {
                return Ok(None);
            };
            tracing::info!(
                "Found MODULE.bazel through tracked DICE file inputs, resolving bzlmod dependencies"
            );
            record_bzlmod_event(
                BzlmodEventKind::BzlmodResolutionCompute,
                root_module_file.path.display().to_string(),
            );
            parsed
        } else {
            // Check if MODULE.bazel exists. This direct fallback remains for
            // bootstrap and completion paths that do not yet have a DICE
            // transaction available.
            if !fs_util::try_exists(&module_bazel_path)? {
                return Ok(None);
            }

            tracing::info!("Found MODULE.bazel, resolving bzlmod dependencies");
            record_bzlmod_event(
                BzlmodEventKind::BzlmodResolutionCompute,
                module_bazel_path.display().to_string(),
            );

            // Parse MODULE.bazel. Bazel treats root module-file parse/compile
            // errors as bzlmod failures, not as a signal to disable bzlmod.
            parse_module_bazel(module_bazel_path.as_path()).with_buck_error_context(|| {
                format!(
                    "Failed to parse root MODULE.bazel at {}",
                    module_bazel_path.display()
                )
            })?
        };

        let mut cells = Vec::new();
        let mut aliases = Vec::new();
        let mut module_symlinks = Vec::new();
        let mut lockfile_seeded_cells = Vec::new();
        let mut scoped_repo_aliases = Vec::new();
        let mut dynamic_extension_aliases: Vec<BzlmodDynamicAlias> = Vec::new();
        let workspace_root = project_root.root().as_path();
        let mut resolved_graph_for_aliases = None;
        let project_root_abs = AbsNormPathBuf::try_from(workspace_root.to_path_buf())?;
        let workspace_id = slug_bzlmod::WorkspaceId::for_project_root(workspace_root.to_path_buf());
        let mut bzlmod_session_data = slug_bzlmod::BzlmodSessionData::for_workspace(workspace_id);
        bzlmod_session_data.repo_env = slug_bzlmod::BzlmodRepoEnvDataValue::for_workspace(
            bzlmod_session_data.cell_graph.workspace_id.clone(),
            Arc::new(options.repo_env.clone()),
        );
        let allowed_yanked_versions = slug_bzlmod::parse_allowed_yanked_versions(
            options.allow_yanked_versions_env.as_deref(),
            &options.allow_yanked_versions_flags,
        )?;
        let visible_lockfile_value = visible_lockfile;
        let hidden_lockfile_value = hidden_lockfile;
        let visible_lockfile = if options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(visible_lockfile) = visible_lockfile_value.as_ref() {
            visible_lockfile.lockfile.clone()
        } else {
            slug_bzlmod::read_lockfile_with_mode(
                project_root.root().as_path(),
                options.lockfile_mode,
            )?
        };
        let hidden_lockfile = if options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(hidden_lockfile) = hidden_lockfile_value.as_ref() {
            hidden_lockfile.lockfile.clone()
        } else if let Some(hidden_lockfile_path) = options.hidden_lockfile_path.as_ref() {
            slug_bzlmod::read_hidden_lockfile_path(hidden_lockfile_path)?
        } else {
            None
        };

        // Resolve active local path overrides first.
        let active_overrides = active_root_overrides(&parsed.module, options.ignore_dev_dependency);
        let local_modules = resolve_local_modules(&active_overrides, workspace_root)?;
        for (name, resolved) in local_modules.iter() {
            let cell_name = CellName::unchecked_new(name)?;
            let (cell_path, symlink) = local_override_cell_path_and_symlink(
                project_root,
                &project_root_abs,
                name,
                resolved.version.as_str(),
                &resolved.relative_path,
            )?;
            if let Some(symlink) = symlink {
                module_symlinks.push(symlink);
            }
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
            if let Some(lockfile) = visible_lockfile.as_ref() {
                resolver.set_yanked_version_policy(
                    allowed_yanked_versions.clone(),
                    options.lockfile_mode,
                    lockfile.registry_file_hashes.clone(),
                    lockfile.selected_yanked_versions.clone(),
                );
            } else {
                resolver.set_yanked_version_policy(
                    allowed_yanked_versions.clone(),
                    options.lockfile_mode,
                    Default::default(),
                    Default::default(),
                );
            }
            resolver.set_ignore_dev_dependency(options.ignore_dev_dependency);
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
            // Bazel computes repo specs for every selected registry module during
            // module resolution; registry/source access errors are direct
            // resolution failures, not warnings followed by a broken cell graph.
            resolver
                .fetch_sources(&mut resolved_graph)
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to fetch selected module sources for root module '{}'",
                        parsed.module.name
                    )
                })?;
            bzlmod_session_data.resolution_facts =
                slug_bzlmod::BzlmodResolutionFactsValue::for_workspace(
                    bzlmod_session_data.cell_graph.workspace_id.clone(),
                    resolved_graph.registry_file_hashes.clone(),
                    resolved_graph.selected_yanked_versions.clone(),
                );

            // Build a set of local override names to skip
            let local_override_names: std::collections::HashSet<_> = active_overrides
                .iter()
                .filter_map(|o| match o {
                    slug_bzlmod::types::Override::LocalPath(local) => {
                        Some(local.module_name.clone())
                    }
                    _ => None,
                })
                .collect();

            for (module_name, module_info) in &resolved_graph.modules {
                // Skip root module and local overrides
                if module_name == &parsed.module.name || local_override_names.contains(module_name)
                {
                    continue;
                }

                // Only create symlinks for modules with cached source paths
                if let Some(source_path) = &module_info.source_path {
                    let entry_name =
                        bazel_canonical_module_repo_name(module_name, &module_info.version);
                    module_symlinks.push(BzlmodExternalModuleSymlink {
                        entry_name,
                        source_path: source_path.clone(),
                    });
                }
            }

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
                        let canonical_repo =
                            bazel_canonical_module_repo_name(module_name, &module_info.version);
                        let external_path = format!("bazel-external/{canonical_repo}");
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

                        let setup = slug_core::cells::external::BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(url.as_str()),
                            source_path: Arc::from(source_path_str.as_str()),
                        };

                        cells.push((cell_name, cell_path, Some(setup)));
                    }
                    ModuleSource::LocalPath { path } => {
                        // Local path modules from overrides are handled separately
                        let (cell_path, symlink) = local_override_cell_path_and_symlink(
                            project_root,
                            &project_root_abs,
                            module_name,
                            &module_info.version,
                            path,
                        )?;
                        if let Some(symlink) = symlink {
                            module_symlinks.push(symlink);
                        }
                        cells.push((cell_name, cell_path, None));
                        tracing::info!("Registered local module: {} -> {}", module_name, path);
                    }
                    ModuleSource::Git { remote, commit, .. } => {
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let canonical_repo =
                            bazel_canonical_module_repo_name(module_name, &module_info.version);
                        let external_path = format!("bazel-external/{canonical_repo}");
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );

                        // Git modules use Bzlmod setup with empty registry URL
                        let setup = slug_core::cells::external::BzlmodCellSetup {
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

                        let canonical_repo =
                            bazel_canonical_module_repo_name(module_name, &module_info.version);
                        let external_path = format!("bazel-external/{canonical_repo}");
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );

                        // Use first URL as the registry URL
                        let url = urls.first().map(|u| u.as_str()).unwrap_or("archive");
                        let setup = slug_core::cells::external::BzlmodCellSetup {
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
            // repo_name if specified, otherwise under name. Slug's cell identity
            // can differ from that apparent name (for example when a selected
            // module is represented by a disambiguated cell), so register the
            // apparent name as an alias whenever it is not already the cell name.
            for dep in &parsed.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                if let Some(target_name) =
                    selected_bzlmod_cell_name_for_dep(&cells, &dep.name, &resolved_graph)
                {
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

            // Populate module version facts so module_version() builtin
            // returns the correct version through DICE-injected command state.
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
            bzlmod_session_data.module_versions =
                slug_bzlmod::BzlmodModuleVersionsDataValue::for_workspace(
                    bzlmod_session_data.cell_graph.workspace_id.clone(),
                    Arc::new(version_map),
                );
        }

        // Build parsed_modules list for extension resolution
        let mut parsed_modules: Vec<(String, ParsedModuleFile)> = Vec::new();
        parsed_modules.push((parsed.module.name.clone(), parsed.clone()));
        let non_root_inputs = non_root_module_file_inputs(project_root, &cells, &module_symlinks);
        let mut non_root_parsed_modules = if let Some(ctx) = dice_ctx.as_mut() {
            let poll_digest = non_root_module_files_poll_digest(project_root, &non_root_inputs)?;
            ctx.compute(&NonRootModuleFilesKey {
                project_root: project_root_abs.clone(),
                inputs: non_root_inputs,
                poll_digest,
            })
            .await?
            .buck_error_context("Computing non-root MODULE.bazel inputs for bzlmod resolution")?
            .parsed_modules
            .clone()
        } else {
            parse_non_root_module_files_direct(&non_root_inputs)?
        };
        parsed_modules.append(&mut non_root_parsed_modules);

        if let Some(resolved_graph) = &resolved_graph_for_aliases {
            for (module_name, parsed_mod) in &parsed_modules {
                for dep in &parsed_mod.module.bazel_deps {
                    let apparent_name = dep.apparent_name();
                    let Some(target_name) =
                        selected_bzlmod_cell_name_for_dep(&cells, &dep.name, resolved_graph)
                    else {
                        continue;
                    };
                    if apparent_name == target_name {
                        continue;
                    }
                    if module_name != &parsed.module.name {
                        scoped_repo_aliases.push(BzlmodScopedRepoAlias {
                            owner_module: module_name.clone(),
                            apparent_name: apparent_name.to_owned(),
                            target_name: target_name.to_owned(),
                        });
                        continue;
                    }
                    if aliases
                        .iter()
                        .any(|(alias, _)| alias.as_str() == apparent_name)
                    {
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
            Vec<slug_bzlmod::types::ExtensionUsage>,
        > = std::collections::HashMap::new();
        for (module_name, parsed_mod) in &parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated = slug_bzlmod::aggregate_extensions_with_policy(
            &module_extensions,
            Some(root_module_name),
            options.ignore_dev_dependency,
        );
        let mut repo_mappings =
            repo_mapping_snapshot_for_modules(&parsed_modules, root_module_name);
        let repo_mapping_overrides = repo_mapping_overrides_for_root(
            &parsed_modules,
            root_module_name,
            options.ignore_dev_dependency,
        );
        let (mut pre_computed_cells, pre_computed_aliases) =
            slug_bzlmod::pre_compute_extension_repo_cells(
                &parsed_modules,
                root_module_name,
                options.ignore_dev_dependency,
            )?;
        let mut extension_mapping_cells = pre_computed_cells.clone();
        add_extension_repo_mapping_rows_from_cells(
            &mut repo_mappings,
            &extension_mapping_cells,
            root_module_name,
            &repo_mapping_overrides,
        );

        // Augment with extension-internal spokes recorded in MODULE.bazel.lock.
        // The use_repo()-driven pass above only registers repos the project
        // explicitly imports (e.g. the `crates` hub), not the spokes the hub's
        // generated BUILD.bazel references via `@crates__<name>//`. Without this
        // pass, warm builds with a populated cache fail with `unknown cell name`
        // because the only path that registers spokes (`get_file_ops_delegate`'s
        // post-extension-eval loop) is gated on the hub's `.slug_repo_complete`
        // marker.
        if let Some(lockfile) = visible_lockfile.as_ref() {
            let extra = slug_bzlmod::pre_compute_extension_repo_cells_from_lockfile(
                lockfile,
                &aggregated,
                root_module_name,
                &mut pre_computed_cells,
                project_root.root().as_path(),
                Some(bzlmod_session_data.repo_env.repo_env.as_ref()),
                Some(&repo_mappings),
                Some(&repo_mapping_overrides),
            );
            lockfile_seeded_cells.extend(extra.iter().map(BzlmodPendingRepoCell::from_pending));
            extension_mapping_cells.extend(extra);
            add_extension_repo_mapping_rows_from_cells(
                &mut repo_mappings,
                &extension_mapping_cells,
                root_module_name,
                &repo_mapping_overrides,
            );
        }
        let hidden_lockfile_path = options.hidden_lockfile_path.clone();
        if let Some(lockfile) = hidden_lockfile.as_ref() {
            let extra = slug_bzlmod::pre_compute_extension_repo_cells_from_lockfile(
                lockfile,
                &aggregated,
                root_module_name,
                &mut pre_computed_cells,
                project_root.root().as_path(),
                Some(bzlmod_session_data.repo_env.repo_env.as_ref()),
                Some(&repo_mappings),
                Some(&repo_mapping_overrides),
            );
            lockfile_seeded_cells.extend(extra.iter().map(BzlmodPendingRepoCell::from_pending));
            extension_mapping_cells.extend(extra);
            add_extension_repo_mapping_rows_from_cells(
                &mut repo_mappings,
                &extension_mapping_cells,
                root_module_name,
                &repo_mapping_overrides,
            );
        }
        add_scoped_repo_aliases_from_mapping_snapshot(&mut scoped_repo_aliases, &repo_mappings);
        slug_util::memory_checkpoint::checkpoint(
            "legacy_cells_bzlmod_precomputed_repos",
            [
                ("parsed_modules", parsed_modules.len()),
                ("precomputed_cells", pre_computed_cells.len()),
                ("precomputed_aliases", pre_computed_aliases.len()),
            ],
        );

        // Aggregate extension usages from all modules and carry them into the
        // DICE-injected bzlmod session state. This data is needed when
        // extension repos are lazily executed inside DICE.
        bzlmod_session_data.extension_aggregations =
            slug_bzlmod::BzlmodExtensionAggregationsDataValue::for_workspace(
                bzlmod_session_data.cell_graph.workspace_id.clone(),
                Arc::new(aggregated),
            );
        bzlmod_session_data.lockfile_inputs = slug_bzlmod::BzlmodLockfileInputsValue::from_values(
            hidden_lockfile_path,
            visible_lockfile_value,
            hidden_lockfile_value,
            options.lockfile_mode,
        );

        // Collect toolchain and execution platform registrations from all modules.
        // Priority order: root module first, then BFS order of dep graph.
        // parsed_modules is already in BFS order (root first from resolution).
        // dev_dependency items from non-root modules are skipped (Bazel 9.0 behavior).
        {
            let (mut all_toolchains, all_exec_platforms) = collect_bzlmod_registered_items(
                &parsed_modules,
                root_module_name,
                options.ignore_dev_dependency,
            );
            // If the module graph depends on rules_python but never registers
            // a py3 toolchain, auto-inject BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS
            // at lowest priority so ctx.toolchains[@rules_python//python:toolchain_type]
            // resolves to a host py_runtime. Users can override by registering
            // their own toolchain earlier in MODULE.bazel.
            //
            // WHY string match on the module name: ParsedModuleFile currently has no
            // typed "is rules_python" flag, and adding one would require threading a
            // new field through slug_bzlmod::types + the MVS resolver + every caller
            // that constructs ParsedModuleFile — well out of scope for an error-
            // handling fix. The constants below keep the magic strings grep-able so a
            // future typed flag can replace them in one place.
            if module_depends_on_rules_python(&parsed_modules)
                && !toolchains_include_bundled_python(&all_toolchains)
            {
                for label in BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS {
                    all_toolchains.push(slug_bzlmod::RegisteredToolchain {
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
            bzlmod_session_data.registered_toolchains =
                slug_bzlmod::RegisteredToolchainsDataValue::for_workspace(
                    bzlmod_session_data.cell_graph.workspace_id.clone(),
                    all_toolchains,
                );
            bzlmod_session_data.registered_execution_platforms =
                slug_bzlmod::RegisteredExecutionPlatformsDataValue::for_workspace(
                    bzlmod_session_data.cell_graph.workspace_id.clone(),
                    all_exec_platforms,
                );

            // Toolchain repo materialization is intentionally lazy. Label
            // resolution and the external-cell delegates own the semantic
            // materialization path; do not poll `bazel-external` here.
        }

        // Convert pre-computed cells to the format expected by
        // BzlmodSessionData. Bazel's identity for extension-generated
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
                repo_env_json: repo_env_json(bzlmod_session_data.repo_env.repo_env.as_ref()),
                materialized: false,
            };
            ext_cells.push((cell_name, cell_path, setup));
        }

        // Build a set of existing cell names (from bzlmod deps + synthetic repos)
        // to avoid creating aliases that conflict with cell names.
        let mut existing_cell_names: std::collections::HashSet<String> = cells
            .iter()
            .map(|(name, _, _)| name.as_str().to_owned())
            .collect();

        // Convert pre-computed aliases. Apparent names from use_repo() are
        // module-scoped in Bazel; Slug still has a global alias table, so keep
        // this to direct apparent names to canonical cells without inventing a
        // second cell identity.
        let mut ext_aliases = Vec::new();
        for alias in pre_computed_aliases {
            let target_name = resolved_graph_for_aliases
                .as_ref()
                .and_then(|resolved_graph| {
                    selected_bzlmod_cell_name_for_dep(&cells, &alias.canonical_name, resolved_graph)
                })
                .unwrap_or(alias.canonical_name.as_str())
                .to_owned();
            let is_generated_override_alias = alias.declaring_module.is_none()
                && alias.apparent_name != target_name
                && slug_bzlmod::parse_canonical_name(&alias.apparent_name).is_some();
            let is_root_declared_alias =
                alias.declaring_module.as_deref() == Some(parsed.module.name.as_str());
            if let Some(owner_module) = alias.declaring_module.as_deref().or_else(|| {
                slug_bzlmod::parse_canonical_name(&alias.canonical_name)
                    .map(|(owner_module, _, _)| owner_module)
            }) {
                scoped_repo_aliases.push(BzlmodScopedRepoAlias {
                    owner_module: owner_module.to_owned(),
                    apparent_name: alias.apparent_name.clone(),
                    target_name: target_name.clone(),
                });
            }
            if is_generated_override_alias {
                // Bazel `override_repo()` replaces the generated repo's
                // content, not its canonical execution identity. Exact labels
                // like `@@rules_rs++rules_rust+rules_rust//...` still render
                // actions and source inputs under
                // `external/rules_rs++rules_rust+rules_rust`, but the package
                // contents come from the selected replacement module. Register
                // that exact generated name as its own cell pointing at the
                // selected cell's path, and discard any stale extension/lockfile
                // registration for the generated repo.
                if let Some(dynamic_alias) =
                    dynamic_alias_for_generated_override(&alias, &target_name)
                {
                    dynamic_extension_aliases.push(dynamic_alias);
                }
                if !existing_cell_names.contains(&alias.apparent_name) {
                    if let Some((_, selected_path, selected_setup)) = cells
                        .iter()
                        .find(|(name, _, _)| name.as_str() == target_name)
                    {
                        let selected_source_path = selected_setup
                            .as_ref()
                            .map(|setup| PathBuf::from(setup.source_path.as_ref()))
                            .unwrap_or_else(|| {
                                project_root
                                    .root()
                                    .as_path()
                                    .join(selected_path.as_project_relative_path().as_str())
                            });
                        let selected_setup = selected_setup.clone().or_else(|| {
                            Some(BzlmodCellSetup {
                                module_name: Arc::from(target_name.as_str()),
                                version: Arc::from(""),
                                registry_url: Arc::from("override_repo"),
                                source_path: Arc::from(
                                    selected_source_path.to_string_lossy().into_owned(),
                                ),
                            })
                        });
                        let identity_path =
                            format!("bazel-external/{}", alias.apparent_name.as_str());
                        cells.push((
                            CellName::unchecked_new(&alias.apparent_name)?,
                            CellRootPathBuf::new(
                                ProjectRelativePath::new(&identity_path)?.to_owned(),
                            ),
                            selected_setup,
                        ));
                        module_symlinks.push(BzlmodExternalModuleSymlink {
                            entry_name: alias.apparent_name.clone(),
                            source_path: selected_source_path,
                        });
                        existing_cell_names.insert(alias.apparent_name.clone());
                    } else {
                        tracing::warn!(
                            "override_repo generated repo '{}' targets '{}', but the selected cell was not registered",
                            alias.apparent_name,
                            target_name
                        );
                    }
                }
                ext_cells.retain(|(name, _, _)| name.as_str() != alias.apparent_name);
                lockfile_seeded_cells.retain(|cell| {
                    cell.canonical_name != alias.apparent_name
                        && cell.internal_name != alias.apparent_name
                });
            }
            if !is_root_declared_alias {
                continue;
            }
            if existing_cell_names.contains(alias.apparent_name.as_str()) {
                tracing::debug!(
                    "Skipping global alias '{}' -> '{}': cell already exists; scoped alias remains registered",
                    alias.apparent_name,
                    target_name
                );
                continue;
            }
            let apparent_name = NonEmptyCellAlias::new(alias.apparent_name)?;
            let canonical_name = CellName::unchecked_new(&target_name)?;
            ext_aliases.push((apparent_name, canonical_name));
        }
        add_scoped_repo_aliases_from_root_overrides(
            &mut scoped_repo_aliases,
            &repo_mapping_overrides,
            root_module_name,
            &cells,
            resolved_graph_for_aliases.as_ref(),
        );
        bzlmod_session_data.repo_mappings = slug_bzlmod::BzlmodRepoMappingsDataValue::for_workspace(
            bzlmod_session_data.cell_graph.workspace_id.clone(),
            Arc::new(repo_mappings),
            Arc::new(repo_mapping_overrides),
        );

        // Add extension aliases to the main aliases list
        aliases.extend(ext_aliases);

        let root_module_name = if parsed.module.name.is_empty() {
            "_main".to_owned()
        } else {
            parsed.module.name.clone()
        };
        let lockfile_repo_env_json = repo_env_json(bzlmod_session_data.repo_env.repo_env.as_ref());
        let cell_graph = slug_bzlmod::BzlmodCellGraphValue {
            workspace_id: bzlmod_session_data.cell_graph.workspace_id.clone(),
            root_module_name: root_module_name.clone(),
            cells: Arc::new(
                cells
                    .iter()
                    .map(|(name, path, setup)| slug_bzlmod::BzlmodCellGraphCell {
                        name: name.as_str().to_owned(),
                        path: path.as_str().to_owned(),
                        module_setup: setup.as_ref().map(|setup| {
                            slug_bzlmod::BzlmodCellGraphModuleSetup {
                                module_name: setup.module_name.to_string(),
                                version: setup.version.to_string(),
                                registry_url: setup.registry_url.to_string(),
                                source_path: setup.source_path.to_string(),
                            }
                        }),
                        bundled: false,
                    })
                    .chain(BZLMOD_ALWAYS_BUNDLED_CELLS.iter().map(|name| {
                        slug_bzlmod::BzlmodCellGraphCell {
                            name: (*name).to_owned(),
                            path: (*name).to_owned(),
                            module_setup: None,
                            bundled: true,
                        }
                    }))
                    .collect(),
            ),
            extension_cells: Arc::new(
                ext_cells
                    .iter()
                    .map(
                        |(_name, path, setup)| slug_bzlmod::BzlmodCellGraphExtensionCell {
                            canonical_name: setup.canonical_name.to_string(),
                            internal_name: setup.internal_name.to_string(),
                            path: path.as_str().to_owned(),
                            extension_id: setup.extension_id.to_string(),
                            spec_hash: setup.spec_hash.to_string(),
                            repo_spec_json: setup.repo_spec_json.to_string(),
                            repo_env_json: setup.repo_env_json.to_string(),
                            materialized: setup.materialized,
                            lazy: false,
                        },
                    )
                    .chain(lockfile_seeded_cells.iter().map(|cell| {
                        slug_bzlmod::BzlmodCellGraphExtensionCell {
                            canonical_name: cell.canonical_name.clone(),
                            internal_name: cell.internal_name.clone(),
                            path: cell.path.clone(),
                            extension_id: cell.extension_id.clone(),
                            spec_hash: cell.spec_hash.clone(),
                            repo_spec_json: cell.repo_spec_json.clone(),
                            repo_env_json: lockfile_repo_env_json.to_string(),
                            materialized: false,
                            lazy: true,
                        }
                    }))
                    .collect(),
            ),
            root_aliases: Arc::new(
                aliases
                    .iter()
                    .map(|(alias, target)| slug_bzlmod::BzlmodCellGraphAlias {
                        apparent_name: alias.as_str().to_owned(),
                        target_name: target.as_str().to_owned(),
                    })
                    .collect(),
            ),
            module_symlinks: Arc::new(
                module_symlinks
                    .iter()
                    .map(|symlink| slug_bzlmod::BzlmodCellGraphModuleSymlink {
                        entry_name: symlink.entry_name.clone(),
                        source_path: Arc::new(symlink.source_path.clone()),
                    })
                    .collect(),
            ),
            scoped_aliases: Arc::new(
                scoped_repo_aliases
                    .iter()
                    .map(|alias| slug_bzlmod::BzlmodCellGraphScopedAlias {
                        owner_module: alias.owner_module.clone(),
                        apparent_name: alias.apparent_name.clone(),
                        target_name: alias.target_name.clone(),
                    })
                    .collect(),
            ),
            dynamic_aliases: Arc::new(
                dynamic_extension_aliases
                    .iter()
                    .map(|alias| slug_bzlmod::BzlmodCellGraphDynamicAlias {
                        apparent_name: alias.apparent_name.clone(),
                        canonical_name: alias.canonical_name.clone(),
                    })
                    .collect(),
            ),
        };
        bzlmod_session_data.cell_graph = cell_graph;

        Ok(Some(bzlmod_session_data))
    }

    pub(crate) fn get_cell_aliases_from_config(
        config: &LegacyBuckConfig,
    ) -> slug_error::Result<impl Iterator<Item = (NonEmptyCellAlias, NonEmptyCellAlias)> + use<>>
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

    pub fn bzlmod_lockfile_mode_from_config_args(
        config_args: &[slug_cli_proto::ConfigOverride],
    ) -> slug_error::Result<slug_bzlmod::LockfileMode> {
        let root_config = LegacyBuckConfig::from_overrides_only(config_args)?;
        Self::bzlmod_lockfile_mode_from_config(&root_config)
    }

    fn bzlmod_lockfile_mode_from_config(
        root_config: &LegacyBuckConfig,
    ) -> slug_error::Result<slug_bzlmod::LockfileMode> {
        match root_config
            .get_section("bzlmod")
            .and_then(|section| section.get("lockfile_mode"))
        {
            Some(value) => slug_bzlmod::LockfileMode::from_str(value.as_str()).ok_or_else(|| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Invalid --lockfile_mode value `{}` for bzlmod",
                    value.as_str()
                )
            }),
            None => Ok(slug_bzlmod::LockfileMode::default()),
        }
    }

    pub(crate) async fn parse_single_cell_with_dice(
        ctx: &mut DiceComputations<'_>,
        _cell_path: &CellRootPath,
    ) -> slug_error::Result<LegacyBuckConfig> {
        let external_data = ctx.get_injected_external_buckconfig_data().await?;
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(&external_data.args))
    }

    pub async fn parse_single_cell(
        &self,
        _cell: CellName,
        _project_fs: &ProjectRoot,
    ) -> slug_error::Result<LegacyBuckConfig> {
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(
            &self.external_data.args,
        ))
    }

    pub(crate) async fn parse_single_cell_with_file_ops(
        &self,
        _cell: CellName,
    ) -> slug_error::Result<LegacyBuckConfig> {
        // Q1=B: all cells return the same CLI-flag-only config.
        Ok(LegacyBuckConfig::from_resolved_flags(
            &self.external_data.args,
        ))
    }

    fn parse_external_cell_origin(
        cell: CellName,
        value: &str,
        config: &LegacyBuckConfig,
    ) -> slug_error::Result<ExternalCellOrigin> {
        #[derive(slug_error::Error, Debug)]
        #[slug(tag = Input)]
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
    resolved_graph: &slug_bzlmod::ResolvedGraph,
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

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash as _;
    use std::hash::Hasher as _;

    use dice::UserComputationData;
    use dice::testing::DiceBuilder;
    use slug_bzlmod::types::Module;
    use slug_bzlmod::types::ParsedModuleFile;
    use slug_bzlmod::types::RegisteredItem;
    use slug_bzlmod::version::Version;
    use slug_core::fs::project::ProjectRootTemp;

    use super::*;
    use crate::dice::data::testing::SetTestingIoProvider;

    fn lockfile_value(path: &str, digest: &str) -> Arc<slug_bzlmod::LockfileContentValue> {
        Arc::new(slug_bzlmod::LockfileContentValue {
            path: Arc::new(PathBuf::from(path)),
            digest: Some(digest.to_owned()),
            tracked_by_dice: false,
            lockfile: None,
        })
    }

    fn legacy_bzlmod_resolution_key(
        hidden_lockfile: Arc<slug_bzlmod::LockfileContentValue>,
    ) -> LegacyBzlmodResolutionDiceKey {
        let project_root = PathBuf::from("/tmp/slug-plan61-hidden-lockfile-key-test");
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), PathBuf::from("/tmp/output-base"));

        LegacyBzlmodResolutionDiceKey {
            project_root: AbsNormPathBuf::try_from(project_root.clone()).unwrap(),
            resolution_key: slug_bzlmod::BzlmodResolutionKey {
                workspace_id,
                command_policy_digest: Arc::from("command-policy"),
            },
            options: BzlmodResolutionOptions {
                lockfile_mode: slug_bzlmod::LockfileMode::Update,
                ignore_dev_dependency: false,
                allow_yanked_versions_env: None,
                allow_yanked_versions_flags: Vec::new(),
                hidden_lockfile_path: Some(PathBuf::from("/tmp/hidden/MODULE.bazel.lock")),
                repo_env: BTreeMap::new(),
                repo_env_digest: "repo-env".to_owned(),
            },
            root_module_file: Arc::new(slug_bzlmod::RootModuleFileValue {
                path: Arc::new(project_root.join("MODULE.bazel")),
                input_digest: Some("root-module".to_owned()),
                input_count: 1,
                parsed: None,
            }),
            visible_lockfile: Some(lockfile_value(
                "/tmp/workspace/MODULE.bazel.lock",
                "visible-lockfile",
            )),
            hidden_lockfile: Some(hidden_lockfile),
            local_override_inputs: Arc::new(LocalOverrideModuleInputsValue {
                digest: "local-overrides".to_owned(),
                has_bazel_deps: false,
                has_extension_usages: false,
                has_repo_rule_invocations: false,
                has_git_overrides: false,
                has_untracked_inputs: false,
            }),
            non_registry_override_inputs: Arc::new(NonRegistryOverrideModuleInputsValue {
                digest: "non-registry-overrides".to_owned(),
                has_inputs: false,
                has_untracked_inputs: false,
            }),
            registry_file_inputs: Arc::new(RegistryFileInputsValue {
                digest: "registry-files".to_owned(),
                has_inputs: false,
                cache_safe: true,
                has_untracked_inputs: false,
            }),
            extension_replay_summary_digest: Some(Arc::from("extension-replay")),
        }
    }

    fn parsed_module(name: &str) -> ParsedModuleFile {
        ParsedModuleFile {
            module: Module::new(name.to_owned(), Version::empty()),
            has_module_directive: true,
            extension_usages: Vec::new(),
            repo_rule_invocations: Vec::new(),
            registered_toolchains: Vec::new(),
            registered_execution_platforms: Vec::new(),
        }
    }

    #[test]
    fn collect_registered_items_honors_ignore_dev_dependency_for_root() {
        let mut root = parsed_module("root");
        root.registered_toolchains.push(RegisteredItem {
            label: "@root_toolchains//:all".to_owned(),
            dev_dependency: true,
        });
        root.registered_execution_platforms.push(RegisteredItem {
            label: "@root_platforms//:all".to_owned(),
            dev_dependency: true,
        });

        let parsed_modules = vec![("root".to_owned(), root)];
        let (toolchains, platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", false);
        assert_eq!(toolchains.len(), 1);
        assert_eq!(platforms.len(), 1);

        let (toolchains, platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", true);
        assert!(toolchains.is_empty());
        assert!(platforms.is_empty());
    }

    #[test]
    fn bzlmod_resolution_policy_includes_hidden_lockfile_path() {
        let first = BzlmodResolutionOptions {
            lockfile_mode: slug_bzlmod::LockfileMode::Update,
            ignore_dev_dependency: false,
            allow_yanked_versions_env: None,
            allow_yanked_versions_flags: Vec::new(),
            hidden_lockfile_path: Some(PathBuf::from("/tmp/hidden-one/MODULE.bazel.lock")),
            repo_env: BTreeMap::new(),
            repo_env_digest: "repo-env".to_owned(),
        };
        let mut second = first.clone();
        second.hidden_lockfile_path = Some(PathBuf::from("/tmp/hidden-two/MODULE.bazel.lock"));

        assert_ne!(first.policy_digest(), second.policy_digest());
        assert!(!bzlmod_resolution_options_policy_eq(&first, &second));

        let mut first_hasher = DefaultHasher::new();
        hash_bzlmod_resolution_options_policy(&first, &mut first_hasher);
        let mut second_hasher = DefaultHasher::new();
        hash_bzlmod_resolution_options_policy(&second, &mut second_hasher);
        assert_ne!(first_hasher.finish(), second_hasher.finish());
    }

    #[test]
    fn bzlmod_resolution_options_reads_repo_env_from_config() -> slug_error::Result<()> {
        let mut repo_env = BTreeMap::new();
        repo_env.insert("PLAN61_REPO_ENV".to_owned(), "from-config".to_owned());
        let config = LegacyBuckConfig::from_overrides_only(&[
            slug_cli_proto::ConfigOverride::flag_no_cell(&format!(
                "bzlmod.repo_env_json={}",
                serde_json::to_string(&repo_env)?
            )),
        ])?;
        let options = BzlmodResolutionOptions::from_config(&config)?;

        assert_eq!(options.repo_env, repo_env);
        assert_eq!(
            options.repo_env_digest,
            slug_bzlmod::repo_env_policy_digest(&repo_env)
        );
        Ok(())
    }

    #[test]
    fn runtime_cell_install_snapshot_derives_from_cell_graph() {
        let workspace_id = slug_bzlmod::WorkspaceId::for_project_root(PathBuf::from(
            "/tmp/slug-plan61-cell-graph-runtime-test",
        ));
        let cell_graph = slug_bzlmod::BzlmodCellGraphValue {
            workspace_id,
            root_module_name: "root".to_owned(),
            cells: Arc::new(Vec::new()),
            extension_cells: Arc::new(vec![
                slug_bzlmod::BzlmodCellGraphExtensionCell {
                    canonical_name: "root+ext+eager".to_owned(),
                    internal_name: "eager".to_owned(),
                    path: "bazel-external/root+ext+eager".to_owned(),
                    extension_id: "@@root//:ext.bzl%ext".to_owned(),
                    spec_hash: "eager-hash".to_owned(),
                    repo_spec_json: "{}".to_owned(),
                    repo_env_json: r#"{"K":"V"}"#.to_owned(),
                    materialized: true,
                    lazy: false,
                },
                slug_bzlmod::BzlmodCellGraphExtensionCell {
                    canonical_name: "root+ext+lazy".to_owned(),
                    internal_name: "lazy".to_owned(),
                    path: "bazel-external/root+ext+lazy".to_owned(),
                    extension_id: "@@root//:ext.bzl%ext".to_owned(),
                    spec_hash: "lazy-hash".to_owned(),
                    repo_spec_json: "{}".to_owned(),
                    repo_env_json: r#"{"K":"V"}"#.to_owned(),
                    materialized: false,
                    lazy: true,
                },
            ]),
            root_aliases: Arc::new(Vec::new()),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(vec![slug_bzlmod::BzlmodCellGraphScopedAlias {
                owner_module: "root".to_owned(),
                apparent_name: "tool".to_owned(),
                target_name: "root+ext+eager".to_owned(),
            }]),
            dynamic_aliases: Arc::new(vec![slug_bzlmod::BzlmodCellGraphDynamicAlias {
                apparent_name: "eager".to_owned(),
                canonical_name: "root+ext+eager".to_owned(),
            }]),
        };

        let snapshot = runtime_cell_install_snapshot(&cell_graph);

        assert_eq!(snapshot.extension_cells.len(), 2);
        assert_eq!(
            snapshot.extension_cells[0].registration,
            slug_core::cells::BzlmodRuntimeExtensionCellRegistration::Eager
        );
        assert!(snapshot.extension_cells[0].setup.materialized);
        assert_eq!(
            snapshot.extension_cells[1].registration,
            slug_core::cells::BzlmodRuntimeExtensionCellRegistration::Lazy
        );
        assert_eq!(
            snapshot.extension_cells[1].setup.repo_env_json.as_ref(),
            r#"{"K":"V"}"#
        );
        assert!(!snapshot.extension_cells[1].setup.materialized);
        assert_eq!(snapshot.scoped_aliases[0].apparent_name, "tool");
        assert_eq!(snapshot.dynamic_aliases[0].canonical_name, "root+ext+eager");
    }

    #[test]
    fn module_setup_derives_from_cell_graph() {
        let cell = slug_bzlmod::BzlmodCellGraphCell {
            name: "dep+".to_owned(),
            path: "bazel-external/dep+".to_owned(),
            module_setup: Some(slug_bzlmod::BzlmodCellGraphModuleSetup {
                module_name: "dep".to_owned(),
                version: "1.2.3".to_owned(),
                registry_url: "https://bcr.bazel.build".to_owned(),
                source_path: "/tmp/dep-src".to_owned(),
            }),
            bundled: false,
        };

        let setup = module_setup_from_cell_graph(&cell).unwrap();

        assert_eq!(setup.module_name.as_ref(), "dep");
        assert_eq!(setup.version.as_ref(), "1.2.3");
        assert_eq!(setup.registry_url.as_ref(), "https://bcr.bazel.build");
        assert_eq!(setup.source_path.as_ref(), "/tmp/dep-src");
    }

    #[tokio::test]
    async fn bzlmod_cell_resolver_uses_bundled_cells_from_cell_graph() -> slug_error::Result<()> {
        let workspace_id = slug_bzlmod::WorkspaceId::for_project_root(PathBuf::from(
            "/tmp/slug-plan61-bundled-cell-graph-test",
        ));
        let mut session_data = slug_bzlmod::BzlmodSessionData::for_workspace(workspace_id.clone());
        session_data.cell_graph = slug_bzlmod::BzlmodCellGraphValue {
            workspace_id,
            root_module_name: "root".to_owned(),
            cells: Arc::new(vec![slug_bzlmod::BzlmodCellGraphCell {
                name: "bazel_tools".to_owned(),
                path: "bazel_tools".to_owned(),
                module_setup: None,
                bundled: true,
            }]),
            extension_cells: Arc::new(Vec::new()),
            root_aliases: Arc::new(Vec::new()),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(Vec::new()),
            dynamic_aliases: Arc::new(Vec::new()),
        };

        let configs = BuckConfigBasedCells::parse_with_file_ops_and_options_inner(
            &[],
            None,
            None,
            None,
            Some(Arc::new(Some(session_data))),
        )
        .await?;
        let bazel_tools = CellName::unchecked_new("bazel_tools")?;
        let cell = configs.cell_resolver.get(bazel_tools)?;

        match cell.external() {
            Some(ExternalCellOrigin::Bundled(name)) => {
                assert_eq!(name.as_str(), "bazel_tools");
            }
            other => panic!("expected bazel_tools to be a bundled cell, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn generated_override_aliases_project_to_dynamic_runtime_aliases() {
        let alias = slug_bzlmod::RepoAlias {
            apparent_name: "root++ext+generated".to_owned(),
            canonical_name: "generated".to_owned(),
            declaring_module: None,
        };
        let scoped_alias = slug_bzlmod::RepoAlias {
            apparent_name: "generated".to_owned(),
            canonical_name: "actual_dep".to_owned(),
            declaring_module: Some("root+".to_owned()),
        };

        let dynamic_alias = dynamic_alias_for_generated_override(&alias, "generated")
            .expect("generated override alias should project to runtime dynamic alias");

        assert_eq!(dynamic_alias.apparent_name, "root++ext+generated");
        assert_eq!(dynamic_alias.canonical_name, "generated");
        assert!(dynamic_alias_for_generated_override(&scoped_alias, "actual_dep").is_none());
        assert!(dynamic_alias_for_generated_override(&alias, "root++ext+generated").is_none());
    }

    #[tokio::test]
    async fn tracked_extension_bzl_digest_matches_legacy_project_load_digest()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file(
            "ext.bzl",
            r#"
load(":helper.bzl", "HELPER")

def _impl(module_ctx):
    pass

ext = module_extension(implementation = _impl)
"#,
        );
        fs.write_file("helper.bzl", "HELPER = 'tracked'\n");

        let extension_id = "@@root//:ext.bzl%ext";
        let repo_mappings = slug_bzlmod::RepoMappingSnapshot::new();
        let direct = slug_bzlmod::compute_bzl_transitive_digest_for_project_with_repo_mappings(
            extension_id,
            fs.path().root().as_path(),
            Some(&repo_mappings),
        );
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let tracked = dice
            .compute(&TrackedExtensionBzlDigestKey {
                project_root: AbsNormPathBuf::try_from(fs.path().root().as_path().to_path_buf())?,
                extension_id: Arc::from(extension_id),
                repo_mappings: Arc::new(repo_mappings),
            })
            .await??;

        assert_eq!(tracked.as_ref(), direct);
        assert!(!<TrackedExtensionBzlDigestKey as Key>::validity(&Ok(
            tracked
        )));
        Ok(())
    }

    #[test]
    fn legacy_bzlmod_resolution_key_includes_hidden_lockfile_identity() {
        let first = legacy_bzlmod_resolution_key(lockfile_value(
            "/tmp/hidden/MODULE.bazel.lock",
            "hidden-lockfile-first",
        ));
        let second = legacy_bzlmod_resolution_key(lockfile_value(
            "/tmp/hidden/MODULE.bazel.lock",
            "hidden-lockfile-second",
        ));

        assert_ne!(first, second);

        let mut first_hasher = DefaultHasher::new();
        first.hash(&mut first_hasher);
        let mut second_hasher = DefaultHasher::new();
        second.hash(&mut second_hasher);

        assert_ne!(first_hasher.finish(), second_hasher.finish());
    }
}
