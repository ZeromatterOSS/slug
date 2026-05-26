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
use fxhash::FxHashMap;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use slug_error::BuckErrorContext;

use crate::cache::ModuleCache;
use crate::fetch::SourceFetcher;
use crate::lockfile::LockfileMode;
use crate::module_names::invalid_bazel_module_name_message;
use crate::module_names::is_valid_bazel_module_name;
use crate::parser::parse_non_root_module_bazel;
use crate::parser::parse_non_root_module_bazel_content;
use crate::registry::RegistryClient;
use crate::types::BazelDep;
use crate::types::LocalPathOverride;
use crate::types::Module;
use crate::types::MultipleVersionOverride;
use crate::types::Override;
use crate::types::SingleVersionOverride;
use crate::version::Version;

/// Errors that can occur during module resolution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
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
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
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
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
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
        "Yanked version detected in your resolved dependency graph: {name}@{version}, \
        for the reason: {reason}.\n\
        Yanked versions may contain serious vulnerabilities and should not be used. \
        To fix this, use a bazel_dep on a newer version of this module. To continue \
        using this version, allow it using the --allow_yanked_versions flag or the \
        BZLMOD_ALLOW_YANKED_VERSIONS env variable."
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

    #[error(
        "Missing checksum for registry file {url} not permitted with --lockfile_mode=error. \
        Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    )]
    MissingRegistryChecksum { url: String },

    #[error("Registry file checksum mismatch for {url}: expected {expected}, got {actual}")]
    RegistryChecksumMismatch {
        url: String,
        expected: String,
        actual: String,
    },
}

// ============================================================================
// MVS (Minimal Version Selection) Algorithm
// ============================================================================

/// A unique key identifying a module in the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Allocative)]
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

/// Bazel-shaped allow-list for yanked module versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedYankedVersions {
    /// Any selected yanked version is allowed.
    All,
    /// Only the listed module keys are allowed.
    Some(HashSet<ModuleKey>),
}

impl Default for AllowedYankedVersions {
    fn default() -> Self {
        Self::Some(HashSet::new())
    }
}

impl AllowedYankedVersions {
    fn allows(&self, key: &ModuleKey) -> bool {
        match self {
            Self::All => true,
            Self::Some(allowed) => allowed.contains(key),
        }
    }
}

/// Parse Bazel's `BZLMOD_ALLOW_YANKED_VERSIONS` env var plus every
/// `--allow_yanked_versions` occurrence.
pub fn parse_allowed_yanked_versions(
    from_env: Option<&str>,
    from_flags: &[String],
) -> slug_error::Result<AllowedYankedVersions> {
    let mut allowed = HashSet::new();

    if let Some(value) = from_env {
        if parse_allowed_yanked_versions_entry(value, &mut allowed, "environment variable")? {
            return Ok(AllowedYankedVersions::All);
        }
    }

    for value in from_flags {
        if parse_allowed_yanked_versions_entry(value, &mut allowed, "command line flag")? {
            return Ok(AllowedYankedVersions::All);
        }
    }

    Ok(AllowedYankedVersions::Some(allowed))
}

fn parse_allowed_yanked_versions_entry(
    value: &str,
    allowed: &mut HashSet<ModuleKey>,
    context: &str,
) -> slug_error::Result<bool> {
    for module in value.split(',') {
        if module == "all" {
            return Ok(true);
        }
        if module.is_empty() {
            continue;
        }

        let Some((name, version)) = module.split_once('@') else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Parsing {context} failed, module versions must be of the form '<module name>@<version>'"
            ));
        };
        if !is_valid_bazel_module_name(name) {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Parsing {context} failed, {}",
                invalid_bazel_module_name_message(name)
            ));
        }
        Version::parse(version).with_buck_error_context(|| {
            format!("Parsing {context} failed, invalid version specified for module: {version}")
        })?;
        allowed.insert(ModuleKey::new(name, version));
    }

    Ok(false)
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Allocative)]
pub enum ModuleSource {
    /// Module from a registry.
    Registry { url: String },
    /// Module from a local path override.
    LocalPath { path: String },
    /// Module from a git override.
    Git {
        remote: String,
        commit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        shallow_since: Option<String>,
        #[serde(default)]
        patches: Vec<String>,
        #[serde(default)]
        patch_strip: u32,
        /// Path where the git repo was cloned during resolution.
        #[serde(skip_serializing_if = "Option::is_none")]
        fetched_path: Option<PathBuf>,
    },
    /// Module from an archive override.
    Archive {
        urls: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        integrity: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strip_prefix: Option<String>,
        #[serde(default)]
        patches: Vec<String>,
        #[serde(default)]
        patch_strip: u32,
        /// Path where the archive was extracted during resolution.
        #[serde(skip_serializing_if = "Option::is_none")]
        fetched_path: Option<PathBuf>,
    },
}

