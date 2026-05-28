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
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::Key;
use dupe::Dupe;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod::BzlmodEventKind;
use slug_bzlmod::BzlmodResolutionOptions;
use slug_bzlmod::LocalOverrideModuleInputsValue;
use slug_bzlmod::ModuleCache;
use slug_bzlmod::ModuleSource;
use slug_bzlmod::NonRegistryOverrideModuleInputsValue;
use slug_bzlmod::NonRootModuleFileInput;
use slug_bzlmod::NonRootModuleFilesValue;
use slug_bzlmod::ParsedModuleFile;
use slug_bzlmod::RegistryFileInputsValue;
use slug_bzlmod::record_bzlmod_event;
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
use slug_fs::paths::RelativePath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;
use slug_fs::paths::abs_path::AbsPath;

use crate::cas_digest::CasDigestConfig;
use crate::dice::data::SetIoProvider;
use crate::external_cells::EXTERNAL_CELLS_IMPL;
use crate::file_ops::dice::DiceFileComputations;
use crate::file_ops::dice::register_bzlmod_config_project_file;
use crate::file_ops::metadata::FileType;
use crate::file_ops::metadata::RawPathMetadata;
use crate::io::fs::FsIoProvider;
use crate::legacy_configs::aggregator::CellsAggregator;
use crate::legacy_configs::args::ResolvedLegacyConfigArg;
use crate::legacy_configs::args::resolve_config_args;
use crate::legacy_configs::args::to_proto_config_args;
use crate::legacy_configs::cells_symlinks::cleanup_stale_symlinks;
use crate::legacy_configs::cells_symlinks::ensure_symlink;
use crate::legacy_configs::configs::LegacyBuckConfig;
use crate::legacy_configs::dice::HasInjectedLegacyConfigs;
use crate::legacy_configs::key::BuckconfigKeyRef;

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

async fn resolve_use_repo_rule_local_bits(
    ctx: &mut DiceComputations<'_>,
    cells: &mut [slug_bzlmod::PendingRepoCell],
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
) -> slug_error::Result<()> {
    let Ok(executor) = slug_bzlmod::STARLARK_REPO_RULE_EXECUTOR_IMPL.get() else {
        return Ok(());
    };

    let mut declaring_cells = HashMap::new();
    for (_cell_name, parsed) in parsed_modules {
        let module_name = if parsed.module.name.is_empty() {
            root_module_name
        } else {
            &parsed.module.name
        };
        for invocation in &parsed.repo_rule_invocations {
            let rule_source = canonicalize_use_repo_rule_source(
                &invocation.rule_source,
                module_name,
                module_name == root_module_name,
            );
            declaring_cells.insert(rule_source, module_name.to_owned());
        }
    }

    for cell in cells {
        if cell.repo_spec_json.is_empty() {
            continue;
        }

        let mut repo_spec: slug_bzlmod::RepoSpec = serde_json::from_str(&cell.repo_spec_json)
            .with_buck_error_context(|| {
                format!(
                    "Failed to parse precomputed RepoSpec for '{}'",
                    cell.canonical_name
                )
            })?;
        if repo_spec.local || cell.extension_id != repo_spec.repo_rule_id {
            continue;
        }

        let Some((rule_bzl_path, rule_name)) = repo_spec.repo_rule_id.rsplit_once('%') else {
            continue;
        };
        if rule_bzl_path.starts_with('@') {
            continue;
        }
        if !declaring_cells.contains_key(&repo_spec.repo_rule_id) {
            continue;
        }
        let is_local = match executor.rule_is_local(ctx, rule_bzl_path, rule_name).await {
            Ok(is_local) => is_local,
            Err(e) => {
                tracing::debug!(
                    "Failed to precompute repository rule local bit for '{}' on '{}': {}; \
                     deferring to normal repository rule execution",
                    repo_spec.repo_rule_id,
                    cell.canonical_name,
                    e
                );
                false
            }
        };
        if is_local {
            repo_spec.local = true;
            cell.spec_hash = repo_spec.compute_hash();
            cell.repo_spec_json =
                serde_json::to_string(&repo_spec).with_buck_error_context(|| {
                    format!(
                        "Failed to serialize local RepoSpec for '{}'",
                        cell.canonical_name
                    )
                })?;
        }
    }

    Ok(())
}

fn canonicalize_use_repo_rule_source(
    rule_source: &str,
    module_name: &str,
    is_root: bool,
) -> String {
    let Some((bzl_file, rule_name)) = rule_source.rsplit_once('%') else {
        return rule_source.to_owned();
    };

    let resolved = if !is_root {
        if bzl_file.starts_with("//") {
            format!("@{}{}", module_name, bzl_file)
        } else if let Some(rest) = bzl_file.strip_prefix(':') {
            format!("@{}//:{}", module_name, rest)
        } else {
            bzl_file.to_owned()
        }
    } else if let Some(rest) = bzl_file.strip_prefix(':') {
        format!("//:{}", rest)
    } else {
        bzl_file.to_owned()
    };

    format!("{resolved}%{rule_name}")
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

fn canonicalize_repo_mapping_snapshot_targets(
    snapshot: &mut slug_bzlmod::RepoMappingSnapshot,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
) {
    for mapping in snapshot.values_mut() {
        for target_name in mapping.values_mut() {
            *target_name =
                canonical_repo_mapping_target_name(None, cells, resolved_graph, target_name);
        }
    }

    let root_repo_mapping = snapshot.get("").cloned();
    for mapping in snapshot.values_mut() {
        for target_name in mapping.values_mut() {
            *target_name = canonical_repo_mapping_target_name(
                root_repo_mapping.as_ref(),
                cells,
                resolved_graph,
                target_name,
            );
        }
    }
}

fn canonical_repo_mapping_target_name(
    root_repo_mapping: Option<&BTreeMap<String, String>>,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
    target_name: &str,
) -> String {
    let mut current = target_name.to_owned();
    let mut seen = BTreeSet::new();

    loop {
        if !seen.insert(current.clone()) {
            return current;
        }
        let next = root_repo_mapping
            .and_then(|mapping| mapping.get(&current))
            .cloned()
            .or_else(|| {
                resolved_graph
                    .and_then(|graph| selected_bzlmod_cell_name_for_dep(cells, &current, graph))
                    .map(str::to_owned)
            });
        let Some(next) = next else {
            return current;
        };
        if next == current {
            return current;
        }
        current = next;
    }
}

fn canonicalize_repo_mapping_targets(
    repo_mappings: &mut slug_bzlmod::RepoMappingSnapshot,
    repo_mapping_overrides: &mut slug_bzlmod::RepoMappingOverrides,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
) {
    canonicalize_repo_mapping_snapshot_targets(repo_mappings, cells, resolved_graph);
    canonicalize_repo_mapping_overrides_targets(
        repo_mapping_overrides,
        repo_mappings,
        cells,
        resolved_graph,
    );
}

fn canonicalize_repo_mapping_overrides_targets(
    overrides: &mut slug_bzlmod::RepoMappingOverrides,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
) {
    let root_repo_mapping = repo_mappings.get("");
    for overrides in overrides.values_mut() {
        for target_name in overrides.values_mut() {
            *target_name = canonical_repo_mapping_target_name(
                root_repo_mapping,
                cells,
                resolved_graph,
                target_name,
            );
        }
    }
}

fn graph_owned_repo_mapping_state(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
    cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    resolved_graph: Option<&slug_bzlmod::ResolvedGraph>,
) -> (
    slug_bzlmod::RepoMappingSnapshot,
    slug_bzlmod::RepoMappingOverrides,
) {
    let mut repo_mappings = repo_mapping_snapshot_for_modules(parsed_modules, root_module_name);
    let mut repo_mapping_overrides =
        repo_mapping_overrides_for_root(parsed_modules, root_module_name, ignore_dev_dependency);
    canonicalize_repo_mapping_targets(
        &mut repo_mappings,
        &mut repo_mapping_overrides,
        cells,
        resolved_graph,
    );
    (repo_mappings, repo_mapping_overrides)
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
    slug_core::cells::reset_dynamic_bzlmod_state_for_workspace(
        project_root.root().to_path_buf(),
        cell_graph.workspace_id.output_base.as_ref().clone(),
    );

    let external_base_dir = project_root.root().as_path().join("bazel-external");
    let buck_out_external_cells_dir = cell_graph
        .workspace_id
        .output_base
        .as_ref()
        .join("external_cells/bzlmod");
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

// Instrumentation-only caches used by Plan 61 guardrails to distinguish a
// semantic clean-graph input change from no-op validation of polled inputs.
static LAST_RECORDED_BZLMOD_RESOLUTION_DIGEST: OnceLock<Mutex<HashMap<String, String>>> =
    OnceLock::new();
static LAST_RECORDED_POLLED_MODULE_PARSE_DIGEST: OnceLock<Mutex<HashMap<PathBuf, String>>> =
    OnceLock::new();
static LAST_RECORDED_LOCKFILE_READ_DIGEST: OnceLock<Mutex<HashMap<PathBuf, String>>> =
    OnceLock::new();

fn bzlmod_resolved_module_graph_key_identity_digest(key: &BzlmodResolvedModuleGraphKey) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn record_clean_bzlmod_resolution_compute_if_changed(key: &BzlmodResolvedModuleGraphKey) {
    let cache_key = format!(
        "{}:{}",
        key.resolution_key.workspace_id.stable_hash(),
        key.resolution_key.command_policy_digest
    );
    let input_digest = bzlmod_resolved_module_graph_key_identity_digest(key);
    let mut last = LAST_RECORDED_BZLMOD_RESOLUTION_DIGEST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if last.get(&cache_key).map(String::as_str) == Some(input_digest.as_str()) {
        return;
    }
    last.insert(cache_key, input_digest);
    record_bzlmod_event(
        BzlmodEventKind::BzlmodResolutionCompute,
        key.root_module_file.path.display().to_string(),
    );
}

fn record_polled_module_parse_if_changed(path: &Path, digest: &str) {
    let mut last = LAST_RECORDED_POLLED_MODULE_PARSE_DIGEST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if last.get(path).map(String::as_str) == Some(digest) {
        return;
    }
    last.insert(path.to_path_buf(), digest.to_owned());
    record_bzlmod_event(BzlmodEventKind::ModuleFileParse, path.display().to_string());
}

fn record_lockfile_read_if_changed(path: &Path, digest: &str) {
    let mut last = LAST_RECORDED_LOCKFILE_READ_DIGEST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if last.get(path).map(String::as_str) == Some(digest) {
        return;
    }
    last.insert(path.to_path_buf(), digest.to_owned());
    record_bzlmod_event(BzlmodEventKind::LockfileRead, path.display().to_string());
}

fn bzlmod_resolution_options_from_config(
    root_config: &LegacyBuckConfig,
) -> slug_error::Result<BzlmodResolutionOptions> {
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
    Ok(BzlmodResolutionOptions {
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
            DiceFileComputations::read_project_file_if_exists(ctx, root_project_path)
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse root MODULE.bazel at {}",
                        root_path.display()
                    )
                })?
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
            false,
        )
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to parse root MODULE.bazel at {}",
                root_path.display()
            )
        })?;
        let input_digest =
            slug_bzlmod::module_file_inputs_digest(&parsed_with_inputs.parsed_with_inputs.inputs);
        let input_count = parsed_with_inputs.parsed_with_inputs.inputs.len();

        Ok(Arc::new(slug_bzlmod::RootModuleFileValue {
            path: root_path,
            input_digest: Some(input_digest),
            input_count,
            parsed: Some(parsed_with_inputs.parsed_with_inputs.parsed),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.path == y.path && x.input_digest == y.input_digest,
            _ => false,
        }
    }
}

struct ParsedModuleFileWithInputTracking {
    parsed_with_inputs: slug_bzlmod::ParsedModuleFileWithInputs,
    has_untracked_inputs: bool,
}

async fn parse_module_with_tracked_project_includes(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    module_path: &Path,
    module_content: String,
    validate_extension_repo_directives: bool,
) -> slug_error::Result<ParsedModuleFileWithInputTracking> {
    let module_root = module_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let mut session = if validate_extension_repo_directives {
        slug_bzlmod::ModuleFileParseSession::new(module_root.clone())
    } else {
        slug_bzlmod::ModuleFileParseSession::new(module_root.clone())
            .allow_ignored_extension_repo_directives()
    };
    let module_digest = slug_bzlmod::compute_sha256_hex(module_content.as_bytes());
    let include_labels = session.eval_segment(module_path, &module_content, module_digest)?;
    let mut pending = Vec::new();
    let mut has_untracked_inputs = false;
    push_pending_include_labels(&mut pending, include_labels, Vec::new());

    while let Some((label, ancestors)) = pending.pop() {
        let include_path = slug_bzlmod::include_label_to_path(&module_root, &label)?;
        let canonical = include_path
            .canonicalize()
            .unwrap_or_else(|_| include_path.clone());
        if ancestors.contains(&canonical) {
            return Err(slug_bzlmod::ModuleParseError::IncludeError(format!(
                "cyclic include of {}",
                label
            ))
            .into());
        }

        let (include_read, tracking) = read_bzlmod_file_for_module_inputs(
            ctx,
            project_fs,
            &include_path,
        )
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to read included MODULE.bazel segment at {:?}: not UTF-8 or unreadable",
                include_path
            )
        })?;
        if tracking != BzlmodFileInputTracking::Project {
            has_untracked_inputs = true;
        }
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

    Ok(ParsedModuleFileWithInputTracking {
        parsed_with_inputs: session.finish()?,
        has_untracked_inputs,
    })
}

