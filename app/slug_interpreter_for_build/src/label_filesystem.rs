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
    project_root: Option<&'a Path>,
    cell_paths: Option<&'a HashMap<String, PathBuf>>,
    root_label_resolution: RootLabelResolution,
}

impl<'a> LabelFilesystemResolver<'a> {
    pub(crate) fn new(_workspace_root: &'a Path) -> Self {
        Self {
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
        let label = slug_bzlmod::canonicalize_label_with_package_context(label_str, "", "", None)?;
        self.resolve_canonical_label(&label)
    }

    pub(crate) fn resolve_canonical_label(&self, label: &CanonicalLabel) -> Option<PathBuf> {
        let repo = label.repo().as_str();
        if repo.is_empty() {
            let fragment = label_path_fragment(label.package(), label.target());
            return Some(match self.root_label_resolution {
                RootLabelResolution::Relative => fragment,
                RootLabelResolution::ProjectAbsolute => self
                    .project_root_path()
                    .map(|root| root.join(&fragment))
                    .unwrap_or(fragment),
            });
        }

        if let Some(repo_path) = self.cell_path_for_repo(repo) {
            return Some(join_label_fragment(
                repo_path,
                label.package(),
                label.target(),
            ));
        }

        None
    }

    fn project_root_path(&self) -> Option<PathBuf> {
        self.project_root.map(Path::to_path_buf)
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
    fn resolves_apparent_repo_from_resolver_owned_cell_paths() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let apparent = "label_fs_alias_tool_test";
        let canonical = "label_fs_alias_owner+http_file+tool_test";
        let mut cell_paths = HashMap::new();
        cell_paths.insert(
            apparent.to_owned(),
            project_root.join("bazel-external").join(canonical),
        );

        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
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
    fn preserves_exact_canonical_module_repo_from_cell_paths() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let module_repo = "label_fs_module+";
        let generated_repo = "label_fs_module++ext+tool_test";
        let mut cell_paths = HashMap::new();
        cell_paths.insert(
            module_repo.to_owned(),
            project_root.join("bazel-external").join(module_repo),
        );
        cell_paths.insert(
            generated_repo.to_owned(),
            project_root.join("bazel-external").join(generated_repo),
        );

        let resolved = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
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
    fn resolver_owned_cell_paths_do_not_use_process_global_aliases_for_missing_repo() {
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
            .resolve_label_string(&format!("@{apparent}//pkg:tool"));

        assert_eq!(resolved, None);
    }

    #[test]
    fn missing_repo_named_root_is_not_special_without_cell_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let project_root = temp.path();
        let repo = "root";

        let cell_paths = HashMap::new();
        let resolver_owned = LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&cell_paths)
            .resolve_label_string(&format!("@{repo}//pkg:tool"));
        assert_eq!(resolver_owned, None);
    }
}