/// Result of MVS resolution - the final resolved dependency graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedGraph {
    /// Map from module name to selected version.
    pub selected_versions: HashMap<String, String>,
    /// Full module information for each selected module.
    ///
    /// `FxHashMap` (fixed-seed, deterministic across invocations) so that
    /// iterating this map produces a stable order for the same content,
    /// without anyone having to sort on read. See Plan 21.2.
    pub modules: FxHashMap<String, ResolvedModuleInfo>,
    /// Resolution order (topological).
    pub resolution_order: Vec<String>,

    /// Registry file hashes collected during module file and source metadata
    /// resolution. Keys are registry file URLs, values are SRI SHA-256 hashes
    /// of the exact file bytes.
    #[serde(default)]
    pub registry_file_hashes: IndexMap<String, String>,

    /// Yanked selected module versions that were explicitly allowed for this
    /// resolution. Keys are `module@version`, values are registry reasons.
    #[serde(default)]
    pub selected_yanked_versions: IndexMap<String, String>,
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
    /// Registry file inputs observed during resolution.
    registry_file_hashes: IndexMap<String, String>,
    /// Bazel command/env yanked-version allow-list.
    allowed_yanked_versions: AllowedYankedVersions,
    /// Lockfile mode controls whether mutable yanked metadata is refreshed.
    lockfile_mode: LockfileMode,
    /// Whether root-module dev dependencies should be ignored for this command.
    ignore_dev_dependency: bool,
    /// Registry file hashes read from the current visible lockfile.
    known_registry_file_hashes: IndexMap<String, String>,
    /// Selected yanked versions read from the current visible lockfile.
    previously_selected_yanked_versions: IndexMap<String, String>,
    /// Root-local override patch files read by the DICE projection bridge.
    override_patch_inputs: Arc<crate::OverridePatchInputs>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum YankedVersionStatus {
    KnownNotYanked,
    KnownYanked(String),
    Unknown,
}

