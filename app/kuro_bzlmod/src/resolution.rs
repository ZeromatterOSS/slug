/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Local module resolution for bzlmod.
//!
//! This module handles resolving `local_path_override()` directives from MODULE.bazel
//! to actual filesystem paths and parsing the local module's MODULE.bazel file.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use kuro_error::BuckErrorContext;
use serde::Deserialize;
use serde::Serialize;

use crate::cache::ModuleCache;
use crate::fetch::SourceFetcher;
use crate::parser::parse_module_bazel;
use crate::parser::parse_module_bazel_content;
use crate::registry::RegistryClient;
use crate::types::BazelDep;
use crate::types::LocalPathOverride;
use crate::types::Module;
use crate::types::MultipleVersionOverride;
use crate::types::Override;
use crate::types::SingleVersionOverride;
use crate::version::Version;

/// Errors that can occur during module resolution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum LocalResolutionError {
    #[error("Local module path does not exist: {0}")]
    PathNotFound(String),

    #[error("Local module is missing MODULE.bazel: {0}")]
    MissingModuleBazel(String),

    #[error("Failed to resolve local module '{module_name}': {reason}")]
    ResolutionFailed { module_name: String, reason: String },

    #[error("Circular dependency detected in local modules: {0}")]
    CircularDependency(String),

    #[error("Local path override references unknown module: {0}")]
    UnknownModule(String),
}

/// Errors that can occur during remote module resolution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum RemoteResolutionError {
    #[error("Failed to fetch module '{name}@{version}' from registry")]
    FetchFailed { name: String, version: String },

    #[error("Module '{name}@{version}' not found in registry")]
    ModuleNotFound { name: String, version: String },

    #[error("Failed to extract source for '{name}@{version}': {reason}")]
    ExtractionFailed {
        name: String,
        version: String,
        reason: String,
    },
}

/// Errors that can occur during MVS resolution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum MvsResolutionError {
    #[error(
        "Compatibility level conflict for module '{name}': \
        version {version1} has compatibility_level={compat1}, \
        version {version2} has compatibility_level={compat2}. \
        Use multiple_version_override to allow both versions."
    )]
    CompatibilityConflict {
        name: String,
        version1: String,
        compat1: u32,
        version2: String,
        compat2: u32,
    },

    #[error(
        "Yanked version selected: {name}@{version}. Reason: {reason}. \
        Use --allow_yanked_versions={name}@{version} to override."
    )]
    YankedVersionSelected {
        name: String,
        version: String,
        reason: String,
    },

    #[error(
        "Version {version} of module '{name}' not in allowed versions list. \
        Allowed versions: {allowed:?}"
    )]
    VersionNotAllowed {
        name: String,
        version: String,
        allowed: Vec<String>,
    },

    #[error("Failed to resolve dependencies for module '{name}@{version}': {reason}")]
    DependencyResolutionFailed {
        name: String,
        version: String,
        reason: String,
    },
}

// ============================================================================
// MVS (Minimal Version Selection) Algorithm
// ============================================================================

/// A unique key identifying a module in the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleKey {
    /// The module name.
    pub name: String,
    /// The module version.
    pub version: String,
}

impl ModuleKey {
    /// Create a new module key.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    /// Create a module key from a BazelDep.
    pub fn from_dep(dep: &BazelDep) -> Self {
        Self {
            name: dep.name.clone(),
            version: dep.version.as_str().to_string(),
        }
    }
}

impl std::fmt::Display for ModuleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

/// Selection group key for MVS algorithm.
///
/// Modules are grouped by name and compatibility level. Within each group,
/// MVS selects the maximum version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectionGroup {
    /// The module name.
    pub module_name: String,
    /// The compatibility level.
    pub compatibility_level: u32,
    /// Target allowed version for multiple_version_override.
    /// If set, only this specific version is allowed in this group.
    pub target_allowed_version: Option<Version>,
}

impl SelectionGroup {
    /// Create a selection group for a module.
    pub fn new(name: &str, compat_level: u32) -> Self {
        Self {
            module_name: name.to_string(),
            compatibility_level: compat_level,
            target_allowed_version: None,
        }
    }

    /// Create a selection group with a target version (for multiple_version_override).
    pub fn with_target_version(name: &str, compat_level: u32, target: Version) -> Self {
        Self {
            module_name: name.to_string(),
            compatibility_level: compat_level,
            target_allowed_version: Some(target),
        }
    }
}

/// Information about a discovered module during resolution.
#[derive(Debug, Clone)]
pub struct DiscoveredModule {
    /// The module key.
    pub key: ModuleKey,
    /// The parsed module.
    pub module: Module,
    /// The compatibility level.
    pub compatibility_level: u32,
    /// Source of this module (registry URL or local path).
    pub source: ModuleSource,
}

/// Source of a resolved module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleSource {
    /// Module from a registry.
    Registry {
        url: String,
    },
    /// Module from a local path override.
    LocalPath {
        path: String,
    },
    /// Module from a git override.
    Git {
        remote: String,
        commit: String,
    },
    /// Module from an archive override.
    Archive {
        urls: Vec<String>,
    },
}

/// Result of MVS resolution - the final resolved dependency graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedGraph {
    /// Map from module name to selected version.
    pub selected_versions: HashMap<String, String>,
    /// Full module information for each selected module.
    pub modules: HashMap<String, ResolvedModuleInfo>,
    /// Resolution order (topological).
    pub resolution_order: Vec<String>,
}

/// Information about a resolved module in the final graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedModuleInfo {
    /// The module name.
    pub name: String,
    /// The selected version.
    pub version: String,
    /// Compatibility level.
    pub compatibility_level: u32,
    /// Direct dependencies (module name -> required version).
    pub dependencies: HashMap<String, String>,
    /// Source of this module.
    pub source: ModuleSource,
    /// Path to extracted source (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
}

/// MVS resolver for bzlmod dependencies.
pub struct MvsResolver {
    /// Registry client for fetching modules.
    registry_client: RegistryClient,
    /// Source fetcher for downloading module sources.
    source_fetcher: SourceFetcher,
    /// Cache for module storage (kept for potential future use).
    #[allow(dead_code)]
    cache: ModuleCache,
    /// All discovered modules during resolution.
    discovered: HashMap<ModuleKey, DiscoveredModule>,
    /// Modules with non-registry overrides (always selected).
    overridden_modules: HashMap<String, DiscoveredModule>,
    /// Single version overrides.
    single_version_overrides: HashMap<String, SingleVersionOverride>,
    /// Multiple version overrides.
    multiple_version_overrides: HashMap<String, MultipleVersionOverride>,
}

