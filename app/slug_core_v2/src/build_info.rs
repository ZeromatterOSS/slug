/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub slug_version: &'static str,
    pub bazel_compatibility_floor: &'static str,
    pub commit: Option<&'static str>,
}

impl BuildInfo {
    pub const fn current() -> Self {
        Self {
            slug_version: env!("CARGO_PKG_VERSION"),
            bazel_compatibility_floor: "9.0.0",
            commit: option_env!("SLUG_V2_COMMIT"),
        }
    }

    pub fn version_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "Slug V2".to_owned(),
            format!("Bazel compatibility: >={}", self.bazel_compatibility_floor),
            format!("Package version: {}", self.slug_version),
        ];
        if let Some(commit) = self.commit {
            lines.push(format!("Commit: {commit}"));
        } else {
            lines.push("Commit: unknown".to_owned());
        }
        lines
    }
}