impl MvsResolver {
    /// Create a new MVS resolver with default BCR registry.
    pub async fn new(cache: ModuleCache) -> slug_error::Result<Self> {
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
            registry_file_hashes: IndexMap::new(),
            allowed_yanked_versions: AllowedYankedVersions::default(),
            lockfile_mode: LockfileMode::default(),
            ignore_dev_dependency: false,
            known_registry_file_hashes: IndexMap::new(),
            previously_selected_yanked_versions: IndexMap::new(),
            override_patch_inputs: Arc::new(crate::OverridePatchInputs::default()),
        })
    }

    /// Create a resolver with a custom registry URL.
    pub async fn with_registry(registry_url: &str, cache: ModuleCache) -> slug_error::Result<Self> {
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
            registry_file_hashes: IndexMap::new(),
            allowed_yanked_versions: AllowedYankedVersions::default(),
            lockfile_mode: LockfileMode::default(),
            ignore_dev_dependency: false,
            known_registry_file_hashes: IndexMap::new(),
            previously_selected_yanked_versions: IndexMap::new(),
            override_patch_inputs: Arc::new(crate::OverridePatchInputs::default()),
        })
    }

    /// Configure Bazel yanked-version policy for this command.
    pub fn set_yanked_version_policy(
        &mut self,
        allowed_yanked_versions: AllowedYankedVersions,
        lockfile_mode: LockfileMode,
        known_registry_file_hashes: IndexMap<String, String>,
        previously_selected_yanked_versions: IndexMap<String, String>,
    ) {
        self.allowed_yanked_versions = allowed_yanked_versions;
        self.lockfile_mode = lockfile_mode;
        self.known_registry_file_hashes = known_registry_file_hashes;
        self.previously_selected_yanked_versions = previously_selected_yanked_versions;
    }

    /// Configure Bazel's command-level dev-dependency policy.
    pub fn set_ignore_dev_dependency(&mut self, ignore_dev_dependency: bool) {
        self.ignore_dev_dependency = ignore_dev_dependency;
    }

    /// Configure DICE-tracked root-local override patch inputs for production resolution.
    pub fn set_override_patch_inputs(&mut self, inputs: Arc<crate::OverridePatchInputs>) {
        self.override_patch_inputs = inputs;
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
            if !sv.version.is_empty() {
                return sv.version.clone();
            }
        }
        dep.version.clone()
    }

    async fn registry_client_for_module(
        &self,
        module_name: &str,
    ) -> slug_error::Result<RegistryClient> {
        let registry = self
            .single_version_overrides
            .get(module_name)
            .and_then(|sv| sv.registry.as_deref())
            .or_else(|| {
                self.multiple_version_overrides
                    .get(module_name)
                    .and_then(|mv| mv.registry.as_deref())
            });

        match registry {
            Some(registry) if registry != self.registry_client.base_url() => {
                RegistryClient::new(registry, self.cache.clone()).await
            }
            _ => Ok(self.registry_client.clone()),
        }
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
    ) -> slug_error::Result<()> {
        let mut queue: VecDeque<(BazelDep, Option<PathBuf>)> = VecDeque::new();
        let mut visited: HashSet<ModuleKey> = HashSet::new();
        // Process overrides first
        self.process_overrides(&root.overrides);

        // Collect non-registry overrides first to avoid borrow issues
        let override_modules: Vec<_> = root
            .bazel_deps
            .iter()
            .filter(|dep| !(self.ignore_dev_dependency && dep.dev_dependency))
            .filter_map(|dep| {
                self.has_non_registry_override(&dep.name, &root.overrides)
                    .cloned()
            })
            .collect();

        // Resolve non-registry overrides
        for override_ in &override_modules {
            let name = match override_ {
                Override::LocalPath(lp) => &lp.module_name,
                Override::Git(g) => &g.module_name,
                Override::Archive(a) => &a.module_name,
                _ => "unknown",
            };
            self.resolve_override_module(override_, workspace_root)
                .await
                .with_buck_error_context(|| {
                    format!("Failed to resolve non-registry override module '{name}'")
                })?;
        }

        // Queue transitive deps of overridden modules (they won't be visited
        // during BFS since overridden modules are skipped in the queue).
        let overridden_deps: Vec<BazelDep> = self
            .overridden_modules
            .values()
            .flat_map(|discovered| {
                discovered
                    .module
                    .bazel_deps
                    .iter()
                    .filter(|d| !d.dev_dependency)
                    .filter(|d| !self.overridden_modules.contains_key(&d.name))
                    .map(|d| BazelDep {
                        name: d.name.clone(),
                        version: self.get_effective_version(d),
                        repo_name: d.repo_name.clone(),
                        dev_dependency: d.dev_dependency,
                    })
            })
            .collect();
        for dep in overridden_deps {
            queue.push_back((dep, None));
        }

        // Add root's direct dependencies to queue
        for dep in &root.bazel_deps {
            if self.ignore_dev_dependency && dep.dev_dependency {
                continue;
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
        &mut self,
        dep: &BazelDep,
    ) -> slug_error::Result<DiscoveredModule> {
        let version_str = dep.version.as_str();
        let key = ModuleKey::from_dep(dep);
        let registry_client = self.registry_client_for_module(&dep.name).await?;
        let registry_url = registry_client.base_url();
        let module_bazel_url = Self::module_bazel_url_for(registry_url, &key);

        tracing::debug!(
            "Fetching {}@{} from registry {}",
            dep.name,
            version_str,
            registry_url
        );

        self.validate_registry_file_hash_for_registry(registry_url, &module_bazel_url, None)?;

        // Fetch MODULE.bazel content
        let module_bazel_file = registry_client
            .fetch_module_bazel_file(&dep.name, version_str)
            .await
            .map_err(|e| MvsResolutionError::DependencyResolutionFailed {
                name: dep.name.clone(),
                version: version_str.to_string(),
                reason: format!("Failed to fetch MODULE.bazel: {}", e),
            })?;
        self.validate_registry_file_hash_for_registry(
            registry_url,
            &module_bazel_file.url,
            Some(&module_bazel_file.hash),
        )?;
        self.registry_file_hashes.insert(
            module_bazel_file.url.clone(),
            module_bazel_file.hash.clone(),
        );

        let module_bazel_content =
            if let Some(override_) = self.single_version_overrides.get(&dep.name) {
                crate::fetch::SourceFetcher::apply_single_version_module_patches_with_inputs(
                    &module_bazel_file.content,
                    &override_.patches,
                    override_.patch_strip,
                    &self.override_patch_inputs,
                )
                .with_buck_error_context(|| {
                    format!(
                        "Failed to apply single_version_override patches for {}@{}",
                        dep.name, version_str
                    )
                })?
            } else {
                module_bazel_file.content.clone()
            };

        // Parse MODULE.bazel
        let filename = format!("{}@{}/MODULE.bazel", dep.name, version_str);
        let parsed = parse_non_root_module_bazel_content(&module_bazel_content, &filename)
            .map_err(|e| MvsResolutionError::DependencyResolutionFailed {
                name: dep.name.clone(),
                version: version_str.to_string(),
                reason: format!("Failed to parse MODULE.bazel: {}", e),
            })?;

        Ok(DiscoveredModule {
            key,
            compatibility_level: parsed.module.compatibility_level,
            module: parsed.module,
            source: ModuleSource::Registry {
                url: registry_url.to_string(),
            },
        })
    }

    /// Resolve a module with a non-registry override.
    async fn resolve_override_module(
        &mut self,
        override_: &Override,
        workspace_root: &Path,
    ) -> slug_error::Result<()> {
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
                // Fetch the git repo to a cache directory and parse its MODULE.bazel
                let patch_digest =
                    crate::fetch::SourceFetcher::local_override_patch_digest_with_inputs(
                        &g.patches,
                        g.patch_strip,
                        &self.override_patch_inputs,
                    )
                    .with_buck_error_context(|| {
                        format!(
                            "Failed to fingerprint patches for git override '{}'",
                            g.module_name
                        )
                    })?;
                let dest_dir = self
                    .cache
                    .git_override_dir_with_patch_digest(g, patch_digest.as_deref());
                let complete_marker = dest_dir.join(".complete");

                if !complete_marker.exists() {
                    tracing::info!(
                        "Fetching git override for {} from {} at {}",
                        g.module_name,
                        g.remote,
                        g.commit
                    );
                    if dest_dir.exists() {
                        let _ = std::fs::remove_dir_all(&dest_dir);
                    }
                    std::fs::create_dir_all(&dest_dir)
                        .buck_error_context("Failed to create git override dir")?;

                    let source_info = crate::registry::SourceInfo {
                        source_type: Some("git_repository".to_string()),
                        url: None,
                        urls: None,
                        integrity: None,
                        strip_prefix: None,
                        overlay: crate::registry::RegistryFileMap::new(),
                        patches: crate::registry::RegistryFileMap::new(),
                        patch_strip: g.patch_strip,
                        remote: Some(g.remote.clone()),
                        commit: Some(g.commit.clone()),
                        shallow_since: g.shallow_since.clone(),
                    };

                    self.source_fetcher
                        .fetch_git_direct(&source_info, &dest_dir)
                        .await?;
                    crate::fetch::SourceFetcher::apply_local_override_patches_with_inputs(
                        &dest_dir,
                        &g.patches,
                        g.patch_strip,
                        &self.override_patch_inputs,
                    )
                    .with_buck_error_context(|| {
                        format!(
                            "Failed to apply patches for git override '{}'",
                            g.module_name
                        )
                    })?;
                    std::fs::write(&complete_marker, "")
                        .buck_error_context("Failed to write git override marker")?;
                } else {
                    tracing::debug!("Using cached git override for {}", g.module_name);
                }

                // Parse MODULE.bazel from the fetched source
                let module_bazel_path = dest_dir.join("MODULE.bazel");
                let parsed_module = if module_bazel_path.exists() {
                    let parsed = parse_non_root_module_bazel(&module_bazel_path)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse MODULE.bazel for git override '{}'",
                                g.module_name
                            )
                        })?;
                    parsed.module
                } else {
                    let mut module = Module::empty();
                    module.name = g.module_name.clone();
                    module
                };

                let version = Version::empty();
                let discovered = DiscoveredModule {
                    key: ModuleKey::new(&g.module_name, version.as_str()),
                    compatibility_level: parsed_module.compatibility_level,
                    module: parsed_module,
                    source: ModuleSource::Git {
                        remote: g.remote.clone(),
                        commit: g.commit.clone(),
                        shallow_since: g.shallow_since.clone(),
                        patches: g.patches.clone(),
                        patch_strip: g.patch_strip,
                        fetched_path: Some(dest_dir),
                    },
                };
                self.overridden_modules
                    .insert(g.module_name.clone(), discovered);
            }
            Override::Archive(a) => {
                // Fetch the archive to a cache directory and parse its MODULE.bazel
                let patch_digest =
                    crate::fetch::SourceFetcher::local_override_patch_digest_with_inputs(
                        &a.patches,
                        a.patch_strip,
                        &self.override_patch_inputs,
                    )
                    .with_buck_error_context(|| {
                        format!(
                            "Failed to fingerprint patches for archive override '{}'",
                            a.module_name
                        )
                    })?;
                let dest_dir = self
                    .cache
                    .archive_override_dir_with_patch_digest(a, patch_digest.as_deref());
                let complete_marker = dest_dir.join(".complete");

                if !complete_marker.exists() {
                    tracing::info!(
                        "Fetching archive override for {} from {:?}",
                        a.module_name,
                        a.urls
                    );
                    if dest_dir.exists() {
                        let _ = std::fs::remove_dir_all(&dest_dir);
                    }
                    std::fs::create_dir_all(&dest_dir)
                        .buck_error_context("Failed to create archive override dir")?;

                    self.source_fetcher
                        .fetch_archive_direct(
                            &a.urls,
                            a.integrity.as_deref(),
                            a.strip_prefix.as_deref(),
                            &dest_dir,
                        )
                        .await?;
                    crate::fetch::SourceFetcher::apply_local_override_patches_with_inputs(
                        &dest_dir,
                        &a.patches,
                        a.patch_strip,
                        &self.override_patch_inputs,
                    )
                    .with_buck_error_context(|| {
                        format!(
                            "Failed to apply patches for archive override '{}'",
                            a.module_name
                        )
                    })?;
                    std::fs::write(&complete_marker, "")
                        .buck_error_context("Failed to write archive override marker")?;
                } else {
                    tracing::debug!("Using cached archive override for {}", a.module_name);
                }

                // Parse MODULE.bazel from the fetched source
                let module_bazel_path = dest_dir.join("MODULE.bazel");
                let parsed_module = if module_bazel_path.exists() {
                    let parsed = parse_non_root_module_bazel(&module_bazel_path)
                        .with_buck_error_context(|| {
                            format!(
                                "Failed to parse MODULE.bazel for archive override '{}'",
                                a.module_name
                            )
                        })?;
                    parsed.module
                } else {
                    let mut module = Module::empty();
                    module.name = a.module_name.clone();
                    module
                };

                let version = Version::empty();
                let discovered = DiscoveredModule {
                    key: ModuleKey::new(&a.module_name, version.as_str()),
                    compatibility_level: parsed_module.compatibility_level,
                    module: parsed_module,
                    source: ModuleSource::Archive {
                        urls: a.urls.clone(),
                        integrity: a.integrity.clone(),
                        strip_prefix: a.strip_prefix.clone(),
                        patches: a.patches.clone(),
                        patch_strip: a.patch_strip,
                        fetched_path: Some(dest_dir),
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
    fn select_versions(&self) -> slug_error::Result<HashMap<String, Version>> {
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

        // Check for compatibility conflicts (same module name, different compat levels).
        // Bazel rejects these unless a multiple_version_override explicitly allows
        // the split.
        Self::check_compatibility_conflicts(&self.multiple_version_overrides, &selection_groups)?;

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

            // When multiple groups have the same key (e.g., compat level conflict),
            // keep the highest version
            match selected.entry(key) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(max_version);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if max_version > *e.get() {
                        e.insert(max_version);
                    }
                }
            }
        }

        // Add overridden modules (they always "win")
        for (name, discovered) in &self.overridden_modules {
            let version =
                Version::parse(&discovered.key.version).unwrap_or_else(|_| Version::empty());
            selected.insert(name.clone(), version);
        }

        Ok(selected)
    }

    /// Find the target allowed version for multiple_version_override.
    fn find_target_allowed_version(
        &self,
        requested: &Version,
        mv: &MultipleVersionOverride,
    ) -> slug_error::Result<Version> {
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
    ///
    /// Bazel rejects selected versions of the same module with different
    /// compatibility levels unless a multiple_version_override explicitly
    /// allows multiple selected versions.
    fn check_compatibility_conflicts(
        multiple_version_overrides: &HashMap<String, MultipleVersionOverride>,
        groups: &HashMap<SelectionGroup, Vec<(Version, &DiscoveredModule)>>,
    ) -> slug_error::Result<()> {
        // Group by module name to check for compat conflicts
        let mut by_name: HashMap<&str, Vec<&SelectionGroup>> = HashMap::new();
        for group in groups.keys() {
            by_name.entry(&group.module_name).or_default().push(group);
        }

        for (name, name_groups) in by_name {
            // Skip if there's a multiple_version_override for this module
            if multiple_version_overrides.contains_key(name) {
                continue;
            }

            // Check if all groups have the same compatibility level
            let compat_levels: HashSet<_> =
                name_groups.iter().map(|g| g.compatibility_level).collect();

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
                    name: name.to_owned(),
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
    ) -> slug_error::Result<ResolvedGraph> {
        let mut modules: FxHashMap<String, ResolvedModuleInfo> = FxHashMap::default();
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
            registry_file_hashes: self.registry_file_hashes.clone(),
            selected_yanked_versions: IndexMap::new(),
        })
    }

    async fn check_yanked_versions(
        &self,
        graph: &ResolvedGraph,
    ) -> slug_error::Result<IndexMap<String, String>> {
        let mut selected_yanked_versions = IndexMap::new();

        for info in graph.modules.values() {
            if !matches!(info.source, ModuleSource::Registry { .. }) {
                continue;
            }

            let ModuleSource::Registry { url } = &info.source else {
                continue;
            };

            let key = ModuleKey::new(&info.name, &info.version);
            let source_json_url = Self::source_json_url_for(url, &key);
            let reason = match self.yanked_reason_from_lockfile(&source_json_url, &key) {
                YankedVersionStatus::KnownNotYanked => None,
                YankedVersionStatus::KnownYanked(reason) => Some(reason),
                YankedVersionStatus::Unknown => {
                    let registry_client = if url == self.registry_client.base_url() {
                        self.registry_client.clone()
                    } else {
                        RegistryClient::new(url, self.cache.clone()).await?
                    };
                    match registry_client.fetch_metadata(&info.name).await {
                        Ok(metadata) => metadata.yanked_versions.get(&info.version).cloned(),
                        Err(e) => {
                            tracing::warn!(
                                "Could not read metadata file for module {} from registry {}: {}",
                                info.name,
                                url,
                                e
                            );
                            None
                        }
                    }
                }
            };

            let Some(reason) = reason else {
                continue;
            };
            if self.allowed_yanked_versions.allows(&key) {
                selected_yanked_versions.insert(key.to_string(), reason);
                continue;
            }

            return Err(MvsResolutionError::YankedVersionSelected {
                name: key.name,
                version: key.version,
                reason,
            }
            .into());
        }

        Ok(selected_yanked_versions)
    }

    fn yanked_reason_from_lockfile(
        &self,
        source_json_url: &str,
        key: &ModuleKey,
    ) -> YankedVersionStatus {
        yanked_reason_from_lockfile_facts(
            self.lockfile_mode,
            &self.known_registry_file_hashes,
            &self.previously_selected_yanked_versions,
            source_json_url,
            key,
        )
    }

    fn source_json_url_for(registry_url: &str, key: &ModuleKey) -> String {
        format!(
            "{}/modules/{}/{}/source.json",
            registry_url, key.name, key.version
        )
    }

    fn module_bazel_url_for(registry_url: &str, key: &ModuleKey) -> String {
        format!(
            "{}/modules/{}/{}/MODULE.bazel",
            registry_url, key.name, key.version
        )
    }

    fn validate_registry_file_hash_for_registry(
        &self,
        registry_base_url: &str,
        url: &str,
        actual_hash: Option<&str>,
    ) -> slug_error::Result<()> {
        validate_registry_file_hash_facts(
            self.lockfile_mode,
            registry_base_url,
            &self.known_registry_file_hashes,
            url,
            actual_hash,
        )
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
    ) -> slug_error::Result<ResolvedGraph> {
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
        let mut graph = self.build_resolved_graph(&selected).await?;
        graph.selected_yanked_versions = self.check_yanked_versions(&graph).await?;

        tracing::info!(
            "MVS resolution complete: {} modules in final graph",
            graph.modules.len()
        );

        Ok(graph)
    }

    /// Fetch sources for all resolved modules.
    ///
    /// This downloads and extracts sources for modules that don't have local overrides.
    pub async fn fetch_sources(&self, graph: &mut ResolvedGraph) -> slug_error::Result<()> {
        let mut first_error = None;

        for (name, info) in &mut graph.modules {
            let source_path = match &info.source {
                ModuleSource::Registry { url } => {
                    // Fetch from registry
                    let result = async {
                        let key = ModuleKey::new(name.as_str(), info.version.as_str());
                        let source_json_url = Self::source_json_url_for(url, &key);
                        self.validate_registry_file_hash_for_registry(url, &source_json_url, None)?;

                        let registry_client = if url == self.registry_client.base_url() {
                            self.registry_client.clone()
                        } else {
                            RegistryClient::new(url, self.cache.clone()).await?
                        };

                        let source_info_file = registry_client
                            .fetch_source_info_file(name, &info.version)
                            .await?;
                        self.validate_registry_file_hash_for_registry(
                            url,
                            &source_info_file.file.url,
                            Some(&source_info_file.file.hash),
                        )?;

                        let single_version_override = self.single_version_overrides.get(name);
                        let source_identity = match single_version_override {
                            Some(override_) => {
                                crate::fetch::SourceFetcher::local_override_patch_effect_digest_with_inputs(
                                    &override_.patches,
                                    override_.patch_strip,
                                    &override_.patch_cmds,
                                    &self.override_patch_inputs,
                                )?
                            }
                            None => None,
                        };
                        let source_was_complete = source_identity.as_deref().is_some_and(|identity| {
                            self.cache.is_source_complete_with_identity(
                                registry_client.base_url(),
                                name,
                                &info.version,
                                Some(identity),
                            )
                        });

                        let source_path = self
                            .source_fetcher
                            .fetch_source_with_identity(
                                registry_client.base_url(),
                                name,
                                &info.version,
                                &source_info_file.source_info,
                                source_identity.as_deref(),
                            )
                            .await?;
                        if let (Some(override_), Some(identity)) =
                            (single_version_override, source_identity.as_deref())
                        {
                            if !source_was_complete {
                                let complete_marker = source_path.join(".complete");
                                match std::fs::remove_file(&complete_marker) {
                                    Ok(()) => {}
                                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                                    Err(e) => {
                                        return Err(e).with_buck_error_context(|| {
                                            format!(
                                                "Failed to clear completion marker for patched source {}@{}",
                                                name, info.version
                                            )
                                        });
                                    }
                                }
                                crate::fetch::SourceFetcher::apply_local_override_patches_with_inputs(
                                    &source_path,
                                    &override_.patches,
                                    override_.patch_strip,
                                    &self.override_patch_inputs,
                                )
                                .with_buck_error_context(|| {
                                    format!(
                                        "Failed to apply single_version_override patches to {}@{} source",
                                        name, info.version
                                    )
                                })?;
                                crate::fetch::SourceFetcher::apply_local_override_patch_cmds(
                                    &source_path,
                                    name,
                                    &override_.patch_cmds,
                                )
                                .with_buck_error_context(|| {
                                    format!(
                                        "Failed to run single_version_override patch_cmds for {}@{} source",
                                        name, info.version
                                    )
                                })?;
                                self.cache.mark_source_complete_with_identity(
                                    registry_client.base_url(),
                                    name,
                                    &info.version,
                                    Some(identity),
                                )?;
                            }
                        }

                        Ok::<_, slug_error::Error>((source_path, source_info_file.file))
                    }
                    .await;

                    match result {
                        Ok((path, registry_file)) => {
                            graph
                                .registry_file_hashes
                                .insert(registry_file.url, registry_file.hash);
                            Some(path)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to fetch source for {}@{}: {}",
                                name,
                                info.version,
                                e
                            );
                            if first_error.is_none() {
                                first_error = Some(e);
                            }
                            None
                        }
                    }
                }
                ModuleSource::LocalPath { path } => {
                    // Local path is already available
                    Some(PathBuf::from(path))
                }
                ModuleSource::Git { fetched_path, .. } => {
                    if let Some(path) = fetched_path {
                        Some(path.clone())
                    } else {
                        tracing::warn!(
                            "Git override for {} has no fetched path (should have been fetched during resolution)",
                            name
                        );
                        None
                    }
                }
                ModuleSource::Archive { fetched_path, .. } => {
                    if let Some(path) = fetched_path {
                        Some(path.clone())
                    } else {
                        tracing::warn!(
                            "Archive override for {} has no fetched path (should have been fetched during resolution)",
                            name
                        );
                        None
                    }
                }
            };

            if let Some(source_path) = source_path {
                info.source_path = Some(source_path);
            }
        }

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