fn parse_module_with_polled_includes(
    module_path: &Path,
    module_content: String,
    validate_extension_repo_directives: bool,
) -> slug_error::Result<slug_bzlmod::ParsedModuleFileWithInputs> {
    let module_root = module_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let mut session = if validate_extension_repo_directives {
        slug_bzlmod::ModuleFileParseSession::new_silent(module_root.clone())
    } else {
        slug_bzlmod::ModuleFileParseSession::new_silent(module_root.clone())
            .allow_ignored_extension_repo_directives()
    };
    let module_digest = slug_bzlmod::compute_sha256_hex(module_content.as_bytes());
    let include_labels = session.eval_segment(module_path, &module_content, module_digest)?;
    let mut pending = Vec::new();
    push_pending_include_labels(&mut pending, include_labels, Vec::new());

    while let Some((label, ancestors)) = pending.pop() {
        let include_path = slug_bzlmod::include_label_to_path(&module_root, &label)?;
        let canonical = include_path
            .canonicalize()
            .unwrap_or_else(|_| include_path.clone());
        if ancestors.contains(&canonical) {
            return Err(slug_bzlmod::ModuleParseError::IncludeError(format!(
                "cyclic include of {}",
                label
            ))
            .into());
        }

        let (include_content, include_digest) = read_absolute_text_file_input(&include_path)
            .with_buck_error_context(|| {
                format!(
                    "Failed to read included MODULE.bazel segment at {:?}: not UTF-8 or unreadable",
                    include_path
                )
            })?;
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct AbsoluteTextFileInputValue {
    content: Option<String>,
    digest: Option<String>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("AbsoluteTextFileInputKey({})", path.display())]
struct AbsoluteTextFileInputKey {
    path: Arc<PathBuf>,
}

#[async_trait]
impl Key for AbsoluteTextFileInputKey {
    type Value = slug_error::Result<Arc<AbsoluteTextFileInputValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Ok(Arc::new(read_absolute_text_file_input_value(&self.path)?))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.content == y.content && x.digest == y.digest,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        // Out-of-project bzlmod inputs still poll disk directly in this child
        // key until a lower-level watched filesystem key is available.
        false
    }
}

async fn read_bzlmod_file_for_module_inputs(
    ctx: &mut DiceComputations<'_>,
    project_fs: &ProjectRoot,
    path: &Path,
) -> slug_error::Result<(Option<(String, String)>, BzlmodFileInputTracking)> {
    if let Some(project_path) = project_relative_path_for_abs_path(project_fs, path) {
        register_bzlmod_config_project_file(project_path.clone());
        let Some(content) =
            DiceFileComputations::read_project_file_if_exists(ctx, &project_path).await?
        else {
            return Ok((None, BzlmodFileInputTracking::Project));
        };
        let digest = slug_bzlmod::compute_sha256_hex(content.as_bytes());
        return Ok((Some((content, digest)), BzlmodFileInputTracking::Project));
    }

    let input = read_absolute_text_file_input_via_dice(ctx, path).await?;
    Ok((
        input
            .content
            .clone()
            .zip(input.digest.clone())
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
        register_bzlmod_config_project_file(project_path.clone());
        return Ok((
            DiceFileComputations::read_project_file_if_exists(ctx, &project_path).await?,
            BzlmodFileInputTracking::Project,
        ));
    }

    let input = read_absolute_text_file_input_via_dice(ctx, path).await?;
    Ok((input.content.clone(), BzlmodFileInputTracking::Polled))
}

async fn read_absolute_text_file_input_via_dice(
    ctx: &mut DiceComputations<'_>,
    path: &Path,
) -> slug_error::Result<Arc<AbsoluteTextFileInputValue>> {
    ctx.compute(&AbsoluteTextFileInputKey {
        path: Arc::new(path.to_path_buf()),
    })
    .await?
}

fn read_absolute_text_file_input_value(
    path: &Path,
) -> slug_error::Result<AbsoluteTextFileInputValue> {
    let (content, digest) = read_absolute_text_file_input(path)?;
    Ok(AbsoluteTextFileInputValue { content, digest })
}

fn read_absolute_text_file_input(
    path: &Path,
) -> slug_error::Result<(Option<String>, Option<String>)> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
        Err(e) => return Err(e.into()),
    };
    let digest = slug_bzlmod::compute_sha256_hex(&bytes);
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

#[cfg(test)]
fn absolute_text_file_digest(path: &Path) -> slug_error::Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(slug_bzlmod::compute_sha256_hex(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("TrackedLockfileContentKey({:?}, {})", kind, path.display())]
struct TrackedLockfileContentKey {
    project_root: AbsNormPathBuf,
    kind: slug_bzlmod::LockfileContentKind,
    path: Arc<PathBuf>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodLockfileInputsBridgeKey({}, {:?})",
    workspace_id.stable_hash(),
    lockfile_mode
)]
struct BzlmodLockfileInputsBridgeKey {
    project_root: AbsNormPathBuf,
    workspace_id: slug_bzlmod::WorkspaceId,
    lockfile_mode: slug_bzlmod::LockfileMode,
    hidden_lockfile_path: Option<PathBuf>,
    root_module_present: bool,
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
        let (content, tracking) =
            read_text_file_for_project_input(ctx, &project_fs, &self.path).await?;
        let tracked_by_dice = tracking == BzlmodFileInputTracking::Project;
        let path = self.path.clone();
        let Some(content) = content else {
            return Ok(Arc::new(slug_bzlmod::LockfileContentValue {
                path,
                digest: None,
                tracked_by_dice,
                lockfile: None,
            }));
        };

        let digest = slug_bzlmod::compute_sri_hash(content.as_bytes());
        record_lockfile_read_if_changed(&path, &digest);
        match slug_bzlmod::parse_lockfile_content(&path, &content) {
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

#[async_trait]
impl Key for BzlmodLockfileInputsBridgeKey {
    type Value = slug_error::Result<Arc<slug_bzlmod::BzlmodLockfileInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if !self.root_module_present || self.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            return Ok(Arc::new(
                slug_bzlmod::BzlmodLockfileInputsValue::from_values(
                    self.hidden_lockfile_path.clone(),
                    None,
                    None,
                    self.lockfile_mode,
                ),
            ));
        }

        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        let visible_path = slug_bzlmod::lockfile_path(project_fs.root().as_path());
        let visible_lockfile = ctx
            .compute(&TrackedLockfileContentKey {
                project_root: self.project_root.clone(),
                kind: slug_bzlmod::LockfileContentKind::Workspace,
                path: Arc::new(visible_path),
            })
            .await?
            .buck_error_context("Computing visible MODULE.bazel.lock for bzlmod resolution")?;
        let hidden_lockfile = match &self.hidden_lockfile_path {
            Some(path) => Some(
                ctx.compute(&TrackedLockfileContentKey {
                    project_root: self.project_root.clone(),
                    kind: slug_bzlmod::LockfileContentKind::Hidden,
                    path: Arc::new(path.clone()),
                })
                .await?
                .buck_error_context(
                    "Computing hidden MODULE.bazel lockfile for bzlmod resolution",
                )?,
            ),
            None => None,
        };

        Ok(Arc::new(
            slug_bzlmod::BzlmodLockfileInputsValue::from_values(
                self.hidden_lockfile_path.clone(),
                Some(visible_lockfile),
                hidden_lockfile,
                self.lockfile_mode,
            ),
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.identity_eq(y),
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

fn bzlmod_resolution_options_policy_eq(
    left: &BzlmodResolutionOptions,
    right: &BzlmodResolutionOptions,
) -> bool {
    left == right
}

fn hash_bzlmod_resolution_options_policy<H: std::hash::Hasher>(
    value: &BzlmodResolutionOptions,
    state: &mut H,
) {
    value.hash(state);
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "TrackedScannedExtensionBzlDigestKey({}, {})",
    project_root.display(),
    extension_id
)]
struct FallbackScannedExtensionBzlDigestKey {
    project_root: AbsNormPathBuf,
    extension_id: Arc<str>,
    repo_mappings: Arc<slug_bzlmod::RepoMappingSnapshot>,
}

#[async_trait]
impl Key for FallbackScannedExtensionBzlDigestKey {
    type Value = slug_error::Result<Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        tracked_scanned_extension_bzl_digest(
            ctx,
            self.extension_id.as_ref(),
            self.project_root.as_path(),
            self.repo_mappings.as_ref(),
        )
        .await
        .map(|digest| Arc::from(digest.as_str()))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

fn tracked_project_bzl_location(
    project_root: &Path,
    repo: &str,
    package: &str,
    name: &str,
) -> slug_bzlmod::BzlLoadLocation {
    let mut path = if repo.is_empty() || repo == "_main" {
        project_root.to_path_buf()
    } else {
        project_root.join("bazel-external").join(repo)
    };
    if !package.is_empty() {
        path.push(package);
    }
    path.push(name);
    slug_bzlmod::BzlLoadLocation {
        path,
        repo: repo.to_owned(),
        package: package.to_owned(),
    }
}

fn tracked_external_repo_candidate(repo: &str) -> String {
    if repo.ends_with('+') || repo.contains('+') {
        repo.to_owned()
    } else {
        format!("{repo}+")
    }
}

fn tracked_mapping_for_source_repo<'a>(
    repo_mappings: &'a slug_bzlmod::RepoMappingSnapshot,
    current_repo: &str,
) -> Option<&'a BTreeMap<String, String>> {
    let mut candidates = Vec::new();
    candidates.push(current_repo.to_owned());
    if current_repo.is_empty() || current_repo == "_main" {
        candidates.push("_main".to_owned());
        candidates.push(String::new());
    } else if current_repo.ends_with('+') {
        candidates.push(current_repo.trim_end_matches('+').to_owned());
    } else if !current_repo.contains('+') {
        candidates.push(format!("{current_repo}+"));
    }
    for candidate in candidates {
        if let Some(mapping) = repo_mappings.get(&candidate) {
            return Some(mapping);
        }
    }
    None
}

fn tracked_split_bzl_label_target(target: &str) -> Option<(&str, &str)> {
    target.split_once(':')
}

fn tracked_label_bzl_location_under_project(
    label: &str,
    project_root: &Path,
    current: Option<&slug_bzlmod::BzlLoadLocation>,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
) -> Option<slug_bzlmod::BzlLoadLocation> {
    if let Some(rest) = label.strip_prefix("@@") {
        let (repo, target) = rest.split_once("//")?;
        let (pkg, name) = tracked_split_bzl_label_target(target)?;
        if !repo.contains('+') {
            return Some(tracked_project_bzl_location(project_root, "", pkg, name));
        }
        return Some(tracked_project_bzl_location(
            project_root,
            &tracked_external_repo_candidate(repo),
            pkg,
            name,
        ));
    }
    if let Some(rest) = label.strip_prefix('@') {
        let (repo, target) = rest.split_once("//")?;
        let (pkg, name) = tracked_split_bzl_label_target(target)?;
        let repo = if repo.contains('+') {
            repo.to_owned()
        } else if let Some(current) = current {
            tracked_mapping_for_source_repo(repo_mappings, &current.repo)
                .and_then(|mapping| mapping.get(repo).cloned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        return Some(tracked_project_bzl_location(project_root, &repo, pkg, name));
    }
    if let Some(rest) = label.strip_prefix("//") {
        let current_repo = current.map(|location| location.repo.as_str()).unwrap_or("");
        let (pkg, name) = tracked_split_bzl_label_target(rest)?;
        return Some(tracked_project_bzl_location(
            project_root,
            current_repo,
            pkg,
            name,
        ));
    }
    if let Some(name) = label.strip_prefix(':') {
        let current = current?;
        return Some(tracked_project_bzl_location(
            project_root,
            &current.repo,
            &current.package,
            name,
        ));
    }
    if label.contains("//") {
        let (repo, target) = label.split_once("//")?;
        let (pkg, name) = tracked_split_bzl_label_target(target)?;
        let repo = if repo.contains('+') {
            repo.to_owned()
        } else if let Some(current) = current {
            tracked_mapping_for_source_repo(repo_mappings, &current.repo)
                .and_then(|mapping| mapping.get(repo).cloned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        return Some(tracked_project_bzl_location(project_root, &repo, pkg, name));
    }
    let current = current?;
    let mut path = current.path.parent()?.to_path_buf();
    path.push(label);
    Some(slug_bzlmod::BzlLoadLocation {
        path,
        repo: current.repo.clone(),
        package: current.package.clone(),
    })
}

async fn tracked_read_bzl_location(
    ctx: &mut DiceComputations<'_>,
    project_root: &Path,
    location: &slug_bzlmod::BzlLoadLocation,
) -> slug_error::Result<Option<(String, String)>> {
    let Ok(rel) = location.path.strip_prefix(project_root) else {
        return Ok(None);
    };
    let rel = rel.to_string_lossy();
    let project_path = ProjectRelativePath::new(rel.as_ref())?;
    register_bzlmod_config_project_file(project_path.to_owned());
    let content = DiceFileComputations::read_project_file_if_exists(ctx, project_path).await?;
    Ok(Some((
        rel.into_owned(),
        content.unwrap_or_else(|| "read_error:missing".to_owned()),
    )))
}

async fn tracked_scanned_extension_bzl_digest(
    ctx: &mut DiceComputations<'_>,
    extension_id: &str,
    project_root: &Path,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
) -> slug_error::Result<String> {
    let label = extension_id.split('%').next().unwrap_or(extension_id);
    let Some(root_location) =
        tracked_label_bzl_location_under_project(label, project_root, None, repo_mappings)
    else {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    };

    let Some((root_rel, root_content)) =
        tracked_read_bzl_location(ctx, project_root, &root_location).await?
    else {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    };
    if root_content == "read_error:missing" {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    }

    let mut seen_locations = BTreeSet::new();
    let mut file_contents = BTreeMap::new();
    let mut queue = VecDeque::from([(root_location, root_rel, root_content)]);
    while let Some((location, rel, content)) = queue.pop_front() {
        let key = (
            location.path.clone(),
            location.repo.clone(),
            location.package.clone(),
        );
        if !seen_locations.insert(key) {
            continue;
        }
        file_contents.insert(rel, content.clone());
        if content == "read_error:missing" {
            continue;
        }
        for load in slug_bzlmod::literal_loads(&location.path, &content) {
            let Some(load_location) = tracked_label_bzl_location_under_project(
                &load,
                project_root,
                Some(&location),
                repo_mappings,
            ) else {
                continue;
            };
            if !load_location.path.starts_with(project_root) {
                continue;
            }
            if let Some((load_rel, load_content)) =
                tracked_read_bzl_location(ctx, project_root, &load_location).await?
            {
                queue.push_back((load_location, load_rel, load_content));
            }
        }
    }

    if file_contents.is_empty() {
        return Ok(slug_bzlmod::compute_bzl_transitive_digest(extension_id));
    }
    Ok(slug_bzlmod::compute_bzl_transitive_digest_from_file_contents(extension_id, &file_contents))
}

async fn fallback_scanned_extension_bzl_digests_for_lockfile_preseed(
    ctx: &mut DiceComputations<'_>,
    project_root: &AbsNormPathBuf,
    aggregated: &HashMap<String, slug_bzlmod::AggregatedExtension>,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
) -> slug_error::Result<HashMap<String, String>> {
    let repo_mappings = Arc::new(repo_mappings.clone());
    let mut digests = HashMap::new();
    for extension_id in aggregated.keys() {
        let key = FallbackScannedExtensionBzlDigestKey {
            project_root: project_root.clone(),
            extension_id: Arc::from(extension_id.as_str()),
            repo_mappings: repo_mappings.clone(),
        };
        let digest = ctx.compute(&key).await??;
        digests.insert(extension_id.clone(), digest.to_string());
    }
    Ok(digests)
}

fn current_extension_for_lockfile_preseed<'a>(
    aggregated: &'a HashMap<String, slug_bzlmod::AggregatedExtension>,
    lockfile_ext_id: &'a str,
) -> Option<(&'a str, &'a slug_bzlmod::AggregatedExtension)> {
    if let Some(extension) = aggregated.get(lockfile_ext_id) {
        return Some((lockfile_ext_id, extension));
    }

    let lockfile_canonical = slug_bzlmod::lockfile_canonical_extension_id(lockfile_ext_id);
    aggregated
        .iter()
        .find(|(current_id, _)| {
            slug_bzlmod::lockfile_canonical_extension_id(current_id) == lockfile_canonical
        })
        .map(|(current_id, extension)| (current_id.as_str(), extension))
}

async fn prevalidated_extension_caches_for_lockfile_preseed(
    ctx: &mut DiceComputations<'_>,
    lockfile: &slug_bzlmod::Lockfile,
    aggregated: &HashMap<String, slug_bzlmod::AggregatedExtension>,
    root_module_name: &str,
    workspace_root: &Path,
    repo_env: &BTreeMap<String, String>,
    bzl_transitive_digests: &HashMap<String, String>,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
    repo_mapping_overrides: &slug_bzlmod::RepoMappingOverrides,
) -> slug_error::Result<fxhash::FxHashMap<String, fxhash::FxHashMap<String, slug_bzlmod::RepoSpec>>>
{
    let mut caches = fxhash::FxHashMap::default();
    for ext_id in lockfile.extension_ids() {
        if ext_id.starts_with(':') || ext_id.starts_with("//") {
            continue;
        }
        let Some((current_ext_id, current_extension)) =
            current_extension_for_lockfile_preseed(aggregated, ext_id)
        else {
            continue;
        };
        let Some(bzl_transitive_digest) = bzl_transitive_digests.get(current_ext_id) else {
            continue;
        };
        let usages_digest = slug_bzlmod::compute_extension_input_hash(current_extension);
        let Some(selected) = lockfile.select_extension_cache_for_workspace(
            current_ext_id,
            bzl_transitive_digest,
            &usages_digest,
            Some(workspace_root),
            Some(repo_env),
            Some(repo_mappings),
            Some(root_module_name),
            Some(repo_mapping_overrides),
        ) else {
            continue;
        };
        if selected_cache_recorded_inputs_current_for_preseed(ctx, current_ext_id, &selected)
            .await?
        {
            selected.record_hit(current_ext_id);
            caches.insert(current_ext_id.to_owned(), selected.repo_specs().clone());
        }
    }
    Ok(caches)
}

async fn selected_cache_recorded_inputs_current_for_preseed(
    ctx: &mut DiceComputations<'_>,
    extension_id: &str,
    selected: &slug_bzlmod::SelectedExtensionCache,
) -> slug_error::Result<bool> {
    let key = PreseedRecordedInputsKey::from_selected_cache(selected);
    match ctx.compute(&key).await? {
        Ok(PreseedRecordedInputsValue::Current) => return Ok(true),
        Ok(PreseedRecordedInputsValue::Stale(reason)) => {
            record_bzlmod_event(
                BzlmodEventKind::ExtensionReplayMissReason,
                format!("{extension_id}:recorded_inputs:{reason}"),
            );
            return Ok(false);
        }
        Ok(PreseedRecordedInputsValue::Unsupported) => {}
        Err(e) => return Err(e),
    }

    slug_bzlmod::selected_cache_recorded_inputs_current(ctx, extension_id, selected).await
}

#[derive(Clone, Dupe, Debug, PartialEq, Eq, Allocative)]
enum PreseedRecordedInputsValue {
    Current,
    Stale(Arc<str>),
    Unsupported,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("PreseedRecordedInputsKey({})", recorded_inputs.len())]
struct PreseedRecordedInputsKey {
    recorded_inputs: Arc<Vec<String>>,
    workspace_root: Option<Arc<PathBuf>>,
    repo_env: Option<Arc<BTreeMap<String, String>>>,
    repo_mappings: Option<Arc<slug_bzlmod::RepoMappingSnapshot>>,
}

impl PreseedRecordedInputsKey {
    fn from_selected_cache(selected: &slug_bzlmod::SelectedExtensionCache) -> Self {
        Self {
            recorded_inputs: Arc::new(selected.recorded_inputs().to_vec()),
            workspace_root: selected
                .workspace_root()
                .map(|path| Arc::new(path.to_path_buf())),
            repo_env: selected.repo_env().map(|env| Arc::new(env.clone())),
            repo_mappings: selected
                .repo_mappings()
                .map(|mappings| Arc::new(mappings.clone())),
        }
    }
}

impl Dupe for PreseedRecordedInputsKey {
    fn dupe(&self) -> Self {
        Self {
            recorded_inputs: self.recorded_inputs.dupe(),
            workspace_root: self.workspace_root.dupe(),
            repo_env: self.repo_env.dupe(),
            repo_mappings: self.repo_mappings.dupe(),
        }
    }
}

#[async_trait]
impl Key for PreseedRecordedInputsKey {
    type Value = slug_error::Result<PreseedRecordedInputsValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match preseed_recorded_inputs_current_through_dice(
            ctx,
            self.recorded_inputs.as_slice(),
            self.workspace_root.as_deref().map(PathBuf::as_path),
            self.repo_env.as_deref(),
            self.repo_mappings.as_deref(),
        )
        .await?
        {
            Some(Ok(())) => Ok(PreseedRecordedInputsValue::Current),
            Some(Err(reason)) => Ok(PreseedRecordedInputsValue::Stale(Arc::from(reason))),
            None => Ok(PreseedRecordedInputsValue::Unsupported),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

async fn preseed_recorded_inputs_current_through_dice(
    ctx: &mut DiceComputations<'_>,
    recorded_inputs: &[String],
    workspace_root: Option<&Path>,
    repo_env: Option<&BTreeMap<String, String>>,
    repo_mappings: Option<&slug_bzlmod::RepoMappingSnapshot>,
) -> slug_error::Result<Option<Result<(), String>>> {
    for raw in recorded_inputs {
        let Some((input, old_value)) = parse_preseed_recorded_input_with_value(raw) else {
            return Ok(Some(Err("recorded_input_malformed".to_owned())));
        };
        match input {
            PreseedRecordedInput::File(path) => {
                let Some(old_value) = old_value else {
                    return Ok(Some(Err("recorded_input_malformed".to_owned())));
                };
                let Some(project_path) = preseed_recorded_file_project_path(&path, workspace_root)
                else {
                    return Ok(None);
                };
                let Some(current) =
                    preseed_recorded_file_marker_value(ctx, project_path.as_ref()).await?
                else {
                    return Ok(None);
                };
                if current != old_value {
                    return Ok(Some(Err(recorded_input_changed_reason_with_values(
                        raw, &old_value, &current,
                    ))));
                }
            }
            PreseedRecordedInput::Dirents(path) => {
                let Some(old_value) = old_value else {
                    return Ok(Some(Err("recorded_input_malformed".to_owned())));
                };
                let Some(project_path) = preseed_recorded_project_path(&path, workspace_root)
                else {
                    return Ok(None);
                };
                let current =
                    preseed_recorded_dirents_marker_value(ctx, project_path.as_ref()).await?;
                if current != old_value {
                    return Ok(Some(Err(recorded_input_changed_reason(raw))));
                }
            }
            PreseedRecordedInput::DirTree(path) => {
                let Some(old_value) = old_value else {
                    return Ok(Some(Err("recorded_input_malformed".to_owned())));
                };
                let Some(project_path) = preseed_recorded_project_path(&path, workspace_root)
                else {
                    return Ok(None);
                };
                let Some(current) =
                    preseed_recorded_dirtree_marker_value(ctx, project_path.as_ref()).await?
                else {
                    return Ok(None);
                };
                if current != old_value {
                    return Ok(Some(Err(recorded_input_changed_reason(raw))));
                }
            }
            PreseedRecordedInput::Env(name) => {
                let Some(repo_env) = repo_env else {
                    return Ok(Some(Err("recorded_input_unsupported".to_owned())));
                };
                let current = repo_env.get(&name).cloned();
                if current != old_value {
                    return Ok(Some(Err(recorded_input_changed_reason(raw))));
                }
            }
            PreseedRecordedInput::RepoMapping {
                source_repo,
                apparent,
            } => {
                let Some(repo_mappings) = repo_mappings else {
                    return Ok(Some(Err("recorded_input_unsupported".to_owned())));
                };
                let Some(source_mapping) = repo_mappings.get(&source_repo) else {
                    return Ok(Some(Err(recorded_input_changed_reason(raw))));
                };
                let current = source_mapping
                    .get(&apparent)
                    .cloned()
                    .or_else(|| Some(apparent.clone()));
                if current != old_value {
                    return Ok(Some(Err(recorded_input_changed_reason(raw))));
                }
            }
            PreseedRecordedInput::Unsupported => return Ok(None),
        }
    }
    Ok(Some(Ok(())))
}

enum PreseedRecordedInput {
    File(PathBuf),
    Dirents(PathBuf),
    DirTree(PathBuf),
    Env(String),
    RepoMapping {
        source_repo: String,
        apparent: String,
    },
    Unsupported,
}

fn parse_preseed_recorded_input_with_value(
    raw: &str,
) -> Option<(PreseedRecordedInput, Option<String>)> {
    let space = raw.find(' ')?;
    if space == 0 {
        return None;
    }
    let input = unescape_preseed_recorded_input_part(&raw[..space])?;
    let value = unescape_preseed_recorded_input_part(&raw[space + 1..]);
    Some((parse_preseed_recorded_input(&input), value))
}

fn parse_preseed_recorded_input(input: &str) -> PreseedRecordedInput {
    let Some((kind, payload)) = input.split_once(':') else {
        return PreseedRecordedInput::Unsupported;
    };
    match kind {
        "FILE" => PreseedRecordedInput::File(PathBuf::from(payload)),
        "DIRENTS" => PreseedRecordedInput::Dirents(PathBuf::from(payload)),
        "DIRTREE" => PreseedRecordedInput::DirTree(PathBuf::from(payload)),
        "ENV" => PreseedRecordedInput::Env(payload.to_owned()),
        "REPO_MAPPING" => payload
            .split_once(',')
            .map(
                |(source_repo, apparent)| PreseedRecordedInput::RepoMapping {
                    source_repo: source_repo.to_owned(),
                    apparent: apparent.to_owned(),
                },
            )
            .unwrap_or(PreseedRecordedInput::Unsupported),
        _ => PreseedRecordedInput::Unsupported,
    }
}

fn unescape_preseed_recorded_input_part(input: &str) -> Option<String> {
    if input == "\\0" {
        return None;
    }
    let mut result = String::with_capacity(input.len());
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            match ch {
                'n' => result.push('\n'),
                's' => result.push(' '),
                other => result.push(other),
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            result.push(ch);
        }
    }
    if escaped {
        result.push('\\');
    }
    Some(result)
}

fn preseed_recorded_file_project_path(
    path: &Path,
    workspace_root: Option<&Path>,
) -> Option<ProjectRelativePathBuf> {
    preseed_recorded_project_path(path, workspace_root)
}

fn preseed_recorded_project_path(
    path: &Path,
    workspace_root: Option<&Path>,
) -> Option<ProjectRelativePathBuf> {
    let raw = path.to_string_lossy();
    for prefix in ["@@//", "@//", "//"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            workspace_root?;
            return ProjectRelativePathBuf::try_from(rest.to_owned()).ok();
        }
    }
    if path.is_absolute() {
        let workspace_root = workspace_root?;
        let relative = path.strip_prefix(workspace_root).ok()?;
        return ProjectRelativePathBuf::try_from(relative.to_path_buf()).ok();
    }
    None
}

async fn preseed_recorded_file_marker_value(
    ctx: &mut DiceComputations<'_>,
    path: &ProjectRelativePath,
) -> slug_error::Result<Option<String>> {
    register_bzlmod_config_project_file(path.to_owned());
    let Some(metadata) =
        DiceFileComputations::read_project_path_metadata_if_exists(ctx, path).await?
    else {
        return Ok(Some("ENOENT".to_owned()));
    };
    match metadata {
        RawPathMetadata::Directory => Ok(Some("DIR".to_owned())),
        RawPathMetadata::File(_) => {
            let Some(content) =
                DiceFileComputations::read_project_file_if_exists(ctx, path).await?
            else {
                return Ok(Some("ENOENT".to_owned()));
            };
            Ok(Some(hex::encode(Sha256::digest(content.as_bytes()))))
        }
        RawPathMetadata::Symlink { .. } => Ok(None),
    }
}

async fn preseed_recorded_dirents_marker_value(
    ctx: &mut DiceComputations<'_>,
    path: &ProjectRelativePath,
) -> slug_error::Result<String> {
    register_bzlmod_config_project_file(path.to_owned());
    let entries = DiceFileComputations::read_project_dir_entry_names(ctx, path).await?;
    Ok(bazel_fingerprint_add_strings_hex(entries.as_slice()))
}

async fn preseed_recorded_dirtree_marker_value(
    ctx: &mut DiceComputations<'_>,
    path: &ProjectRelativePath,
) -> slug_error::Result<Option<String>> {
    register_bzlmod_config_project_file(path.to_owned());
    let entries = match DiceFileComputations::read_project_dir_entries(ctx, path).await {
        Ok(entries) => entries,
        Err(_) => return Ok(Some("ENOENT".to_owned())),
    };
    let entry_names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
    let mut subdir_digests = Vec::new();
    let mut file_values = Vec::new();
    for (entry, file_type) in entries.iter() {
        let Some(entry_path) = project_child_path(path, entry) else {
            return Ok(None);
        };
        match *file_type {
            FileType::Directory => {
                let Some(digest) = Box::pin(preseed_recorded_dirtree_marker_value(
                    ctx,
                    entry_path.as_ref(),
                ))
                .await?
                else {
                    return Ok(None);
                };
                subdir_digests.push(digest);
                file_values.push((2, None));
            }
            FileType::File => {
                let content = match DiceFileComputations::read_project_file_if_exists(
                    ctx,
                    entry_path.as_ref(),
                )
                .await
                {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        file_values.push((1, None));
                        continue;
                    }
                    Err(_) => return Ok(None),
                };
                file_values.push((0, Some(Sha256::digest(content.as_bytes()).to_vec())));
            }
            FileType::Symlink => return Ok(None),
            FileType::Unknown => file_values.push((1, None)),
        }
    }

    let mut bytes = Vec::new();
    bazel_fingerprint_add_strings(&entry_names, &mut bytes);
    bazel_fingerprint_add_strings(&subdir_digests, &mut bytes);
    for (file_state_type_ordinal, digest) in file_values {
        encode_varint(file_state_type_ordinal, &mut bytes);
        if let Some(digest) = digest {
            bytes.extend_from_slice(&digest);
        }
    }
    Ok(Some(hex::encode(Sha256::digest(&bytes))))
}

fn project_child_path(parent: &ProjectRelativePath, child: &str) -> Option<ProjectRelativePathBuf> {
    let path = if parent.as_str().is_empty() {
        child.to_owned()
    } else {
        format!("{}/{}", parent.as_str(), child)
    };
    ProjectRelativePathBuf::try_from(path).ok()
}

fn bazel_fingerprint_add_strings_hex(inputs: &[String]) -> String {
    let mut bytes = Vec::new();
    bazel_fingerprint_add_strings(inputs, &mut bytes);
    hex::encode(Sha256::digest(&bytes))
}

fn bazel_fingerprint_add_strings(inputs: &[String], bytes: &mut Vec<u8>) {
    encode_varint(inputs.len() as u64, bytes);
    for input in inputs {
        let input = input.as_bytes();
        encode_varint(input.len() as u64, bytes);
        bytes.extend_from_slice(input);
    }
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(((value as u8) & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn recorded_input_changed_reason(raw: &str) -> String {
    let identity = raw
        .split_once(' ')
        .map(|(identity, _)| identity)
        .unwrap_or(raw);
    format!("recorded_input_changed:{identity}")
}

fn recorded_input_changed_reason_with_values(raw: &str, old: &str, current: &str) -> String {
    format!(
        "{}:old={}:current={}",
        recorded_input_changed_reason(raw),
        old,
        current
    )
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("LocalOverrideModuleInputsKey({})", project_root.display())]
struct LocalOverrideModuleInputsKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRegistryOverrideModuleInputsKey({})", project_root.display())]
struct NonRegistryOverrideModuleInputsKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, PathBuf)>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("LocalOverrideModuleInputsPollKey({})", project_root.display())]
#[cfg(test)]
struct LocalOverrideModuleInputsPollKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRegistryOverrideModuleInputsPollKey({})", project_root.display())]
#[cfg(test)]
struct NonRegistryOverrideModuleInputsPollKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, PathBuf)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
#[cfg(test)]
struct BzlmodInputsPollValue {
    digest: String,
    has_polled_inputs: bool,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("RegistryFileInputsKey")]
struct RegistryFileInputsKey {
    project_root: AbsNormPathBuf,
    registry_file_hashes: Vec<(String, String)>,
    #[cfg(test)]
    cache_base_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("RegistryFileInputsPollKey({})", project_root.display())]
#[cfg(test)]
struct RegistryFileInputsPollKey {
    project_root: AbsNormPathBuf,
    registry_file_hashes: Vec<(String, String)>,
    #[cfg(test)]
    cache_base_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct BazelRegistryJsonForValidation {
    mirrors: Option<Vec<String>>,
    #[serde(rename = "moduleBasePath")]
    module_base_path: Option<String>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRootModuleFilesKey({}, {})", project_root.display(), inputs.len())]
struct NonRootModuleFilesKey {
    project_root: AbsNormPathBuf,
    inputs: Vec<NonRootModuleFileInput>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("NonRootModuleFilesPollKey({}, {})", project_root.display(), inputs.len())]
#[cfg(test)]
struct NonRootModuleFilesPollKey {
    project_root: AbsNormPathBuf,
    inputs: Vec<NonRootModuleFileInput>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("OverridePatchInputsKey({}, {})", project_root.display(), patch_labels.len())]
struct OverridePatchInputsKey {
    project_root: AbsNormPathBuf,
    main_repo_name: Option<String>,
    patch_labels: Vec<String>,
}

fn non_registry_override_kind(module_dir: &Path) -> &'static str {
    match module_dir.file_name().and_then(|name| name.to_str()) {
        Some(name) if name.starts_with("git-") => "git override",
        Some(name) if name.starts_with("archive-") => "archive override",
        _ => "non-registry override",
    }
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
    let mut has_untracked_inputs = false;
    let mut parsed_modules = Vec::new();

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
        let (module_read, tracking) = read_bzlmod_file_for_module_inputs(
            ctx,
            &project_fs,
            &module_bazel_path,
        )
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to parse MODULE.bazel for local module '{}' at {:?}: expected valid UTF-8",
                module_name, module_bazel_path
            )
        })?;
        if tracking != BzlmodFileInputTracking::Project {
            has_untracked_inputs = true;
        }
        match module_read {
            Some((content, content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs = if tracking == BzlmodFileInputTracking::Project {
                    parse_module_with_tracked_project_includes(
                        ctx,
                        &project_fs,
                        &module_bazel_path,
                        content,
                        false,
                    )
                    .await
                } else {
                    record_polled_module_parse_if_changed(&module_bazel_path, &content_digest);
                    parse_module_with_polled_includes(&module_bazel_path, content, false).map(
                        |parsed_with_inputs| ParsedModuleFileWithInputTracking {
                            parsed_with_inputs,
                            has_untracked_inputs: true,
                        },
                    )
                }
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for local module '{}' at {:?}",
                        module_name, module_bazel_path
                    )
                })?;
                has_untracked_inputs |= parsed_with_inputs.has_untracked_inputs;
                for input in &parsed_with_inputs.parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }

                let parsed = parsed_with_inputs.parsed_with_inputs.parsed;
                has_bazel_deps |= !parsed.module.bazel_deps.is_empty();
                has_extension_usages |= !parsed.extension_usages.is_empty();
                has_repo_rule_invocations |= !parsed.repo_rule_invocations.is_empty();
                has_git_overrides |= parsed
                    .module
                    .overrides
                    .iter()
                    .any(|override_| matches!(override_, slug_bzlmod::Override::Git(_)));

                for override_ in &parsed.module.overrides {
                    if let slug_bzlmod::Override::LocalPath(local) = override_ {
                        queue.push_back((
                            local.module_name.clone(),
                            normalized_module_dir.clone(),
                            local.path.clone(),
                        ));
                    }
                }
                parsed_modules.push((module_name.clone(), parsed));
            }
            None => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
        }
    }

    Ok(LocalOverrideModuleInputsValue {
        digest: hex::encode(hasher.finalize()),
        parsed_modules,
        has_bazel_deps,
        has_extension_usages,
        has_repo_rule_invocations,
        has_git_overrides,
        has_untracked_inputs,
    })
}

