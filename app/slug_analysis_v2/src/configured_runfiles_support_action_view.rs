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
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_build_api_v2::RunfilesSymlinkMode;
use slug_configuration_v2::HostPathFlavor;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::ConfiguredExecGroup;
use crate::key::ConfiguredTargetKey;

/// Borrowed default runfiles-support query metadata; no execution authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredRunfilesSupportActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredRunfilesSupportActionView<'a> {
    pub fn from_configured_action(action: &'a ConfiguredAction) -> Result<Self, &'static str> {
        let spec = action
            .runfiles_support_spec()
            .ok_or("configured runfiles-support view requires a typed support action")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err(
                "configured runfiles-support action requires a selected execution platform",
            );
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err(
                "configured runfiles-support action requires raw and effective platform facts",
            );
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured runfiles-support query does not support aspect provenance");
        }
        if context.exec_group() != &ConfiguredExecGroup::Default
            || action.exec_group().is_some()
            || !action.exec_properties().is_empty()
        {
            return Err(
                "configured runfiles-support query requires default action group and properties",
            );
        }
        let owner = context.owner();
        let configuration = owner
            .configuration()
            .slug_configuration()
            .ok_or("configured runfiles-support query requires structural configuration")?;
        if configuration
            .configured_action_path_flavor()
            .map_err(|_| "configured runfiles-support query requires retained Host path facts")?
            != HostPathFlavor::Unix
        {
            return Err("configured runfiles-support query does not support Windows");
        }
        let support = spec.support();
        if support.runfiles.is_empty() {
            return Err("configured runfiles-support query requires nonempty runfiles");
        }
        let manifest = support
            .manifest
            .as_ref()
            .ok_or("configured runfiles-support query requires a public manifest")?;
        let mapping = support
            .repo_mapping_manifest
            .as_ref()
            .ok_or("configured runfiles-support query requires a repository mapping manifest")?;
        for (artifact, kind) in [
            (&support.tree, ActionOutputKind::RunfilesTree),
            (&support.input_manifest, ActionOutputKind::File),
            (manifest, ActionOutputKind::File),
            (mapping, ActionOutputKind::File),
        ] {
            let AnalysisArtifact::Derived {
                owner: retained,
                output,
            } = artifact
            else {
                return Err("configured runfiles-support roles must be configured Derived outputs");
            };
            if retained.label() != owner.label()
                || retained.configuration() != configuration.canonical_bytes()
                || retained.toolchain_execution_platform() != owner.toolchain_execution_platform()
            {
                return Err("configured runfiles-support roles must have the exact action owner");
            }
            if output.kind() != kind || output.metadata() != ActionOutputMetadata::Ordinary {
                return Err(
                    "configured runfiles-support roles require ordinary expected output kinds",
                );
            }
            if output.root().configuration() != configuration {
                return Err(
                    "configured runfiles-support roles require the action configuration root",
                );
            }
        }
        let selected = match spec {
            RunfilesSupportActionSpec::RepoMappingManifest {
                packages,
                workspace_name,
                emit_compact_repo_mapping,
                ..
            } => {
                if packages.is_empty()
                    || workspace_name != &support.runfiles.repository_prefix
                    || !*emit_compact_repo_mapping
                {
                    return Err(
                        "configured repository mapping query requires default retained metadata",
                    );
                }
                mapping
            }
            RunfilesSupportActionSpec::SourceSymlinkManifest { remotable, .. } => {
                if *remotable {
                    return Err(
                        "configured source manifest query requires default nonremotable policy",
                    );
                }
                &support.input_manifest
            }
            RunfilesSupportActionSpec::SymlinkTree { mode, .. } => {
                if *mode != RunfilesSymlinkMode::Create {
                    return Err("configured symlink-tree query requires retained Create mode");
                }
                manifest
            }
            RunfilesSupportActionSpec::RunfilesTree { .. } => &support.tree,
        };
        let AnalysisArtifact::Derived { output, .. } = selected else {
            unreachable!("checked support roles are Derived")
        };
        if spec.output() != output {
            return Err("configured runfiles-support output differs from its exact support role");
        }
        let files = RetainedArtifactInputs::new(support.runfiles.files.clone())
            .map_err(|_| "configured runfiles-support files must be typed artifacts")?;
        let mut invalid_input = None;
        files
            .visit(|artifact| {
                if invalid_input.is_none() {
                    invalid_input = check_input(artifact).err();
                }
            })
            .map_err(|_| "configured runfiles-support files must be typed artifacts")?;
        if let Some(error) = invalid_input {
            return Err(error);
        }
        support
            .runfiles
            .symlinks
            .visit(|link| check_input(&link.artifact))?;
        support
            .runfiles
            .root_symlinks
            .visit(|link| check_input(&link.artifact))?;
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a RunfilesSupportActionSpec {
        self.0
            .runfiles_support_spec()
            .expect("checked typed support action")
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

    pub fn output(&self) -> &'a ActionOutput {
        self.spec().output()
    }
}

fn check_input(artifact: &AnalysisArtifact) -> Result<(), &'static str> {
    match artifact {
        AnalysisArtifact::Source(_) => Ok(()),
        AnalysisArtifact::Derived { output, .. }
            if matches!(
                output.kind(),
                ActionOutputKind::File | ActionOutputKind::Directory | ActionOutputKind::Symlink
            ) =>
        {
            Ok(())
        }
        _ => Err(
            "configured runfiles-support query requires Source or Derived file/tree/symlink inputs",
        ),
    }
}