fn yanked_reason_from_lockfile_facts(
    lockfile_mode: LockfileMode,
    known_registry_file_hashes: &IndexMap<String, String>,
    previously_selected_yanked_versions: &IndexMap<String, String>,
    source_json_url: &str,
    key: &ModuleKey,
) -> YankedVersionStatus {
    if lockfile_mode == LockfileMode::Refresh {
        return YankedVersionStatus::Unknown;
    }

    if let Some(reason) = previously_selected_yanked_versions.get(&key.to_string()) {
        return YankedVersionStatus::KnownYanked(reason.clone());
    }

    if known_registry_file_hashes.contains_key(source_json_url) {
        return YankedVersionStatus::KnownNotYanked;
    }

    YankedVersionStatus::Unknown
}

fn validate_registry_file_hash_facts(
    lockfile_mode: LockfileMode,
    registry_base_url: &str,
    known_registry_file_hashes: &IndexMap<String, String>,
    url: &str,
    actual_hash: Option<&str>,
) -> slug_error::Result<()> {
    if registry_base_url.starts_with("file:") {
        return Ok(());
    }

    let expected_hash = known_registry_file_hashes.get(url);

    if lockfile_mode == LockfileMode::Error && expected_hash.is_none() {
        return Err(MvsResolutionError::MissingRegistryChecksum {
            url: url.to_owned(),
        }
        .into());
    }

    if let (Some(expected), Some(actual)) = (expected_hash, actual_hash) {
        if expected != actual {
            return Err(MvsResolutionError::RegistryChecksumMismatch {
                url: url.to_owned(),
                expected: expected.clone(),
                actual: actual.to_owned(),
            }
            .into());
        }
    }

    Ok(())
}

