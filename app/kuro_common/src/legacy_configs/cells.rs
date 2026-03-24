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
use std::collections::HashSet;
use std::path::Path;
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
use kuro_bzlmod::synthetic_repos::collect_synthetic_repos_with_root;
use kuro_bzlmod::synthetic_repos::materialize_synthetic_repos;
use kuro_bzlmod::types::ParsedModuleFile;
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
use kuro_core::kuro_env;
use kuro_error::BuckErrorContext;
use kuro_fs::fs_util;
use kuro_fs::paths::RelativePath;
use kuro_fs::paths::abs_path::AbsPath;
use kuro_fs::paths::forward_rel_path::ForwardRelativePath;

use crate::dice::cells::HasCellResolver;
use crate::dice::data::HasIoProvider;
use crate::external_cells::EXTERNAL_CELLS_IMPL;
use crate::legacy_configs::aggregator::CellsAggregator;
use crate::legacy_configs::args::ResolvedLegacyConfigArg;
use crate::legacy_configs::args::resolve_config_args;
use crate::legacy_configs::args::to_proto_config_args;
use crate::legacy_configs::configs::LegacyBuckConfig;
use crate::legacy_configs::dice::HasInjectedLegacyConfigs;
use crate::legacy_configs::file_ops::ConfigDirEntry;
use crate::legacy_configs::file_ops::ConfigParserFileOps;
use crate::legacy_configs::file_ops::ConfigPath;
use crate::legacy_configs::file_ops::DefaultConfigParserFileOps;
use crate::legacy_configs::file_ops::DiceConfigFileOps;
use crate::legacy_configs::file_ops::push_all_files_from_a_directory;
use crate::legacy_configs::key::BuckconfigKeyRef;
use crate::legacy_configs::parser::LegacyConfigParser;
use crate::legacy_configs::path::DEFAULT_EXTERNAL_CONFIG_SOURCES;
use crate::legacy_configs::path::DEFAULT_PROJECT_CONFIG_SOURCES;
use crate::legacy_configs::path::DOT_BUCKCONFIG_LOCAL;
use crate::legacy_configs::path::ExternalConfigSource;
use crate::legacy_configs::path::ProjectConfigSource;

/// Ensure a symlink exists from `link` to `target`. Modeled after Bazel's
/// [`FileSystemUtils.ensureSymbolicLink`](https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/vfs/FileSystemUtils.java).
///
/// - If symlink already points to target: no-op
/// - If symlink points elsewhere: replace it
/// - If non-symlink exists: return error
fn ensure_symlink(link: &Path, target: &Path) -> std::io::Result<()> {
    // Check if symlink already exists and points to the correct target
    if let Ok(existing) = std::fs::read_link(link) {
        if existing == target {
            return Ok(());
        }
        // Stale symlink pointing to wrong target - remove it
        if cfg!(windows) {
            // On Windows, symlinks to directories need remove_dir
            let _ = std::fs::remove_dir(link);
            let _ = std::fs::remove_file(link);
        } else {
            std::fs::remove_file(link)?;
        }
    } else if link.exists() {
        // Path exists but is not a symlink (real directory) - don't touch it
        tracing::warn!(
            "bazel-external/{} is a real directory, not a symlink - skipping",
            link.file_name().unwrap_or_default().to_string_lossy()
        );
        return Ok(());
    }

    // Create parent directories
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create symlink (platform-specific)
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link);

    #[cfg(windows)]
    {
        // Try symlink first (requires Developer Mode or admin privileges)
        match std::os::windows::fs::symlink_dir(target, link) {
            Ok(()) => return Ok(()),
            Err(symlink_err) => {
                // Fall back to junction point (no special privileges needed)
                let output = std::process::Command::new("cmd")
                    .args(["/c", "mklink", "/j"])
                    .arg(link)
                    .arg(target)
                    .output();
                match output {
                    Ok(o) if o.status.success() => return Ok(()),
                    _ => return Err(symlink_err),
                }
            }
        }
    }
}

/// Remove stale symlinks from bazel-external/ that don't correspond to any resolved module.
/// This handles the case where a module is removed from MODULE.bazel or its version changes.
fn cleanup_stale_symlinks(
    external_base_dir: &Path,
    valid_entries: &std::collections::HashSet<String>,
) {
    if !external_base_dir.exists() {
        return;
    }

    let entries = match std::fs::read_dir(external_base_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!("Could not read bazel-external/ for cleanup: {}", e);
            return;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_entries.contains(&name) {
            let path = entry.path();
            // Only remove symlinks/junctions, not real directories
            if path.is_symlink() || (cfg!(windows) && is_junction(&path)) {
                if let Err(e) = if cfg!(windows) {
                    std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path))
                } else {
                    std::fs::remove_file(&path)
                } {
                    tracing::debug!(
                        "Could not remove stale symlink bazel-external/{}: {}",
                        name,
                        e
                    );
                } else {
                    tracing::info!("Removed stale symlink: bazel-external/{}", name);
                }
            }
        }
    }
}

/// Check if a path is a Windows junction point.
#[cfg(windows)]
fn is_junction(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn is_junction(_path: &Path) -> bool {
    false
}

/// Buckconfigs can partially be loaded from within dice. However, some parts of what makes up the
/// buckconfig comes from outside the buildgraph, and this type represents those parts.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct ExternalBuckconfigData {
    // The result of parsing the buckconfigs coming from either global (e.g. /etc/buckconfig.d) or
    // user (e.g. ~/.buckconfig.d or $home_dir/.buckconfig.local) files/dirs outside of the repo
    // The order matters here and reflects the same order these are processed in buck, see
    // https://fburl.com/code/8ue78p1j
    external_path_configs: Vec<ExternalPathBuckconfigData>,
    // The result of parsing the buckconfigs coming from command line args (e.g. --config or --config-file)
    args: Vec<ResolvedLegacyConfigArg>,
}

