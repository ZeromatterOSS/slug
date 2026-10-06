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
use slug_build_api_v2::BuildInfoCallbackRecipe;
use slug_build_api_v2::BuildInfoTransformKind;
use slug_build_api_v2::BuildInfoTransformSpec;
use slug_build_api_v2::WorkspaceStatusArtifact;
use slug_build_api_v2::WorkspaceStatusArtifactKind;
use slug_configuration_v2::HostPathFlavor;
use slug_workspace_v2::NormalizedAbsolutePath;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::ConfiguredExecGroup;
use crate::key::ConfiguredTargetKey;

/// Borrowed finite BuildInfo query metadata; no callback or execution authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredBuildInfoActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredBuildInfoActionView<'a> {
    pub fn from_configured_action(
        action: &'a ConfiguredAction,
        workspace: &NormalizedAbsolutePath,
    ) -> Result<Self, &'static str> {
        let spec = action
            .build_info_transform_spec()
            .ok_or("configured BuildInfo view requires a typed transform")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("configured BuildInfo action requires a selected execution platform");
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err("configured BuildInfo action requires raw and effective platform facts");
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured BuildInfo query does not support aspect provenance");
        }
        if context.exec_group() != &ConfiguredExecGroup::Default
            || action.exec_group().is_some()
            || !action.exec_properties().is_empty()
        {
            return Err("configured BuildInfo query requires default action group and properties");
        }
        let configuration = context
            .owner()
            .configuration()
            .slug_configuration()
            .ok_or("configured BuildInfo query requires structural configuration")?;
        if configuration
            .configured_action_path_flavor()
            .map_err(|_| "configured BuildInfo query requires retained Host path facts")?
            != HostPathFlavor::Unix
        {
            return Err("configured BuildInfo query does not support Windows");
        }
        if spec.status().root().workspace() != workspace {
            return Err("configured BuildInfo status belongs to a different command workspace");
        }
        let (status_kind, metadata) = match spec.callback().kind() {
            BuildInfoTransformKind::Info => (
                WorkspaceStatusArtifactKind::Stable,
                ActionOutputMetadata::Ordinary,
            ),
            BuildInfoTransformKind::Version => (
                WorkspaceStatusArtifactKind::Volatile,
                ActionOutputMetadata::Constant,
            ),
        };
        let output = spec.output();
        if spec.status().kind() != status_kind
            || !matches!(action.outputs(), [declared] if declared == output)
            || output.kind() != ActionOutputKind::File
            || output.metadata() != metadata
        {
            return Err("configured BuildInfo status and regular output must match callback role");
        }
        if output.root().configuration() != configuration {
            return Err("configured BuildInfo output requires the action configuration root");
        }
        match spec.template() {
            AnalysisArtifact::Source(_) => {}
            AnalysisArtifact::Derived { owner, output }
                if output.kind() == ActionOutputKind::File
                    && output.metadata() == ActionOutputMetadata::Ordinary =>
            {
                // A template may belong to another owner/configuration.
                if output.root().configuration().canonical_bytes() != owner.configuration() {
                    return Err("configured BuildInfo template root differs from its own owner");
                }
            }
            _ => {
                return Err(
                    "configured BuildInfo query requires a Source or Derived ordinary File template",
                );
            }
        }
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a BuildInfoTransformSpec {
        self.0
            .build_info_transform_spec()
            .expect("checked typed BuildInfo transform")
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

    pub fn status(&self) -> &'a WorkspaceStatusArtifact {
        self.spec().status()
    }

    pub fn template(&self) -> &'a AnalysisArtifact {
        self.spec().template()
    }

    pub fn callback(&self) -> &'a BuildInfoCallbackRecipe {
        self.spec().callback()
    }

    pub fn output(&self) -> &'a ActionOutput {
        self.spec().output()
    }
}