#[cfg(test)]
fn local_override_inputs_poll_digest(
    project_fs: &ProjectRoot,
    project_root: &AbsNormPathBuf,
    overrides: &[(String, String)],
) -> slug_error::Result<BzlmodInputsPollValue> {
    let mut hasher = Sha256::new();
    hasher.update(b"local-override-module-inputs-poll-v1");
    hasher.update([0]);
    let mut has_polled_inputs = false;

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
        has_polled_inputs = true;

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
                    parse_module_with_polled_includes(&module_bazel_path, content, false)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse MODULE.bazel for local module '{}' at {:?}",
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
                    if let slug_bzlmod::Override::LocalPath(local) = override_ {
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

    Ok(BzlmodInputsPollValue {
        digest: hex::encode(hasher.finalize()),
        has_polled_inputs,
    })
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
    let mut has_untracked_inputs = false;
    let mut parsed_modules = Vec::new();

    for (module_name, module_dir) in overrides {
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => AbsNormPathBuf::try_from(normalize_path_lexically(module_dir.clone()))?,
        };
        let override_kind = non_registry_override_kind(normalized_module_dir.as_path());
        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        hasher.update(module_name.as_bytes());
        hasher.update([0]);
        hasher.update(normalized_module_dir.to_string_lossy().as_bytes());
        hasher.update([0]);

        let (module_read, tracking) =
            read_bzlmod_file_for_module_inputs(ctx, &project_fs, &module_bazel_path)
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for {} '{}' at {:?}: expected valid UTF-8",
                        override_kind, module_name, module_bazel_path
                    )
                })?;
        if tracking != BzlmodFileInputTracking::Project {
            has_untracked_inputs = true;
        }
        match module_read {
            Some((content, content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs = parse_module_with_tracked_project_includes(
                    ctx,
                    &project_fs,
                    &module_bazel_path,
                    content,
                    false,
                )
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse MODULE.bazel for {} '{}' at {:?}",
                        override_kind, module_name, module_bazel_path
                    )
                })?;
                has_untracked_inputs |= parsed_with_inputs.has_untracked_inputs;
                for input in &parsed_with_inputs.parsed_with_inputs.inputs {
                    hasher.update(input.path.to_string_lossy().as_bytes());
                    hasher.update([0]);
                    hasher.update(input.digest.as_bytes());
                    hasher.update([0]);
                }
                parsed_modules.push((
                    module_name.clone(),
                    parsed_with_inputs.parsed_with_inputs.parsed,
                ));
            }
            None => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
        }
    }

    Ok(NonRegistryOverrideModuleInputsValue {
        digest: hex::encode(hasher.finalize()),
        parsed_modules,
        has_inputs: !overrides.is_empty(),
        has_untracked_inputs,
    })
}

async fn override_patch_inputs(
    ctx: &mut DiceComputations<'_>,
    project_root: &AbsNormPathBuf,
    main_repo_name: Option<&str>,
    patch_labels: &[String],
) -> slug_error::Result<slug_bzlmod::OverridePatchInputs> {
    let project_fs = ProjectRoot::new_unchecked(project_root.clone());
    let mut hasher = Sha256::new();
    hasher.update(b"override-patch-inputs-v1");
    hasher.update([0]);
    let mut inputs = Vec::new();
    let mut has_untracked_inputs = false;

    for patch_label in patch_labels {
        let path = slug_bzlmod::fetch::override_patch_label_path(
            project_root.as_path(),
            main_repo_name,
            patch_label,
        )?;
        let (content, tracking) = read_text_file_for_project_input(ctx, &project_fs, &path).await?;
        let Some(content) = content else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Override patch '{}' was not found at {}",
                patch_label,
                path.display()
            ));
        };
        let content = content.into_bytes();
        let digest = slug_bzlmod::compute_sha256_hex(&content);
        if tracking != BzlmodFileInputTracking::Project {
            has_untracked_inputs = true;
        }

        hasher.update(patch_label.as_bytes());
        hasher.update([0]);
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
        inputs.push(slug_bzlmod::OverridePatchInput {
            label: patch_label.clone(),
            path,
            digest,
            content,
        });
    }

    Ok(slug_bzlmod::OverridePatchInputs {
        digest: hex::encode(hasher.finalize()),
        inputs,
        has_untracked_inputs,
    })
}