#[derive(PartialEq, Eq, Allocative, Clone)]
pub struct ExternalPathBuckconfigData {
    pub(crate) parse_state: LegacyConfigParser,
    pub(crate) origin_path: ConfigPath,
}

impl ExternalBuckconfigData {
    pub fn testing_default() -> Self {
        Self {
            external_path_configs: Vec::new(),
            args: Vec::new(),
        }
    }

    pub fn filter_values<F>(self, filter: F) -> Self
    where
        F: Fn(&BuckconfigKeyRef) -> bool,
    {
        Self {
            external_path_configs: self
                .external_path_configs
                .into_iter()
                .map(|o| ExternalPathBuckconfigData {
                    parse_state: o.parse_state.filter_values(&filter),
                    origin_path: o.origin_path,
                })
                .collect(),
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

    async fn get_local_config_components(
        project_root: &ProjectRoot,
    ) -> Vec<kuro_data::BuckconfigComponent> {
        use kuro_data::buckconfig_component::Data::GlobalExternalConfigFile;
        let file_ops = &mut DefaultConfigParserFileOps {
            project_fs: project_root.dupe(),
        };
        let mut local_config_components = Vec::new();
        if let Ok(legacy_cells) =
            BuckConfigBasedCells::parse_with_config_args(&project_root, &[]).await
        {
            let path = ForwardRelativePath::new(DOT_BUCKCONFIG_LOCAL).expect(
                "Internal error: .buckconfig.local should always be a valid forward relative path",
            );
            for (_cell, cell_instance) in legacy_cells.cell_resolver.cells() {
                let relative_path = cell_instance.path().as_project_relative_path().join(path);
                let origin_path = relative_path.to_string();
                let local_config = ConfigPath::Project(relative_path);

                let mut parser = LegacyConfigParser::new();
                if parser
                    .parse_file(&local_config, None, true, file_ops)
                    .await
                    .is_ok()
                {
                    let values = parser.to_proto_external_config_values(false);
                    if values.is_empty() {
                        // Don't create an empty component for cells with non-existing .buckconfig.local
                        continue;
                    }
                    local_config_components.push(kuro_data::BuckconfigComponent {
                        data: Some(GlobalExternalConfigFile(kuro_data::GlobalExternalConfig {
                            values,
                            origin_path,
                        })),
                    });
                }
            }
        }
        local_config_components
    }

    pub async fn get_buckconfig_components(
        &self,
        project_root: &ProjectRoot,
    ) -> Vec<kuro_data::BuckconfigComponent> {
        use kuro_data::buckconfig_component::Data::GlobalExternalConfigFile;
        let mut res: Vec<kuro_data::BuckconfigComponent> = self
            .external_path_configs
            .clone()
            .into_iter()
            .map(|o| {
                let external_file = kuro_data::GlobalExternalConfig {
                    values: o.parse_state.to_proto_external_config_values(false),
                    origin_path: o.origin_path.to_string(),
                };
                kuro_data::BuckconfigComponent {
                    data: Some(GlobalExternalConfigFile(external_file)),
                }
            })
            .collect();

        res.extend(Self::get_local_config_components(project_root).await);
        res.extend(to_proto_config_args(&self.args));
        res
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
    pub config_paths: HashSet<ConfigPath>,
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
        project_fs: &ProjectRoot,
        cwd: &ProjectRelativePath,
    ) -> kuro_error::Result<CellAliasResolver> {
        self.get_cell_alias_resolver_for_cwd_fast_with_file_ops(
            &mut DefaultConfigParserFileOps {
                project_fs: project_fs.dupe(),
            },
            cwd,
        )
        .await
    }

    pub(crate) async fn get_cell_alias_resolver_for_cwd_fast_with_file_ops(
        &self,
        file_ops: &mut dyn ConfigParserFileOps,
        cwd: &ProjectRelativePath,
    ) -> kuro_error::Result<CellAliasResolver> {
        let cell_name = self.cell_resolver.find(cwd);
        let cell_path = self.cell_resolver.get(cell_name)?.path();

        let follow_includes = false;

        let config_paths = get_project_buckconfig_paths(cell_path, file_ops).await?;
        let config = LegacyBuckConfig::finish_parse(
            self.external_data.external_path_configs.clone(),
            &config_paths,
            cell_path,
            file_ops,
            &[],
            follow_includes,
        )
        .await?;

        // In bzlmod mode, all cell aliases come from MODULE.bazel.
        // Per-cell .buckconfig [repository_aliases] are irrelevant and must be skipped,
        // since they may reference cell names (like "root") that don't exist in bzlmod.
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
        Self::parse_with_file_ops_and_options(
            &mut DefaultConfigParserFileOps {
                project_fs: project_fs.dupe(),
            },
            config_args,
            false, /* follow includes */
            Some(project_fs),
        )
        .await
    }

    pub async fn testing_parse_with_file_ops(
        file_ops: &mut dyn ConfigParserFileOps,
        config_args: &[kuro_cli_proto::ConfigOverride],
    ) -> kuro_error::Result<Self> {
        Self::parse_with_file_ops_and_options(
            file_ops,
            config_args,
            true, /* follow includes */
            None, /* project_fs for bzlmod */
        )
        .await
    }

    async fn parse_with_file_ops_and_options(
        file_ops: &mut dyn ConfigParserFileOps,
        config_args: &[kuro_cli_proto::ConfigOverride],
        follow_includes: bool,
        project_fs: Option<&ProjectRoot>,
    ) -> kuro_error::Result<Self> {
        Self::parse_with_file_ops_and_options_inner(
            file_ops,
            config_args,
            follow_includes,
            project_fs,
        )
        .await
        .buck_error_context("Parsing cells")
    }

    async fn parse_with_file_ops_and_options_inner(
        file_ops: &mut dyn ConfigParserFileOps,
        config_args: &[kuro_cli_proto::ConfigOverride],
        follow_includes: bool,
        project_fs: Option<&ProjectRoot>,
    ) -> kuro_error::Result<Self> {
        // Tracing file ops to record config file accesses on command invocation.
        struct TracingFileOps<'a> {
            inner: &'a mut dyn ConfigParserFileOps,
            trace: HashSet<ConfigPath>,
        }

        #[async_trait::async_trait]
        impl ConfigParserFileOps for TracingFileOps<'_> {
            async fn read_file_lines_if_exists(
                &mut self,
                path: &ConfigPath,
            ) -> kuro_error::Result<Option<Vec<String>>> {
                let res = self.inner.read_file_lines_if_exists(path).await?;

                if res.is_some() {
                    self.trace.insert(path.clone());
                }

                Ok(res)
            }

            async fn read_dir(
                &mut self,
                path: &ConfigPath,
            ) -> kuro_error::Result<Vec<ConfigDirEntry>> {
                self.inner.read_dir(path).await
            }
        }

        let mut file_ops = TracingFileOps {
            inner: file_ops,
            trace: Default::default(),
        };

        // NOTE: This will _not_ perform IO unless it needs to.
        let processed_config_args = resolve_config_args(&config_args, &mut file_ops).await?;

        let external_paths = get_external_buckconfig_paths(&mut file_ops).await?;
        let started_parse = LegacyBuckConfig::start_parse_for_external_files(
            &external_paths,
            &mut file_ops,
            follow_includes,
        )
        .await?;

        let root_path = CellRootPathBuf::new(ProjectRelativePath::empty().to_owned());

        let buckconfig_paths = get_project_buckconfig_paths(&root_path, &mut file_ops).await?;

        let root_config = LegacyBuckConfig::finish_parse(
            started_parse.clone(),
            buckconfig_paths.as_slice(),
            &root_path,
            &mut file_ops,
            &processed_config_args,
            follow_includes,
        )
        .await?;

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
            config_paths: file_ops.trace,
            external_data: ExternalBuckconfigData {
                external_path_configs: started_parse,
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

            match ModuleCache::new() {
                Ok(cache) => match MvsResolver::new(cache).await {
                    Ok(mut resolver) => {
                        match resolver.resolve(&parsed.module, workspace_root).await {
                            Ok(mut resolved_graph) => {
                                tracing::info!(
                                    "MVS resolved {} total modules (including transitive)",
                                    resolved_graph.modules.len()
                                );

                                // Fetch sources for all resolved modules (downloads and extracts)
                                if let Err(e) = resolver.fetch_sources(&mut resolved_graph).await {
                                    tracing::warn!("Failed to fetch some module sources: {}", e);
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
                                let external_base_dir =
                                    project_root.root().as_path().join("bazel-external");
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
                                    if module_name == &parsed.module.name
                                        || local_override_names.contains(module_name)
                                    {
                                        continue;
                                    }

                                    // Only create symlinks for modules with cached source paths
                                    if let Some(source_path) = &module_info.source_path {
                                        let entry_name =
                                            format!("{}+{}", module_name, module_info.version);
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
                                        let buck_out_link =
                                            buck_out_external_cells_dir.join(&entry_name);
                                        if let Err(e) = ensure_symlink(&buck_out_link, source_path)
                                        {
                                            tracing::warn!(
                                                "Failed to create external_cells symlink for \
                                                 {}@{}: {}",
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
                                cleanup_stale_symlinks(
                                    &buck_out_external_cells_dir,
                                    &valid_symlink_names,
                                );

                                // Register ALL resolved modules as cells
                                for (module_name, module_info) in &resolved_graph.modules {
                                    // Skip the root module and local overrides
                                    if module_name == &parsed.module.name
                                        || local_override_names.contains(module_name)
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
                                            let external_path = format!(
                                                "bazel-external/{}+{}",
                                                module_name, module_info.version
                                            );
                                            let cell_path = CellRootPathBuf::new(
                                                ProjectRelativePath::new(&external_path)?
                                                    .to_owned(),
                                            );

                                            tracing::info!(
                                                "Registered module: {}@{} -> {} (external path: {})",
                                                module_name,
                                                module_info.version,
                                                source_path_str,
                                                external_path
                                            );

                                            let setup =
                                                kuro_core::cells::external::BzlmodCellSetup {
                                                    module_name: Arc::from(module_name.as_str()),
                                                    version: Arc::from(
                                                        module_info.version.as_str(),
                                                    ),
                                                    registry_url: Arc::from(url.as_str()),
                                                    source_path: Arc::from(
                                                        source_path_str.as_str(),
                                                    ),
                                                };

                                            cells.push((cell_name, cell_path, Some(setup)));
                                        }
                                        ModuleSource::LocalPath { path } => {
                                            // Local path modules from overrides are handled separately
                                            let cell_path = CellRootPathBuf::new(
                                                ProjectRelativePath::new(path)?.to_owned(),
                                            );
                                            cells.push((cell_name, cell_path, None));
                                            tracing::info!(
                                                "Registered local module: {} -> {}",
                                                module_name,
                                                path
                                            );
                                        }
                                        ModuleSource::Git { remote, commit, .. } => {
                                            let source_path_str = module_info
                                                .source_path
                                                .as_ref()
                                                .map(|p| p.to_string_lossy().to_string())
                                                .unwrap_or_default();

                                            let external_path = format!(
                                                "bazel-external/{}+{}",
                                                module_name, module_info.version
                                            );
                                            let cell_path = CellRootPathBuf::new(
                                                ProjectRelativePath::new(&external_path)?
                                                    .to_owned(),
                                            );

                                            // Git modules use Bzlmod setup with empty registry URL
                                            let setup =
                                                kuro_core::cells::external::BzlmodCellSetup {
                                                    module_name: Arc::from(module_name.as_str()),
                                                    version: Arc::from(
                                                        module_info.version.as_str(),
                                                    ),
                                                    registry_url: Arc::from(
                                                        format!("git+{}", remote).as_str(),
                                                    ),
                                                    source_path: Arc::from(
                                                        source_path_str.as_str(),
                                                    ),
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

                                            let external_path = format!(
                                                "bazel-external/{}+{}",
                                                module_name, module_info.version
                                            );
                                            let cell_path = CellRootPathBuf::new(
                                                ProjectRelativePath::new(&external_path)?
                                                    .to_owned(),
                                            );

                                            // Use first URL as the registry URL
                                            let url = urls
                                                .first()
                                                .map(|u| u.as_str())
                                                .unwrap_or("archive");
                                            let setup =
                                                kuro_core::cells::external::BzlmodCellSetup {
                                                    module_name: Arc::from(module_name.as_str()),
                                                    version: Arc::from(
                                                        module_info.version.as_str(),
                                                    ),
                                                    registry_url: Arc::from(url),
                                                    source_path: Arc::from(
                                                        source_path_str.as_str(),
                                                    ),
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

                                // Handle repo_name aliases from root module's direct deps
                                // Transitive repo_name aliasing requires parsing each module's MODULE.bazel
                                // which we defer to a future enhancement
                                for dep in &parsed.module.bazel_deps {
                                    if let Some(repo_name) = &dep.repo_name {
                                        if repo_name != &dep.name {
                                            let cell_name = CellName::unchecked_new(&dep.name)?;
                                            let alias_name =
                                                NonEmptyCellAlias::new(repo_name.clone())?;
                                            tracing::info!(
                                                "Creating repo_name alias: {} -> {}",
                                                repo_name,
                                                dep.name
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
                            Err(e) => {
                                tracing::warn!("MVS resolution failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create MVS resolver: {}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to create module cache: {}", e);
                }
            }
        }

        // Collect extension usages from all resolved modules and generate synthetic repos
        let synthetic_cells =
            Self::generate_synthetic_extension_repos(project_root, &parsed, &cells).await?;
        cells.extend(synthetic_cells);

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

        // Pre-compute extension repo cells from use_repo() declarations alone.
        // This is the Bazel 9.0-compatible approach: canonical names are deterministic
        // from MODULE.bazel topology, no extension execution or lockfile needed.
        let root_module_name = if parsed.module.name.is_empty() {
            "_main"
        } else {
            &parsed.module.name
        };
        let (pre_computed_cells, pre_computed_aliases) =
            kuro_bzlmod::pre_compute_extension_repo_cells(&parsed_modules, root_module_name)?;

        // Aggregate extension usages from all modules and store globally.
        // This data is needed by DICE when extension repos are lazily executed.
        let mut module_extensions: std::collections::HashMap<
            String,
            Vec<kuro_bzlmod::types::ExtensionUsage>,
        > = std::collections::HashMap::new();
        for (module_name, parsed_mod) in &parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated = kuro_bzlmod::aggregate_extensions(&module_extensions);
        kuro_bzlmod::set_extension_aggregations(
            aggregated,
            root_module_name.to_owned(),
            project_root.root().to_path_buf(),
        );

        // Convert pre-computed cells to the format expected by BzlmodResolutionResult
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

        // Convert pre-computed aliases, skipping those that conflict with existing cells.
        // This happens when synthetic repos create cells with bare names (e.g., "cui__camino-1.1.6")
        // that would conflict with an alias of the same name from use_repo() declarations.
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
            ext_aliases.push((apparent_name, canonical_name));
        }

        // Add extension aliases to the main aliases list
        aliases.extend(ext_aliases);

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

    /// Generate synthetic repos for known module extensions.
    ///
    /// This function:
    /// 1. Collects MODULE.bazel from all resolved dependencies
    /// 2. Extracts extension usages (use_extension + use_repo)
    /// 3. Generates synthetic repos for known extensions (e.g., bazel_features)
    /// 4. Materializes them to bazel-external/
    async fn generate_synthetic_extension_repos(
        project_root: &ProjectRoot,
        root_parsed: &kuro_bzlmod::types::ParsedModuleFile,
        resolved_cells: &[(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)],
    ) -> kuro_error::Result<Vec<(CellName, CellRootPathBuf, Option<BzlmodCellSetup>)>> {
        let mut parsed_modules: Vec<(String, ParsedModuleFile)> = Vec::new();

        // Add root module
        parsed_modules.push((root_parsed.module.name.clone(), root_parsed.clone()));

        // Parse MODULE.bazel from each resolved dependency
        for (cell_name, _cell_path, setup) in resolved_cells {
            if let Some(bzlmod_setup) = setup {
                // Read MODULE.bazel from the cached source
                let module_bazel_path = std::path::PathBuf::from(bzlmod_setup.source_path.as_ref())
                    .join("MODULE.bazel");
                if module_bazel_path.exists() {
                    match parse_module_bazel(&module_bazel_path) {
                        Ok(dep_parsed) => {
                            let module_key = if dep_parsed.module.name.is_empty() {
                                cell_name.as_str().to_string()
                            } else {
                                dep_parsed.module.name.clone()
                            };
                            parsed_modules.push((module_key, dep_parsed));
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Failed to parse MODULE.bazel for {}: {}",
                                cell_name.as_str(),
                                e
                            );
                        }
                    }
                }
            }
        }

        // Collect synthetic repos from all extension usages
        let synthetic_repos =
            collect_synthetic_repos_with_root(&parsed_modules, Some(project_root.root().as_path()));
        if synthetic_repos.is_empty() {
            return Ok(Vec::new());
        }

        tracing::info!(
            "Generating {} synthetic extension repos",
            synthetic_repos.len()
        );

        // Materialize synthetic repos to bazel-external/
        let synthetic_base_dir = project_root.root().as_path().join("bazel-external");
        match materialize_synthetic_repos(&synthetic_repos, &synthetic_base_dir) {
            Ok(paths) => {
                let mut cells = Vec::new();
                for (repo, _path) in synthetic_repos.iter().zip(paths.iter()) {
                    let cell_name = CellName::unchecked_new(&repo.name)?;
                    let external_path = format!("bazel-external/{}", repo.name);
                    let cell_path =
                        CellRootPathBuf::new(ProjectRelativePath::new(&external_path)?.to_owned());

                    tracing::info!(
                        "Registered synthetic repo: {} -> {}",
                        repo.name,
                        external_path
                    );

                    // Synthetic repos don't need BzlmodCellSetup - they're local
                    cells.push((cell_name, cell_path, None));
                }
                Ok(cells)
            }
            Err(e) => {
                tracing::warn!("Failed to materialize synthetic repos: {}", e);
                Ok(Vec::new())
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

    /// Register extension-generated repository cells with a CellsAggregator.
    ///
    /// This is called after module extension execution to register the repositories
    /// created by extensions (e.g., from pip.parse(), go_deps, etc.) as cells.
    ///
    /// Extension repos are registered as "pending" cells - they aren't materialized
    /// until first accessed, at which point DICE triggers lazy execution via
    /// `ExtensionRepoExecutionKey`.
    ///
    /// # Arguments
    /// * `aggregator` - The CellsAggregator to add cells to
    /// * `pending_cells` - Pending repo cell definitions from `build_extension_cells()`
    /// * `aliases` - Repo aliases from `build_use_repo_aliases()`
    ///
    /// # Example
    /// ```ignore
    /// // After extension execution:
    /// let ext_result = dice.compute(&ModuleExtensionExecutionKey::new(...)).await?;
    /// let use_repos = extract_use_repos_for_extension(&ext_id, &extension_usages);
    /// let ext_defs = build_extension_cell_definitions(&ext_result, &use_repos)?;
    /// register_extension_cells(&mut aggregator, ext_defs.cells, ext_defs.aliases)?;
    /// ```
    #[allow(dead_code)] // Infrastructure for future DICE-based extension execution
    pub(crate) fn register_extension_cells(
        aggregator: &mut CellsAggregator,
        pending_cells: &[kuro_bzlmod::PendingRepoCell],
        aliases: &[kuro_bzlmod::RepoAlias],
    ) -> kuro_error::Result<()> {
        // Register each extension repo cell
        for cell in pending_cells {
            let cell_name = CellName::unchecked_new(&cell.canonical_name)?;

            // Create ExtensionRepoCellSetup from PendingRepoCell
            let setup = ExtensionRepoCellSetup {
                canonical_name: Arc::from(cell.canonical_name.as_str()),
                extension_id: Arc::from(cell.extension_id.as_str()),
                internal_name: Arc::from(cell.internal_name.as_str()),
                spec_hash: Arc::from(cell.spec_hash.as_str()),
                repo_spec_json: Arc::from(cell.repo_spec_json.as_str()),
                materialized: false,
            };

            tracing::info!(
                "Registering extension repo cell: {} -> {} (pending: true)",
                cell.canonical_name,
                cell.path,
            );

            // Mark as external cell with ExtensionRepo origin
            // Note: The cell must already be in the aggregator's cell_infos for mark_external_cell to work.
            // For dynamic registration, we would need to extend CellsAggregator to add new cells.
            // For now, this function documents the expected pattern for future DICE integration.
            if let Err(e) =
                aggregator.mark_external_cell(cell_name, ExternalCellOrigin::ExtensionRepo(setup))
            {
                tracing::debug!(
                    "Could not mark extension repo '{}' as external: {} (may not be pre-registered)",
                    cell.canonical_name,
                    e
                );
            }
        }

        // Note: Aliases are typically added during CellsAggregator construction.
        // For dynamic extension repos, the aliases would need to be added to
        // the root_aliases map before make_cell_resolver() is called.
        for alias in aliases {
            tracing::info!(
                "Extension repo alias: {} -> {}",
                alias.apparent_name,
                alias.canonical_name
            );
        }

        Ok(())
    }

    pub(crate) async fn parse_single_cell_with_dice(
        ctx: &mut DiceComputations<'_>,
        cell_path: &CellRootPath,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        let resolver = ctx.get_cell_resolver().await?;
        let io_provider = ctx.global_data().get_io_provider();
        let project_fs = io_provider.project_root();
        let external_data = ctx.get_injected_external_buckconfig_data().await?;

        let mut file_ops = DiceConfigFileOps::new(ctx, project_fs, &resolver);

        Self::parse_single_cell_with_file_ops_inner(&external_data, &mut file_ops, cell_path).await
    }

    pub async fn parse_single_cell(
        &self,
        cell: CellName,
        project_fs: &ProjectRoot,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        self.parse_single_cell_with_file_ops(
            cell,
            &mut DefaultConfigParserFileOps {
                project_fs: project_fs.dupe(),
            },
        )
        .await
    }

    pub(crate) async fn parse_single_cell_with_file_ops(
        &self,
        cell: CellName,
        file_ops: &mut dyn ConfigParserFileOps,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        Self::parse_single_cell_with_file_ops_inner(
            &self.external_data,
            file_ops,
            self.cell_resolver.get(cell)?.path(),
        )
        .await
    }

    async fn parse_single_cell_with_file_ops_inner(
        external_data: &ExternalBuckconfigData,
        file_ops: &mut dyn ConfigParserFileOps,
        cell_path: &CellRootPath,
    ) -> kuro_error::Result<LegacyBuckConfig> {
        let config_paths = get_project_buckconfig_paths(cell_path, file_ops).await?;
        LegacyBuckConfig::finish_parse(
            external_data.external_path_configs.clone(),
            &config_paths,
            cell_path,
            file_ops,
            external_data.args.as_ref(),
            /* follow includes */ true,
        )
        .await
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

async fn get_external_buckconfig_paths(
    file_ops: &mut dyn ConfigParserFileOps,
) -> kuro_error::Result<Vec<ConfigPath>> {
    let skip_default_external_config = kuro_env!(
        "BUCK2_TEST_SKIP_DEFAULT_EXTERNAL_CONFIG",
        bool,
        applicability = testing
    )?;

    let mut buckconfig_paths: Vec<ConfigPath> = Vec::new();

    if !skip_default_external_config {
        for buckconfig in DEFAULT_EXTERNAL_CONFIG_SOURCES {
            match buckconfig {
                ExternalConfigSource::UserFile(file) => {
                    let home_dir = dirs::home_dir();
                    if let Some(home_dir_path) = home_dir {
                        let buckconfig_path = ForwardRelativePath::new(file)?;
                        buckconfig_paths.push(ConfigPath::Global(
                            AbsPath::new(&home_dir_path)?.join(buckconfig_path.as_str()),
                        ));
                    }
                }
                ExternalConfigSource::UserFolder(folder) => {
                    let home_dir = dirs::home_dir();
                    if let Some(home_dir_path) = home_dir {
                        let buckconfig_path = ForwardRelativePath::new(folder)?;
                        let buckconfig_folder_abs_path =
                            AbsPath::new(&home_dir_path)?.join(buckconfig_path.as_str());
                        push_all_files_from_a_directory(
                            &mut buckconfig_paths,
                            &ConfigPath::Global(buckconfig_folder_abs_path),
                            file_ops,
                        )
                        .await?;
                    }
                }
                ExternalConfigSource::GlobalFile(file) => {
                    buckconfig_paths.push(ConfigPath::Global(AbsPath::new(*file)?.to_owned()));
                }
                ExternalConfigSource::GlobalFolder(folder) => {
                    let buckconfig_folder_abs_path = AbsPath::new(*folder)?.to_owned();
                    push_all_files_from_a_directory(
                        &mut buckconfig_paths,
                        &ConfigPath::Global(buckconfig_folder_abs_path),
                        file_ops,
                    )
                    .await?;
                }
            }
        }
    }

    let extra_external_config =
        kuro_env!("BUCK2_TEST_EXTRA_EXTERNAL_CONFIG", applicability = testing)?;

    if let Some(f) = extra_external_config {
        buckconfig_paths.push(ConfigPath::Global(AbsPath::new(f)?.to_owned()));
    }

    Ok(buckconfig_paths)
}

async fn get_project_buckconfig_paths(
    path: &CellRootPath,
    file_ops: &mut dyn ConfigParserFileOps,
) -> kuro_error::Result<Vec<ConfigPath>> {
    let mut buckconfig_paths: Vec<ConfigPath> = Vec::new();

    for buckconfig in DEFAULT_PROJECT_CONFIG_SOURCES {
        match buckconfig {
            ProjectConfigSource::CellRelativeFile(file) => {
                let buckconfig_path = ForwardRelativePath::new(file)?;
                buckconfig_paths.push(ConfigPath::Project(
                    path.as_project_relative_path().join(buckconfig_path),
                ));
            }
            ProjectConfigSource::CellRelativeFolder(folder) => {
                let buckconfig_folder_path = ForwardRelativePath::new(folder)?;
                let buckconfig_folder_path =
                    path.as_project_relative_path().join(buckconfig_folder_path);
                push_all_files_from_a_directory(
                    &mut buckconfig_paths,
                    &ConfigPath::Project(buckconfig_folder_path),
                    file_ops,
                )
                .await?;
            }
        }
    }

    Ok(buckconfig_paths)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dice::DiceComputations;
    use indoc::indoc;
    use kuro_cli_proto::ConfigOverride;
    use kuro_core::cells::cell_root_path::CellRootPath;
    use kuro_core::cells::cell_root_path::CellRootPathBuf;
    use kuro_core::cells::external::ExternalCellOrigin;
    use kuro_core::cells::external::GitCellSetup;
    use kuro_core::cells::name::CellName;

    use crate::external_cells::EXTERNAL_CELLS_IMPL;
    use crate::external_cells::ExternalCellsImpl;
    use crate::file_ops::delegate::FileOpsDelegate;
    use crate::legacy_configs::cells::BuckConfigBasedCells;
    use crate::legacy_configs::configs::testing::TestConfigParserFileOps;
    use crate::legacy_configs::configs::tests::assert_config_value;
    use crate::legacy_configs::key::BuckconfigKeyRef;

    #[tokio::test]
    async fn test_cells() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                                other_alias = other/
                                third_party = third_party/
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = ..
                                other = .
                                third_party = ../third_party/
                        "#
                ),
            ),
            (
                "third_party/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                third_party = .
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let resolver = &cells.cell_resolver;

        let root_instance = resolver.get(CellName::testing_new("root"))?;
        let other_instance = resolver.get(CellName::testing_new("other"))?;
        let tp_instance = resolver.get(CellName::testing_new("third_party"))?;

        assert_eq!("", root_instance.path().as_str());
        assert_eq!("other", other_instance.path().as_str());
        assert_eq!("third_party", tp_instance.path().as_str());

        assert_eq!(
            "other",
            resolver
                .root_cell_cell_alias_resolver()
                .resolve("other_alias")?
                .as_str()
        );

        let tp_resolver = cells
            .get_cell_alias_resolver_for_cwd_fast_with_file_ops(
                &mut file_ops,
                tp_instance.path().as_project_relative_path(),
            )
            .await?;

        assert_eq!("other", tp_resolver.resolve("other_alias")?.as_str());

        Ok(())
    }

    #[tokio::test]
    async fn test_multi_cell_with_config_file() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                                other_alias = other/
                                third_party = third_party/
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = ..
                                other = .
                                third_party = ../third_party/
                            [buildfile]
                                name = TARGETS
                        "#
                ),
            ),
            (
                "third_party/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                third_party = .
                            [buildfile]
                                name_v2 = OKAY
                                name = OKAY_v1
                        "#
                ),
            ),
            (
                "other/cli-conf",
                indoc!(
                    r#"
                            [foo]
                                bar = blah
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(
            &mut file_ops,
            &[ConfigOverride::file(
                "cli-conf",
                Some(CellRootPathBuf::testing_new("other")),
            )],
        )
        .await?;

        let root_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        let other_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;
        let tp_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("third_party"), &mut file_ops)
            .await?;

        assert_eq!(
            root_config.get(BuckconfigKeyRef {
                section: "foo",
                property: "bar"
            }),
            Some("blah")
        );
        assert_eq!(
            other_config.get(BuckconfigKeyRef {
                section: "foo",
                property: "bar"
            }),
            Some("blah")
        );
        assert_eq!(
            tp_config.get(BuckconfigKeyRef {
                section: "foo",
                property: "bar"
            }),
            Some("blah")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_multi_cell_no_repositories_in_non_root_cell() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [foo]
                                bar = baz
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let other_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;

        assert_eq!(
            other_config.get(BuckconfigKeyRef {
                section: "foo",
                property: "bar"
            }),
            Some("baz")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_multi_cell_with_cell_relative() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                        "#
                ),
            ),
            (
                "global-conf",
                indoc!(
                    r#"
                            [apple]
                                test_tool = xctool
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = ..
                                other = .
                            [buildfile]
                                name = TARGETS
                        "#
                ),
            ),
            (
                "other/app-conf",
                indoc!(
                    r#"
                            [apple]
                                ide = Xcode
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(
            &mut file_ops,
            &[
                ConfigOverride::file("app-conf", Some(CellRootPathBuf::testing_new("other"))),
                ConfigOverride::file("global-conf", Some(CellRootPathBuf::testing_new(""))),
            ],
        )
        .await?;

        let other_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;

        assert_eq!(
            other_config.get(BuckconfigKeyRef {
                section: "apple",
                property: "ide"
            }),
            Some("Xcode")
        );
        assert_eq!(
            other_config.get(BuckconfigKeyRef {
                section: "apple",
                property: "test_tool"
            }),
            Some("xctool")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_local_config_file_overwrite_config_file() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                            [apple]
                                key = value1
                                key2 = value2
                        "#
                ),
            ),
            (
                ".buckconfig.local",
                indoc!(
                    r#"
                            [orange]
                                key = value3
                            [apple]
                                key2 = value5
                                key3 = value4
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        // No local override
        assert_config_value(&config, "apple", "key", "value1");
        // local override to new value
        assert_config_value(&config, "apple", "key2", "value5");
        // local override new field
        assert_config_value(&config, "apple", "key3", "value4");
        // local override new section
        assert_config_value(&config, "orange", "key", "value3");

        Ok(())
    }

    #[tokio::test]
    async fn test_multi_cell_local_config_file_overwrite_config_file() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                            [apple]
                                key = value1
                                key2 = value2
                        "#
                ),
            ),
            (
                ".buckconfig.local",
                indoc!(
                    r#"
                            [orange]
                                key = value3
                            [apple]
                                key2 = value5
                                key3 = value4
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = ..
                                other = .
                            [apple]
                                key = othervalue1
                                key2 = othervalue2
                        "#
                ),
            ),
            (
                "other/.buckconfig.local",
                indoc!(
                    r#"
                            [orange]
                                key = othervalue3
                            [apple]
                                key2 = othervalue5
                                key3 = othervalue4
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let root_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        let other_config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;

        // No local override
        assert_config_value(&root_config, "apple", "key", "value1");
        // local override to new value
        assert_config_value(&root_config, "apple", "key2", "value5");
        // local override new field
        assert_config_value(&root_config, "apple", "key3", "value4");
        // local override new section
        assert_config_value(&root_config, "orange", "key", "value3");

        // No local override
        assert_config_value(&other_config, "apple", "key", "othervalue1");
        // local override to new value
        assert_config_value(&other_config, "apple", "key2", "othervalue5");
        // local override new field
        assert_config_value(&other_config, "apple", "key3", "othervalue4");
        // local override new section
        assert_config_value(&other_config, "orange", "key", "othervalue3");

        Ok(())
    }

    #[tokio::test]
    async fn test_config_arg_with_no_buckconfig() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                        [repositories]
                            root = .
                            other = other
                    "#
            ),
        )])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(
            &mut file_ops,
            &[ConfigOverride::flag_no_cell("some_section.key=value1")],
        )
        .await?;
        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;

        assert_config_value(&config, "some_section", "key", "value1");

        Ok(())
    }

    #[tokio::test]
    async fn test_cell_config_section_name() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                            [repositories]
                                root = .
                                other = other/
                            [repository_aliases]
                                other_alias = other
                        "#
            ),
        )])?;

        let resolver = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await?
            .cell_resolver;

        assert_eq!(
            "other",
            resolver
                .root_cell_cell_alias_resolver()
                .resolve("other_alias")?
                .as_str(),
        );

        Ok(())
    }

    fn initialize_external_cells_impl() {
        struct TestExternalCellsImpl;

        #[async_trait::async_trait]
        impl ExternalCellsImpl for TestExternalCellsImpl {
            async fn get_file_ops_delegate(
                &self,
                _ctx: &mut DiceComputations<'_>,
                _cell_name: CellName,
                _origin: ExternalCellOrigin,
            ) -> kuro_error::Result<Arc<dyn FileOpsDelegate>> {
                // Not used in these tests
                unreachable!()
            }

            fn check_bundled_cell_exists(&self, cell_name: CellName) -> kuro_error::Result<()> {
                if cell_name.as_str() == "test_bundled_cell" {
                    Ok(())
                } else {
                    Err(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "No bundled cell with name `{}`",
                        cell_name
                    ))
                }
            }

            async fn expand(
                &self,
                _ctx: &mut DiceComputations<'_>,
                _cell_name: CellName,
                _origin: ExternalCellOrigin,
                _path: &CellRootPath,
            ) -> kuro_error::Result<()> {
                // Not used in these tests
                unreachable!()
            }
        }

        static INIT: std::sync::Once = std::sync::Once::new();

        // Sometimes multiple unittests are run in the same process
        INIT.call_once(|| {
            EXTERNAL_CELLS_IMPL.init(&TestExternalCellsImpl);
        });
    }

    #[tokio::test]
    async fn test_external_cell_configs() -> kuro_error::Result<()> {
        initialize_external_cells_impl();

        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                    [cells]
                        root = .
                        test_bundled_cell = other1/
                        other2 = other2/
                    [cell_aliases]
                        other_alias = test_bundled_cell
                    [external_cells]
                        other_alias = bundled
                "#
            ),
        )])?;

        let resolver = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await?
            .cell_resolver;

        let other1 = resolver
            .root_cell_cell_alias_resolver()
            .resolve("other_alias")
            .unwrap();
        let other2 = resolver
            .root_cell_cell_alias_resolver()
            .resolve("other2")
            .unwrap();

        assert_eq!(
            resolver.get(other1).unwrap().external(),
            Some(&ExternalCellOrigin::Bundled(CellName::testing_new(
                "test_bundled_cell"
            ))),
        );
        assert_eq!(resolver.get(other2).unwrap().external(), None,);
        assert_eq!(
            resolver
                .root_cell_cell_alias_resolver()
                .resolve("other_alias")
                .unwrap()
                .as_str(),
            "test_bundled_cell",
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_nested_external_cell_configs() -> kuro_error::Result<()> {
        initialize_external_cells_impl();

        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                    [cells]
                        root = .
                        test_bundled_cell = foo/
                        bar = foo/bar/
                    [external_cells]
                        test_bundled_cell = bundled
                "#
            ),
        )])?;

        BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await
            .err()
            .unwrap();

        Ok(())
    }

    #[tokio::test]
    async fn test_missing_bundled_cell() -> kuro_error::Result<()> {
        initialize_external_cells_impl();

        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                    [cells]
                        root = .
                        foo = foo/
                        bar = foo/bar/
                    [external_cells]
                        foo = bundled
                "#
            ),
        )])?;

        let e = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await
            .err()
            .unwrap();

        let e = format!("{e:?}");
        assert!(e.contains("No bundled cell"), "error: {e}");

        Ok(())
    }

    #[tokio::test]
    async fn test_git_external_cell() -> kuro_error::Result<()> {
        initialize_external_cells_impl();

        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                    [cells]
                        root = .
                        libfoo = foo/
                    [external_cells]
                        libfoo = git
                    [external_cell_libfoo]
                        git_origin = https://github.com/jeff/libfoo.git
                        commit_hash = aaaaaaaabbbbbbbbccccccccddddddddeeeeeeee
                "#
            ),
        )])?;

        let resolver = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await?
            .cell_resolver;

        let instance = resolver.get(CellName::testing_new("libfoo")).unwrap();

        assert_eq!(
            instance.external(),
            Some(&ExternalCellOrigin::Git(GitCellSetup {
                git_origin: "https://github.com/jeff/libfoo.git".into(),
                commit: "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeee".into(),
                object_format: None,
            })),
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_git_external_cell_invalid_sha1() -> kuro_error::Result<()> {
        initialize_external_cells_impl();

        let mut file_ops = TestConfigParserFileOps::new(&[(
            ".buckconfig",
            indoc!(
                r#"
                    [cells]
                        root = .
                        libfoo = foo/
                    [external_cells]
                        libfoo = git
                    [external_cell_libfoo]
                        git_origin = https://github.com/jeff/libfoo.git
                        commit_hash = abcde
                "#
            ),
        )])?;

        let e = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[])
            .await
            .err()
            .unwrap();

        let e = format!("{e:?}");
        assert!(e.contains("not a valid SHA1 digest"), "error: {e}");

        Ok(())
    }
}