// ============================================================================
// Lockfile-Integrated Resolution
// ============================================================================

/// Resolve dependencies with lockfile support.
///
/// This is the main entry point for bzlmod resolution. It:
/// 1. Checks for an existing lockfile and validates it
/// 2. Uses the lockfile if valid (fast path)
/// 3. Runs MVS resolution if lockfile is invalid or missing
/// 4. Leaves `MODULE.bazel.lock` untouched during ordinary builds
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
    _module_bazel_path: &Path,
    mode: LockfileMode,
) -> slug_error::Result<ResolvedGraph> {
    // In Bazel 9.0+ format, the lockfile no longer caches the module dependency graph.
    // Module resolution always runs fresh (it's fast - just MODULE.bazel parsing + MVS).
    // Slug may read Bazel-authored lockfile data elsewhere for extension cache
    // hits and startup-time spoke pre-seeding, but build-time resolution must
    // not create or rewrite `MODULE.bazel.lock`.
    let _ = mode;

    // Always resolve fresh
    let cache = ModuleCache::new()?;
    let mut resolver = MvsResolver::new(cache).await?;
    let graph = resolver.resolve(root, workspace_root).await?;

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
) -> slug_error::Result<ResolvedLocalModule> {
    // Resolve the path relative to workspace root
    let module_path = workspace_root.join(&override_info.path);

    // Verify the path exists
    if !module_path.exists() {
        return Err(LocalResolutionError::PathNotFound(override_info.path.clone()).into());
    }

    // Look for MODULE.bazel in the local module
    let module_bazel_path = module_path.join("MODULE.bazel");

    let (parsed_module, has_module_file) = if module_bazel_path.exists() {
        let parsed =
            parse_non_root_module_bazel(&module_bazel_path).with_buck_error_context(|| {
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
/// use slug_bzlmod::resolution::resolve_local_modules;
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
) -> slug_error::Result<ResolvedLocalModules> {
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
                    let nested_resolved = resolve_local_override(local, &resolved.absolute_path)?;
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
    /// in the Slug cell resolver.
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
    pub async fn new(cache: ModuleCache) -> slug_error::Result<Self> {
        let registry_client = RegistryClient::bcr(cache.clone()).await?;
        let source_fetcher = SourceFetcher::new(cache).await?;

        Ok(Self {
            registry_client,
            source_fetcher,
        })
    }

    /// Create a resolver with a custom registry URL.
    pub async fn with_registry(registry_url: &str, cache: ModuleCache) -> slug_error::Result<Self> {
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
    ) -> slug_error::Result<ResolvedRemoteModule> {
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
                tracing::error!(
                    "Failed to fetch MODULE.bazel for {}@{}: {}",
                    name,
                    version_str,
                    e
                );
                RemoteResolutionError::FetchFailed {
                    name: name.clone(),
                    version: version_str.to_string(),
                }
            })?;

        // Parse the MODULE.bazel
        let filename = format!("{}@{}/MODULE.bazel", name, version_str);
        let parsed = parse_non_root_module_bazel_content(&module_bazel_content, &filename)
            .map_err(|e| {
                tracing::error!(
                    "Failed to parse MODULE.bazel for {}@{}: {}",
                    name,
                    version_str,
                    e
                );
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
                tracing::error!(
                    "Failed to fetch source.json for {}@{}: {}",
                    name,
                    version_str,
                    e
                );
                RemoteResolutionError::FetchFailed {
                    name: name.clone(),
                    version: version_str.to_string(),
                }
            })?;

        // Download and extract source
        let source_path = self
            .source_fetcher
            .fetch_source(
                self.registry_client.base_url(),
                name,
                version_str,
                &source_info,
            )
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
    ) -> slug_error::Result<ResolvedRemoteModules> {
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
) -> slug_error::Result<(ResolvedLocalModules, ResolvedRemoteModules)> {
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
    use std::fs;

    use tempfile::TempDir;

    use super::*;

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
        fs::write(local_lib_dir.join("BUILD.bazel"), "# Build targets here").unwrap();

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
    fn allowed_yanked_versions_parse_unions_env_and_flags() {
        let flags = vec![
            "beta@2.0.0,,gamma@3.0.0".to_owned(),
            "delta@4.0.0".to_owned(),
        ];
        let parsed = parse_allowed_yanked_versions(Some("alpha@1.0.0"), &flags).unwrap();
        let AllowedYankedVersions::Some(allowed) = parsed else {
            panic!("expected an explicit allow-list");
        };
        assert!(allowed.contains(&ModuleKey::new("alpha", "1.0.0")));
        assert!(allowed.contains(&ModuleKey::new("beta", "2.0.0")));
        assert!(allowed.contains(&ModuleKey::new("gamma", "3.0.0")));
        assert!(allowed.contains(&ModuleKey::new("delta", "4.0.0")));
    }

    #[test]
    fn allowed_yanked_versions_all_disables_checking() {
        let parsed =
            parse_allowed_yanked_versions(Some("alpha@1.0.0"), &["beta@2.0.0,all".to_owned()])
                .unwrap();
        assert_eq!(parsed, AllowedYankedVersions::All);
    }

    #[test]
    fn allowed_yanked_versions_rejects_bad_format_and_module_name() {
        let bad_format =
            parse_allowed_yanked_versions(None, &["alpha+1.0.0".to_owned()]).unwrap_err();
        assert!(
            bad_format
                .to_string()
                .contains("module versions must be of the form")
        );

        let bad_name =
            parse_allowed_yanked_versions(None, &["Alpha@1.0.0".to_owned()]).unwrap_err();
        assert!(bad_name.to_string().contains("invalid module name"));
    }

    #[test]
    fn lockfile_yanked_facts_follow_bazel_refresh_and_source_hash_rules() {
        let key = ModuleKey::new("alpha", "1.0.0");
        let source_json = "https://bcr.bazel.build/modules/alpha/1.0.0/source.json";
        let mut known_hashes = IndexMap::new();
        known_hashes.insert(source_json.to_owned(), "sha256-deadbeef".to_owned());
        let mut selected_yanked = IndexMap::new();
        selected_yanked.insert(key.to_string(), "security issue".to_owned());

        assert_eq!(
            yanked_reason_from_lockfile_facts(
                LockfileMode::Update,
                &known_hashes,
                &selected_yanked,
                source_json,
                &key,
            ),
            YankedVersionStatus::KnownYanked("security issue".to_owned())
        );
        assert_eq!(
            yanked_reason_from_lockfile_facts(
                LockfileMode::Update,
                &known_hashes,
                &IndexMap::new(),
                source_json,
                &key,
            ),
            YankedVersionStatus::KnownNotYanked
        );
        assert_eq!(
            yanked_reason_from_lockfile_facts(
                LockfileMode::Refresh,
                &known_hashes,
                &selected_yanked,
                source_json,
                &key,
            ),
            YankedVersionStatus::Unknown
        );
    }

    #[test]
    fn registry_checksum_error_mode_requires_known_http_hash() {
        let url = "https://bcr.bazel.build/modules/alpha/1.0.0/MODULE.bazel";
        let err = validate_registry_file_hash_facts(
            LockfileMode::Error,
            "https://bcr.bazel.build",
            &IndexMap::new(),
            url,
            None,
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("Missing checksum for registry file"));
        assert!(message.contains("--lockfile_mode=error"));
        assert!(message.contains("bazel mod deps --lockfile_mode=update"));
    }

    #[test]
    fn registry_checksum_error_mode_ignores_file_registry_missing_hash() {
        validate_registry_file_hash_facts(
            LockfileMode::Error,
            "file:///tmp/registry",
            &IndexMap::new(),
            "file:///tmp/registry/modules/alpha/1.0.0/MODULE.bazel",
            None,
        )
        .unwrap();
    }

    #[test]
    fn registry_checksum_update_mode_allows_missing_hash() {
        validate_registry_file_hash_facts(
            LockfileMode::Update,
            "https://bcr.bazel.build",
            &IndexMap::new(),
            "https://bcr.bazel.build/modules/alpha/1.0.0/source.json",
            None,
        )
        .unwrap();
    }

    #[test]
    fn registry_checksum_known_hash_mismatch_fails() {
        let url = "https://bcr.bazel.build/modules/alpha/1.0.0/source.json";
        let mut known_hashes = IndexMap::new();
        known_hashes.insert(url.to_owned(), "sha256-expected".to_owned());

        let err = validate_registry_file_hash_facts(
            LockfileMode::Update,
            "https://bcr.bazel.build",
            &known_hashes,
            url,
            Some("sha256-actual"),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("checksum mismatch"));
        assert!(message.contains("sha256-expected"));
        assert!(message.contains("sha256-actual"));
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
    fn compatibility_conflicts_fail_without_multiple_version_override() {
        let module_v1 = Module::new("dep".to_owned(), Version::parse("1.0.0").unwrap());
        let module_v2 = Module::new("dep".to_owned(), Version::parse("2.0.0").unwrap());
        let discovered_v1 = DiscoveredModule {
            key: ModuleKey::new("dep", "1.0.0"),
            module: module_v1,
            compatibility_level: 1,
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_owned(),
            },
        };
        let discovered_v2 = DiscoveredModule {
            key: ModuleKey::new("dep", "2.0.0"),
            module: module_v2,
            compatibility_level: 2,
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_owned(),
            },
        };
        let mut groups = HashMap::new();
        groups.insert(
            SelectionGroup::new("dep", 1),
            vec![(Version::parse("1.0.0").unwrap(), &discovered_v1)],
        );
        groups.insert(
            SelectionGroup::new("dep", 2),
            vec![(Version::parse("2.0.0").unwrap(), &discovered_v2)],
        );

        let err = MvsResolver::check_compatibility_conflicts(&HashMap::new(), &groups).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("dep"));
        assert!(message.contains("compatibility_level"));
    }

    #[test]
    fn compatibility_conflicts_are_allowed_by_multiple_version_override() {
        let module_v1 = Module::new("dep".to_owned(), Version::parse("1.0.0").unwrap());
        let module_v2 = Module::new("dep".to_owned(), Version::parse("2.0.0").unwrap());
        let discovered_v1 = DiscoveredModule {
            key: ModuleKey::new("dep", "1.0.0"),
            module: module_v1,
            compatibility_level: 1,
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_owned(),
            },
        };
        let discovered_v2 = DiscoveredModule {
            key: ModuleKey::new("dep", "2.0.0"),
            module: module_v2,
            compatibility_level: 2,
            source: ModuleSource::Registry {
                url: "https://bcr.bazel.build".to_owned(),
            },
        };
        let mut groups = HashMap::new();
        groups.insert(
            SelectionGroup::new("dep", 1),
            vec![(Version::parse("1.0.0").unwrap(), &discovered_v1)],
        );
        groups.insert(
            SelectionGroup::new("dep", 2),
            vec![(Version::parse("2.0.0").unwrap(), &discovered_v2)],
        );
        let overrides = HashMap::from([(
            "dep".to_owned(),
            MultipleVersionOverride {
                module_name: "dep".to_owned(),
                versions: vec![
                    Version::parse("1.0.0").unwrap(),
                    Version::parse("2.0.0").unwrap(),
                ],
                registry: None,
            },
        )]);

        MvsResolver::check_compatibility_conflicts(&overrides, &groups).unwrap();
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
            shallow_since: None,
            patches: Vec::new(),
            patch_strip: 0,
            fetched_path: None,
        };
        let json = serde_json::to_string(&git_source).unwrap();
        assert!(json.contains("Git"));
        assert!(json.contains("abc123"));

        // Test Archive source
        let archive_source = ModuleSource::Archive {
            urls: vec!["https://example.com/archive.tar.gz".to_string()],
            integrity: None,
            strip_prefix: None,
            patches: Vec::new(),
            patch_strip: 0,
            fetched_path: None,
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
        assert_eq!(
            info.dependencies.get("bazel_skylib"),
            Some(&"1.5.0".to_string())
        );
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
