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

use slug_bzlmod::CanonicalLabel;

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
    allow_legacy_fallbacks: bool,
}

impl<'a> LabelFilesystemResolver<'a> {
    pub(crate) fn new(workspace_root: &'a Path) -> Self {
        Self {
            workspace_root,
            project_root: None,
            cell_paths: None,
            root_label_resolution: RootLabelResolution::Relative,
            allow_legacy_fallbacks: true,
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

    pub(crate) fn without_legacy_fallbacks(mut self) -> Self {
        self.allow_legacy_fallbacks = false;
        self
    }

    pub(crate) fn resolve_label_string(&self, label_str: &str) -> Option<PathBuf> {
        let label = slug_bzlmod::canonicalize_label_with_package_context(label_str, "", "", None)?;
        Some(self.resolve_canonical_label(&label))
    }

    pub(crate) fn resolve_canonical_label(&self, label: &CanonicalLabel) -> PathBuf {
        let repo = label.repo().as_str();
        if repo.is_empty() {
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

        if !self.allow_legacy_fallbacks {
            return join_label_fragment(PathBuf::from(repo), label.package(), label.target());
        }

        if slug_core::cells::is_root_cell_name(repo) {
            let fragment = label_path_fragment(label.package(), label.target());
            return match self.root_label_resolution {
                RootLabelResolution::Relative => fragment,
                RootLabelResolution::ProjectAbsolute => self
                    .project_root_path()
                    .map(|root| root.join(&fragment))
                    .unwrap_or(fragment),
            };
        }

        if let Some(cell_path) = slug_core::cells::get_dynamic_extension_cell(repo) {
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

        let resolved_repo = slug_core::cells::resolve_dynamic_extension_cell_alias(repo)
            .unwrap_or_else(|| repo.to_owned());
        let repo_names = || {
            if resolved_repo == repo {
                [Some(repo), None]
            } else {
                [Some(resolved_repo.as_str()), Some(repo)]
            }
        };

        for repo_name in repo_names().into_iter().flatten() {
            if let Some(repo_path) = self.cell_path_for_repo(repo_name) {
                return join_label_fragment(repo_path, label.package(), label.target());
            }
        }

        for repo_name in repo_names().into_iter().flatten() {
            if let Some(cell_path) = slug_core::cells::get_dynamic_extension_cell(repo_name) {
                if let Some(project_root) = self.project_root_path() {
                    return join_label_fragment(
                        project_root.join(cell_path),
                        label.package(),
                        label.target(),
                    );
                }
            }
        }

        for repo_name in [resolved_repo.as_str(), repo] {
            if let Some(path) =
                self.scan_bazel_external_fallback(repo_name, label.package(), label.target())
            {
                return path;
            }
        }

        join_label_fragment(
            PathBuf::from(resolved_repo),
            label.package(),
            label.target(),
        )
    }

    fn project_root_path(&self) -> Option<PathBuf> {
        self.project_root.map(Path::to_path_buf).or_else(|| {
            self.allow_legacy_fallbacks
                .then(slug_core::cells::get_dynamic_project_root)
                .flatten()
        })
    }

    fn cell_path_for_repo(&self, repo: &str) -> Option<PathBuf> {
        let cell_paths = self.cell_paths?;
        cell_paths
            .get(repo)
            .or_else(|| {
                let module_prefix = format!("{}+", repo);
                cell_paths
                    .iter()
                    .find(|(name, _)| {
                        name.starts_with(&module_prefix) && name.matches('+').count() == 1
                    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_dynamic_extension_apparent_alias_to_canonical_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let apparent = "label_fs_alias_tool_test";
        let canonical = "label_fs_alias_owner+http_file+tool_test";
        slug_core::cells::register_dynamic_extension_cell(
            canonical.to_owned(),
            format!("bazel-external/{canonical}"),
        );
        slug_core::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            canonical.to_owned(),
        );

        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
            .resolve_label_string(&format!("@{apparent}//file:downloaded"))
            .unwrap();

        assert_eq!(
            resolved,
            project_root
                .join("bazel-external")
                .join(canonical)
                .join("file")
                .join("downloaded")
        );
    }

    #[test]
    fn preserves_exact_canonical_module_repo_before_dynamic_aliases() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let module_repo = "label_fs_module+";
        let generated_repo = "label_fs_module++ext+tool_test";
        std::fs::create_dir_all(project_root.join("bazel-external").join(module_repo)).unwrap();
        slug_core::cells::register_dynamic_extension_cell(
            generated_repo.to_owned(),
            format!("bazel-external/{generated_repo}"),
        );
        slug_core::cells::register_dynamic_extension_cell_alias(
            module_repo.to_owned(),
            generated_repo.to_owned(),
        );

        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
            .resolve_label_string(&format!("@@{module_repo}//:data"))
            .unwrap();

        assert_eq!(
            resolved,
            project_root
                .join("bazel-external")
                .join(module_repo)
                .join("data")
        );
    }

    #[test]
    fn apparent_module_repo_cell_path_ignores_generated_repo_prefixes() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let mut cell_paths = HashMap::new();
        cell_paths.insert(
            "label_fs_owner++ext+spoke".to_owned(),
            project_root
                .join("bazel-external")
                .join("label_fs_owner++ext+spoke"),
        );
        cell_paths.insert(
            "label_fs_owner+".to_owned(),
            project_root.join("bazel-external").join("label_fs_owner+"),
        );

        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
            .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
            .resolve_label_string("@@label_fs_owner//:data")
            .unwrap();

        assert_eq!(
            resolved,
            project_root
                .join("bazel-external")
                .join("label_fs_owner+")
                .join("data")
        );
    }

    #[test]
    fn resolver_owned_cell_paths_do_not_use_legacy_fallbacks_for_missing_repo() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let apparent = "label_fs_resolver_missing_alias";
        let wrong_global = "label_fs_wrong_owner++ext+generated";
        slug_core::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        slug_core::cells::register_dynamic_extension_cell(
            wrong_global.to_owned(),
            format!("bazel-external/{wrong_global}"),
        );

        let cell_paths = HashMap::new();
        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
            .without_legacy_fallbacks()
            .resolve_label_string(&format!("@{apparent}//pkg:tool"))
            .unwrap();

        assert_eq!(resolved, PathBuf::from(apparent).join("pkg").join("tool"));
    }

    #[test]
    fn resolver_owned_cell_paths_do_not_use_legacy_root_cell_name_for_missing_repo() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let repo = "root";

        let legacy_resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
            .resolve_label_string(&format!("@{repo}//pkg:tool"))
            .unwrap();
        assert_eq!(legacy_resolved, project_root.join("pkg").join("tool"));

        let cell_paths = HashMap::new();
        let resolver_owned = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
            .without_legacy_fallbacks()
            .resolve_label_string(&format!("@{repo}//pkg:tool"))
            .unwrap();
        assert_eq!(resolver_owned, PathBuf::from(repo).join("pkg").join("tool"));
    }
}