impl MvsResolver {
    /// Create a new MVS resolver with default BCR registry.
    pub async fn new(cache: ModuleCache) -> kuro_error::Result<Self> {
        let registry_client = RegistryClient::bcr(cache.clone()).await?;
        let source_fetcher = SourceFetcher::new(cache.clone()).await?;

        Ok(Self {
            registry_client,
            source_fetcher,
            cache,
            discovered: HashMap::new(),
            overridden_modules: HashMap::new(),
            single_version_overrides: HashMap::new(),
            multiple_version_overrides: HashMap::new(),
        })
    }

    /// Create a resolver with a custom registry URL.
    pub async fn with_registry(registry_url: &str, cache: ModuleCache) -> kuro_error::Result<Self> {
        let registry_client = RegistryClient::new(registry_url, cache.clone()).await?;
        let source_fetcher = SourceFetcher::new(cache.clone()).await?;

        Ok(Self {
            registry_client,
            source_fetcher,
            cache,
            discovered: HashMap::new(),
            overridden_modules: HashMap::new(),
            single_version_overrides: HashMap::new(),
            multiple_version_overrides: HashMap::new(),
        })
    }

    /// Process overrides from the root module.
    fn process_overrides(&mut self, overrides: &[Override]) {
        for override_ in overrides {
            match override_ {
                Override::SingleVersion(sv) => {
                    self.single_version_overrides
                        .insert(sv.module_name.clone(), sv.clone());
                }
                Override::MultipleVersion(mv) => {
                    self.multiple_version_overrides
                        .insert(mv.module_name.clone(), mv.clone());
                }
                // Local, Git, Archive overrides are handled during discovery
                _ => {}
            }
        }
    }

    /// Get the effective version for a dependency considering overrides.
    fn get_effective_version(&self, dep: &BazelDep) -> Version {
        // Check for single version override
        if let Some(sv) = self.single_version_overrides.get(&dep.name) {
            return sv.version.clone();
        }
        dep.version.clone()
    }