#[cfg(test)]
fn non_registry_override_inputs_poll_digest(
    project_fs: &ProjectRoot,
    overrides: &[(String, PathBuf)],
) -> slug_error::Result<BzlmodInputsPollValue> {
    let mut hasher = Sha256::new();
    hasher.update(b"non-registry-override-module-inputs-poll-v1");
    hasher.update([0]);
    let mut has_polled_inputs = false;

    for (module_name, module_dir) in overrides {
        let normalized_module_dir = match module_dir.as_path().canonicalize() {
            Ok(canonical) => AbsNormPathBuf::try_from(canonical)?,
            Err(_) => AbsNormPathBuf::try_from(normalize_path_lexically(module_dir.clone()))?,
        };
        let override_kind = non_registry_override_kind(normalized_module_dir.as_path());
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
        has_polled_inputs = true;

        let module_bazel_path = normalized_module_dir.as_path().join("MODULE.bazel");
        match read_absolute_text_file_input(&module_bazel_path)? {
            (Some(content), Some(content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs =
                    parse_module_with_polled_includes(&module_bazel_path, content, false)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse MODULE.bazel for {} '{}' at {:?}",
                                override_kind, module_name, module_bazel_path
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

    Ok(BzlmodInputsPollValue {
        digest: hex::encode(hasher.finalize()),
        has_polled_inputs,
    })
}

#[cfg(test)]
fn non_root_module_files_poll_digest(
    project_fs: &ProjectRoot,
    inputs: &[NonRootModuleFileInput],
) -> slug_error::Result<BzlmodInputsPollValue> {
    let mut hasher = Sha256::new();
    hasher.update(b"non-root-module-files-poll-v1");
    hasher.update([0]);
    let mut has_polled_inputs = false;

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

        has_polled_inputs = true;
        match read_absolute_text_file_input(&input.module_bazel_path)? {
            (Some(content), Some(content_digest)) => {
                hasher.update(b"present");
                hasher.update([0]);
                hasher.update(content_digest.as_bytes());
                hasher.update([0]);
                let parsed_with_inputs =
                    parse_module_with_polled_includes(&input.module_bazel_path, content, false)
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

    Ok(BzlmodInputsPollValue {
        digest: hex::encode(hasher.finalize()),
        has_polled_inputs,
    })
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
    let mut has_untracked_inputs = false;

    for input in inputs {
        hasher.update(input.module_key.as_bytes());
        hasher.update([0]);
        hasher.update(input.module_bazel_path.to_string_lossy().as_bytes());
        hasher.update([0]);

        let (module_read, tracking) =
            read_bzlmod_file_for_module_inputs(ctx, &project_fs, &input.module_bazel_path).await?;
        if tracking != BzlmodFileInputTracking::Project {
            has_untracked_inputs = true;
        }
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
            false,
        )
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to parse non-root MODULE.bazel for '{}' at {:?}",
                input.module_key, input.module_bazel_path
            )
        })?;
        has_untracked_inputs |= parsed_with_inputs.has_untracked_inputs;
        for parsed_input in &parsed_with_inputs.parsed_with_inputs.inputs {
            hasher.update(parsed_input.path.to_string_lossy().as_bytes());
            hasher.update([0]);
            hasher.update(parsed_input.digest.as_bytes());
            hasher.update([0]);
        }

        let parsed = parsed_with_inputs.parsed_with_inputs.parsed;
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
        has_untracked_inputs,
    })
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
#[cfg(test)]
impl Key for LocalOverrideModuleInputsPollKey {
    type Value = slug_error::Result<Arc<BzlmodInputsPollValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        local_override_inputs_poll_digest(&project_fs, &self.project_root, &self.overrides)
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
            Ok(value) => !value.has_polled_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
#[cfg(test)]
impl Key for NonRegistryOverrideModuleInputsPollKey {
    type Value = slug_error::Result<Arc<BzlmodInputsPollValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        non_registry_override_inputs_poll_digest(&project_fs, &self.overrides).map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_polled_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
#[cfg(test)]
impl Key for RegistryFileInputsPollKey {
    type Value = slug_error::Result<Arc<BzlmodInputsPollValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        #[cfg(test)]
        {
            if let Some(cache_base_dir) = &self.cache_base_dir {
                let cache = ModuleCache::with_base_dir(cache_base_dir.clone())?;
                return registry_file_inputs_poll_digest_for_cache(
                    &project_fs,
                    &cache,
                    &self.registry_file_hashes,
                )
                .map(Arc::new);
            }
        }
        registry_file_inputs_poll_digest(&project_fs, &self.registry_file_hashes).map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_polled_inputs,
            Err(_) => false,
        }
    }
}

#[async_trait]
#[cfg(test)]
impl Key for NonRootModuleFilesPollKey {
    type Value = slug_error::Result<Arc<BzlmodInputsPollValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let project_fs = ProjectRoot::new_unchecked(self.project_root.clone());
        non_root_module_files_poll_digest(&project_fs, &self.inputs).map(Arc::new)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_polled_inputs,
            Err(_) => false,
        }
    }
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
        x.is_ok()
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
        x.is_ok()
    }
}

fn cached_registry_file_path(cache: &ModuleCache, url: &str) -> Option<PathBuf> {
    if let Some(registry_url) = url.strip_suffix("/bazel_registry.json") {
        return Some(cache.bazel_registry_json_path(registry_url));
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

fn registry_module_file_parts(url: &str) -> Option<(&str, &str, &str, &str)> {
    let (registry_url, module_path) = url.split_once("/modules/")?;
    let mut parts = module_path.split('/');
    let name = parts.next()?;
    let version = parts.next()?;
    let file_name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((registry_url, name, version, file_name))
}

async fn fetch_missing_registry_file(
    cache: &ModuleCache,
    url: &str,
) -> slug_error::Result<Option<String>> {
    if let Some(registry_url) = url.strip_suffix("/bazel_registry.json") {
        if registry_url.starts_with("file:") {
            return Ok(None);
        }
        let client = slug_bzlmod::RegistryClient::new(registry_url, cache.clone()).await?;
        let file = client.fetch_bazel_registry_json_file().await?;
        return Ok(Some(file.content));
    }

    let Some((registry_url, name, version, file_name)) = registry_module_file_parts(url) else {
        return Ok(None);
    };
    if registry_url.starts_with("file:") {
        return Ok(None);
    }
    let client = slug_bzlmod::RegistryClient::new(registry_url, cache.clone()).await?;
    match file_name {
        "MODULE.bazel" => Ok(Some(
            client.fetch_module_bazel_file(name, version).await?.content,
        )),
        "source.json" => Ok(Some(
            client
                .fetch_source_info_file(name, version)
                .await?
                .file
                .content,
        )),
        _ => Ok(None),
    }
}

#[cfg(test)]
fn registry_file_inputs_poll_digest(
    project_fs: &ProjectRoot,
    registry_file_hashes: &[(String, String)],
) -> slug_error::Result<BzlmodInputsPollValue> {
    let cache = ModuleCache::new()?;
    registry_file_inputs_poll_digest_for_cache(project_fs, &cache, registry_file_hashes)
}

#[cfg(test)]
fn registry_file_inputs_poll_digest_for_cache(
    project_fs: &ProjectRoot,
    cache: &ModuleCache,
    registry_file_hashes: &[(String, String)],
) -> slug_error::Result<BzlmodInputsPollValue> {
    let mut hasher = Sha256::new();
    hasher.update(b"registry-file-inputs-poll-v1");
    hasher.update([0]);
    let mut has_polled_inputs = false;

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
        if project_relative_path_for_abs_path(project_fs, &path).is_some() {
            hasher.update(b"project-tracked");
            hasher.update([0]);
            continue;
        }
        has_polled_inputs = true;
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

    Ok(BzlmodInputsPollValue {
        digest: hex::encode(hasher.finalize()),
        has_polled_inputs,
    })
}

fn validate_bazel_registry_json_file(url: &str, content: &str) -> slug_error::Result<()> {
    if !url.ends_with("/bazel_registry.json") {
        return Ok(());
    }

    // Bazel's IndexRegistry treats blank JSON as absent metadata, otherwise it
    // parses this top-level file with a BazelRegistryJson shape
    // (mirrors/moduleBasePath) before using registry mirrors. Keep the
    // transitional registry-file input bridge honest by rejecting malformed
    // cached metadata even when the lockfile hash matches.
    if content.trim().is_empty() {
        return Ok(());
    }

    let BazelRegistryJsonForValidation {
        mirrors,
        module_base_path,
    } = serde_json::from_str::<BazelRegistryJsonForValidation>(content)
        .with_buck_error_context(|| format!("Failed to parse bazel_registry.json at {url}"))?;
    drop((mirrors, module_base_path));
    Ok(())
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
        let cache = {
            #[cfg(test)]
            {
                if let Some(cache_base_dir) = &self.cache_base_dir {
                    ModuleCache::with_base_dir(cache_base_dir.clone())?
                } else {
                    ModuleCache::new()?
                }
            }
            #[cfg(not(test))]
            {
                ModuleCache::new()?
            }
        };
        let mut cache_safe = true;
        let mut has_untracked_inputs = false;
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
            let (content, tracking) =
                read_text_file_for_project_input(ctx, &project_fs, &path).await?;
            if tracking != BzlmodFileInputTracking::Project {
                has_untracked_inputs = true;
            }
            match content {
                Some(content) => {
                    hasher.update(b"present");
                    hasher.update([0]);
                    let actual_hash = slug_bzlmod::compute_sha256_hex(content.as_bytes());
                    if &actual_hash != expected_hash {
                        return Err(slug_error::slug_error!(
                            slug_error::ErrorTag::Input,
                            "Registry file checksum mismatch for {}: expected {}, got {}",
                            url,
                            expected_hash,
                            actual_hash
                        ));
                    }
                    validate_bazel_registry_json_file(url, &content)?;
                    hasher.update(actual_hash);
                }
                None => match fetch_missing_registry_file(&cache, url).await? {
                    Some(content) => {
                        hasher.update(b"present");
                        hasher.update([0]);
                        let actual_hash = slug_bzlmod::compute_sha256_hex(content.as_bytes());
                        if &actual_hash != expected_hash {
                            return Err(slug_error::slug_error!(
                                slug_error::ErrorTag::Input,
                                "Registry file checksum mismatch for {}: expected {}, got {}",
                                url,
                                expected_hash,
                                actual_hash
                            ));
                        }
                        validate_bazel_registry_json_file(url, &content)?;
                        hasher.update(actual_hash);
                    }
                    None => {
                        hasher.update(b"missing");
                        return Err(slug_error::slug_error!(
                            slug_error::ErrorTag::Input,
                            "Registry file checksum mismatch for {}: expected {}, got missing file",
                            url,
                            expected_hash
                        ));
                    }
                },
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
            Ok(value) => value.cache_safe,
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
impl Key for OverridePatchInputsKey {
    type Value = slug_error::Result<Arc<slug_bzlmod::OverridePatchInputs>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        override_patch_inputs(
            ctx,
            &self.project_root,
            self.main_repo_name.as_deref(),
            &self.patch_labels,
        )
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

#[derive(Clone, Debug, Display, Allocative)]
#[display(
    "BzlmodResolvedModuleGraphKey({}, {})",
    project_root.display(),
    resolution_key.command_policy_digest
)]
struct BzlmodResolvedModuleGraphKey {
    project_root: AbsNormPathBuf,
    resolution_key: slug_bzlmod::BzlmodResolutionKey,
    options: BzlmodResolutionOptions,
    validate_root_extension_repo_directives: bool,
    root_module_file: Arc<slug_bzlmod::RootModuleFileValue>,
    lockfile_inputs: Arc<slug_bzlmod::BzlmodLockfileInputsValue>,
    local_override_inputs: Arc<LocalOverrideModuleInputsValue>,
    non_registry_override_inputs: Arc<NonRegistryOverrideModuleInputsValue>,
    registry_file_inputs: Arc<RegistryFileInputsValue>,
    override_patch_inputs: Arc<slug_bzlmod::OverridePatchInputs>,
}

impl PartialEq for BzlmodResolvedModuleGraphKey {
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.resolution_key == other.resolution_key
            && bzlmod_resolution_options_policy_eq(&self.options, &other.options)
            && self.validate_root_extension_repo_directives
                == other.validate_root_extension_repo_directives
            && self.root_module_file.path == other.root_module_file.path
            && self.root_module_file.input_digest == other.root_module_file.input_digest
            && self.lockfile_inputs.identity_eq(&other.lockfile_inputs)
            && self.local_override_inputs.digest == other.local_override_inputs.digest
            && self.non_registry_override_inputs.digest == other.non_registry_override_inputs.digest
            && self.registry_file_inputs.digest == other.registry_file_inputs.digest
            && self.override_patch_inputs.digest == other.override_patch_inputs.digest
    }
}

impl Eq for BzlmodResolvedModuleGraphKey {}

impl std::hash::Hash for BzlmodResolvedModuleGraphKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.project_root.hash(state);
        self.resolution_key.hash(state);
        hash_bzlmod_resolution_options_policy(&self.options, state);
        self.validate_root_extension_repo_directives.hash(state);
        self.root_module_file.path.hash(state);
        self.root_module_file.input_digest.hash(state);
        self.lockfile_inputs.hash_identity(state);
        self.local_override_inputs.digest.hash(state);
        self.non_registry_override_inputs.digest.hash(state);
        self.registry_file_inputs.digest.hash(state);
        self.override_patch_inputs.digest.hash(state);
    }
}

