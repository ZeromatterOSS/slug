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
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use kuro_error::BuckErrorContext;

use crate::parser::parse_module_bazel;
use crate::types::LocalPathOverride;
use crate::types::Module;
use crate::types::Override;
use crate::version::Version;

/// Errors that can occur during local module resolution.
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
}