    /// Check if a module has a non-registry override.
    fn has_non_registry_override<'a>(
        &self,
        name: &str,
        overrides: &'a [Override],
    ) -> Option<&'a Override> {
        overrides.iter().find(|o| match o {
            Override::LocalPath(lp) => lp.module_name == name,
            Override::Git(g) => g.module_name == name,
            Override::Archive(a) => a.module_name == name,
            _ => false,
        })
    }

    /// Discover all modules in the dependency graph.
    ///
    /// This is Phase 1 of MVS: recursively fetch all modules and their dependencies.
    async fn discover_modules(
        &mut self,
        root: &Module,
        workspace_root: &Path,
    ) -> kuro_error::Result<()> {
        let mut queue: VecDeque<(BazelDep, Option<PathBuf>)> = VecDeque::new();
        let mut visited: HashSet<ModuleKey> = HashSet::new();

        // Process overrides first
        self.process_overrides(&root.overrides);

        // Collect non-registry overrides first to avoid borrow issues
        let override_modules: Vec<_> = root
            .bazel_deps
            .iter()
            .filter(|dep| !dep.dev_dependency)
            .filter_map(|dep| {
                self.has_non_registry_override(&dep.name, &root.overrides)
                    .cloned()
            })
            .collect();

        // Resolve non-registry overrides
        for override_ in override_modules {
            self.resolve_override_module(&override_, workspace_root)
                .await?;
        }

        // Add root's direct dependencies to queue
        for dep in &root.bazel_deps {
            if dep.dev_dependency {
                continue; // Skip dev dependencies for now
            }

            // Skip if we already resolved an override for this module
            if self.overridden_modules.contains_key(&dep.name) {
                continue;
            }

            let effective_dep = BazelDep {
                name: dep.name.clone(),
                version: self.get_effective_version(dep),
                repo_name: dep.repo_name.clone(),
                dev_dependency: dep.dev_dependency,
            };
            queue.push_back((effective_dep, None));
        }

        // BFS through dependency graph
        while let Some((dep, _parent_path)) = queue.pop_front() {
            let key = ModuleKey::from_dep(&dep);

            if visited.contains(&key) {
                continue;
            }
            visited.insert(key.clone());

            // Skip if we have a non-registry override for this module
            if self.overridden_modules.contains_key(&dep.name) {
                continue;
            }

            // Fetch module from registry
            let discovered = self.fetch_and_discover_module(&dep).await?;

            // Add transitive dependencies to queue
            for transitive_dep in &discovered.module.bazel_deps {
                if transitive_dep.dev_dependency {
                    continue;
                }

                // Check for overrides on transitive deps
                if self.overridden_modules.contains_key(&transitive_dep.name) {
                    continue;
                }

                let effective_dep = BazelDep {
                    name: transitive_dep.name.clone(),
                    version: self.get_effective_version(transitive_dep),
                    repo_name: transitive_dep.repo_name.clone(),
                    dev_dependency: transitive_dep.dev_dependency,
                };

                let trans_key = ModuleKey::from_dep(&effective_dep);
                if !visited.contains(&trans_key) {
                    queue.push_back((effective_dep, None));
                }
            }

            self.discovered.insert(key, discovered);
        }

        Ok(())
    }

    /// Fetch a module from registry and create DiscoveredModule.
    async fn fetch_and_discover_module(
        &self,
        dep: &BazelDep,
    ) -> kuro_error::Result<DiscoveredModule> {
        let version_str = dep.version.as_str();

        tracing::debug!("Fetching {}@{} from registry", dep.name, version_str);

        // Fetch MODULE.bazel content
        let module_bazel_content = self
            .registry_client
            .fetch_module_bazel(&dep.name, version_str)
            .await
            .map_err(|e| {
                MvsResolutionError::DependencyResolutionFailed {
                    name: dep.name.clone(),
                    version: version_str.to_string(),
                    reason: format!("Failed to fetch MODULE.bazel: {}", e),
                }
            })?;

        // Parse MODULE.bazel
        let filename = format!("{}@{}/MODULE.bazel", dep.name, version_str);
        let parsed = parse_module_bazel_content(&module_bazel_content, &filename).map_err(|e| {
            MvsResolutionError::DependencyResolutionFailed {
                name: dep.name.clone(),
                version: version_str.to_string(),
                reason: format!("Failed to parse MODULE.bazel: {}", e),
            }
        })?;

        Ok(DiscoveredModule {
            key: ModuleKey::from_dep(dep),
            compatibility_level: parsed.module.compatibility_level,
            module: parsed.module,
            source: ModuleSource::Registry {
                url: self.registry_client.base_url().to_string(),
            },
        })
    }

    /// Resolve a module with a non-registry override.
    async fn resolve_override_module(
        &mut self,
        override_: &Override,
        workspace_root: &Path,
    ) -> kuro_error::Result<()> {
        match override_ {
            Override::LocalPath(lp) => {
                let resolved = resolve_local_override(lp, workspace_root)?;
                let discovered = DiscoveredModule {
                    key: ModuleKey::new(&lp.module_name, resolved.version.as_str()),
                    compatibility_level: resolved.module.compatibility_level,
                    module: resolved.module,
                    source: ModuleSource::LocalPath {
                        path: lp.path.clone(),
                    },
                };
                self.overridden_modules
                    .insert(lp.module_name.clone(), discovered);
            }
            Override::Git(g) => {
                // For git overrides, we need to fetch and parse
                // For now, create a placeholder - full implementation would clone the repo
                tracing::warn!(
                    "Git override for {} not fully implemented yet",
                    g.module_name
                );
                let discovered = DiscoveredModule {
                    key: ModuleKey::new(&g.module_name, ""),
                    compatibility_level: 0,
                    module: Module::empty(),
                    source: ModuleSource::Git {
                        remote: g.remote.clone(),
                        commit: g.commit.clone(),
                    },
                };
                self.overridden_modules
                    .insert(g.module_name.clone(), discovered);
            }
            Override::Archive(a) => {
                // For archive overrides, we need to fetch and parse
                tracing::warn!(
                    "Archive override for {} not fully implemented yet",
                    a.module_name
                );
                let discovered = DiscoveredModule {
                    key: ModuleKey::new(&a.module_name, ""),
                    compatibility_level: 0,
                    module: Module::empty(),
                    source: ModuleSource::Archive {
                        urls: a.urls.clone(),
                    },
                };
                self.overridden_modules
                    .insert(a.module_name.clone(), discovered);
            }
            _ => {}
        }
        Ok(())
    }

    /// Run MVS selection: group by selection key and pick max version per group.
    fn select_versions(&self) -> kuro_error::Result<HashMap<String, Version>> {
        let mut selection_groups: HashMap<SelectionGroup, Vec<(Version, &DiscoveredModule)>> =
            HashMap::new();

        // Group discovered modules by selection key
        for (key, discovered) in &self.discovered {
            let version = Version::parse(&key.version).unwrap_or_else(|_| Version::empty());

            // Check for multiple_version_override
            let group = if let Some(mv) = self.multiple_version_overrides.get(&key.name) {
                // Find the target version this should map to
                let target = self.find_target_allowed_version(&version, mv)?;
                SelectionGroup::with_target_version(
                    &key.name,
                    discovered.compatibility_level,
                    target,
                )
            } else {
                SelectionGroup::new(&key.name, discovered.compatibility_level)
            };

            selection_groups
                .entry(group)
                .or_default()
                .push((version, discovered));
        }

        // Check for compatibility conflicts (same module name, different compat levels)
        self.check_compatibility_conflicts(&selection_groups)?;

        // Select maximum version per group
        let mut selected: HashMap<String, Version> = HashMap::new();

        for (group, versions) in selection_groups {
            let max_version = versions
                .into_iter()
                .max_by(|(v1, _), (v2, _)| v1.cmp(v2))
                .map(|(v, _)| v)
                .unwrap();

            // For multiple_version_override, we might have multiple selected versions
            // for the same module name but different target versions
            let key = if group.target_allowed_version.is_some() {
                format!("{}+{}", group.module_name, max_version)
            } else {
                group.module_name.clone()
            };

            selected.insert(key, max_version);
        }

        // Add overridden modules (they always "win")
        for (name, discovered) in &self.overridden_modules {
            let version = Version::parse(&discovered.key.version).unwrap_or_else(|_| Version::empty());
            selected.insert(name.clone(), version);
        }

        Ok(selected)
    }

    /// Find the target allowed version for multiple_version_override.
    fn find_target_allowed_version(
        &self,
        requested: &Version,
        mv: &MultipleVersionOverride,
    ) -> kuro_error::Result<Version> {
        // Find the nearest higher (or equal) allowed version at same compat level
        let allowed_versions: Vec<&Version> = mv.versions.iter().collect();

        // If the requested version is in the allowed list, use it
        if allowed_versions.iter().any(|v| *v == requested) {
            return Ok(requested.clone());
        }

        // Find the nearest higher allowed version
        let higher: Vec<_> = allowed_versions
            .iter()
            .filter(|v| **v >= requested)
            .collect();

        if let Some(target) = higher.into_iter().min_by(|a, b| a.cmp(b)) {
            return Ok((*target).clone());
        }

        Err(MvsResolutionError::VersionNotAllowed {
            name: mv.module_name.clone(),
            version: requested.to_string(),
            allowed: mv.versions.iter().map(|v| v.to_string()).collect(),
        }
        .into())
    }

    /// Check for compatibility level conflicts.
    fn check_compatibility_conflicts(
        &self,
        groups: &HashMap<SelectionGroup, Vec<(Version, &DiscoveredModule)>>,
    ) -> kuro_error::Result<()> {
        // Group by module name to check for compat conflicts
        let mut by_name: HashMap<&str, Vec<&SelectionGroup>> = HashMap::new();
        for group in groups.keys() {
            by_name
                .entry(&group.module_name)
                .or_default()
                .push(group);
        }

        for (name, name_groups) in by_name {
            // Skip if there's a multiple_version_override for this module
            if self.multiple_version_overrides.contains_key(name) {
                continue;
            }

            // Check if all groups have the same compatibility level
            let compat_levels: HashSet<_> = name_groups
                .iter()
                .map(|g| g.compatibility_level)
                .collect();

            if compat_levels.len() > 1 {
                // Find two conflicting versions for the error message
                let g1 = name_groups[0];
                let g2 = name_groups
                    .iter()
                    .find(|g| g.compatibility_level != g1.compatibility_level)
                    .unwrap();

                let v1 = groups
                    .get(g1)
                    .and_then(|vs| vs.first())
                    .map(|(v, _)| v.to_string())
                    .unwrap_or_default();
                let v2 = groups
                    .get(*g2)
                    .and_then(|vs| vs.first())
                    .map(|(v, _)| v.to_string())
                    .unwrap_or_default();

                return Err(MvsResolutionError::CompatibilityConflict {
                    name: name.to_string(),
                    version1: v1,
                    compat1: g1.compatibility_level,
                    version2: v2,
                    compat2: g2.compatibility_level,
                }
                .into());
            }
        }

        Ok(())
    }

    /// Build the final resolved graph with rewritten dependencies.
    async fn build_resolved_graph(
        &self,
        selected: &HashMap<String, Version>,
    ) -> kuro_error::Result<ResolvedGraph> {
        let mut modules = HashMap::new();
        let mut resolution_order = Vec::new();

        // Build module info for each selected version
        for (name, version) in selected {
            // Handle multiple version override keys (name+version format)
            let actual_name = if name.contains('+') {
                name.split('+').next().unwrap().to_string()
            } else {
                name.clone()
            };

            // Find the discovered module
            let key = ModuleKey::new(&actual_name, version.as_str());

            let (module, source) = if let Some(discovered) = self.discovered.get(&key) {
                (discovered.module.clone(), discovered.source.clone())
            } else if let Some(overridden) = self.overridden_modules.get(&actual_name) {
                (overridden.module.clone(), overridden.source.clone())
            } else {
                // Module not found - this shouldn't happen
                tracing::warn!("Module {} not found in discovered or overridden", key);
                continue;
            };

            // Rewrite dependencies to point to selected versions
            let mut dependencies = HashMap::new();
            for dep in &module.bazel_deps {
                if dep.dev_dependency {
                    continue;
                }
                if let Some(selected_version) = selected.get(&dep.name) {
                    dependencies.insert(dep.name.clone(), selected_version.to_string());
                }
            }

            let info = ResolvedModuleInfo {
                name: actual_name.clone(),
                version: version.to_string(),
                compatibility_level: module.compatibility_level,
                dependencies,
                source,
                source_path: None, // Will be filled when sources are fetched
            };

            resolution_order.push(actual_name.clone());
            modules.insert(actual_name, info);
        }

        Ok(ResolvedGraph {
            selected_versions: selected
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
            modules,
            resolution_order,
        })
    }

    /// Run the full MVS resolution algorithm.
    ///
    /// # Algorithm
    ///
    /// 1. Discover all modules by traversing the dependency graph
    /// 2. Process overrides (single_version, multiple_version, local_path, etc.)
    /// 3. Group modules by selection key (name + compatibility_level)
    /// 4. Check for compatibility level conflicts
    /// 5. Select maximum version per group (MVS)
    /// 6. Build final resolved graph with rewritten dependencies
    ///
    /// # Arguments
    ///
    /// * `root` - The root module (from parsing the project's MODULE.bazel)
    /// * `workspace_root` - The workspace root directory
    ///
    /// # Returns
    ///
    /// A `ResolvedGraph` containing the selected versions and module information.
    pub async fn resolve(
        &mut self,
        root: &Module,
        workspace_root: &Path,
    ) -> kuro_error::Result<ResolvedGraph> {
        tracing::info!("Starting MVS resolution for module '{}'", root.name);

        // Phase 1: Discover all modules
        self.discover_modules(root, workspace_root).await?;

        tracing::debug!(
            "Discovered {} modules ({} with overrides)",
            self.discovered.len(),
            self.overridden_modules.len()
        );

        // Phase 2-4: Select versions using MVS
        let selected = self.select_versions()?;

        tracing::debug!("Selected {} unique module versions", selected.len());

        // Phase 5: Build resolved graph
        let graph = self.build_resolved_graph(&selected).await?;

        tracing::info!(
            "MVS resolution complete: {} modules in final graph",
            graph.modules.len()
        );

        Ok(graph)
    }

    /// Fetch sources for all resolved modules.
    ///
    /// This downloads and extracts sources for modules that don't have local overrides.
    pub async fn fetch_sources(
        &self,
        graph: &mut ResolvedGraph,
    ) -> kuro_error::Result<()> {
        for (name, info) in &mut graph.modules {
            match &info.source {
                ModuleSource::Registry { url: _ } => {
                    // Fetch from registry
                    let source_info = self
                        .registry_client
                        .fetch_source_info(name, &info.version)
                        .await?;

                    let source_path = self
                        .source_fetcher
                        .fetch_source(
                            self.registry_client.base_url(),
                            name,
                            &info.version,
                            &source_info,
                        )
                        .await?;

                    info.source_path = Some(source_path);
                }
                ModuleSource::LocalPath { path } => {
                    // Local path is already available
                    info.source_path = Some(PathBuf::from(path));
                }
                ModuleSource::Git { .. } | ModuleSource::Archive { .. } => {
                    // TODO: Implement git/archive fetching in fetch_sources
                    tracing::warn!("Git/Archive source fetching not yet implemented for {}", name);
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// Lockfile-Integrated Resolution
// ============================================================================

use crate::lockfile::Lockfile;
use crate::lockfile::LockfileMode;
use crate::lockfile::lockfile_path;

/// Resolve dependencies with lockfile support.
///
/// This is the main entry point for bzlmod resolution. It:
/// 1. Checks for an existing lockfile and validates it
/// 2. Uses the lockfile if valid (fast path)
/// 3. Runs MVS resolution if lockfile is invalid or missing
/// 4. Updates the lockfile after resolution
///
/// # Arguments
///
/// * `root` - The root module (from parsing the project's MODULE.bazel)
/// * `workspace_root` - The workspace root directory
/// * `module_bazel_path` - Path to the root MODULE.bazel file
/// * `mode` - Lockfile mode controlling resolution behavior
///
/// # Returns
///
/// A `ResolvedGraph` containing the selected versions and module information.
pub async fn resolve_with_lockfile(
    root: &Module,
    workspace_root: &Path,
    module_bazel_path: &Path,
    mode: LockfileMode,
) -> kuro_error::Result<ResolvedGraph> {
    let lock_path = lockfile_path(workspace_root);

    // Handle lockfile mode
    match mode {
        LockfileMode::Off => {
            // Don't use lockfile, always resolve
            return resolve_fresh(root, workspace_root, module_bazel_path, None).await;
        }
        LockfileMode::Refresh => {
            // Always re-resolve but update lockfile
            return resolve_fresh(root, workspace_root, module_bazel_path, Some(&lock_path)).await;
        }
        LockfileMode::Update | LockfileMode::Error => {
            // Check lockfile first
        }
    }

    // Try to use existing lockfile
    if lock_path.exists() {
        match Lockfile::read(&lock_path) {
            Ok(lockfile) => {
                if lockfile.is_valid_for(root, module_bazel_path) {
                    tracing::info!("Using cached resolution from lockfile");
                    return Ok(lockfile.to_resolved_graph());
                } else {
                    tracing::info!("Lockfile is stale, re-resolving");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read lockfile: {}, re-resolving", e);
            }
        }
    }

    // Check if we're in error mode and would need to update
    if mode == LockfileMode::Error {
        return Err(crate::lockfile::LockfileError::LockfileModeError.into());
    }

    // Resolve fresh
    resolve_fresh(root, workspace_root, module_bazel_path, Some(&lock_path)).await
}

/// Perform fresh MVS resolution and optionally update lockfile.
async fn resolve_fresh(
    root: &Module,
    workspace_root: &Path,
    module_bazel_path: &Path,
    lockfile_path: Option<&Path>,
) -> kuro_error::Result<ResolvedGraph> {
    let cache = ModuleCache::new()?;
    let mut resolver = MvsResolver::new(cache).await?;

    let graph = resolver.resolve(root, workspace_root).await?;

    // Update lockfile if path provided
    if let Some(path) = lockfile_path {
        let lockfile = Lockfile::from_resolved_graph(&graph, module_bazel_path)?;
        lockfile.write(path)?;
        tracing::info!("Updated lockfile at {}", path.display());
    }

    Ok(graph)
}

/// A resolved local module.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ResolvedLocalModule {
    /// The module name.
    pub name: String,

    /// The resolved version from the local module's MODULE.bazel.
    pub version: Version,

    /// The absolute path to the module directory.
    pub absolute_path: PathBuf,

    /// The path relative to workspace root.
    pub relative_path: String,

    /// The parsed module information.
    pub module: Module,

    /// Whether this module has a MODULE.bazel file.
    pub has_module_file: bool,
}

/// Result of resolving local path overrides.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ResolvedLocalModules {
    /// Map from module name to resolved module information.
    pub modules: HashMap<String, ResolvedLocalModule>,

    /// Order in which modules were resolved (topological order).
    pub resolution_order: Vec<String>,
}

impl ResolvedLocalModules {
    /// Creates an empty resolution result.
    pub fn empty() -> Self {
        Self {
            modules: HashMap::new(),
            resolution_order: Vec::new(),
        }
    }

    /// Returns true if there are no resolved local modules.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Gets a resolved module by name.
    pub fn get(&self, name: &str) -> Option<&ResolvedLocalModule> {
        self.modules.get(name)
    }

    /// Returns an iterator over all resolved modules.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ResolvedLocalModule)> {
        self.modules.iter()
    }
}

/// Resolve a single local path override.
///
/// # Arguments
///
/// * `override_info` - The local path override to resolve.
/// * `workspace_root` - The workspace root directory.
///
/// # Returns
///
/// A `ResolvedLocalModule` containing the parsed module information.
pub fn resolve_local_override(
    override_info: &LocalPathOverride,
    workspace_root: &Path,
) -> kuro_error::Result<ResolvedLocalModule> {
    // Resolve the path relative to workspace root
    let module_path = workspace_root.join(&override_info.path);

    // Verify the path exists
    if !module_path.exists() {
        return Err(LocalResolutionError::PathNotFound(
            override_info.path.clone(),
        )
        .into());
    }

    // Look for MODULE.bazel in the local module
    let module_bazel_path = module_path.join("MODULE.bazel");

    let (parsed_module, has_module_file) = if module_bazel_path.exists() {
        let parsed = parse_module_bazel(&module_bazel_path).with_buck_error_context(|| {
            format!(
                "Failed to parse MODULE.bazel for local module '{}' at {:?}",
                override_info.module_name, module_bazel_path
            )
        })?;
        (parsed.module, parsed.has_module_directive)
    } else {
        // No MODULE.bazel - create an empty module with the override name
        let mut module = Module::empty();
        module.name = override_info.module_name.clone();
        (module, false)
    };

    // Use the module name from MODULE.bazel if present, otherwise use the override name
    let name = if parsed_module.name.is_empty() {
        override_info.module_name.clone()
    } else {
        parsed_module.name.clone()
    };

    Ok(ResolvedLocalModule {
        name,
        version: parsed_module.version.clone(),
        absolute_path: module_path
            .canonicalize()
            .unwrap_or_else(|_| module_path.clone()),
        relative_path: override_info.path.clone(),
        module: parsed_module,
        has_module_file,
    })
}

/// Resolve all local path overrides from a module.
///
/// This function takes the overrides from a parsed MODULE.bazel file and resolves
/// each `local_path_override()` directive to a `ResolvedLocalModule`.
///
/// # Arguments
///
/// * `overrides` - The list of overrides from MODULE.bazel.
/// * `workspace_root` - The workspace root directory.
///
/// # Returns
///
/// A `ResolvedLocalModules` containing all resolved local modules.
///
/// # Example
///
/// ```ignore
/// use kuro_bzlmod::resolution::resolve_local_modules;
/// use std::path::Path;
///
/// let parsed = parse_module_bazel(module_bazel_path).unwrap();
/// let resolved = resolve_local_modules(&parsed.module.overrides, Path::new("/path/to/workspace")).unwrap();
///
/// for (name, module) in resolved.iter() {
///     println!("Local module: {} at {:?}", name, module.absolute_path);
/// }
/// ```
pub fn resolve_local_modules(
    overrides: &[Override],
    workspace_root: &Path,
) -> kuro_error::Result<ResolvedLocalModules> {
    let mut modules = HashMap::new();
    let mut resolution_order = Vec::new();

    // First pass: resolve all local path overrides
    for override_info in overrides {
        if let Override::LocalPath(local) = override_info {
            let resolved = resolve_local_override(local, workspace_root)?;
            let name = resolved.name.clone();

            if modules.contains_key(&name) {
                return Err(LocalResolutionError::ResolutionFailed {
                    module_name: name,
                    reason: "Duplicate local path override".to_owned(),
                }
                .into());
            }

            resolution_order.push(name.clone());
            modules.insert(name, resolved);
        }
    }

    // Second pass: resolve transitive local path overrides from local modules
    // This handles cases where a local module has its own local_path_override()
    let mut to_process: Vec<String> = resolution_order.clone();
    let mut processed: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(name) = to_process.pop() {
        if processed.contains(&name) {
            continue;
        }
        processed.insert(name.clone());

        // Get the module's overrides
        let module = modules.get(&name).cloned();
        if let Some(resolved) = module {
            for override_info in &resolved.module.overrides {
                if let Override::LocalPath(local) = override_info {
                    // Resolve path relative to the local module's directory
                    let nested_resolved =
                        resolve_local_override(local, &resolved.absolute_path)?;
                    let nested_name = nested_resolved.name.clone();

                    if !modules.contains_key(&nested_name) {
                        resolution_order.push(nested_name.clone());
                        modules.insert(nested_name.clone(), nested_resolved);
                        to_process.push(nested_name);
                    }
                }
            }
        }
    }

    Ok(ResolvedLocalModules {
        modules,
        resolution_order,
    })
}

/// Information about a local module for cell registration.
///
/// This is the output format for integrating with the cell system.
#[derive(Debug, Clone)]
pub struct LocalModuleCellInfo {
    /// The cell name to use (derived from module name).
    pub cell_name: String,

    /// The bzlmod module name.
    pub module_name: Arc<str>,

    /// Path relative to workspace root.
    pub path: Arc<str>,
}

impl ResolvedLocalModules {
    /// Convert resolved modules to cell registration information.
    ///
    /// This provides the information needed to register local modules as cells
    /// in the Kuro cell resolver.
    pub fn to_cell_infos(&self) -> Vec<LocalModuleCellInfo> {
        self.modules
            .values()
            .map(|resolved| LocalModuleCellInfo {
                cell_name: resolved.name.clone(),
                module_name: Arc::from(resolved.name.as_str()),
                path: Arc::from(resolved.relative_path.as_str()),
            })
            .collect()
    }
}

// ============================================================================
// Remote Module Resolution (BCR)
// ============================================================================

/// A resolved remote module from a registry.
#[derive(Debug, Clone, Allocative)]
pub struct ResolvedRemoteModule {
    /// The module name.
    pub name: String,

    /// The resolved version.
    pub version: Version,

    /// The registry URL this was fetched from.
    pub registry_url: String,

    /// The absolute path to the extracted source directory.
    pub source_path: PathBuf,

    /// The parsed module information.
    pub module: Module,
}

/// Result of resolving remote dependencies.
#[derive(Debug, Clone, Default, Allocative)]
pub struct ResolvedRemoteModules {
    /// Map from module name to resolved module information.
    pub modules: HashMap<String, ResolvedRemoteModule>,

    /// Order in which modules were resolved.
    pub resolution_order: Vec<String>,
}

impl ResolvedRemoteModules {
    /// Creates an empty resolution result.
    pub fn empty() -> Self {
        Self {
            modules: HashMap::new(),
            resolution_order: Vec::new(),
        }
    }

    /// Returns true if there are no resolved remote modules.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Gets a resolved module by name.
    pub fn get(&self, name: &str) -> Option<&ResolvedRemoteModule> {
        self.modules.get(name)
    }

    /// Returns an iterator over all resolved modules.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ResolvedRemoteModule)> {
        self.modules.iter()
    }

    /// Convert resolved modules to cell registration information.
    pub fn to_cell_infos(&self) -> Vec<LocalModuleCellInfo> {
        self.modules
            .values()
            .map(|resolved| LocalModuleCellInfo {
                cell_name: resolved.name.clone(),
                module_name: Arc::from(resolved.name.as_str()),
                path: Arc::from(resolved.source_path.to_string_lossy().as_ref()),
            })
            .collect()
    }
}

/// Resolver for remote modules from registries.
pub struct RemoteModuleResolver {
    registry_client: RegistryClient,
    source_fetcher: SourceFetcher,
}

impl RemoteModuleResolver {
    /// Create a new remote module resolver.
    pub async fn new(cache: ModuleCache) -> kuro_error::Result<Self> {
        let registry_client = RegistryClient::bcr(cache.clone()).await?;
        let source_fetcher = SourceFetcher::new(cache).await?;

        Ok(Self {
            registry_client,
            source_fetcher,
        })
    }

    /// Create a resolver with a custom registry URL.
    pub async fn with_registry(
        registry_url: &str,
        cache: ModuleCache,
    ) -> kuro_error::Result<Self> {
        let registry_client = RegistryClient::new(registry_url, cache.clone()).await?;
        let source_fetcher = SourceFetcher::new(cache).await?;

        Ok(Self {
            registry_client,
            source_fetcher,
        })
    }

    /// Resolve a single dependency from the registry.
    pub async fn resolve_dependency(
        &self,
        dep: &BazelDep,
    ) -> kuro_error::Result<ResolvedRemoteModule> {
        let name = &dep.name;
        let version = &dep.version;
        let version_str = version.as_str();

        tracing::info!("Resolving {}@{} from BCR", name, version_str);

        // Fetch MODULE.bazel from registry
        let module_bazel_content = self
            .registry_client
            .fetch_module_bazel(name, version_str)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch MODULE.bazel for {}@{}: {}", name, version_str, e);
                RemoteResolutionError::FetchFailed {
                    name: name.clone(),
                    version: version_str.to_string(),
                }
            })?;

        // Parse the MODULE.bazel
        let filename = format!("{}@{}/MODULE.bazel", name, version_str);
        let parsed = parse_module_bazel_content(&module_bazel_content, &filename)
            .map_err(|e| {
                tracing::error!("Failed to parse MODULE.bazel for {}@{}: {}", name, version_str, e);
                RemoteResolutionError::FetchFailed {
                    name: name.clone(),
                    version: version_str.to_string(),
                }
            })?;

        // Fetch source.json
        let source_info = self
            .registry_client
            .fetch_source_info(name, version_str)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch source.json for {}@{}: {}", name, version_str, e);
                RemoteResolutionError::FetchFailed {
                    name: name.clone(),
                    version: version_str.to_string(),
                }
            })?;

        // Download and extract source
        let source_path = self
            .source_fetcher
            .fetch_source(self.registry_client.base_url(), name, version_str, &source_info)
            .await
            .map_err(|e| RemoteResolutionError::ExtractionFailed {
                name: name.clone(),
                version: version_str.to_string(),
                reason: e.to_string(),
            })?;

        Ok(ResolvedRemoteModule {
            name: name.clone(),
            version: version.clone(),
            registry_url: self.registry_client.base_url().to_string(),
            source_path,
            module: parsed.module,
        })
    }

    /// Resolve all bazel_dep declarations from a module.
    ///
    /// This fetches each dependency from the registry, downloads and extracts
    /// the source, and returns the resolved modules.
    pub async fn resolve_dependencies(
        &self,
        deps: &[BazelDep],
        overrides: &[Override],
    ) -> kuro_error::Result<ResolvedRemoteModules> {
        let mut modules = HashMap::new();
        let mut resolution_order = Vec::new();

        // Build set of modules with local overrides (skip fetching these)
        let local_override_names: std::collections::HashSet<_> = overrides
            .iter()
            .filter_map(|o| match o {
                Override::LocalPath(local) => Some(local.module_name.clone()),
                _ => None,
            })
            .collect();

        for dep in deps {
            // Skip if there's a local override for this module
            if local_override_names.contains(&dep.name) {
                tracing::debug!(
                    "Skipping {}@{} - has local_path_override",
                    dep.name,
                    dep.version
                );
                continue;
            }

            // Skip if already resolved
            if modules.contains_key(&dep.name) {
                continue;
            }

            match self.resolve_dependency(dep).await {
                Ok(resolved) => {
                    let name = resolved.name.clone();
                    resolution_order.push(name.clone());
                    modules.insert(name, resolved);
                }
                Err(e) => {
                    tracing::error!("Failed to resolve {}@{}: {}", dep.name, dep.version, e);
                    return Err(e);
                }
            }
        }

        Ok(ResolvedRemoteModules {
            modules,
            resolution_order,
        })
    }

    /// Get the registry client for direct access.
    pub fn registry_client(&self) -> &RegistryClient {
        &self.registry_client
    }
}

