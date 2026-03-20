/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Synthetic repository types for module extensions.
//!
//! Previously this module contained hardcoded Rust implementations for known
//! module extensions (bazel_features, rules_cc, rules_rust, etc.). Those have
//! been removed — all extensions now execute via DICE-based Starlark evaluation.
//!
//! This module retains only the `SyntheticRepo` struct and materialization
//! function, which are still referenced by the cell registration pipeline.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;

use crate::types::ParsedModuleFile;

/// A synthetic repository generated for a known extension.
#[derive(Debug, Clone)]
pub struct SyntheticRepo {
    /// Repository name (e.g., "bazel_features_version").
    pub name: String,

    /// Files to create in the repository.
    /// Key is the relative path, value is the content.
    pub files: HashMap<String, String>,
}

/// Collect all extension-generated repos needed by parsed modules.
///
/// All extensions now execute via DICE, so this always returns an empty Vec.
pub fn collect_synthetic_repos(
    _parsed_modules: &[(String, ParsedModuleFile)],
) -> Vec<SyntheticRepo> {
    Vec::new()
}

/// Collect synthetic repos with access to the project root for reading workspace files.
///
/// All extensions now execute via DICE, so this always returns an empty Vec.
pub fn collect_synthetic_repos_with_root(
    _parsed_modules: &[(String, ParsedModuleFile)],
    _project_root: Option<&Path>,
) -> Vec<SyntheticRepo> {
    Vec::new()
}

/// Materialize synthetic repos to the filesystem.
///
/// Creates the repo directories and writes all files.
pub fn materialize_synthetic_repos(
    repos: &[SyntheticRepo],
    base_dir: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for repo in repos {
        let repo_path = base_dir.join(&repo.name);
        fs::create_dir_all(&repo_path).with_context(|| {
            format!("Failed to create synthetic repo directory: {:?}", repo_path)
        })?;

        for (file_path, content) in &repo.files {
            let full_path = repo_path.join(file_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            // Skip writing if content is unchanged to avoid spurious mtime updates.
            let should_write = match fs::read_to_string(&full_path) {
                Ok(existing) => existing != *content,
                Err(_) => true,
            };
            if should_write {
                let mut file = fs::File::create(&full_path)
                    .with_context(|| format!("Failed to create file: {:?}", full_path))?;
                file.write_all(content.as_bytes())?;
            }
        }

        paths.push(repo_path);
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_synthetic_repos_returns_empty() {
        let repos = collect_synthetic_repos(&[]);
        assert!(repos.is_empty());
    }

    #[test]
    fn test_materialize_synthetic_repos() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut files = HashMap::new();
        files.insert("BUILD.bazel".to_string(), "# test\n".to_string());
        let repos = vec![SyntheticRepo {
            name: "test_repo".to_string(),
            files,
        }];

        let paths = materialize_synthetic_repos(&repos, temp_dir.path()).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(temp_dir.path().join("test_repo/BUILD.bazel").exists());
    }
}
