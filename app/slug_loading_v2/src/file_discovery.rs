/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::Path;
use std::path::PathBuf;

pub const MODULE_FILE: &str = "MODULE.bazel";
pub const BUILD_FILE_PRIMARY: &str = "BUILD.bazel";
pub const BUILD_FILE_FALLBACK: &str = "BUILD";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoot {
    path: PathBuf,
}

impl WorkspaceRoot {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn find_workspace_root(start: &Path) -> Result<WorkspaceRoot, String> {
    let mut current = if start.is_file() {
        start.parent().map(Path::to_path_buf)
    } else {
        Some(start.to_path_buf())
    };
    while let Some(path) = current {
        if path.join(MODULE_FILE).is_file() {
            return Ok(WorkspaceRoot { path });
        }
        current = path.parent().map(Path::to_path_buf);
    }
    Err(format!(
        "missing {MODULE_FILE}; Slug V2 requires Bazel 9 bzlmod workspaces"
    ))
}

pub fn find_build_file(package_dir: &Path) -> Option<PathBuf> {
    [BUILD_FILE_PRIMARY, BUILD_FILE_FALLBACK]
        .into_iter()
        .map(|name| package_dir.join(name))
        .find(|path| path.is_file())
}

pub fn is_bazel_build_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, BUILD_FILE_PRIMARY | BUILD_FILE_FALLBACK))
}

pub fn is_bzl_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "bzl")
}
