/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod dice;
pub mod events;
pub mod reapi;
pub mod starlark;

pub use dice::WorkspaceBuildEvaluation;
pub use dice::WorkspaceDirectoryObservation;
pub use dice::WorkspaceEvaluation;
pub use dice::WorkspaceFileObservation;
pub use dice::WorkspaceObservation;
pub use dice::WorkspaceRevision;
pub use dice::WorkspaceRuntime;
pub use dice::evaluate_workspace;
pub use dice::evaluate_workspace_targets;
pub use dice::observe_workspace;
pub use dice::observe_workspace_files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    OneShot,
    Daemon,
}

impl RuntimeMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneShot => "one-shot",
            Self::Daemon => "daemon",
        }
    }
}
