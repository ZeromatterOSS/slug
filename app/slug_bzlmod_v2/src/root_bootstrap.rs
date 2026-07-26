/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathIoErrorKind;

/// Exact Bazel 9.2 root-module reminder written when `MODULE.bazel` is absent.
pub const ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES: &[u8; 399] = b"\
###############################################################################
# Bazel now uses Bzlmod by default to manage external dependencies.
# Please consider migrating your external dependencies from WORKSPACE to MODULE.bazel.
#
# For more details, please check https://github.com/bazelbuild/bazel/issues/18958
###############################################################################
";

/// SHA-256 of [`ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES`].
pub const ROOT_MODULE_BOOTSTRAP_REMINDER_SHA256: [u8; 32] = [
    0x0e, 0x3e, 0x31, 0x51, 0x45, 0xac, 0x7e, 0xe7, 0xa4, 0xe0, 0xac, 0x82, 0x5e, 0x1c, 0x5e, 0x03,
    0xc0, 0x68, 0xec, 0x12, 0x54, 0xdd, 0x42, 0xc3, 0xca, 0xae, 0xcb, 0x27, 0xe9, 0x21, 0xdc, 0x4d,
];

/// Exact warning emitted by Bazel 9.2 after creating the root module file.
pub const ROOT_MODULE_BOOTSTRAP_WARNING_TEXT: &str = "\
--enable_bzlmod is set, but no MODULE.bazel file was found at the workspace root. \
Bazel will create an empty MODULE.bazel file. Please consider migrating your external \
dependencies from WORKSPACE to MODULE.bazel. For more details, please refer to \
https://github.com/bazelbuild/bazel/issues/18958.";

/// Outside-DICE request to ensure the logical root `MODULE.bazel` exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootModuleBootstrapRequest {
    pub workspace: NormalizedAbsolutePath,
}

impl RootModuleBootstrapRequest {
    /// Derives the logical module path without filesystem resolution or I/O.
    pub fn module_path(&self) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(self.workspace.as_path().join("MODULE.bazel"))
            .expect("joining a basename to a normalized absolute workspace remains absolute")
    }
}

/// Token carrying Bazel's exact create-time warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootModuleBootstrapWarning;

impl RootModuleBootstrapWarning {
    pub const fn text(&self) -> &'static str {
        ROOT_MODULE_BOOTSTRAP_WARNING_TEXT
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RootModuleBootstrapApplyResult {
    AlreadyPresent,
    Created(RootModuleBootstrapWarning),
}

/// Failure to create the logical root module file outside DICE.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleBootstrapCreateError {
    pub module_path: NormalizedAbsolutePath,
    pub kind: PathIoErrorKind,
    pub raw_os_error: Option<i32>,
}
