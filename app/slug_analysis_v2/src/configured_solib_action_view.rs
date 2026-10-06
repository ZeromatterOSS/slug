/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::SolibSymlinkSpec;
use slug_configuration_v2::HostPathFlavor;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::ConfiguredExecGroup;
use crate::key::ConfiguredTargetKey;

/// Borrowed generated-File Solib query metadata; no execution authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredSolibActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredSolibActionView<'a> {
    pub fn from_configured_action(action: &'a ConfiguredAction) -> Result<Self, &'static str> {
        let spec = action
            .solib_symlink_spec()
            .ok_or("configured Solib view requires a typed SolibSymlink")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("configured Solib action requires a selected execution platform");
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err("configured Solib action requires raw and effective platform facts");
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured Solib query does not support aspect provenance");
        }
        if context.exec_group() != &ConfiguredExecGroup::Default
            || action.exec_group().is_some()
            || !action.exec_properties().is_empty()
        {
            return Err("configured Solib query requires default action group and properties");
        }
        let configuration = context
            .owner()
            .configuration()
            .slug_configuration()
            .ok_or("configured Solib query requires structural configuration")?;
        if configuration
            .configured_action_path_flavor()
            .map_err(|_| "configured Solib query requires retained Host path facts")?
            != HostPathFlavor::Unix
        {
            return Err("configured Solib query does not support Windows");
        }
        let output = spec.output();
        if !matches!(action.outputs(), [declared] if declared == output)
            || output.kind() != ActionOutputKind::File
            || output.metadata() != ActionOutputMetadata::Ordinary
        {
            return Err("configured Solib query requires one ordinary regular File output");
        }
        if output.root().configuration() != configuration {
            return Err("configured Solib output requires the action configuration root");
        }
        let AnalysisArtifact::Derived {
            owner: library_owner,
            output: library,
        } = spec.input()
        else {
            return Err(
                "configured Solib query requires a generated ordinary regular File library",
            );
        };
        if library.kind() != ActionOutputKind::File
            || library.metadata() != ActionOutputMetadata::Ordinary
        {
            return Err(
                "configured Solib query requires a generated ordinary regular File library",
            );
        }
        // The library may have a different owner/configuration from the consumer.
        if library.root().configuration().canonical_bytes() != library_owner.configuration() {
            return Err("configured Solib library root differs from its own configured owner");
        }
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a SolibSymlinkSpec {
        self.0
            .solib_symlink_spec()
            .expect("checked typed Solib action")
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
            .expect("checked selected platform")
    }

    pub fn library(&self) -> &'a AnalysisArtifact {
        self.spec().input()
    }

    pub fn output(&self) -> &'a ActionOutput {
        self.spec().output()
    }
}
