/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::key::ConfiguredTargetKey;

/// Borrowed typed Symlink metadata for query; no execution or staging authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredSymlinkActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredSymlinkActionView<'a> {
    pub fn from_configured_action(action: &'a ConfiguredAction) -> Result<Self, &'static str> {
        let spec = action
            .symlink_spec()
            .ok_or("configured Symlink view requires a typed Symlink")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("configured Symlink action requires a selected execution platform");
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err("configured Symlink action requires raw and effective platform facts");
        }
        if spec.output().kind() != ActionOutputKind::File {
            return Err("configured Symlink action requires one regular File output");
        }
        if spec.output().metadata() != ActionOutputMetadata::Ordinary {
            return Err("configured Symlink action does not support constant output metadata");
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured Symlink query does not support aspect provenance");
        }
        if let SymlinkTarget::Artifact {
            input,
            use_exec_root_for_source,
            ..
        } = spec.target()
        {
            if *use_exec_root_for_source {
                return Err(
                    "configured Symlink query does not support alternate source-root policy",
                );
            }
            if !matches!(input, AnalysisArtifact::Source(_))
                && !matches!(input, AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::File)
            {
                return Err(
                    "configured Symlink query requires a Source or Derived regular File target",
                );
            }
        }
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a SymlinkSpec {
        self.0
            .symlink_spec()
            .expect("configured Symlink view validates the typed spec")
    }

    pub fn context(&self) -> &'a ConfiguredActionOwnerContext {
        self.0.context().as_ref()
    }

    pub fn owner(&self) -> &'a ConfiguredTargetKey {
        self.context().owner()
    }

    pub fn execution_platform(&self) -> &'a ConfiguredTargetKey {
        self.context()
            .execution_platform()
            .expect("configured Symlink view validates a selected platform")
    }

    pub fn mnemonic(&self) -> &'static str {
        match self.spec().target() {
            SymlinkTarget::Artifact {
                require_executable: true,
                ..
            } => "ExecutableSymlink",
            _ => "Symlink",
        }
    }
}
