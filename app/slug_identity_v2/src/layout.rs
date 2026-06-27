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

use crate::repo::CanonicalRepoName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelLayout {
    workspace_root: PathBuf,
    output_base: PathBuf,
    workspace_name: String,
}

impl BazelLayout {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        output_base: impl Into<PathBuf>,
        workspace_name: impl Into<String>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            output_base: output_base.into(),
            workspace_name: workspace_name.into(),
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn output_base(&self) -> &Path {
        &self.output_base
    }

    pub fn execroot(&self) -> PathBuf {
        self.output_base.join("execroot").join(&self.workspace_name)
    }

    pub fn bazel_out(&self, config: &str) -> PathBuf {
        self.execroot().join("bazel-out").join(config)
    }

    pub fn bin_dir(&self, config: &str) -> PathBuf {
        self.bazel_out(config).join("bin")
    }

    pub fn testlogs_dir(&self, config: &str) -> PathBuf {
        self.bazel_out(config).join("testlogs")
    }

    pub fn external_repo_dir(&self, repo: &CanonicalRepoName) -> PathBuf {
        if repo.is_root() {
            self.execroot()
        } else {
            self.execroot().join("external").join(repo.as_str())
        }
    }
}