#[async_trait]
impl Key for BzlmodResolvedModuleGraphKey {
    type Value = slug_error::Result<Arc<Option<slug_bzlmod::BzlmodResolvedGraphOutputsValue>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        BuckConfigBasedCells::compute_bzlmod_resolved_module_graph(self, ctx).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => match (x.as_ref(), y.as_ref()) {
                (Some(x), Some(y)) => {
                    x.graph_digest == y.graph_digest
                        && x.module_versions == y.module_versions
                        && x.resolution_facts == y.resolution_facts
                        && x.registered_toolchains == y.registered_toolchains
                        && x.registered_execution_platforms == y.registered_execution_platforms
                        && x.extension_aggregations == y.extension_aggregations
                        && x.repo_mappings == y.repo_mappings
                        && x.cell_graph == y.cell_graph
                }
                (None, None) => true,
                _ => false,
            },
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
        let project_root = project_fs.root().to_path_buf();
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), project_root.join("buck-out/v2"));
        let clean_cell_graph = Self::compute_clean_bzlmod_cell_graph_without_persisted_updater(
            project_fs,
            config_args,
            None,
        )
        .await?;
        Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            Some(clean_cell_graph),
            workspace_id,
        )
        .await
        .buck_error_context("Parsing cells")
    }

    pub async fn parse_with_config_args_and_output_base(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        output_base: PathBuf,
    ) -> slug_error::Result<Self> {
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_fs.root().to_path_buf(), output_base);
        let clean_cell_graph = Self::compute_clean_bzlmod_cell_graph_without_persisted_updater(
            project_fs,
            config_args,
            Some(workspace_id.output_base.as_ref().clone()),
        )
        .await?;
        Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            Some(clean_cell_graph),
            workspace_id,
        )
        .await
        .buck_error_context("Parsing cells")
    }

    async fn compute_clean_bzlmod_cell_graph_without_persisted_updater(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        output_base: Option<PathBuf>,
    ) -> slug_error::Result<Arc<Option<slug_bzlmod::BzlmodCellGraphValue>>> {
        // Client bootstrap and shell-completion paths do not have the daemon's
        // persisted DICE updater yet, but they must still use the same clean
        // bzlmod graph producer as the persisted config-load path. The IO
        // provider is text-input only for this bootstrap graph; CAS digest
        // policy is irrelevant to the module/lockfile reads it performs.
        let mut dice = Dice::builder();
        dice.set_io_provider(Arc::new(FsIoProvider::new(
            project_fs.dupe(),
            CasDigestConfig::testing_default(),
        )));
        let dice = dice.build(DetectCycles::Enabled);
        let updater = dice.updater();
        let mut dice_ctx = updater.existing_state().await;
        let clean_key = Self::build_bzlmod_resolved_module_graph_key(
            project_fs,
            config_args,
            &mut dice_ctx,
            output_base,
            false,
        )
        .await?;
        let clean_resolved_module_graph = dice_ctx.compute(&clean_key).await?.buck_error_context(
            "Computing clean bzlmod resolved graph for bootstrap cell parsing",
        )?;

        Ok(Arc::new(
            clean_resolved_module_graph
                .as_ref()
                .as_ref()
                .map(|value| value.cell_graph.clone()),
        ))
    }

    pub async fn parse_with_config_args_and_persisted_bzlmod_cell_graph(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        updater: &mut DiceTransactionUpdater,
        output_base: Option<PathBuf>,
    ) -> slug_error::Result<Self> {
        let (key, clean_resolved_module_graph) = {
            let mut dice_ctx = updater.existing_state().await;
            let clean_key = Self::build_bzlmod_resolved_module_graph_key(
                project_fs,
                config_args,
                &mut dice_ctx,
                output_base,
                true,
            )
            .await?;
            let clean_resolved_module_graph = dice_ctx
                .compute(&clean_key)
                .await?
                .buck_error_context("Computing clean bzlmod resolved graph for DICE injection")?;
            (clean_key, clean_resolved_module_graph)
        };
        if let Some(clean_graph) = clean_resolved_module_graph.as_ref().as_ref() {
            tracing::debug!(
                graph_digest = clean_graph.graph_digest.as_ref(),
                module_count = clean_graph.graph.modules.len(),
                selected_version_count = clean_graph.graph.selected_versions.len(),
                "Using clean bzlmod resolved graph data for DICE injections"
            );
        }
        let empty_cell_graph_for_dice = slug_bzlmod::BzlmodCellGraphValue::empty_for_workspace(
            key.resolution_key.workspace_id.clone(),
        );
        let cell_graph_for_dice = if slug_bzlmod::MODULE_EXTENSION_EXECUTOR_IMPL.get().is_ok() {
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || empty_cell_graph_for_dice.clone(),
                |value| {
                    let mut cell_graph = slug_bzlmod::BzlmodCellGraphValue::empty_for_workspace(
                        key.resolution_key.workspace_id.clone(),
                    );
                    cell_graph.root_module_name = value.module_versions.root_module_name.clone();
                    cell_graph
                },
            )
        } else {
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || empty_cell_graph_for_dice.clone(),
                |value| value.cell_graph.clone(),
            )
        };
        let (module_versions_for_dice, resolution_facts_for_dice) =
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || {
                    (
                        slug_bzlmod::BzlmodModuleVersionsDataValue::for_workspace(
                            key.resolution_key.workspace_id.clone(),
                            Arc::new(HashMap::new()),
                        ),
                        slug_bzlmod::BzlmodResolutionFactsValue::for_workspace(
                            key.resolution_key.workspace_id.clone(),
                            indexmap::IndexMap::new(),
                            indexmap::IndexMap::new(),
                        ),
                    )
                },
                |value| {
                    (
                        value.module_versions.clone(),
                        value.resolution_facts.clone(),
                    )
                },
            );
        let (registered_toolchains_for_dice, registered_execution_platforms_for_dice) =
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || {
                    (
                        slug_bzlmod::RegisteredToolchainsDataValue::for_workspace(
                            key.resolution_key.workspace_id.clone(),
                            Vec::new(),
                        ),
                        slug_bzlmod::RegisteredExecutionPlatformsDataValue::for_workspace(
                            key.resolution_key.workspace_id.clone(),
                            Vec::new(),
                        ),
                    )
                },
                |value| {
                    (
                        value.registered_toolchains.clone(),
                        value.registered_execution_platforms.clone(),
                    )
                },
            );
        let extension_aggregations_for_dice =
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || {
                    slug_bzlmod::BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                        key.resolution_key.workspace_id.clone(),
                        String::new(),
                        Arc::new(HashMap::new()),
                    )
                },
                |value| value.extension_aggregations.clone(),
            );
        let repo_mappings_for_dice = clean_resolved_module_graph.as_ref().as_ref().map_or_else(
            || {
                slug_bzlmod::BzlmodRepoMappingsDataValue::for_workspace(
                    key.resolution_key.workspace_id.clone(),
                    Arc::new(slug_bzlmod::RepoMappingSnapshot::new()),
                    Arc::new(slug_bzlmod::RepoMappingOverrides::new()),
                )
            },
            |value| value.repo_mappings.clone(),
        );

        let configs = Self::parse_with_file_ops_and_options_inner(
            config_args,
            Some(project_fs),
            Some(Arc::new(
                clean_resolved_module_graph
                    .as_ref()
                    .as_ref()
                    .map(|data| data.cell_graph.clone()),
            )),
            key.resolution_key.workspace_id.clone(),
        )
        .await
        .buck_error_context("Parsing cells")?;
        let cell_graph_resolution_digest =
            clean_resolved_module_graph.as_ref().as_ref().map_or_else(
                || Arc::from("empty-bzlmod-cell-graph"),
                |value| value.graph_digest.clone(),
            );
        let resolved_graph_for_dice = clean_resolved_module_graph
            .as_ref()
            .as_ref()
            .map(|value| value.graph.clone());
        slug_bzlmod::SetBzlmodDiceInputs::set_bzlmod_cell_graph_data_with_inputs_digest_and_resolved_graph(
            updater,
            cell_graph_resolution_digest,
            cell_graph_for_dice,
            resolved_graph_for_dice,
            module_versions_for_dice,
            slug_bzlmod::BzlmodLockfileInputsDataValue::for_workspace(
                key.resolution_key.workspace_id.clone(),
                key.lockfile_inputs.clone(),
            ),
            slug_bzlmod::BzlmodRepoEnvDataValue::for_workspace(
                key.resolution_key.workspace_id.clone(),
                Arc::new(key.options.repo_env.clone()),
            ),
            registered_toolchains_for_dice,
            registered_execution_platforms_for_dice,
            extension_aggregations_for_dice,
            resolution_facts_for_dice,
            repo_mappings_for_dice,
        )?;
        Ok(configs)
    }

    async fn build_bzlmod_resolved_module_graph_key(
        project_fs: &ProjectRoot,
        config_args: &[slug_cli_proto::ConfigOverride],
        dice_ctx: &mut DiceComputations<'_>,
        output_base: Option<PathBuf>,
        validate_root_extension_repo_directives: bool,
    ) -> slug_error::Result<BzlmodResolvedModuleGraphKey> {
        let root_config = LegacyBuckConfig::from_overrides_only(config_args)?;
        let options = bzlmod_resolution_options_from_config(&root_config)?;
        let project_root_path = project_fs.root().to_path_buf();
        let workspace_id = slug_bzlmod::WorkspaceId::new(
            project_root_path.clone(),
            output_base.unwrap_or_else(|| project_root_path.join("buck-out/v2")),
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
            .buck_error_context("Computing root MODULE.bazel for clean bzlmod graph")?;
        let project_root = AbsNormPathBuf::try_from(project_root_path)?;
        let lockfile_inputs = dice_ctx
            .compute(&BzlmodLockfileInputsBridgeKey {
                project_root: project_root.clone(),
                workspace_id: workspace_id.clone(),
                lockfile_mode: options.lockfile_mode,
                hidden_lockfile_path: options.hidden_lockfile_path.clone(),
                root_module_present: root_module_file.parsed.is_some(),
            })
            .await?
            .buck_error_context("Computing bzlmod lockfile inputs for clean graph")?;
        let local_overrides = slug_bzlmod::local_overrides_from_root_module(
            root_module_file.as_ref(),
            options.ignore_dev_dependency,
        );
        let local_override_inputs = dice_ctx
            .compute(&LocalOverrideModuleInputsKey {
                project_root: project_root.clone(),
                overrides: local_overrides,
            })
            .await?
            .buck_error_context("Computing local override MODULE.bazel inputs for clean graph")?;
        let non_registry_overrides =
            slug_bzlmod::non_registry_override_module_dirs_from_root_module(
                root_module_file.as_ref(),
                options.ignore_dev_dependency,
            )?;
        let non_registry_override_inputs = dice_ctx
            .compute(&NonRegistryOverrideModuleInputsKey {
                project_root: project_root.clone(),
                overrides: non_registry_overrides,
            })
            .await?
            .buck_error_context(
                "Computing non-registry override MODULE.bazel inputs for clean graph",
            )?;
        let registry_file_hashes = lockfile_inputs
            .visible_lockfile
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
        let registry_file_inputs = dice_ctx
            .compute(&RegistryFileInputsKey {
                project_root: project_root.clone(),
                registry_file_hashes,
                #[cfg(test)]
                cache_base_dir: None,
            })
            .await?
            .buck_error_context("Computing registry file inputs for clean bzlmod graph")?;
        let (main_repo_name, override_patch_labels) =
            slug_bzlmod::override_patch_labels_from_root_module(
                root_module_file.as_ref(),
                options.ignore_dev_dependency,
            );
        let override_patch_inputs = dice_ctx
            .compute(&OverridePatchInputsKey {
                project_root: project_root.clone(),
                main_repo_name,
                patch_labels: override_patch_labels,
            })
            .await?
            .buck_error_context("Computing override patch inputs for clean bzlmod graph")?;

        Ok(BzlmodResolvedModuleGraphKey {
            project_root,
            resolution_key,
            options,
            validate_root_extension_repo_directives,
            root_module_file,
            lockfile_inputs,
            local_override_inputs,
            non_registry_override_inputs,
            registry_file_inputs,
            override_patch_inputs,
        })
    }

    /// Testing entry point: equivalent to `parse_with_config_args` with no project root.
    pub async fn testing_parse(
        config_args: &[slug_cli_proto::ConfigOverride],
    ) -> slug_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(
            config_args,
            None,
            None,
            slug_bzlmod::WorkspaceId::no_project_sentinel(),
        )
        .await
        .buck_error_context("Parsing cells")
    }

    async fn parse_with_file_ops_and_options_inner(
        config_args: &[slug_cli_proto::ConfigOverride],
        project_fs: Option<&ProjectRoot>,
        provided_bzlmod_cell_graph: Option<Arc<Option<slug_bzlmod::BzlmodCellGraphValue>>>,
        _workspace_id: slug_bzlmod::WorkspaceId,
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
        // Non-bzlmod parsing still injects empty bzlmod DICE inputs for legacy
        // consumers. The caller must choose the workspace identity explicitly.
        let mut bzlmod_runtime_cell_snapshot = None;

        // ===== Bzlmod Integration =====
        // When MODULE.bazel exists, ALL cell definitions come from bzlmod resolution.
        // The root cell name is derived from module(name = "...") in MODULE.bazel.
        // .buckconfig [cells], [cell_aliases], and [external_cells] sections are skipped.
        let mut bzlmod_aliases: Vec<(NonEmptyCellAlias, CellName)> = Vec::new();
        if let Some(cell_graph) =
            if let Some(provided_bzlmod_cell_graph) = provided_bzlmod_cell_graph {
                provided_bzlmod_cell_graph.as_ref().clone()
            } else if let Some(project_fs) = project_fs {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "project bzlmod cell parsing requires a clean graph producer for {}",
                    project_fs.root().display()
                ));
            } else {
                None
            }
        {
            let runtime_cell_snapshot = runtime_cell_install_snapshot(&cell_graph);
            if let Some(project_fs) = project_fs {
                replay_bzlmod_runtime_state(&cell_graph, project_fs);
            }
            has_module_bazel = true;
            let cell_graph = &cell_graph;

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
        })
    }

    async fn build_bzlmod_cell_graph_from_clean_resolution(
        project_root: &ProjectRoot,
        project_root_abs: &AbsNormPathBuf,
        workspace_id: slug_bzlmod::WorkspaceId,
        options: &BzlmodResolutionOptions,
        parsed: &ParsedModuleFile,
        graph: &slug_bzlmod::ResolvedGraph,
        parsed_modules: &[(String, ParsedModuleFile)],
        visible_lockfile: Option<&slug_bzlmod::Lockfile>,
        hidden_lockfile: Option<&slug_bzlmod::Lockfile>,
        dice_ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<slug_bzlmod::BzlmodCellGraphValue> {
        let mut cells = Vec::new();
        let mut aliases = Vec::new();
        let mut module_symlinks = Vec::new();
        let mut lockfile_seeded_cells = Vec::new();
        let mut scoped_repo_aliases = Vec::new();
        let mut dynamic_extension_aliases: Vec<BzlmodDynamicAlias> = Vec::new();
        let workspace_root = project_root.root().as_path();
        let repo_env = Arc::new(options.repo_env.clone());
        let root_module_name = if parsed.module.name.is_empty() {
            "_main"
        } else {
            parsed.module.name.as_str()
        };

        if !parsed.module.bazel_deps.is_empty() {
            let mut sorted_modules: Vec<_> = graph.modules.iter().collect();
            sorted_modules.sort_by(|a, b| a.0.cmp(b.0));
            for (module_name, module_info) in sorted_modules {
                if module_name == &parsed.module.name || module_name == root_module_name {
                    continue;
                }

                let canonical_repo =
                    bazel_canonical_module_repo_name(module_name, &module_info.version);
                let cell_name = CellName::unchecked_new(&canonical_repo)?;

                match &module_info.source {
                    ModuleSource::Registry { url } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodExternalModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: source_path.clone(),
                            });
                        }
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let external_path = format!("bazel-external/{canonical_repo}");
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );
                        let setup = BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(url.as_str()),
                            source_path: Arc::from(source_path_str.as_str()),
                        };
                        cells.push((cell_name, cell_path, Some(setup)));
                    }
                    ModuleSource::LocalPath { path } => {
                        let (cell_path, symlink) = local_override_cell_path_and_symlink(
                            project_root,
                            project_root_abs,
                            module_name,
                            &module_info.version,
                            path,
                        )?;
                        if let Some(symlink) = symlink {
                            module_symlinks.push(symlink);
                        }
                        cells.push((cell_name, cell_path, None));
                    }
                    ModuleSource::Git {
                        remote, commit: _, ..
                    } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodExternalModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: source_path.clone(),
                            });
                        }
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let external_path = format!("bazel-external/{canonical_repo}");
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );
                        let setup = BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(format!("git+{}", remote).as_str()),
                            source_path: Arc::from(source_path_str.as_str()),
                        };
                        cells.push((cell_name, cell_path, Some(setup)));
                    }
                    ModuleSource::Archive { urls, .. } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodExternalModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: source_path.clone(),
                            });
                        }
                        let source_path_str = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let external_path = format!("bazel-external/{canonical_repo}");
                        let cell_path = CellRootPathBuf::new(
                            ProjectRelativePath::new(&external_path)?.to_owned(),
                        );
                        let url = urls.first().map(|u| u.as_str()).unwrap_or("archive");
                        let setup = BzlmodCellSetup {
                            module_name: Arc::from(module_name.as_str()),
                            version: Arc::from(module_info.version.as_str()),
                            registry_url: Arc::from(url),
                            source_path: Arc::from(source_path_str.as_str()),
                        };
                        cells.push((cell_name, cell_path, Some(setup)));
                    }
                }
            }

            if let Some(repo_name) = &parsed.module.repo_name {
                if repo_name != &parsed.module.name {
                    let alias_name = NonEmptyCellAlias::new(repo_name.clone())?;
                    let cell_name = CellName::unchecked_new(&parsed.module.name)?;
                    aliases.push((alias_name, cell_name));
                }
            }

            for dep in &parsed.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                if let Some(target_name) =
                    selected_bzlmod_cell_name_for_dep(&cells, &dep.name, graph)
                {
                    let cell_name = CellName::unchecked_new(target_name)?;
                    let alias_name = NonEmptyCellAlias::new(apparent_name.to_owned())?;
                    aliases.push((alias_name, cell_name));
                }
            }

            for module_name in graph.modules.keys() {
                if module_name == &parsed.module.name || module_name == root_module_name {
                    continue;
                }
                if aliases
                    .iter()
                    .any(|(alias, _)| alias.as_str() == module_name)
                {
                    continue;
                }
                if let Some(target_name) =
                    selected_bzlmod_cell_name_for_dep(&cells, module_name, graph)
                {
                    if module_name == target_name {
                        continue;
                    }
                    let alias_name = NonEmptyCellAlias::new(module_name.clone())?;
                    let cell_name = CellName::unchecked_new(target_name)?;
                    aliases.push((alias_name, cell_name));
                }
            }
        }

        for (module_name, parsed_mod) in parsed_modules {
            for dep in &parsed_mod.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                let Some(target_name) = selected_bzlmod_cell_name_for_dep(&cells, &dep.name, graph)
                else {
                    continue;
                };
                if apparent_name == target_name {
                    continue;
                }
                if module_name != root_module_name {
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
                aliases.push((alias_name, cell_name));
            }
        }

        let mut module_extensions: HashMap<String, Vec<slug_bzlmod::ExtensionUsage>> =
            HashMap::new();
        for (module_name, parsed_mod) in parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated = slug_bzlmod::aggregate_extensions_with_policy(
            &module_extensions,
            Some(root_module_name),
            options.ignore_dev_dependency,
        );
        let (mut repo_mappings, repo_mapping_overrides) = graph_owned_repo_mapping_state(
            parsed_modules,
            root_module_name,
            options.ignore_dev_dependency,
            &cells,
            Some(graph),
        );
        let (mut pre_computed_cells, pre_computed_aliases) =
            slug_bzlmod::pre_compute_extension_repo_cells(
                parsed_modules,
                root_module_name,
                options.ignore_dev_dependency,
            )?;
        resolve_use_repo_rule_local_bits(
            dice_ctx,
            &mut pre_computed_cells,
            parsed_modules,
            root_module_name,
        )
        .await?;

        let mut extension_mapping_cells = pre_computed_cells.clone();
        add_extension_repo_mapping_rows_from_cells(
            &mut repo_mappings,
            &extension_mapping_cells,
            root_module_name,
            &repo_mapping_overrides,
        );

        if let Some(lockfile) = visible_lockfile {
            let fallback_scanned_bzl_transitive_digests =
                fallback_scanned_extension_bzl_digests_for_lockfile_preseed(
                    dice_ctx,
                    project_root_abs,
                    &aggregated,
                    &repo_mappings,
                )
                .await?;
            let prevalidated_caches = prevalidated_extension_caches_for_lockfile_preseed(
                dice_ctx,
                lockfile,
                &aggregated,
                root_module_name,
                workspace_root,
                repo_env.as_ref(),
                &fallback_scanned_bzl_transitive_digests,
                &repo_mappings,
                &repo_mapping_overrides,
            )
            .await?;
            let extra =
                slug_bzlmod::pre_compute_extension_repo_cells_from_lockfile_with_prevalidated_caches(
                    lockfile,
                    &aggregated,
                    root_module_name,
                    &mut pre_computed_cells,
                    workspace_root,
                    Some(repo_env.as_ref()),
                    &fallback_scanned_bzl_transitive_digests,
                    Some(&repo_mappings),
                    Some(&repo_mapping_overrides),
                    Some(&prevalidated_caches),
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
        if let Some(lockfile) = hidden_lockfile {
            let fallback_scanned_bzl_transitive_digests =
                fallback_scanned_extension_bzl_digests_for_lockfile_preseed(
                    dice_ctx,
                    project_root_abs,
                    &aggregated,
                    &repo_mappings,
                )
                .await?;
            let prevalidated_caches = prevalidated_extension_caches_for_lockfile_preseed(
                dice_ctx,
                lockfile,
                &aggregated,
                root_module_name,
                workspace_root,
                repo_env.as_ref(),
                &fallback_scanned_bzl_transitive_digests,
                &repo_mappings,
                &repo_mapping_overrides,
            )
            .await?;
            let extra =
                slug_bzlmod::pre_compute_extension_repo_cells_from_lockfile_with_prevalidated_caches(
                    lockfile,
                    &aggregated,
                    root_module_name,
                    &mut pre_computed_cells,
                    workspace_root,
                    Some(repo_env.as_ref()),
                    &fallback_scanned_bzl_transitive_digests,
                    Some(&repo_mappings),
                    Some(&repo_mapping_overrides),
                    Some(&prevalidated_caches),
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
                repo_env_json: repo_env_json(repo_env.as_ref()),
                materialized: false,
            };
            ext_cells.push((cell_name, cell_path, setup));
        }

        let mut existing_cell_names: HashSet<String> = cells
            .iter()
            .map(|(name, _, _)| name.as_str().to_owned())
            .collect();
        let mut ext_aliases = Vec::new();
        for alias in pre_computed_aliases {
            let target_name =
                selected_bzlmod_cell_name_for_dep(&cells, &alias.canonical_name, graph)
                    .unwrap_or(alias.canonical_name.as_str())
                    .to_owned();
            let is_generated_override_alias = alias.declaring_module.is_none()
                && alias.apparent_name != target_name
                && slug_bzlmod::parse_canonical_name(&alias.apparent_name).is_some();
            let is_root_declared_alias =
                alias.declaring_module.as_deref() == Some(root_module_name);
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
                                workspace_root
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
            Some(graph),
        );

        aliases.extend(ext_aliases);

        let lockfile_repo_env_json = repo_env_json(repo_env.as_ref());
        Ok(slug_bzlmod::BzlmodCellGraphValue {
            workspace_id,
            root_module_name: root_module_name.to_owned(),
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
        })
    }

    async fn compute_bzlmod_resolved_module_graph(
        key: &BzlmodResolvedModuleGraphKey,
        dice_ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Arc<Option<slug_bzlmod::BzlmodResolvedGraphOutputsValue>>> {
        let Some(parsed) = key.root_module_file.parsed.clone() else {
            return Ok(Arc::new(None));
        };
        if key.validate_root_extension_repo_directives && !key.options.ignore_dev_dependency {
            slug_bzlmod::validate_parsed_root_extension_repo_directives(&parsed)?;
        }

        let visible_lockfile = if key.options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(visible_lockfile) = key.lockfile_inputs.visible_lockfile.as_ref() {
            visible_lockfile.lockfile.clone()
        } else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "clean DICE bzlmod resolved graph requires tracked visible lockfile input"
            ));
        };
        let hidden_lockfile = if key.options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(hidden_lockfile) = key.lockfile_inputs.hidden_lockfile.as_ref() {
            hidden_lockfile.lockfile.clone()
        } else if key.options.hidden_lockfile_path.is_some() {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "clean DICE bzlmod resolved graph requires tracked hidden lockfile input"
            ));
        } else {
            None
        };

        let workspace_root = key.project_root.as_path();
        let project_fs = ProjectRoot::new_unchecked(key.project_root.clone());
        let graph_inputs = slug_bzlmod::resolve_graph_with_module_file_inputs(
            &parsed,
            workspace_root,
            &key.options,
            key.local_override_inputs.as_ref(),
            key.non_registry_override_inputs.as_ref(),
            key.override_patch_inputs.clone(),
            visible_lockfile.as_deref(),
        )
        .await?;
        let graph = graph_inputs.graph;
        let mut parsed_modules = graph_inputs.parsed_modules;
        if !graph_inputs.non_root_module_file_inputs.is_empty() {
            let non_root_value = dice_ctx
                .compute(&NonRootModuleFilesKey {
                    project_root: key.project_root.clone(),
                    inputs: graph_inputs.non_root_module_file_inputs,
                })
                .await
                .with_buck_error_context(|| {
                    format!(
                        "Failed to parse non-root MODULE.bazel files via DICE while computing clean graph for root module '{}'",
                        parsed.module.name
                    )
                })?;
            let non_root_value = non_root_value.with_buck_error_context(|| {
                format!(
                    "Failed to parse non-root MODULE.bazel files for '{}'",
                    parsed.module.name
                )
            })?;
            slug_bzlmod::append_resolved_non_root_modules(
                &mut parsed_modules,
                &graph,
                key.non_registry_override_inputs.as_ref(),
                &non_root_value.parsed_modules,
            );
        } else {
            slug_bzlmod::append_resolved_non_root_modules(
                &mut parsed_modules,
                &graph,
                key.non_registry_override_inputs.as_ref(),
                &[],
            );
        }
        let mut clean_cells = Vec::new();
        let mut sorted_graph_modules: Vec<_> = graph.modules.iter().collect();
        sorted_graph_modules.sort_by(|left, right| left.0.cmp(right.0));
        for (module_name, module_info) in sorted_graph_modules {
            let canonical_repo =
                bazel_canonical_module_repo_name(module_name, &module_info.version);
            let external_path = format!("bazel-external/{canonical_repo}");
            clean_cells.push((
                CellName::unchecked_new(&canonical_repo)?,
                CellRootPathBuf::new(ProjectRelativePath::new(&external_path)?.to_owned()),
                None,
            ));
        }

        let projections = slug_bzlmod::resolved_graph_projection_values(
            key.resolution_key.workspace_id.clone(),
            &parsed,
            &parsed_modules,
            &graph,
            key.options.ignore_dev_dependency,
        );
        let (repo_mapping_snapshot, repo_mapping_overrides) = graph_owned_repo_mapping_state(
            &parsed_modules,
            &projections.module_versions.root_module_name,
            key.options.ignore_dev_dependency,
            &clean_cells,
            Some(&graph),
        );
        let repo_mappings = slug_bzlmod::BzlmodRepoMappingsDataValue::for_workspace(
            key.resolution_key.workspace_id.clone(),
            Arc::new(repo_mapping_snapshot),
            Arc::new(repo_mapping_overrides),
        );
        let cell_graph = Self::build_bzlmod_cell_graph_from_clean_resolution(
            &project_fs,
            &key.project_root,
            key.resolution_key.workspace_id.clone(),
            &key.options,
            &parsed,
            &graph,
            &parsed_modules,
            visible_lockfile.as_deref(),
            hidden_lockfile.as_deref(),
            dice_ctx,
        )
        .await?;
        let graph_digest = slug_bzlmod::bzlmod_resolved_graph_digest(&graph);
        record_clean_bzlmod_resolution_compute_if_changed(key);
        Ok(Arc::new(Some(
            slug_bzlmod::BzlmodResolvedGraphOutputsValue {
                graph: Arc::new(graph),
                graph_digest: Arc::from(graph_digest.as_str()),
                module_versions: projections.module_versions,
                resolution_facts: projections.resolution_facts,
                registered_toolchains: projections.registered_toolchains,
                registered_execution_platforms: projections.registered_execution_platforms,
                extension_aggregations: projections.extension_aggregations,
                repo_mappings,
                cell_graph,
            },
        )))
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

    let selected_version = resolved_graph
        .modules
        .get(dep_name)
        .map(|module| module.version.as_str())
        .or_else(|| {
            resolved_graph
                .selected_versions
                .get(dep_name)
                .map(String::as_str)
        })?;
    let canonical_name = bazel_canonical_module_repo_name(dep_name, selected_version);
    if let Some((name, _, _)) = cells
        .iter()
        .find(|(name, _, _)| name.as_str() == canonical_name)
    {
        return Some(name.as_str());
    }

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
    use slug_bzlmod::Module;
    use slug_bzlmod::ParsedModuleFile;
    use slug_bzlmod::RegisteredItem;
    use slug_bzlmod::Version;
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

    #[test]
    fn tracked_lockfile_content_key_validity_follows_tracking_provenance() {
        let polled: slug_error::Result<Arc<slug_bzlmod::LockfileContentValue>> =
            Ok(lockfile_value("/tmp/hidden/MODULE.bazel.lock", "hidden"));
        assert!(!<TrackedLockfileContentKey as Key>::validity(&polled));

        let tracked: slug_error::Result<Arc<slug_bzlmod::LockfileContentValue>> =
            Ok(Arc::new(slug_bzlmod::LockfileContentValue {
                tracked_by_dice: true,
                ..(*polled.unwrap()).clone()
            }));
        assert!(<TrackedLockfileContentKey as Key>::validity(&tracked));
    }

    fn minimal_lockfile_json(marker: &str) -> String {
        format!(
            r#"{{
  "lockFileVersion": 26,
  "facts": {{
    "marker": "{marker}"
  }}
}}
"#
        )
    }

    async fn compute_bzlmod_lockfile_inputs_bridge(
        dice: &mut DiceComputations<'_>,
        project_fs: &ProjectRoot,
        lockfile_mode: slug_bzlmod::LockfileMode,
        hidden_lockfile_path: Option<PathBuf>,
        root_module_present: bool,
    ) -> slug_error::Result<Arc<slug_bzlmod::BzlmodLockfileInputsValue>> {
        let project_root = project_fs.root().to_path_buf();
        dice.compute(&BzlmodLockfileInputsBridgeKey {
            project_root: AbsNormPathBuf::try_from(project_root.clone())?,
            workspace_id: slug_bzlmod::WorkspaceId::new(
                project_root.clone(),
                project_root.join("buck-out/v2"),
            ),
            lockfile_mode,
            hidden_lockfile_path,
            root_module_present,
        })
        .await?
    }

    fn bzlmod_resolved_module_graph_key(
        hidden_lockfile: Arc<slug_bzlmod::LockfileContentValue>,
    ) -> BzlmodResolvedModuleGraphKey {
        let project_root = PathBuf::from("/tmp/slug-plan61-hidden-lockfile-key-test");
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), PathBuf::from("/tmp/output-base"));
        let lockfile_inputs = Arc::new(slug_bzlmod::BzlmodLockfileInputsValue::from_values(
            Some(PathBuf::from("/tmp/hidden/MODULE.bazel.lock")),
            Some(lockfile_value(
                "/tmp/workspace/MODULE.bazel.lock",
                "visible-lockfile",
            )),
            Some(hidden_lockfile),
            slug_bzlmod::LockfileMode::Update,
        ));

        BzlmodResolvedModuleGraphKey {
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
            validate_root_extension_repo_directives: true,
            root_module_file: Arc::new(slug_bzlmod::RootModuleFileValue {
                path: Arc::new(project_root.join("MODULE.bazel")),
                input_digest: Some("root-module".to_owned()),
                input_count: 1,
                parsed: None,
            }),
            lockfile_inputs,
            local_override_inputs: Arc::new(LocalOverrideModuleInputsValue {
                digest: "local-overrides".to_owned(),
                parsed_modules: Vec::new(),
                has_bazel_deps: false,
                has_extension_usages: false,
                has_repo_rule_invocations: false,
                has_git_overrides: false,
                has_untracked_inputs: false,
            }),
            non_registry_override_inputs: Arc::new(NonRegistryOverrideModuleInputsValue {
                digest: "non-registry-overrides".to_owned(),
                parsed_modules: Vec::new(),
                has_inputs: false,
                has_untracked_inputs: false,
            }),
            registry_file_inputs: Arc::new(RegistryFileInputsValue {
                digest: "registry-files".to_owned(),
                has_inputs: false,
                cache_safe: true,
                has_untracked_inputs: false,
            }),
            override_patch_inputs: Arc::new(slug_bzlmod::OverridePatchInputs::default()),
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
            slug_bzlmod::collect_bzlmod_registered_items(&parsed_modules, "root", false);
        assert_eq!(toolchains.len(), 1);
        assert_eq!(platforms.len(), 1);

        let (toolchains, platforms) =
            slug_bzlmod::collect_bzlmod_registered_items(&parsed_modules, "root", true);
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
        let options = bzlmod_resolution_options_from_config(&config)?;

        assert_eq!(options.repo_env, repo_env);
        assert_eq!(
            options.repo_env_digest,
            slug_bzlmod::repo_env_policy_digest(&repo_env)
        );
        Ok(())
    }

    #[tokio::test]
    async fn clean_resolved_module_graph_key_uses_explicit_output_base() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        let output_base = fs
            .path()
            .resolve(ProjectRelativePath::new("buck-out/custom-isolation")?);
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let key = BuckConfigBasedCells::build_bzlmod_resolved_module_graph_key(
            fs.path(),
            &[],
            &mut dice,
            Some(output_base.to_path_buf()),
            true,
        )
        .await?;

        assert_eq!(
            key.resolution_key
                .workspace_id
                .output_base
                .as_ref()
                .as_path(),
            output_base.as_path()
        );
        Ok(())
    }

    #[tokio::test]
    async fn clean_resolved_module_graph_produces_local_override_facts() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file(
            "MODULE.bazel",
            r#"
module(name = "root", version = "1.0")
bazel_dep(name = "dep", version = "1.0")
local_path_override(module_name = "dep", path = "dep")
"#,
        );
        fs.write_file(
            "dep/MODULE.bazel",
            r#"
module(name = "dep", version = "1.0")
"#,
        );
        let config_args = [slug_cli_proto::ConfigOverride::flag_no_cell(
            "bzlmod.lockfile_mode=off",
        )];
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let clean_key = BuckConfigBasedCells::build_bzlmod_resolved_module_graph_key(
            fs.path(),
            &config_args,
            &mut dice,
            None,
            true,
        )
        .await?;
        let clean = dice
            .compute(&clean_key)
            .await?
            .buck_error_context("Computing clean shadow graph")?;
        let clean = clean
            .as_ref()
            .as_ref()
            .expect("local override workspace should produce clean bzlmod data")
            .clone();

        let mut expected_versions = HashMap::new();
        expected_versions.insert("root".to_owned(), "1.0".to_owned());
        expected_versions.insert("dep".to_owned(), "1.0".to_owned());
        assert_eq!(
            clean.module_versions.module_versions.as_ref(),
            &expected_versions
        );
        assert_eq!(clean.module_versions.root_module_name, "root");
        assert!(clean.resolution_facts.registry_file_hashes.is_empty());
        assert!(clean.resolution_facts.selected_yanked_versions.is_empty());
        assert_eq!(
            clean.graph.selected_versions.get("dep").map(String::as_str),
            Some("1.0")
        );
        assert!(clean.graph_digest.as_ref().len() >= 64);
        Ok(())
    }

    #[tokio::test]
    async fn persisted_cell_graph_injects_clean_root_module_version_data() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        fs.write_file(
            "MODULE.bazel",
            r#"
module(name = "root", version = "1.0")
register_toolchains("@root_toolchains//:all")
register_execution_platforms("@root_platforms//:all")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, "generated")
"#,
        );
        let config_args = [slug_cli_proto::ConfigOverride::flag_no_cell(
            "bzlmod.lockfile_mode=off",
        )];
        let dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();

        let configs = BuckConfigBasedCells::parse_with_config_args_and_persisted_bzlmod_cell_graph(
            fs.path(),
            &config_args,
            &mut updater,
            None,
        )
        .await?;

        assert!(configs.is_bzlmod);
        let mut dice = updater.commit().await;
        let module_versions = slug_bzlmod::module_versions_for_current_workspace(&mut dice).await?;
        let mut expected_versions = HashMap::new();
        expected_versions.insert("root".to_owned(), "1.0".to_owned());
        assert_eq!(module_versions.module_versions.as_ref(), &expected_versions);
        assert_eq!(module_versions.invalidation.root_module_name, "root");

        let resolution_facts = dice
            .compute(&slug_bzlmod::BzlmodResolutionFactsKey::for_workspace_id(
                module_versions.workspace_id.clone(),
            ))
            .await??;
        assert!(resolution_facts.registry_file_hashes.is_empty());
        assert!(resolution_facts.selected_yanked_versions.is_empty());

        let registered_toolchains =
            slug_bzlmod::registered_toolchains_for_current_workspace(&mut dice).await?;
        assert_eq!(registered_toolchains.registered_toolchains.len(), 1);
        assert_eq!(
            registered_toolchains.registered_toolchains[0].label,
            "@root_toolchains//:all"
        );

        let registered_execution_platforms =
            slug_bzlmod::registered_execution_platforms_for_current_workspace(&mut dice).await?;
        assert_eq!(
            registered_execution_platforms.registered_execution_platforms,
            ["@root_platforms//:all".to_owned()]
        );

        let extension_aggregations = dice
            .compute(&slug_bzlmod::BzlmodExtensionAggregationsDataKey)
            .await?;
        assert_eq!(extension_aggregations.root_module_name, "root");
        assert_eq!(extension_aggregations.extension_aggregations.len(), 1);

        let repo_mappings = dice
            .compute(&slug_bzlmod::BzlmodRepoMappingsKey::for_workspace_id(
                module_versions.workspace_id.clone(),
            ))
            .await??;
        assert_eq!(
            repo_mappings
                .repo_mappings
                .get("")
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("_main+ext+generated")
        );

        let cell_graph = slug_bzlmod::bzlmod_cell_graph_for_current_workspace(&mut dice).await?;
        assert_eq!(cell_graph.root_module_name, "root");
        assert!(
            cell_graph
                .extension_cells
                .iter()
                .any(|cell| cell.canonical_name == "_main+ext+generated")
        );
        let stale_key_error = dice
            .compute(&slug_bzlmod::BzlmodCellGraphKey::for_workspace_id(
                module_versions.workspace_id.clone(),
            ))
            .await?
            .unwrap_err();
        assert!(
            stale_key_error.to_string().contains("resolution digest"),
            "{stale_key_error:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn absolute_text_file_input_key_tracks_polled_transitions() -> slug_error::Result<()> {
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-absolute-text-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let path = external.path().join("MODULE.bazel");
        let mut dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let missing = read_absolute_text_file_input_via_dice(&mut dice, &path).await?;
        assert!(missing.content.is_none());
        assert!(missing.digest.is_none());

        std::fs::write(&path, "module(name = \"created\")\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let created = read_absolute_text_file_input_via_dice(&mut dice, &path).await?;
        assert_eq!(
            created.content.as_deref(),
            Some("module(name = \"created\")\n")
        );
        assert_ne!(missing.as_ref(), created.as_ref());

        std::fs::write(&path, "module(name = \"edited\")\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let edited = read_absolute_text_file_input_via_dice(&mut dice, &path).await?;
        assert_eq!(
            edited.content.as_deref(),
            Some("module(name = \"edited\")\n")
        );
        assert_ne!(created.as_ref(), edited.as_ref());

        std::fs::remove_file(&path).unwrap();
        let mut dice = dice.into_updater().commit().await;
        let deleted = read_absolute_text_file_input_via_dice(&mut dice, &path).await?;
        assert!(deleted.content.is_none());
        assert!(deleted.digest.is_none());
        assert_eq!(missing.as_ref(), deleted.as_ref());
        Ok(())
    }

    #[tokio::test]
    async fn absolute_text_file_input_key_reads_current_file_content() -> slug_error::Result<()> {
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-absolute-text-observed-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let path = external.path().join("MODULE.bazel");
        std::fs::write(&path, "module(name = \"first\")\n").unwrap();
        std::fs::write(&path, "module(name = \"second\")\n").unwrap();

        let mut dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let value = dice
            .compute(&AbsoluteTextFileInputKey {
                path: Arc::new(path),
            })
            .await??;

        assert_eq!(
            value.content.as_deref(),
            Some("module(name = \"second\")\n")
        );
        assert!(!<AbsoluteTextFileInputKey as Key>::validity(&Ok(value)));
        Ok(())
    }

    #[tokio::test]
    async fn out_of_project_module_include_reads_use_polled_text_key() -> slug_error::Result<()> {
        let project = tempfile::Builder::new()
            .prefix("slug-plan61-project-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let project_root = ProjectRoot::new(AbsNormPathBuf::try_from(
            project.path().canonicalize().unwrap(),
        )?)?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-module-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        let include_path = external.path().join("deps.MODULE.bazel");
        std::fs::write(
            &module_path,
            "module(name = \"external_root\")\ninclude(\"//:deps.MODULE.bazel\")\n",
        )
        .unwrap();
        std::fs::write(
            &include_path,
            "bazel_dep(name = \"dep\", version = \"1.0\")\n",
        )
        .unwrap();
        let mut dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let (module_read, tracking) =
            read_bzlmod_file_for_module_inputs(&mut dice, &project_root, &module_path).await?;
        assert_eq!(tracking, BzlmodFileInputTracking::Polled);
        let (module_content, _) = module_read.expect("external MODULE should exist");
        let parsed = parse_module_with_tracked_project_includes(
            &mut dice,
            &project_root,
            &module_path,
            module_content,
            true,
        )
        .await?;
        let first_digest =
            slug_bzlmod::module_file_inputs_digest(&parsed.parsed_with_inputs.inputs);

        std::fs::write(
            &include_path,
            "bazel_dep(name = \"dep\", version = \"2.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let (module_read, tracking) =
            read_bzlmod_file_for_module_inputs(&mut dice, &project_root, &module_path).await?;
        assert_eq!(tracking, BzlmodFileInputTracking::Polled);
        let (module_content, _) = module_read.expect("external MODULE should exist");
        let parsed = parse_module_with_tracked_project_includes(
            &mut dice,
            &project_root,
            &module_path,
            module_content,
            true,
        )
        .await?;
        let second_digest =
            slug_bzlmod::module_file_inputs_digest(&parsed.parsed_with_inputs.inputs);

        assert_ne!(first_digest, second_digest);
        Ok(())
    }

    #[tokio::test]
    async fn local_override_module_inputs_poll_key_repolls_out_of_project_module()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-local-override-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        std::fs::write(&module_path, "module(name = \"dep\")\n").unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let overrides = vec![(
            "dep".to_owned(),
            external.path().to_string_lossy().into_owned(),
        )];
        let key = LocalOverrideModuleInputsPollKey {
            project_root: project_root.clone(),
            overrides: overrides.clone(),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_polled_inputs);

        std::fs::write(
            &module_path,
            "module(name = \"dep\")\nbazel_dep(name = \"other\", version = \"1.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_polled_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn non_registry_override_module_inputs_poll_key_repolls_out_of_project_module()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-non-registry-override-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        std::fs::write(&module_path, "module(name = \"dep\")\n").unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let overrides = vec![("dep".to_owned(), external.path().to_path_buf())];
        let key = NonRegistryOverrideModuleInputsPollKey {
            project_root: project_root.clone(),
            overrides: overrides.clone(),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_polled_inputs);

        std::fs::write(
            &module_path,
            "module(name = \"dep\")\nbazel_dep(name = \"other\", version = \"1.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_polled_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn local_override_module_inputs_key_repolls_same_out_of_project_key()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-local-override-parent-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        std::fs::write(&module_path, "module(name = \"dep\")\n").unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let key = LocalOverrideModuleInputsKey {
            project_root,
            overrides: vec![(
                "dep".to_owned(),
                external.path().to_string_lossy().into_owned(),
            )],
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_untracked_inputs);
        assert!(<LocalOverrideModuleInputsKey as Key>::validity(&Ok(
            first.clone()
        )));

        std::fs::write(
            &module_path,
            "module(name = \"dep\")\nbazel_dep(name = \"other\", version = \"1.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_untracked_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn non_registry_override_module_inputs_key_repolls_same_out_of_project_key()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-non-registry-parent-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        std::fs::write(&module_path, "module(name = \"dep\")\n").unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let key = NonRegistryOverrideModuleInputsKey {
            project_root,
            overrides: vec![("dep".to_owned(), external.path().to_path_buf())],
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_untracked_inputs);
        assert!(<NonRegistryOverrideModuleInputsKey as Key>::validity(&Ok(
            first.clone()
        )));

        std::fs::write(
            &module_path,
            "module(name = \"dep\")\nbazel_dep(name = \"other\", version = \"1.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_untracked_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[test]
    fn registry_file_inputs_poll_digest_marks_project_cache_tracked() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let cache = ModuleCache::with_base_dir(fs.path().root().as_path().join("cache"))?;
        let registry_url = "https://bcr.bazel.build/bazel_registry.json".to_owned();
        let registry_path = cache
            .registry_dir("https://bcr.bazel.build")
            .join("bazel_registry.json");
        std::fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        std::fs::write(&registry_path, "{}\n").unwrap();
        let registry_hash = slug_bzlmod::compute_sha256_hex("{}\n".as_bytes());
        let registry_file_hashes = vec![(registry_url, registry_hash)];

        let first =
            registry_file_inputs_poll_digest_for_cache(fs.path(), &cache, &registry_file_hashes)?;
        assert!(!first.has_polled_inputs);

        std::fs::write(&registry_path, "{\"mirrors\": []}\n").unwrap();
        let second =
            registry_file_inputs_poll_digest_for_cache(fs.path(), &cache, &registry_file_hashes)?;

        assert!(!second.has_polled_inputs);
        assert_eq!(first.digest, second.digest);
        Ok(())
    }

    #[test]
    fn registry_file_inputs_poll_digest_repolls_out_of_project_cache() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-registry-cache-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let cache = ModuleCache::with_base_dir(external.path().join("cache"))?;
        let registry_url = "https://bcr.bazel.build/bazel_registry.json".to_owned();
        let registry_path = cache
            .registry_dir("https://bcr.bazel.build")
            .join("bazel_registry.json");
        std::fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        std::fs::write(&registry_path, "{}\n").unwrap();
        let registry_hash = slug_bzlmod::compute_sha256_hex("{}\n".as_bytes());
        let registry_file_hashes = vec![(registry_url, registry_hash)];

        let first =
            registry_file_inputs_poll_digest_for_cache(fs.path(), &cache, &registry_file_hashes)?;
        assert!(first.has_polled_inputs);

        std::fs::write(&registry_path, "{\"mirrors\": []}\n").unwrap();
        let second =
            registry_file_inputs_poll_digest_for_cache(fs.path(), &cache, &registry_file_hashes)?;

        assert!(second.has_polled_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn registry_file_inputs_poll_key_repolls_out_of_project_cache() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-registry-cache-key-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let cache_base_dir = external.path().join("cache");
        let cache = ModuleCache::with_base_dir(cache_base_dir.clone())?;
        let registry_url = "https://bcr.bazel.build/bazel_registry.json".to_owned();
        let registry_path = cache
            .registry_dir("https://bcr.bazel.build")
            .join("bazel_registry.json");
        std::fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        std::fs::write(&registry_path, "{}\n").unwrap();
        let registry_hash = slug_bzlmod::compute_sha256_hex("{}\n".as_bytes());
        let registry_file_hashes = vec![(registry_url, registry_hash)];
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let key = RegistryFileInputsPollKey {
            project_root,
            registry_file_hashes,
            cache_base_dir: Some(cache_base_dir),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_polled_inputs);

        std::fs::write(&registry_path, "{\"mirrors\": []}\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_polled_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn registry_file_inputs_key_repolls_same_out_of_project_key() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-registry-cache-parent-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let cache_base_dir = external.path().join("cache");
        let cache = ModuleCache::with_base_dir(cache_base_dir.clone())?;
        let registry_url = "https://bcr.bazel.build/bazel_registry.json".to_owned();
        let registry_path = cache
            .registry_dir("https://bcr.bazel.build")
            .join("bazel_registry.json");
        std::fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        std::fs::write(&registry_path, "{}\n").unwrap();
        let registry_hash = slug_bzlmod::compute_sha256_hex("{}\n".as_bytes());
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let key = RegistryFileInputsKey {
            project_root,
            registry_file_hashes: vec![(registry_url.clone(), registry_hash)],
            cache_base_dir: Some(cache_base_dir),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_untracked_inputs);
        assert!(<RegistryFileInputsKey as Key>::validity(&Ok(first)));

        std::fs::write(&registry_path, "{\"mirrors\": []}\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let err = dice.compute(&key).await?.unwrap_err().to_string();

        assert!(err.contains("Registry file checksum mismatch"), "{err}");
        assert!(err.contains(&registry_url), "{err}");
        Ok(())
    }

    #[test]
    fn validate_bazel_registry_json_file_rejects_malformed_metadata() -> slug_error::Result<()> {
        let registry_url = "https://bcr.bazel.build/bazel_registry.json";
        validate_bazel_registry_json_file(registry_url, "\n\t  ")?;
        validate_bazel_registry_json_file(registry_url, "{}\n")?;
        validate_bazel_registry_json_file(registry_url, "{\"mirrors\": []}\n")?;
        validate_bazel_registry_json_file(
            "https://bcr.bazel.build/modules/dep/1.0/source.json",
            "{not json}\n",
        )?;

        let err = validate_bazel_registry_json_file(registry_url, "{not json}\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Failed to parse bazel_registry.json"), "{err}");
        Ok(())
    }

    #[test]
    fn non_root_module_files_poll_digest_marks_project_inputs_tracked() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let module_path = fs.path().root().as_path().join("libs/dep/MODULE.bazel");
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, "module(name = \"dep\", version = \"1.0\")\n").unwrap();
        let inputs = vec![NonRootModuleFileInput {
            module_key: "dep".to_owned(),
            module_bazel_path: module_path.clone(),
        }];

        let first = non_root_module_files_poll_digest(fs.path(), &inputs)?;
        assert!(!first.has_polled_inputs);

        std::fs::write(&module_path, "module(name = \"dep\", version = \"2.0\")\n").unwrap();
        let second = non_root_module_files_poll_digest(fs.path(), &inputs)?;

        assert!(!second.has_polled_inputs);
        assert_eq!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn non_root_module_files_poll_key_repolls_out_of_project_include()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-non-root-module-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        let include_path = external.path().join("deps.MODULE.bazel");
        std::fs::write(
            &module_path,
            "module(name = \"dep\", version = \"1.0\")\ninclude(\"//:deps.MODULE.bazel\")\n",
        )
        .unwrap();
        std::fs::write(
            &include_path,
            "bazel_dep(name = \"included_dep\", version = \"1.0\")\n",
        )
        .unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let inputs = vec![NonRootModuleFileInput {
            module_key: "dep".to_owned(),
            module_bazel_path: module_path,
        }];
        let key = NonRootModuleFilesPollKey {
            project_root: project_root.clone(),
            inputs: inputs.clone(),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_polled_inputs);

        std::fs::write(
            &include_path,
            "bazel_dep(name = \"included_dep\", version = \"2.0\")\n",
        )
        .unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_polled_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn non_root_module_files_key_repolls_same_out_of_project_key() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let external = tempfile::Builder::new()
            .prefix("slug-plan61-non-root-module-parent-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let module_path = external.path().join("MODULE.bazel");
        std::fs::write(&module_path, "module(name = \"dep\", version = \"1.0\")\n").unwrap();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().to_path_buf())?;
        let key = NonRootModuleFilesKey {
            project_root,
            inputs: vec![NonRootModuleFileInput {
                module_key: "dep".to_owned(),
                module_bazel_path: module_path.clone(),
            }],
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.has_untracked_inputs);
        assert!(!<NonRootModuleFilesKey as Key>::validity(
            &Ok(first.clone())
        ));

        std::fs::write(&module_path, "module(name = \"dep\", version = \"2.0\")\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let second = dice.compute(&key).await??;

        assert!(second.has_untracked_inputs);
        assert_ne!(first.digest, second.digest);
        Ok(())
    }

    #[tokio::test]
    async fn persisted_empty_bzlmod_inputs_preserves_explicit_output_base() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        let output_base = fs
            .path()
            .resolve(ProjectRelativePath::new("buck-out/custom-isolation")?);
        let dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let config_args = [slug_cli_proto::ConfigOverride::flag_no_cell("cells.root=.")];

        let configs = BuckConfigBasedCells::parse_with_config_args_and_persisted_bzlmod_cell_graph(
            fs.path(),
            &config_args,
            &mut updater,
            Some(output_base.to_path_buf()),
        )
        .await?;

        assert!(!configs.is_bzlmod);
        let mut dice = updater.commit().await;
        let cell_graph = slug_bzlmod::bzlmod_cell_graph_for_current_workspace(&mut dice).await?;
        assert_eq!(
            cell_graph.workspace_id.output_base.as_ref().as_path(),
            output_base.as_path()
        );
        Ok(())
    }

    #[tokio::test]
    async fn clean_no_updater_bzlmod_cell_graph_preserves_explicit_output_base()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        let output_base = fs
            .path()
            .resolve(ProjectRelativePath::new("buck-out/custom-direct")?);

        let cell_graph =
            BuckConfigBasedCells::compute_clean_bzlmod_cell_graph_without_persisted_updater(
                fs.path(),
                &[],
                Some(output_base.to_path_buf()),
            )
            .await?
            .as_ref()
            .clone()
            .expect("MODULE.bazel should produce a bzlmod cell graph");
        let configs = BuckConfigBasedCells::parse_with_config_args_and_output_base(
            fs.path(),
            &[],
            output_base.to_path_buf(),
        )
        .await?;

        assert!(configs.is_bzlmod);
        assert_eq!(
            cell_graph.workspace_id.output_base.as_ref().as_path(),
            output_base.as_path()
        );
        Ok(())
    }

    #[tokio::test]
    async fn project_bzlmod_parse_requires_clean_graph_input() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        let workspace_id = slug_bzlmod::WorkspaceId::new(
            fs.path().root().to_path_buf(),
            fs.path()
                .resolve(ProjectRelativePath::new("buck-out/missing-clean-graph")?)
                .to_path_buf(),
        );

        let err = match BuckConfigBasedCells::parse_with_file_ops_and_options_inner(
            &[],
            Some(fs.path()),
            None,
            workspace_id,
        )
        .await
        {
            Ok(_) => panic!("project parse without a clean bzlmod graph unexpectedly succeeded"),
            Err(err) => err,
        };

        let err = format!("{err:#}");
        assert!(
            err.contains("requires a clean graph producer"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn selected_bzlmod_cell_name_for_dep_prefers_canonical_module_repo() -> slug_error::Result<()> {
        let cells = vec![(
            CellName::unchecked_new("dep+")?,
            CellRootPathBuf::new(ProjectRelativePath::new("bazel-external/dep+")?.to_owned()),
            None,
        )];
        let mut resolved_graph = slug_bzlmod::ResolvedGraph::default();
        resolved_graph.modules.insert(
            "dep".to_owned(),
            slug_bzlmod::ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://bcr.bazel.build".to_owned(),
                },
                source_path: None,
            },
        );

        assert_eq!(
            selected_bzlmod_cell_name_for_dep(&cells, "dep", &resolved_graph),
            Some("dep+")
        );
        Ok(())
    }

    #[test]
    fn repo_mapping_snapshot_targets_use_canonical_module_cells() -> slug_error::Result<()> {
        let cells = vec![
            (
                CellName::unchecked_new("dep+")?,
                CellRootPathBuf::new(ProjectRelativePath::new("bazel-external/dep+")?.to_owned()),
                None,
            ),
            (
                CellName::unchecked_new("other+")?,
                CellRootPathBuf::new(ProjectRelativePath::new("bazel-external/other+")?.to_owned()),
                None,
            ),
        ];
        let mut resolved_graph = slug_bzlmod::ResolvedGraph::default();
        for module_name in ["dep", "other"] {
            resolved_graph.modules.insert(
                module_name.to_owned(),
                slug_bzlmod::ResolvedModuleInfo {
                    name: module_name.to_owned(),
                    version: "1.0".to_owned(),
                    compatibility_level: 0,
                    dependencies: HashMap::new(),
                    source: ModuleSource::Registry {
                        url: "https://bcr.bazel.build".to_owned(),
                    },
                    source_path: None,
                },
            );
        }
        let mut snapshot = slug_bzlmod::RepoMappingSnapshot::new();
        snapshot.insert(
            "root".to_owned(),
            BTreeMap::from([
                ("dep".to_owned(), "dep".to_owned()),
                ("already_canonical".to_owned(), "other+".to_owned()),
            ]),
        );

        canonicalize_repo_mapping_snapshot_targets(&mut snapshot, &cells, Some(&resolved_graph));

        let mapping = snapshot.get("root").expect("root mapping should exist");
        assert_eq!(mapping.get("dep").map(String::as_str), Some("dep+"));
        assert_eq!(
            mapping.get("already_canonical").map(String::as_str),
            Some("other+")
        );
        Ok(())
    }

    #[test]
    fn repo_mapping_override_targets_use_canonical_root_mapping_targets() -> slug_error::Result<()>
    {
        let cells = vec![(
            CellName::unchecked_new("dep+")?,
            CellRootPathBuf::new(ProjectRelativePath::new("bazel-external/dep+")?.to_owned()),
            None,
        )];
        let mut resolved_graph = slug_bzlmod::ResolvedGraph::default();
        resolved_graph.modules.insert(
            "dep".to_owned(),
            slug_bzlmod::ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://bcr.bazel.build".to_owned(),
                },
                source_path: None,
            },
        );
        let mut snapshot = slug_bzlmod::RepoMappingSnapshot::new();
        snapshot.insert(
            String::new(),
            BTreeMap::from([
                ("helper_alias".to_owned(), "dep+".to_owned()),
                ("_main+ext+generated".to_owned(), "helper_alias".to_owned()),
            ]),
        );
        snapshot.insert("root".to_owned(), snapshot[""].clone());
        let extension_id = slug_bzlmod::canonical_extension_id("//:ext.bzl", "ext", "root");
        let generated_repo = "root+ext+generated".to_owned();
        let mut overrides = slug_bzlmod::RepoMappingOverrides::new();
        overrides.insert(
            extension_id.clone(),
            BTreeMap::from([("generated".to_owned(), "helper_alias".to_owned())]),
        );

        canonicalize_repo_mapping_targets(
            &mut snapshot,
            &mut overrides,
            &cells,
            Some(&resolved_graph),
        );
        assert_eq!(
            snapshot
                .get("")
                .and_then(|mapping| mapping.get("_main+ext+generated"))
                .map(String::as_str),
            Some("dep+")
        );
        assert_eq!(
            overrides
                .get(&extension_id)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep+")
        );

        assert!(slug_bzlmod::add_extension_generated_repo_mappings(
            &mut snapshot,
            &extension_id,
            "root",
            [("generated".to_owned(), generated_repo.clone())],
            overrides.get(&extension_id),
        ));
        assert_eq!(
            snapshot
                .get(&generated_repo)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep+")
        );
        Ok(())
    }

    #[test]
    fn graph_owned_repo_mapping_state_removes_root_apparent_override_targets() {
        let mut root = parsed_module("root");
        let mut dep = slug_bzlmod::BazelDep::new("dep".to_owned(), Version::empty());
        dep.repo_name = Some("helper_alias".to_owned());
        root.module.bazel_deps.push(dep);
        let mut usage = slug_bzlmod::ExtensionUsage::new("//:ext.bzl".to_owned(), "ext".to_owned());
        usage
            .repo_overrides
            .push(("generated".to_owned(), "helper_alias".to_owned()));
        root.extension_usages.push(usage);

        let (snapshot, overrides) =
            graph_owned_repo_mapping_state(&[("root".to_owned(), root)], "root", false, &[], None);
        let extension_id = slug_bzlmod::canonical_extension_id("//:ext.bzl", "ext", "root");

        assert_eq!(
            snapshot
                .get("")
                .and_then(|mapping| mapping.get("_main+ext+generated"))
                .map(String::as_str),
            Some("dep")
        );
        assert_eq!(
            overrides
                .get(&extension_id)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep")
        );
    }

    #[tokio::test]
    async fn bzlmod_cell_resolver_uses_canonical_module_cells_from_cell_graph()
    -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-canonical-cell-graph-test");
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), project_root.join("buck-out/v2"));
        let projection_data = slug_bzlmod::BzlmodCellGraphValue {
            workspace_id,
            root_module_name: "root".to_owned(),
            cells: Arc::new(vec![slug_bzlmod::BzlmodCellGraphCell {
                name: "dep+".to_owned(),
                path: "dep".to_owned(),
                module_setup: None,
                bundled: false,
            }]),
            extension_cells: Arc::new(Vec::new()),
            root_aliases: Arc::new(vec![slug_bzlmod::BzlmodCellGraphAlias {
                apparent_name: "dep".to_owned(),
                target_name: "dep+".to_owned(),
            }]),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(Vec::new()),
            dynamic_aliases: Arc::new(Vec::new()),
        };
        let configs = BuckConfigBasedCells::parse_with_file_ops_and_options_inner(
            &[],
            None,
            Some(Arc::new(Some(projection_data))),
            slug_bzlmod::WorkspaceId::no_project_sentinel(),
        )
        .await?;
        assert!(
            configs
                .cell_resolver
                .get(CellName::unchecked_new("dep+")?)
                .is_ok()
        );
        assert!(
            configs
                .cell_resolver
                .get(CellName::unchecked_new("dep")?)
                .is_err()
        );
        assert_eq!(
            configs
                .cell_resolver
                .root_cell_cell_alias_resolver()
                .resolve_declared_or_runtime_alias("dep")
                .map(|name| name.as_str().to_owned()),
            Some("dep+".to_owned())
        );
        let parsed = slug_core::provider::label::ProvidersLabel::parse(
            "@dep//:target",
            CellName::unchecked_new("root")?,
            &configs.cell_resolver,
            configs.cell_resolver.root_cell_cell_alias_resolver(),
        )?;
        assert_eq!(parsed.target().pkg().cell_name().as_str(), "dep+");
        Ok(())
    }

    #[test]
    fn runtime_cell_install_snapshot_derives_from_cell_graph() {
        let project_root = PathBuf::from("/tmp/slug-plan61-cell-graph-runtime-test");
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), project_root.join("buck-out/v2"));
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
        assert!(snapshot.extension_cells[0].setup.materialized);
        assert_eq!(
            snapshot.extension_cells[1].setup.repo_env_json.as_ref(),
            r#"{"K":"V"}"#
        );
        assert!(!snapshot.extension_cells[1].setup.materialized);
        assert_eq!(snapshot.scoped_aliases[0].apparent_name, "tool");
        assert_eq!(snapshot.dynamic_aliases[0].canonical_name, "root+ext+eager");
    }

    #[test]
    fn bzlmod_runtime_state_uses_workspace_output_base_for_external_cell_symlinks()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("dep_src/BUILD.bazel", "");
        let source_path = fs
            .path()
            .resolve(ProjectRelativePath::new("dep_src")?)
            .to_path_buf();
        let output_base = fs
            .path()
            .resolve(ProjectRelativePath::new("buck-out/custom-runtime")?);
        let workspace_id = slug_bzlmod::WorkspaceId::new(
            fs.path().root().to_path_buf(),
            output_base.to_path_buf(),
        );
        let cell_graph = slug_bzlmod::BzlmodCellGraphValue {
            workspace_id,
            root_module_name: "root".to_owned(),
            cells: Arc::new(Vec::new()),
            extension_cells: Arc::new(vec![slug_bzlmod::BzlmodCellGraphExtensionCell {
                canonical_name: "root+ext+repo".to_owned(),
                internal_name: "repo".to_owned(),
                path: "bazel-external/root+ext+repo".to_owned(),
                extension_id: "@@root//:ext.bzl%ext".to_owned(),
                spec_hash: "repo-hash".to_owned(),
                repo_spec_json: "{}".to_owned(),
                repo_env_json: "{}".to_owned(),
                materialized: false,
                lazy: true,
            }]),
            root_aliases: Arc::new(Vec::new()),
            module_symlinks: Arc::new(vec![slug_bzlmod::BzlmodCellGraphModuleSymlink {
                entry_name: "dep".to_owned(),
                source_path: Arc::new(source_path.clone()),
            }]),
            scoped_aliases: Arc::new(vec![slug_bzlmod::BzlmodCellGraphScopedAlias {
                owner_module: "root".to_owned(),
                apparent_name: "repo".to_owned(),
                target_name: "root+ext+repo".to_owned(),
            }]),
            dynamic_aliases: Arc::new(vec![slug_bzlmod::BzlmodCellGraphDynamicAlias {
                apparent_name: "repo".to_owned(),
                canonical_name: "root+ext+repo".to_owned(),
            }]),
        };

        replay_bzlmod_runtime_state(&cell_graph, fs.path());

        let explicit_link = output_base.as_path().join("external_cells/bzlmod/dep");
        assert_eq!(std::fs::read_link(explicit_link)?, source_path);
        let default_link = fs.path().resolve(ProjectRelativePath::new(
            "buck-out/v2/external_cells/bzlmod/dep",
        )?);
        assert!(!default_link.exists());
        assert!(slug_core::cells::get_dynamic_extension_cell("root+ext+repo").is_none());
        assert!(slug_core::cells::get_dynamic_extension_cell_setup("root+ext+repo").is_none());
        assert!(slug_core::cells::resolve_dynamic_extension_cell_alias("repo").is_none());
        Ok(())
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
        let project_root = PathBuf::from("/tmp/slug-plan61-bundled-cell-graph-test");
        let workspace_id =
            slug_bzlmod::WorkspaceId::new(project_root.clone(), project_root.join("buck-out/v2"));
        let projection_data = slug_bzlmod::BzlmodCellGraphValue {
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
            Some(Arc::new(Some(projection_data))),
            slug_bzlmod::WorkspaceId::no_project_sentinel(),
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
    async fn fallback_scanned_extension_bzl_digest_matches_legacy_project_load_digest()
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
        let direct = slug_bzlmod::compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
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
        let fallback_scanned = dice
            .compute(&FallbackScannedExtensionBzlDigestKey {
                project_root: AbsNormPathBuf::try_from(fs.path().root().as_path().to_path_buf())?,
                extension_id: Arc::from(extension_id),
                repo_mappings: Arc::new(repo_mappings.clone()),
            })
            .await??;

        assert_eq!(fallback_scanned.as_ref(), direct);
        assert!(<FallbackScannedExtensionBzlDigestKey as Key>::validity(
            &Ok(fallback_scanned)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn fallback_scanned_extension_bzl_digest_tracks_missing_project_load_digest()
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

        let extension_id = "@@root//:ext.bzl%ext";
        let repo_mappings = slug_bzlmod::RepoMappingSnapshot::new();
        let project_root = AbsNormPathBuf::try_from(fs.path().root().as_path().to_path_buf())?;
        let key = FallbackScannedExtensionBzlDigestKey {
            project_root: project_root.clone(),
            extension_id: Arc::from(extension_id),
            repo_mappings: Arc::new(repo_mappings.clone()),
        };
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let fallback_scanned_missing = dice.compute(&key).await??;
        assert_ne!(
            fallback_scanned_missing.as_ref(),
            slug_bzlmod::compute_bzl_transitive_digest(extension_id)
        );
        assert!(<FallbackScannedExtensionBzlDigestKey as Key>::validity(
            &Ok(fallback_scanned_missing.clone())
        ));

        fs.write_file("helper.bzl", "HELPER = 'created'\n");
        let mut updater = dice.into_updater();
        let mut changes = crate::file_ops::dice::FileChangeTracker::new();
        changes.project_file_added_or_removed(ProjectRelativePathBuf::unchecked_new(
            "helper.bzl".to_owned(),
        ));
        changes.write_to_dice(&mut updater)?;
        let mut dice = updater.commit().await;
        let direct_created =
            slug_bzlmod::compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                extension_id,
                fs.path().root().as_path(),
                Some(&repo_mappings),
            );
        let fallback_scanned_created = dice
            .compute(&FallbackScannedExtensionBzlDigestKey {
                project_root,
                extension_id: Arc::from(extension_id),
                repo_mappings: Arc::new(repo_mappings),
            })
            .await??;

        assert_ne!(fallback_scanned_missing, fallback_scanned_created);
        assert_eq!(fallback_scanned_created.as_ref(), direct_created);
        assert!(<FallbackScannedExtensionBzlDigestKey as Key>::validity(
            &Ok(fallback_scanned_created)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn preseed_recorded_inputs_track_workspace_file_through_dice() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("watched.txt", "first\n");
        register_bzlmod_config_project_file(ProjectRelativePathBuf::unchecked_new(
            "watched.txt".to_owned(),
        ));
        let recorded_inputs = vec![slug_bzlmod::recorded_file_input_with_recorded_path(
            Path::new("@@//watched.txt"),
            &fs.path().root().as_path().join("watched.txt"),
        )?];
        let repo_env = BTreeMap::new();
        let repo_mappings = slug_bzlmod::RepoMappingSnapshot::new();
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        assert_eq!(
            preseed_recorded_inputs_current_through_dice(
                &mut dice,
                &recorded_inputs,
                Some(fs.path().root().as_path()),
                Some(&repo_env),
                Some(&repo_mappings),
            )
            .await?,
            Some(Ok(()))
        );

        fs.write_file("watched.txt", "second\n");
        let mut updater = dice.into_updater();
        let mut changes = crate::file_ops::dice::FileChangeTracker::new();
        changes.project_file_contents_changed(ProjectRelativePathBuf::unchecked_new(
            "watched.txt".to_owned(),
        ));
        changes.write_to_dice(&mut updater)?;
        let mut dice = updater.commit().await;

        let result = preseed_recorded_inputs_current_through_dice(
            &mut dice,
            &recorded_inputs,
            Some(fs.path().root().as_path()),
            Some(&repo_env),
            Some(&repo_mappings),
        )
        .await?;
        assert!(matches!(result, Some(Err(reason)) if reason.contains("recorded_input_changed")));
        Ok(())
    }

    #[tokio::test]
    async fn preseed_recorded_inputs_track_workspace_dirents_through_dice() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        std::fs::create_dir(fs.path().root().as_path().join("watched_dir"))?;
        fs.write_file("watched_dir/first.txt", "first\n");
        let recorded =
            slug_bzlmod::recorded_dirents_input(&fs.path().root().as_path().join("watched_dir"))?;
        let (_, digest) = recorded
            .split_once(' ')
            .expect("recorded dirents marker has digest");
        let recorded_inputs = vec![format!("DIRENTS:@@//watched_dir {digest}")];
        let repo_env = BTreeMap::new();
        let repo_mappings = slug_bzlmod::RepoMappingSnapshot::new();
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        assert_eq!(
            preseed_recorded_inputs_current_through_dice(
                &mut dice,
                &recorded_inputs,
                Some(fs.path().root().as_path()),
                Some(&repo_env),
                Some(&repo_mappings),
            )
            .await?,
            Some(Ok(()))
        );

        fs.write_file("watched_dir/second.txt", "second\n");
        let mut dice = dice.into_updater().commit().await;
        let result = preseed_recorded_inputs_current_through_dice(
            &mut dice,
            &recorded_inputs,
            Some(fs.path().root().as_path()),
            Some(&repo_env),
            Some(&repo_mappings),
        )
        .await?;
        assert!(matches!(result, Some(Err(reason)) if reason.contains("recorded_input_changed")));
        Ok(())
    }

    #[tokio::test]
    async fn preseed_recorded_inputs_track_workspace_dirtree_through_dice() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        std::fs::create_dir(fs.path().root().as_path().join("watched_tree"))?;
        std::fs::create_dir(fs.path().root().as_path().join("watched_tree/nested"))?;
        fs.write_file("watched_tree/nested/leaf.txt", "first\n");
        let recorded =
            slug_bzlmod::recorded_dirtree_input(&fs.path().root().as_path().join("watched_tree"))?;
        let (_, digest) = recorded
            .split_once(' ')
            .expect("recorded dirtree marker has digest");
        let recorded_inputs = vec![format!("DIRTREE:@@//watched_tree {digest}")];
        let repo_env = BTreeMap::new();
        let repo_mappings = slug_bzlmod::RepoMappingSnapshot::new();
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        assert_eq!(
            preseed_recorded_inputs_current_through_dice(
                &mut dice,
                &recorded_inputs,
                Some(fs.path().root().as_path()),
                Some(&repo_env),
                Some(&repo_mappings),
            )
            .await?,
            Some(Ok(()))
        );

        fs.write_file("watched_tree/nested/leaf.txt", "second\n");
        let mut dice = dice.into_updater().commit().await;
        let result = preseed_recorded_inputs_current_through_dice(
            &mut dice,
            &recorded_inputs,
            Some(fs.path().root().as_path()),
            Some(&repo_env),
            Some(&repo_mappings),
        )
        .await?;
        assert!(matches!(result, Some(Err(reason)) if reason.contains("recorded_input_changed")));
        Ok(())
    }

    #[tokio::test]
    async fn bzlmod_lockfile_inputs_bridge_tracks_visible_lockfile_edits() -> slug_error::Result<()>
    {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        fs.write_file("MODULE.bazel.lock", &minimal_lockfile_json("first"));
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = compute_bzlmod_lockfile_inputs_bridge(
            &mut dice,
            fs.path(),
            slug_bzlmod::LockfileMode::Update,
            None,
            true,
        )
        .await?;
        let first_digest = first
            .visible_lockfile_digest
            .clone()
            .expect("visible lockfile should be read");
        assert!(
            first
                .visible_lockfile
                .as_ref()
                .expect("visible lockfile value should be present")
                .tracked_by_dice
        );

        fs.write_file("MODULE.bazel.lock", &minimal_lockfile_json("second"));
        let mut updater = dice.into_updater();
        let mut changes = crate::file_ops::dice::FileChangeTracker::new();
        changes.project_file_contents_changed(ProjectRelativePathBuf::unchecked_new(
            "MODULE.bazel.lock".to_owned(),
        ));
        changes.write_to_dice(&mut updater)?;
        let mut dice = updater.commit().await;
        let second = compute_bzlmod_lockfile_inputs_bridge(
            &mut dice,
            fs.path(),
            slug_bzlmod::LockfileMode::Update,
            None,
            true,
        )
        .await?;

        assert_ne!(
            first_digest,
            second
                .visible_lockfile_digest
                .as_deref()
                .expect("edited visible lockfile should be read")
        );
        assert!(second.hidden_lockfile.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn bzlmod_lockfile_inputs_bridge_tracks_hidden_lockfile_fail_open()
    -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        fs.write_file("MODULE.bazel.lock", &minimal_lockfile_json("visible"));
        let hidden_dir = tempfile::Builder::new()
            .prefix("slug-plan61-hidden-lockfile-inputs-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let hidden_path = hidden_dir.path().join("MODULE.bazel.lock");
        std::fs::write(&hidden_path, minimal_lockfile_json("hidden")).unwrap();
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = compute_bzlmod_lockfile_inputs_bridge(
            &mut dice,
            fs.path(),
            slug_bzlmod::LockfileMode::Update,
            Some(hidden_path.clone()),
            true,
        )
        .await?;
        assert!(first.hidden_lockfile_digest.is_some());
        assert!(
            !first
                .hidden_lockfile
                .as_ref()
                .expect("hidden lockfile value should be present")
                .tracked_by_dice
        );

        std::fs::write(&hidden_path, "{ this is not json }\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let invalid = compute_bzlmod_lockfile_inputs_bridge(
            &mut dice,
            fs.path(),
            slug_bzlmod::LockfileMode::Update,
            Some(hidden_path.clone()),
            true,
        )
        .await?;

        assert!(invalid.hidden_lockfile_digest.is_none());
        assert!(
            invalid
                .hidden_lockfile
                .as_ref()
                .expect("hidden lockfile path should still be represented")
                .lockfile
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn bzlmod_lockfile_inputs_bridge_mode_off_reads_no_lockfiles() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        fs.write_file("MODULE.bazel", r#"module(name = "root")"#);
        fs.write_file("MODULE.bazel.lock", "{ this is not json }\n");
        let mut dice = DiceBuilder::new()
            .set_data(|data| {
                data.set_testing_io_provider(&fs);
            })
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let inputs = compute_bzlmod_lockfile_inputs_bridge(
            &mut dice,
            fs.path(),
            slug_bzlmod::LockfileMode::Off,
            None,
            true,
        )
        .await?;

        assert_eq!(inputs.lockfile_mode, slug_bzlmod::LockfileMode::Off);
        assert!(inputs.visible_lockfile.is_none());
        assert!(inputs.hidden_lockfile.is_none());
        Ok(())
    }

    #[test]
    fn bzlmod_resolved_module_graph_key_includes_hidden_lockfile_identity() {
        let first = bzlmod_resolved_module_graph_key(lockfile_value(
            "/tmp/hidden/MODULE.bazel.lock",
            "hidden-lockfile-first",
        ));
        let second = bzlmod_resolved_module_graph_key(lockfile_value(
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
