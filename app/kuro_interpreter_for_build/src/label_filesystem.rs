/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

//! Shared label-to-filesystem resolution for external loading contexts.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use kuro_bzlmod::CanonicalLabel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootLabelResolution {
    /// Preserve the historical repository_ctx.path behavior: root-repo labels
    /// return workspace/project-relative fragments and RepositoryPath anchors
    /// them later.
    Relative,
    /// Return absolute paths rooted at the project root when it is known.
    ProjectAbsolute,
}

pub(crate) struct LabelFilesystemResolver<'a> {
    workspace_root: &'a Path,
    project_root: Option<&'a Path>,
    cell_paths: Option<&'a HashMap<String, PathBuf>>,
    root_label_resolution: RootLabelResolution,
}

impl<'a> LabelFilesystemResolver<'a> {
    pub(crate) fn new(workspace_root: &'a Path) -> Self {
        Self {
            workspace_root,
            project_root: None,
            cell_paths: None,
            root_label_resolution: RootLabelResolution::Relative,
        }
    }

    pub(crate) fn with_project_root(mut self, project_root: Option<&'a Path>) -> Self {
        self.project_root = project_root;
        self
    }

    pub(crate) fn with_cell_paths(mut self, cell_paths: &'a HashMap<String, PathBuf>) -> Self {
        self.cell_paths = Some(cell_paths);
        self
    }

    pub(crate) fn with_root_label_resolution(mut self, mode: RootLabelResolution) -> Self {
        self.root_label_resolution = mode;
        self
    }

    pub(crate) fn resolve_label_string(&self, label_str: &str) -> Option<PathBuf> {
        let label = kuro_bzlmod::canonicalize_label_with_package_context(label_str, "", "", None)?;
        Some(self.resolve_canonical_label(&label))
    }

    pub(crate) fn resolve_canonical_label(&self, label: &CanonicalLabel) -> PathBuf {
        let repo = label.repo().as_str();
        let is_root = repo.is_empty() || kuro_core::cells::is_root_cell_name(repo);
        if is_root {
            let fragment = label_path_fragment(label.package(), label.target());
            return match self.root_label_resolution {
                RootLabelResolution::Relative => fragment,
                RootLabelResolution::ProjectAbsolute => self
                    .project_root_path()
                    .map(|root| root.join(&fragment))
                    .unwrap_or(fragment),
            };
        }

        if let Some(repo_path) = self.cell_path_for_repo(repo) {
            return join_label_fragment(repo_path, label.package(), label.target());
        }

        if let Some(cell_path) = kuro_core::cells::get_dynamic_extension_cell(repo) {
            if let Some(project_root) = self.project_root_path() {
                return join_label_fragment(
                    project_root.join(cell_path),
                    label.package(),
                    label.target(),
                );
            }
        }

        if let Some(path) = self.scan_bazel_external_fallback(repo, label.package(), label.target())
        {
            return path;
        }

        join_label_fragment(PathBuf::from(repo), label.package(), label.target())
    }

    fn project_root_path(&self) -> Option<PathBuf> {
        self.project_root
            .map(Path::to_path_buf)
            .or_else(kuro_core::cells::get_dynamic_project_root)
    }

    fn cell_path_for_repo(&self, repo: &str) -> Option<PathBuf> {
        let cell_paths = self.cell_paths?;
        cell_paths
            .get(repo)
            .or_else(|| {
                cell_paths
                    .iter()
                    .find(|(name, _)| name.starts_with(&format!("{}+", repo)))
                    .map(|(_, path)| path)
            })
            .cloned()
    }

    fn scan_bazel_external_fallback(
        &self,
        repo: &str,
        package: &str,
        target: &str,
    ) -> Option<PathBuf> {
        let scan_dirs = self.bazel_external_scan_dirs();
        if scan_dirs.is_empty() {
            return None;
        }

        tracing::debug!(
            repo,
            "Falling back to bazel-external directory scanning for label resolution"
        );

        for scan_dir in scan_dirs {
            let exact = scan_dir.join(repo);
            if exact.exists() {
                return Some(join_label_fragment(exact, package, target));
            }

            if let Ok(entries) = std::fs::read_dir(&scan_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if !name.starts_with(&format!("{}+", repo)) {
                        continue;
                    }
                    if name.matches('+').count() > 1 {
                        continue;
                    }
                    return Some(join_label_fragment(entry.path(), package, target));
                }
            }

            if let Ok(entries) = std::fs::read_dir(&scan_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.matches('+').count() < 2 {
                        continue;
                    }
                    let last_segment = name.rsplit('+').next().unwrap_or("");
                    if last_segment != repo {
                        continue;
                    }
                    return Some(join_label_fragment(path, package, target));
                }
            }
        }

        None
    }

    fn bazel_external_scan_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(project_root) = self.project_root_path() {
            dirs.push(project_root.join("bazel-external"));
        }
        let workspace_external = self.workspace_root.join("bazel-external");
        if workspace_external.exists() && !dirs.iter().any(|dir| dir == &workspace_external) {
            dirs.push(workspace_external);
        }
        dirs
    }
}

pub(crate) fn is_bazel_label_string(value: &str) -> bool {
    value.starts_with('@') || value.starts_with("//")
}

fn label_path_fragment(package: &str, target: &str) -> PathBuf {
    if package.is_empty() {
        PathBuf::from(target)
    } else {
        Path::new(package).join(target)
    }
}

fn join_label_fragment(mut base: PathBuf, package: &str, target: &str) -> PathBuf {
    if !package.is_empty() {
        base.push(package);
    }
    base.push(target);
    base
}