/// Convenience function to resolve all dependencies (both local and remote).
///
/// This is the main entry point for dependency resolution.
pub async fn resolve_all_dependencies(
    root_module: &Module,
    workspace_root: &Path,
) -> kuro_error::Result<(ResolvedLocalModules, ResolvedRemoteModules)> {
    // Resolve local overrides first
    let local_modules = resolve_local_modules(&root_module.overrides, workspace_root)?;

    // Create cache and resolver for remote modules
    let cache = ModuleCache::new()?;
    let resolver = RemoteModuleResolver::new(cache).await?;

    // Resolve remote dependencies (skipping those with local overrides)
    let remote_modules = resolver
        .resolve_dependencies(&root_module.bazel_deps, &root_module.overrides)
        .await?;

    Ok((local_modules, remote_modules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_workspace() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create root MODULE.bazel
        let root_module = r#"
module(name = "root", version = "1.0.0")

local_path_override(
    module_name = "local_lib",
    path = "libs/local_lib",
)
"#;
        fs::write(dir.path().join("MODULE.bazel"), root_module).unwrap();

        // Create local module directory
        let local_lib_dir = dir.path().join("libs/local_lib");
        fs::create_dir_all(&local_lib_dir).unwrap();

        // Create local module's MODULE.bazel
        let local_module = r#"
module(name = "local_lib", version = "2.0.0")
"#;
        fs::write(local_lib_dir.join("MODULE.bazel"), local_module).unwrap();

        // Create a BUILD.bazel for the local module
        fs::write(
            local_lib_dir.join("BUILD.bazel"),
            "# Build targets here",
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_resolve_single_local_module() {
        let workspace = create_test_workspace();

        let override_info = LocalPathOverride {
            module_name: "local_lib".to_owned(),
            path: "libs/local_lib".to_owned(),
        };

        let resolved = resolve_local_override(&override_info, workspace.path()).unwrap();

        assert_eq!(resolved.name, "local_lib");
        assert_eq!(resolved.version.as_str(), "2.0.0");
        assert!(resolved.has_module_file);
        assert!(resolved.absolute_path.exists());
    }

    #[test]
    fn test_resolve_local_module_without_module_bazel() {
        let dir = TempDir::new().unwrap();

        // Create local module directory without MODULE.bazel
        let local_dir = dir.path().join("my_local");
        fs::create_dir_all(&local_dir).unwrap();
        fs::write(local_dir.join("BUILD.bazel"), "# Build").unwrap();

        let override_info = LocalPathOverride {
            module_name: "my_local".to_owned(),
            path: "my_local".to_owned(),
        };

        let resolved = resolve_local_override(&override_info, dir.path()).unwrap();

        assert_eq!(resolved.name, "my_local");
        assert!(!resolved.has_module_file);
        assert!(resolved.version.is_empty());
    }

    #[test]
    fn test_resolve_nonexistent_path() {
        let dir = TempDir::new().unwrap();

        let override_info = LocalPathOverride {
            module_name: "nonexistent".to_owned(),
            path: "does/not/exist".to_owned(),
        };

        let result = resolve_local_override(&override_info, dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_multiple_local_modules() {
        let dir = TempDir::new().unwrap();

        // Create two local modules
        for name in &["lib_a", "lib_b"] {
            let lib_dir = dir.path().join(name);
            fs::create_dir_all(&lib_dir).unwrap();
            fs::write(
                lib_dir.join("MODULE.bazel"),
                format!("module(name = \"{}\", version = \"1.0.0\")", name),
            )
            .unwrap();
        }

        let overrides = vec![
            Override::LocalPath(LocalPathOverride {
                module_name: "lib_a".to_owned(),
                path: "lib_a".to_owned(),
            }),
            Override::LocalPath(LocalPathOverride {
                module_name: "lib_b".to_owned(),
                path: "lib_b".to_owned(),
            }),
        ];

        let resolved = resolve_local_modules(&overrides, dir.path()).unwrap();

        assert_eq!(resolved.modules.len(), 2);
        assert!(resolved.get("lib_a").is_some());
        assert!(resolved.get("lib_b").is_some());
    }

    #[test]
    fn test_to_cell_infos() {
        let dir = TempDir::new().unwrap();

        let lib_dir = dir.path().join("my_lib");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(
            lib_dir.join("MODULE.bazel"),
            "module(name = \"my_lib\", version = \"1.0.0\")",
        )
        .unwrap();

        let overrides = vec![Override::LocalPath(LocalPathOverride {
            module_name: "my_lib".to_owned(),
            path: "my_lib".to_owned(),
        })];

        let resolved = resolve_local_modules(&overrides, dir.path()).unwrap();
        let cell_infos = resolved.to_cell_infos();

        assert_eq!(cell_infos.len(), 1);
        assert_eq!(cell_infos[0].cell_name, "my_lib");
        assert_eq!(cell_infos[0].module_name.as_ref(), "my_lib");
        assert_eq!(cell_infos[0].path.as_ref(), "my_lib");
    }

    // ========================================================================
    // MVS Algorithm Tests
    // ========================================================================

    #[test]
    fn test_module_key_creation() {
        let key = ModuleKey::new("rules_cc", "0.0.9");
        assert_eq!(key.name, "rules_cc");
        assert_eq!(key.version, "0.0.9");
        assert_eq!(key.to_string(), "rules_cc@0.0.9");
    }

    #[test]
    fn test_module_key_from_dep() {
        let dep = BazelDep::new("rules_rust".to_string(), Version::parse("0.40.0").unwrap());
        let key = ModuleKey::from_dep(&dep);
        assert_eq!(key.name, "rules_rust");
        assert_eq!(key.version, "0.40.0");
    }

    #[test]
    fn test_selection_group_basic() {
        let group = SelectionGroup::new("my_module", 0);
        assert_eq!(group.module_name, "my_module");
        assert_eq!(group.compatibility_level, 0);
        assert!(group.target_allowed_version.is_none());
    }

    #[test]
    fn test_selection_group_with_target() {
        let target = Version::parse("2.0.0").unwrap();
        let group = SelectionGroup::with_target_version("my_module", 1, target.clone());
        assert_eq!(group.module_name, "my_module");
        assert_eq!(group.compatibility_level, 1);
        assert_eq!(group.target_allowed_version, Some(target));
    }

    #[test]
    fn test_selection_group_equality() {
        let group1 = SelectionGroup::new("foo", 0);
        let group2 = SelectionGroup::new("foo", 0);
        let group3 = SelectionGroup::new("foo", 1);
        let group4 = SelectionGroup::new("bar", 0);

        assert_eq!(group1, group2);
        assert_ne!(group1, group3); // Different compat level
        assert_ne!(group1, group4); // Different name
    }

    #[test]
    fn test_module_source_serialization() {
        // Test Registry source
        let registry_source = ModuleSource::Registry {
            url: "https://bcr.bazel.build".to_string(),
        };
        let json = serde_json::to_string(&registry_source).unwrap();
        assert!(json.contains("Registry"));
        assert!(json.contains("bcr.bazel.build"));

        // Test LocalPath source
        let local_source = ModuleSource::LocalPath {
            path: "../local_module".to_string(),
        };
        let json = serde_json::to_string(&local_source).unwrap();
        assert!(json.contains("LocalPath"));
        assert!(json.contains("../local_module"));

        // Test Git source
        let git_source = ModuleSource::Git {
            remote: "https://github.com/example/repo.git".to_string(),
            commit: "abc123".to_string(),
        };
        let json = serde_json::to_string(&git_source).unwrap();
        assert!(json.contains("Git"));
        assert!(json.contains("abc123"));

        // Test Archive source
        let archive_source = ModuleSource::Archive {
            urls: vec!["https://example.com/archive.tar.gz".to_string()],
        };
        let json = serde_json::to_string(&archive_source).unwrap();
        assert!(json.contains("Archive"));
    }

    #[test]
    fn test_resolved_graph_default() {
        let graph = ResolvedGraph::default();
        assert!(graph.selected_versions.is_empty());
        assert!(graph.modules.is_empty());
        assert!(graph.resolution_order.is_empty());
    }

    #[test]
    fn test_resolved_module_info_creation() {
        let info = ResolvedModuleInfo {
            name: "rules_cc".to_string(),
            version: "0.0.9".to_string(),
            compatibility_level: 0,
            dependencies: HashMap::from([("bazel_skylib".to_string(), "1.5.0".to_string())]),
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_string(),
            },
            source_path: None,
        };

        assert_eq!(info.name, "rules_cc");
        assert_eq!(info.version, "0.0.9");
        assert_eq!(info.dependencies.len(), 1);
        assert_eq!(info.dependencies.get("bazel_skylib"), Some(&"1.5.0".to_string()));
    }

    #[test]
    fn test_discovered_module() {
        let module = Module::new("test".to_string(), Version::parse("1.0.0").unwrap());
        let discovered = DiscoveredModule {
            key: ModuleKey::new("test", "1.0.0"),
            module: module.clone(),
            compatibility_level: 0,
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_string(),
            },
        };

        assert_eq!(discovered.key.name, "test");
        assert_eq!(discovered.compatibility_level, 0);
    }

    #[test]
    fn test_mvs_resolution_error_display() {
        let err = MvsResolutionError::CompatibilityConflict {
            name: "protobuf".to_string(),
            version1: "3.18.0".to_string(),
            compat1: 1,
            version2: "4.0.0".to_string(),
            compat2: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("protobuf"));
        assert!(msg.contains("3.18.0"));
        assert!(msg.contains("4.0.0"));
        assert!(msg.contains("compatibility_level"));
    }

    #[test]
    fn test_mvs_yanked_error() {
        let err = MvsResolutionError::YankedVersionSelected {
            name: "bad_module".to_string(),
            version: "1.0.0".to_string(),
            reason: "Security vulnerability".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bad_module"));
        assert!(msg.contains("Yanked"));
        assert!(msg.contains("Security vulnerability"));
    }

    #[test]
    fn test_mvs_version_not_allowed() {
        let err = MvsResolutionError::VersionNotAllowed {
            name: "foo".to_string(),
            version: "1.5.0".to_string(),
            allowed: vec!["1.0.0".to_string(), "2.0.0".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("foo"));
        assert!(msg.contains("1.5.0"));
        assert!(msg.contains("allowed versions"));
    }

    // Test MVS version selection logic
    #[test]
    fn test_version_selection_picks_max() {
        // Simulate what MVS does: pick the maximum version among candidates
        let versions = vec![
            Version::parse("1.0.0").unwrap(),
            Version::parse("1.2.0").unwrap(),
            Version::parse("1.1.0").unwrap(),
        ];

        let max = versions.into_iter().max().unwrap();
        assert_eq!(max.as_str(), "1.2.0");
    }

    #[test]
    fn test_version_selection_with_empty() {
        // Empty versions (from overrides) should win
        let versions = vec![
            Version::parse("999.999.999").unwrap(),
            Version::empty(),
            Version::parse("1.0.0").unwrap(),
        ];

        let max = versions.into_iter().max().unwrap();
        assert!(max.is_empty()); // Empty version wins
    }

    #[test]
    fn test_version_selection_with_prerelease() {
        // Prerelease should be less than release
        let versions = vec![
            Version::parse("1.0.0-alpha").unwrap(),
            Version::parse("1.0.0-beta").unwrap(),
            Version::parse("1.0.0").unwrap(),
        ];

        let max = versions.into_iter().max().unwrap();
        assert_eq!(max.as_str(), "1.0.0"); // Release wins over prerelease
    }
}
