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
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::Key;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod::BzlmodEventKind;
use slug_bzlmod::ModuleCache;
use slug_bzlmod::ModuleSource;
use slug_bzlmod::MvsResolver;
use slug_bzlmod::ResolvedGraph;
use slug_bzlmod::parse_module_bazel;
use slug_bzlmod::record_bzlmod_event;
use slug_bzlmod::resolve_local_modules;
use slug_bzlmod::types::ParsedModuleFile;
use slug_bzlmod::types::TagValue;
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
use slug_error::BuckErrorContext;
use slug_fs::fs_util;
use slug_fs::paths::RelativePath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;

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

fn register_lockfile_seeded_dynamic_cells(
    project_root: &ProjectRoot,
    cells: &[BzlmodPendingRepoCell],
) {
    for cell in cells {
        let setup = ExtensionRepoCellSetup {
            canonical_name: Arc::from(cell.canonical_name.as_str()),
            extension_id: Arc::from(cell.extension_id.as_str()),
            internal_name: Arc::from(cell.internal_name.as_str()),
            spec_hash: Arc::from(cell.spec_hash.as_str()),
            repo_spec_json: Arc::from(cell.repo_spec_json.as_str()),
            materialized: false,
        };
        slug_core::cells::register_dynamic_extension_cell_with_setup_lazy(
            cell.canonical_name.clone(),
            cell.path.clone(),
            setup.clone(),
        );
        if cell.internal_name != cell.canonical_name {
            slug_core::cells::register_dynamic_extension_cell_with_setup_lazy(
                cell.internal_name.clone(),
                cell.path.clone(),
                setup.clone(),
            );
        }
        if cell.repo_spec_json.is_empty() {
            continue;
        }
        match serde_json::from_str::<slug_bzlmod::RepoSpec>(&cell.repo_spec_json) {
            Ok(repo_spec) => {
                let registration = slug_bzlmod::SpokeRegistration {
                    extension_id: Arc::from(cell.extension_id.as_str()),
                    repo_spec: Arc::new(repo_spec),
                    project_root: Arc::new(project_root.root().to_path_buf()),
                };
                slug_bzlmod::register_spoke(cell.canonical_name.clone(), registration.clone());
                if cell.internal_name != cell.canonical_name {
                    slug_bzlmod::register_spoke(cell.internal_name.clone(), registration);
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse lockfile-seeded RepoSpec for '{}': {}",
                    cell.canonical_name,
                    e
                );
            }
        }
    }
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
) -> slug_bzlmod::RepoMappingOverrides {
    let mut overrides = slug_bzlmod::RepoMappingOverrides::new();
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
            if usage.repo_overrides.is_empty() {
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

/// Result of bzlmod dependency resolution.
#[derive(Clone, PartialEq, Eq, Allocative)]
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
    module_symlinks: Vec<BzlmodExternalModuleSymlink>,
    lockfile_seeded_cells: Vec<BzlmodPendingRepoCell>,
    scoped_repo_aliases: Vec<BzlmodScopedRepoAlias>,
    dynamic_extension_aliases: Vec<BzlmodDynamicAlias>,
    eager_repo_rule_invocations: Vec<slug_bzlmod::RepositoryInvocation>,
    /// DICE-injected bzlmod facts derived from this resolution.
    session_data: slug_bzlmod::BzlmodSessionData,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodExternalModuleSymlink {
    entry_name: String,
    source_path: PathBuf,
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

impl BzlmodResolutionResult {
    fn replay_runtime_state(&self, project_root: &ProjectRoot) {
        slug_core::cells::reset_dynamic_bzlmod_state_for_project_root(
            project_root.root().to_path_buf(),
        );
        slug_bzlmod::reset_spoke_materialization_project_root(project_root.root().to_path_buf());

        let external_base_dir = project_root.root().as_path().join("bazel-external");
        let buck_out_external_cells_dir = project_root
            .root()
            .as_path()
            .join("buck-out/v2/external_cells/bzlmod");
        let mut valid_symlink_names = std::collections::HashSet::new();
        for symlink in &self.module_symlinks {
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

        register_lockfile_seeded_dynamic_cells(project_root, &self.lockfile_seeded_cells);

        for (name, path, setup) in &self.extension_cells {
            slug_core::cells::register_dynamic_extension_cell_with_setup(
                setup.canonical_name.to_string(),
                path.as_str().to_owned(),
                setup.dupe(),
            );
            if setup.internal_name != setup.canonical_name {
                slug_core::cells::register_dynamic_extension_cell_with_setup(
                    setup.internal_name.to_string(),
                    path.as_str().to_owned(),
                    setup.dupe(),
                );
            }
            if setup.repo_spec_json.is_empty() {
                continue;
            }
            match serde_json::from_str::<slug_bzlmod::RepoSpec>(&setup.repo_spec_json) {
                Ok(repo_spec) => {
                    let registration = slug_bzlmod::SpokeRegistration {
                        extension_id: setup.extension_id.dupe(),
                        repo_spec: Arc::new(repo_spec),
                        project_root: Arc::new(project_root.root().to_path_buf()),
                    };
                    slug_bzlmod::register_spoke(name.as_str().to_owned(), registration.clone());
                    slug_bzlmod::register_spoke(
                        setup.canonical_name.to_string(),
                        registration.clone(),
                    );
                    if setup.internal_name != setup.canonical_name {
                        slug_bzlmod::register_spoke(setup.internal_name.to_string(), registration);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse precomputed RepoSpec for '{}': {}",
                        setup.canonical_name,
                        e
                    );
                }
            }
        }

        for alias in &self.scoped_repo_aliases {
            slug_core::cells::register_scoped_bzlmod_repo_alias(
                alias.owner_module.clone(),
                alias.apparent_name.clone(),
                alias.target_name.clone(),
            );
        }
        for alias in &self.dynamic_extension_aliases {
            slug_core::cells::register_dynamic_extension_cell_alias(
                alias.apparent_name.clone(),
                alias.canonical_name.clone(),
            );
        }

        for invocation in &self.eager_repo_rule_invocations {
            match slug_bzlmod::execute_repository_rule(invocation, project_root.root().as_path()) {
                Ok(_) => tracing::debug!(
                    "Replayed MODULE.bazel repo rule materialization for '{}'",
                    invocation.name
                ),
                Err(e) => tracing::warn!(
                    "Failed to replay MODULE.bazel repo rule materialization for '{}': {}",
                    invocation.name,
                    e
                ),
            }
        }

        let cell_pairs: Vec<(String, String)> = self
            .cells
            .iter()
            .map(|(name, path, _)| (name.as_str().to_owned(), path.as_str().to_owned()))
            .chain(
                self.extension_cells
                    .iter()
                    .map(|(name, path, _)| (name.as_str().to_owned(), path.as_str().to_owned())),
            )
            .collect();
        slug_core::cells::ensure_external_symlinks_for_cells(&cell_pairs);
        for (alias, canonical) in &self.aliases {
            let alias_str = alias.as_str();
            if let Some((_, path, _)) = self
                .cells
                .iter()
                .find(|(name, _, _)| name.as_str() == canonical.as_str())
            {
                slug_core::cells::ensure_external_symlink(alias_str, path.as_str());
            } else if let Some((_, path, _)) = self
                .extension_cells
                .iter()
                .find(|(name, _, _)| name.as_str() == canonical.as_str())
            {
                slug_core::cells::ensure_external_symlink(alias_str, path.as_str());
            }
        }
        slug_core::cells::repair_external_symlink_targets(project_root.root().as_path());
    }
}

static LEGACY_BZLMOD_RESOLUTION_CACHE: LazyLock<
    Mutex<HashMap<LegacyBzlmodResolutionDiceKey, Arc<Option<BzlmodResolutionResult>>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct BzlmodResolutionOptions {
    lockfile_mode: slug_bzlmod::LockfileMode,
    allow_yanked_versions_env: Option<String>,
    allow_yanked_versions_flags: Vec<String>,
    hidden_lockfile_path: Option<PathBuf>,
}

impl BzlmodResolutionOptions {
    fn from_config(root_config: &LegacyBuckConfig) -> slug_error::Result<Self> {
        let bzlmod_section = root_config.get_section("bzlmod");
        Ok(Self {
            lockfile_mode: BuckConfigBasedCells::bzlmod_lockfile_mode_from_config(root_config)?,
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
        })
    }

    fn policy_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", self.lockfile_mode).as_bytes());
        hasher.update([0]);
        if let Some(value) = &self.allow_yanked_versions_env {
            hasher.update(value.as_bytes());
        }
        hasher.update([0]);
        for value in &self.allow_yanked_versions_flags {
            hasher.update(value.as_bytes());
            hasher.update([0]);
        }
        hex::encode(hasher.finalize())
    }
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
    extension_replay_summary_digest: Option<Arc<str>>,
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
        && left.allow_yanked_versions_env == right.allow_yanked_versions_env
        && left.allow_yanked_versions_flags == right.allow_yanked_versions_flags
}

fn hash_bzlmod_resolution_options_policy<H: std::hash::Hasher>(
    value: &BzlmodResolutionOptions,
    state: &mut H,
) {
    value.lockfile_mode.hash(state);
    value.allow_yanked_versions_env.hash(state);
    value.allow_yanked_versions_flags.hash(state);
}

fn legacy_bzlmod_resolution_bridge_cacheable(key: &LegacyBzlmodResolutionDiceKey) -> bool {
    let Some(parsed) = key.root_module_file.parsed.as_ref() else {
        return true;
    };
    if !parsed.repo_rule_invocations.is_empty() {
        return false;
    }
    if parsed
        .module
        .overrides
        .iter()
        .any(|override_| matches!(override_, slug_bzlmod::types::Override::Git(_)))
    {
        return false;
    }
    if key.local_override_inputs.has_git_overrides
        || key.local_override_inputs.has_extension_usages
        || key.local_override_inputs.has_repo_rule_invocations
    {
        return false;
    }

    let local_override_names = parsed
        .module
        .overrides
        .iter()
        .filter_map(|override_| match override_ {
            slug_bzlmod::types::Override::LocalPath(local) => Some(local.module_name.as_str()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let has_registry_or_remote_dep = parsed
        .module
        .bazel_deps
        .iter()
        .any(|dep| !local_override_names.contains(dep.name.as_str()));
    if (has_registry_or_remote_dep || key.local_override_inputs.has_bazel_deps)
        && key
            .visible_lockfile
            .as_ref()
            .and_then(|value| value.digest.as_ref())
            .is_none()
    {
        return false;
    }
    if !parsed.extension_usages.is_empty() {
        return key.extension_replay_summary_digest.is_some()
            || !parsed.module.bazel_deps.is_empty()
            || !parsed.module.overrides.is_empty();
    }
    let lockfile_has_extension_entries = |lockfile: &slug_bzlmod::Lockfile| {
        !lockfile.module_extensions.is_empty() || !lockfile.facts.is_empty()
    };
    if key
        .visible_lockfile
        .as_ref()
        .and_then(|value| value.lockfile.as_deref())
        .is_some_and(lockfile_has_extension_entries)
    {
        return false;
    }
    if key
        .hidden_lockfile
        .as_ref()
        .and_then(|value| value.lockfile.as_deref())
        .is_some_and(lockfile_has_extension_entries)
    {
        return false;
    }
    true
}

fn legacy_bzlmod_resolution_result_bridge_cacheable(
    value: &Arc<Option<BzlmodResolutionResult>>,
) -> bool {
    value.as_ref().as_ref().is_none_or(|result| {
        result.lockfile_seeded_cells.is_empty() && result.eager_repo_rule_invocations.is_empty()
    })
}

fn root_extension_replay_summary_digest(
    parsed: &ParsedModuleFile,
    project_root: &Path,
    visible_lockfile: Option<&slug_bzlmod::Lockfile>,
    hidden_lockfile: Option<&slug_bzlmod::Lockfile>,
) -> Option<String> {
    if parsed.extension_usages.is_empty()
        || !parsed.module.bazel_deps.is_empty()
        || !parsed.module.overrides.is_empty()
        || !parsed.repo_rule_invocations.is_empty()
    {
        return None;
    }

    let root_module_name = if parsed.module.name.is_empty() {
        "_main"
    } else {
        &parsed.module.name
    };
    let parsed_modules = vec![(root_module_name.to_owned(), parsed.clone())];
    let mut module_extensions = HashMap::new();
    module_extensions.insert(root_module_name.to_owned(), parsed.extension_usages.clone());
    let aggregated =
        slug_bzlmod::aggregate_extensions_with_root(&module_extensions, Some(root_module_name));
    if aggregated.is_empty() {
        return None;
    }

    let repo_env = slug_bzlmod::legacy_bzlmod_repo_env();
    let repo_mappings = repo_mapping_snapshot_for_modules(&parsed_modules, root_module_name);
    let repo_mapping_overrides = repo_mapping_overrides_for_root(&parsed_modules, root_module_name);

    let mut hasher = Sha256::new();
    hasher.update(b"root-extension-replay-summary-v1");
    hasher.update([0]);
    for (name, value) in &repo_env {
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
        let extension = aggregated.get(&extension_id)?;
        let bzl_transitive_digest =
            slug_bzlmod::compute_bzl_transitive_digest_for_project(&extension_id, project_root);
        let usages_digest = slug_bzlmod::compute_extension_input_hash(extension);
        let visible_specs = visible_lockfile.and_then(|lockfile| {
            lockfile.get_extension_cache_for_workspace(
                &extension_id,
                &bzl_transitive_digest,
                &usages_digest,
                Some(project_root),
                Some(&repo_env),
                Some(&repo_mappings),
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
                Some(project_root),
                Some(&repo_env),
                Some(&repo_mappings),
                Some(root_module_name),
                Some(&repo_mapping_overrides),
            )
        }) {
            ("hidden", cached_specs)
        } else {
            return None;
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
            let spec = cached_specs.get(&repo_name)?;
            hasher.update(repo_name.as_bytes());
            hasher.update([0]);
            hasher.update(spec.compute_hash().as_bytes());
            hasher.update([0]);
        }
    }

    Some(hex::encode(hasher.finalize()))
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("LocalOverrideModuleInputsKey({})", project_root.display())]
struct LocalOverrideModuleInputsKey {
    project_root: AbsNormPathBuf,
    overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct LocalOverrideModuleInputsValue {
    digest: String,
    has_bazel_deps: bool,
    has_extension_usages: bool,
    has_repo_rule_invocations: bool,
    has_git_overrides: bool,
}

fn local_overrides_from_root_module(
    root_module_file: &slug_bzlmod::RootModuleFileValue,
) -> Vec<(String, String)> {
    root_module_file
        .parsed
        .as_ref()
        .map(|parsed| {
            parsed
                .module
                .overrides
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

fn local_override_module_inputs_digest(
    project_root: &Path,
    overrides: &[(String, String)],
) -> slug_error::Result<LocalOverrideModuleInputsValue> {
    let mut hasher = Sha256::new();
    hasher.update(b"local-override-module-inputs-v2");
    hasher.update([0]);
    let mut queue = VecDeque::new();
    for (module_name, path) in overrides {
        queue.push_back((
            module_name.clone(),
            project_root.to_path_buf(),
            path.clone(),
        ));
    }

    let mut visited_module_dirs = HashSet::new();
    let mut has_bazel_deps = false;
    let mut has_extension_usages = false;
    let mut has_repo_rule_invocations = false;
    let mut has_git_overrides = false;

    while let Some((module_name, base, path)) = queue.pop_front() {
        let module_dir = base.join(&path);
        let normalized_module_dir = module_dir
            .canonicalize()
            .unwrap_or_else(|_| module_dir.clone());
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

        let module_bazel_path = normalized_module_dir.join("MODULE.bazel");
        match std::fs::read(&module_bazel_path) {
            Ok(content) => {
                hasher.update(b"present");
                hasher.update([0]);
                let content_digest = slug_bzlmod::lockfile::compute_sha256_hex(&content);
                let content = String::from_utf8(content).map_err(|e| {
                    slug_error::slug_error!(
                        slug_error::ErrorTag::Input,
                        "Failed to read MODULE.bazel for local override '{}' at {:?}: {}",
                        module_name,
                        module_bazel_path,
                        e
                    )
                })?;
                let parsed_with_inputs = slug_bzlmod::parser::parse_module_bazel_content_from_path(
                    &module_bazel_path,
                    &content,
                    content_digest,
                )
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
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                hasher.update(b"missing");
                hasher.update([0]);
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(LocalOverrideModuleInputsValue {
        digest: hex::encode(hasher.finalize()),
        has_bazel_deps,
        has_extension_usages,
        has_repo_rule_invocations,
        has_git_overrides,
    })
}

#[async_trait]
impl Key for LocalOverrideModuleInputsKey {
    type Value = slug_error::Result<Arc<LocalOverrideModuleInputsValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        local_override_module_inputs_digest(
            AsRef::<Path>::as_ref(&self.project_root),
            &self.overrides,
        )
        .map(Arc::new)
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

#[async_trait]
impl Key for LegacyBzlmodResolutionDiceKey {
    type Value = slug_error::Result<Arc<Option<BzlmodResolutionResult>>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        BuckConfigBasedCells::resolve_bzlmod_resolution_from_key(self).await
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
        let bridge_cache_candidate = legacy_bzlmod_resolution_bridge_cacheable(&key);
        let mut should_seed_bridge_cache = false;
        let bzlmod_resolution = if bridge_cache_candidate {
            let cached_resolution = LEGACY_BZLMOD_RESOLUTION_CACHE
                .lock()
                .ok()
                .and_then(|cache| cache.get(&key).cloned());
            if let Some(bzlmod_resolution) = cached_resolution {
                should_seed_bridge_cache = true;
                bzlmod_resolution
            } else {
                let bzlmod_resolution = dice_ctx
                    .compute(&key)
                    .await?
                    .buck_error_context("Computing bzlmod resolution through DICE")?;
                if legacy_bzlmod_resolution_result_bridge_cacheable(&bzlmod_resolution) {
                    if let Ok(mut cache) = LEGACY_BZLMOD_RESOLUTION_CACHE.lock() {
                        cache.insert(key.clone(), bzlmod_resolution.clone());
                    }
                    should_seed_bridge_cache = true;
                }
                bzlmod_resolution
            }
        } else {
            Self::resolve_bzlmod_resolution_from_key(&key)
                .await
                .buck_error_context("Computing uncached bzlmod resolution")?
        };
        if should_seed_bridge_cache {
            updater.changed_to([(key, Ok(bzlmod_resolution.clone()))])?;
        }

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
        let resolution_key = slug_bzlmod::BzlmodResolutionKey {
            workspace_id: workspace_id.clone(),
            command_policy_digest: Arc::from(options.policy_digest().as_str()),
        };
        let root_module_file = dice_ctx
            .compute(&slug_bzlmod::RootModuleFileKey {
                workspace_id: workspace_id.clone(),
            })
            .await?
            .buck_error_context("Computing root MODULE.bazel for bzlmod resolution")?;
        let project_root = AbsNormPathBuf::try_from(project_root_path)?;
        let local_override_inputs = dice_ctx
            .compute(&LocalOverrideModuleInputsKey {
                project_root: project_root.clone(),
                overrides: local_overrides_from_root_module(root_module_file.as_ref()),
            })
            .await?
            .buck_error_context(
                "Computing local override MODULE.bazel inputs for bzlmod resolution",
            )?;
        let visible_lockfile = if root_module_file.parsed.is_some()
            && options.lockfile_mode != slug_bzlmod::LockfileMode::Off
        {
            Some(
                dice_ctx
                    .compute(&slug_bzlmod::LockfileContentKey {
                        workspace_id: workspace_id.clone(),
                        kind: slug_bzlmod::LockfileContentKind::Workspace,
                        path: Arc::new(slug_bzlmod::lockfile_path(project_fs.root().as_path())),
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
                Some(
                    dice_ctx
                        .compute(&slug_bzlmod::LockfileContentKey {
                            workspace_id,
                            kind: slug_bzlmod::LockfileContentKind::Hidden,
                            path: Arc::new(path.clone()),
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
        let extension_replay_summary_digest = root_module_file.parsed.as_ref().and_then(|parsed| {
            root_extension_replay_summary_digest(
                parsed,
                project_fs.root().as_path(),
                visible_lockfile
                    .as_ref()
                    .and_then(|value| value.lockfile.as_deref()),
                hidden_lockfile
                    .as_ref()
                    .and_then(|value| value.lockfile.as_deref()),
            )
        });
        let key = LegacyBzlmodResolutionDiceKey {
            project_root,
            resolution_key,
            options,
            root_module_file,
            visible_lockfile,
            hidden_lockfile,
            local_override_inputs,
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
        dice_bzlmod_resolution: Option<Arc<Option<BzlmodResolutionResult>>>,
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
        let mut bzlmod_session_data = slug_bzlmod::BzlmodSessionData::default();

        // ===== Bzlmod Integration =====
        // When MODULE.bazel exists, ALL cell definitions come from bzlmod resolution.
        // The root cell name is derived from module(name = "...") in MODULE.bazel.
        // .buckconfig [cells], [cell_aliases], and [external_cells] sections are skipped.
        let mut bzlmod_aliases: Vec<(NonEmptyCellAlias, CellName)> = Vec::new();
        if let Some(bzlmod_result) = if let Some(dice_bzlmod_resolution) = dice_bzlmod_resolution {
            dice_bzlmod_resolution.as_ref().clone()
        } else if let Some(project_fs) = project_fs {
            let options = BzlmodResolutionOptions::from_config(&root_config)?;
            Self::resolve_bzlmod_dependencies_with_options(
                project_fs,
                &options,
                root_module_file.as_deref(),
                visible_lockfile.as_deref(),
                None,
            )
            .await?
        } else {
            None
        } {
            if let Some(project_fs) = project_fs {
                bzlmod_result.replay_runtime_state(project_fs);
            }
            has_module_bazel = true;
            bzlmod_session_data = bzlmod_result.session_data;

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

            // Plan 28: auto-register @slug_builtins for bzlmod projects.
            // The cell ships exports.bzl whose public symbols are
            // injected into every BUILD/.bzl by `bazel_builtins_autoload`.
            let kb_name = CellName::unchecked_new("slug_builtins")?;
            if !cell_definitions.iter().any(|(n, _)| *n == kb_name) {
                let kb_path =
                    CellRootPathBuf::new(ProjectRelativePath::new("slug_builtins")?.to_owned());
                cell_definitions.push((kb_name, kb_path));
                bzlmod_bundled_cells.push(kb_name);
                tracing::info!("Auto-registered bundled cell: slug_builtins");
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
            slug_core::cells::register_dynamic_extension_cell_alias(
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
            bzlmod_session_data,
        })
    }

    async fn resolve_bzlmod_resolution_from_key(
        key: &LegacyBzlmodResolutionDiceKey,
    ) -> slug_error::Result<Arc<Option<BzlmodResolutionResult>>> {
        let root_module_file = key.root_module_file.clone();
        if root_module_file.parsed.is_none() {
            return Ok(Arc::new(None));
        }

        let project_root = ProjectRoot::new_unchecked(key.project_root.clone());
        Self::resolve_bzlmod_dependencies_with_options(
            &project_root,
            &key.options,
            Some(root_module_file.as_ref()),
            key.visible_lockfile.as_deref(),
            key.hidden_lockfile.as_deref(),
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
        visible_lockfile: Option<&slug_bzlmod::LockfileContentValue>,
        hidden_lockfile: Option<&slug_bzlmod::LockfileContentValue>,
    ) -> slug_error::Result<Option<BzlmodResolutionResult>> {
        let module_bazel_rel = ProjectRelativePath::new("MODULE.bazel")?;
        let module_bazel_path = project_root.resolve(module_bazel_rel);

        let parsed = if let Some(root_module_file) = root_module_file {
            let Some(parsed) = root_module_file.parsed.clone() else {
                return Ok(None);
            };
            tracing::info!(
                "Found MODULE.bazel through RootModuleFileKey, resolving bzlmod dependencies"
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
        let mut dynamic_extension_aliases = Vec::new();
        let mut eager_repo_rule_invocations = Vec::new();
        let workspace_root = project_root.root().as_path();
        let mut resolved_graph_for_aliases = None;
        let mut bzlmod_session_data = slug_bzlmod::BzlmodSessionData::default();
        bzlmod_session_data.repo_env = slug_bzlmod::legacy_bzlmod_repo_env();
        let allowed_yanked_versions = slug_bzlmod::parse_allowed_yanked_versions(
            options.allow_yanked_versions_env.as_deref(),
            &options.allow_yanked_versions_flags,
        )?;
        let visible_lockfile = if options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(visible_lockfile) = visible_lockfile {
            visible_lockfile.lockfile.clone()
        } else {
            slug_bzlmod::read_lockfile_with_mode(
                project_root.root().as_path(),
                options.lockfile_mode,
            )?
        };
        let hidden_lockfile = if options.lockfile_mode == slug_bzlmod::LockfileMode::Off {
            None
        } else if let Some(hidden_lockfile) = hidden_lockfile {
            hidden_lockfile.lockfile.clone()
        } else if let Some(hidden_lockfile_path) = options.hidden_lockfile_path.as_ref() {
            slug_bzlmod::read_hidden_lockfile_path(hidden_lockfile_path)?
        } else {
            None
        };

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
            bzlmod_session_data.registry_file_hashes = resolved_graph.registry_file_hashes.clone();
            bzlmod_session_data.selected_yanked_versions =
                resolved_graph.selected_yanked_versions.clone();

            // Build a set of local override names to skip
            let local_override_names: std::collections::HashSet<_> = parsed
                .module
                .overrides
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
                    let entry_name = format!("{}+{}", module_name, module_info.version);
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

                        let external_path =
                            format!("bazel-external/{}+{}", module_name, module_info.version);
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
            bzlmod_session_data.module_versions = version_map;

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
            Vec<slug_bzlmod::types::ExtensionUsage>,
        > = std::collections::HashMap::new();
        for (module_name, parsed_mod) in &parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated =
            slug_bzlmod::aggregate_extensions_with_root(&module_extensions, Some(root_module_name));
        bzlmod_session_data.repo_mappings =
            repo_mapping_snapshot_for_modules(&parsed_modules, root_module_name);
        bzlmod_session_data.repo_mapping_overrides =
            repo_mapping_overrides_for_root(&parsed_modules, root_module_name);
        let (mut pre_computed_cells, pre_computed_aliases) =
            slug_bzlmod::pre_compute_extension_repo_cells(&parsed_modules, root_module_name)?;
        let mut extension_mapping_cells = pre_computed_cells.clone();
        add_extension_repo_mapping_rows_from_cells(
            &mut bzlmod_session_data.repo_mappings,
            &extension_mapping_cells,
            root_module_name,
            &bzlmod_session_data.repo_mapping_overrides,
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
                Some(&bzlmod_session_data.repo_env),
                Some(&bzlmod_session_data.repo_mappings),
                Some(&bzlmod_session_data.repo_mapping_overrides),
            );
            lockfile_seeded_cells.extend(extra.iter().map(BzlmodPendingRepoCell::from_pending));
            extension_mapping_cells.extend(extra);
            add_extension_repo_mapping_rows_from_cells(
                &mut bzlmod_session_data.repo_mappings,
                &extension_mapping_cells,
                root_module_name,
                &bzlmod_session_data.repo_mapping_overrides,
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
                Some(&bzlmod_session_data.repo_env),
                Some(&bzlmod_session_data.repo_mappings),
                Some(&bzlmod_session_data.repo_mapping_overrides),
            );
            lockfile_seeded_cells.extend(extra.iter().map(BzlmodPendingRepoCell::from_pending));
            extension_mapping_cells.extend(extra);
            add_extension_repo_mapping_rows_from_cells(
                &mut bzlmod_session_data.repo_mappings,
                &extension_mapping_cells,
                root_module_name,
                &bzlmod_session_data.repo_mapping_overrides,
            );
        }
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
        bzlmod_session_data.extension_aggregations = aggregated;
        bzlmod_session_data.root_module_name = root_module_name.to_owned();
        bzlmod_session_data.project_root = project_root.root().to_path_buf();
        bzlmod_session_data.hidden_lockfile_path = hidden_lockfile_path;
        bzlmod_session_data.lockfile_mode = options.lockfile_mode;

        // Collect toolchain and execution platform registrations from all modules.
        // Priority order: root module first, then BFS order of dep graph.
        // parsed_modules is already in BFS order (root first from resolution).
        // dev_dependency items from non-root modules are skipped (Bazel 9.0 behavior).
        {
            let mut all_toolchains: Vec<slug_bzlmod::RegisteredToolchain> = Vec::new();
            let mut all_exec_platforms = Vec::new();
            for (module_name, parsed_mod) in &parsed_modules {
                let is_root = module_name == root_module_name
                    || module_name == "_main"
                    || parsed_mod.module.name == root_module_name;
                let repo_mapping =
                    slug_bzlmod::BzlmodRepoMapping::for_module(parsed_mod, root_module_name);
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
                    all_toolchains.push(slug_bzlmod::RegisteredToolchain {
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
            bzlmod_session_data.registered_toolchains = all_toolchains.clone();
            bzlmod_session_data.registered_execution_platforms = all_exec_platforms;

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
            ext_cells.push((cell_name, cell_path, setup));
        }

        // Build a set of existing cell names (from bzlmod deps + synthetic repos)
        // to avoid creating aliases that conflict with cell names.
        let existing_cell_names: std::collections::HashSet<&str> =
            cells.iter().map(|(name, _, _)| name.as_str()).collect();

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
                dynamic_extension_aliases.push(BzlmodDynamicAlias {
                    apparent_name: alias.apparent_name.clone(),
                    canonical_name: target_name.clone(),
                });
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
            if !is_generated_override_alias {
                dynamic_extension_aliases.push(BzlmodDynamicAlias {
                    apparent_name: apparent_name.as_str().to_owned(),
                    canonical_name: canonical_name.as_str().to_owned(),
                });
            }
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
                    let is_custom_rule = !slug_bzlmod::is_builtin_repo_rule(rule_name);

                    if is_custom_rule {
                        // Register as extension cell for lazy DICE-based Starlark execution.
                        // In Bazel, use_repo_rule() is syntactic sugar for an implicit extension.
                        let extension_id = invocation.rule_source.clone();
                        let mut repo_spec =
                            slug_bzlmod::RepoSpec::new(invocation.rule_source.clone());
                        for (k, v) in &invocation.attrs {
                            repo_spec
                                .attributes
                                .insert(k.clone(), tag_value_to_attr_value(v));
                        }
                        let spec_hash = repo_spec.compute_hash();
                        let repo_spec_json = serde_json::to_string(&repo_spec).unwrap_or_default();

                        if let Ok(cell_name) = CellName::unchecked_new(&cell_name_str) {
                            if let Ok(cell_path) = ProjectRelativePath::new(&cell_path_str)
                                .map(|p| CellRootPathBuf::new(p.to_owned()))
                            {
                                let setup = ExtensionRepoCellSetup {
                                    canonical_name: Arc::from(cell_name_str.as_str()),
                                    extension_id: Arc::from(extension_id.as_str()),
                                    internal_name: Arc::from(cell_name_str.as_str()),
                                    spec_hash: Arc::from(spec_hash.as_str()),
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
                    let mut inv = slug_bzlmod::RepositoryInvocation::new(
                        invocation.name.clone(),
                        rule_name.to_owned(),
                    );
                    inv.rule_source = Some(invocation.rule_source.clone());
                    for (k, v) in &invocation.attrs {
                        inv.attrs.insert(k.clone(), tag_value_to_repo_attr(v));
                    }

                    // Materialize the repo
                    match slug_bzlmod::execute_repository_rule(&inv, &project_root_path) {
                        Ok(_result) => {
                            tracing::info!(
                                "Materialized MODULE.bazel repo '{}' from '{}'",
                                cell_name_str,
                                module_name
                            );
                            eager_repo_rule_invocations.push(inv);
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

        Ok(Some(BzlmodResolutionResult {
            root_module_name,
            cells,
            extension_cells: ext_cells,
            aliases,
            module_symlinks,
            lockfile_seeded_cells,
            scoped_repo_aliases,
            dynamic_extension_aliases,
            eager_repo_rule_invocations,
            session_data: bzlmod_session_data,
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

/// Extract the repo name from a toolchain/platform label.
/// E.g., "@local_config_cc_toolchains//:all" → "local_config_cc_toolchains"
///       "//cc/private/toolchain/test:default_test_runner_toolchain" → None (relative)
fn extract_repo_name_from_label(label: &str) -> Option<String> {
    let parsed = slug_bzlmod::canonicalize_label_with_package_context(label, "", "", None)?;
    let repo = parsed.repo().as_str();
    if repo.is_empty() {
        None
    } else {
        Some(repo.to_owned())
    }
}

/// Convert a TagValue to a RepoSpec AttrValue (for extension cell repo specs).
fn tag_value_to_attr_value(tv: &TagValue) -> slug_bzlmod::repository_invocations::AttrValue {
    use slug_bzlmod::repository_invocations::AttrValue;
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

fn tag_value_to_repo_attr(tv: &TagValue) -> slug_bzlmod::RepoAttrValue {
    match tv {
        TagValue::String(s) => {
            if s.starts_with("//") || s.starts_with("@") || s.starts_with(":") {
                slug_bzlmod::RepoAttrValue::Label(s.clone())
            } else {
                slug_bzlmod::RepoAttrValue::String(s.clone())
            }
        }
        TagValue::Int(i) => slug_bzlmod::RepoAttrValue::Int(*i),
        TagValue::Bool(b) => slug_bzlmod::RepoAttrValue::Bool(*b),
        TagValue::None => slug_bzlmod::RepoAttrValue::None,
        TagValue::Label(s) => slug_bzlmod::RepoAttrValue::Label(s.clone()),
        TagValue::List(items) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|v| match v {
                    TagValue::String(s) | TagValue::Label(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            slug_bzlmod::RepoAttrValue::StringList(strings)
        }
        TagValue::Dict(entries) => {
            let map: indexmap::IndexMap<String, slug_bzlmod::RepoAttrValue> = entries
                .iter()
                .map(|(k, v)| (k.clone(), tag_value_to_repo_attr(v)))
                .collect();
            slug_bzlmod::RepoAttrValue::Dict(map)
        }
    }
}
