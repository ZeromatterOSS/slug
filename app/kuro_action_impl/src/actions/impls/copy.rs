/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Cow;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use gazebo::prelude::*;
use indexmap::IndexSet;
use indexmap::indexset;
use kuro_artifact::artifact::build_artifact::BuildArtifact;
use kuro_build_api::actions::Action;
use kuro_build_api::actions::ActionExecutionCtx;
use kuro_build_api::actions::UnregisteredAction;
use kuro_build_api::actions::box_slice_set::BoxSliceSet;
use kuro_build_api::actions::execute::action_executor::ActionExecutionKind;
use kuro_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use kuro_build_api::actions::execute::action_executor::ActionOutputs;
use kuro_build_api::actions::execute::error::ExecuteError;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_signals::env::WaitingData;
use kuro_core::category::CategoryRef;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_error::BuckErrorContext;
use kuro_execute::artifact::artifact_dyn::ArtifactDyn;
use kuro_execute::artifact_utils::ArtifactValueBuilder;
use kuro_execute::artifact_value::ArtifactValue;
use kuro_execute::directory::ActionDirectoryEntry;
use kuro_execute::directory::new_symlink;
use kuro_execute::execute::command_executor::ActionExecutionTimingData;
use kuro_execute::materialize::materializer::CopiedArtifact;
use kuro_execute::materialize::materializer::DeclareArtifactPayload;
use starlark::values::OwnedFrozenValue;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum CopyActionValidationError {
    #[error("Exactly one output file must be specified for a copy action, got {0}")]
    WrongNumberOfOutputs(usize),
    #[error("Only artifact inputs are supported in copy actions, got {0}")]
    UnsupportedInput(ArtifactGroup),
}

#[derive(Debug, Allocative)]
pub(crate) enum CopyMode {
    Copy {
        // Override the destination executable bit to +x (true) or -x (false)
        executable_bit_override: Option<bool>,
    },
    Symlink,
}

#[derive(Allocative)]
pub(crate) struct UnregisteredCopyAction {
    src: ArtifactGroup,
    copy: CopyMode,
}

impl UnregisteredCopyAction {
    pub(crate) fn new(src: ArtifactGroup, copy: CopyMode) -> Self {
        Self { src, copy }
    }
}

impl UnregisteredAction for UnregisteredCopyAction {
    fn register(
        self: Box<Self>,
        outputs: IndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> kuro_error::Result<Box<dyn Action>> {
        Ok(Box::new(CopyAction::new(self.copy, self.src, outputs)?))
    }
}

#[derive(Debug, Allocative)]
struct CopyAction {
    copy: CopyMode,
    inputs: BoxSliceSet<ArtifactGroup>,
    outputs: BoxSliceSet<BuildArtifact>,
}

impl CopyAction {
    fn new(
        copy: CopyMode,
        src: ArtifactGroup,
        outputs: IndexSet<BuildArtifact>,
    ) -> kuro_error::Result<Self> {
        // TODO: Exclude other variants once they become available here. For now, this is a noop.
        match src {
            ArtifactGroup::Artifact(..) | ArtifactGroup::Promise(..) => {}
            ArtifactGroup::TransitiveSetProjection(..) | ArtifactGroup::Depset(..) => {
                return Err(CopyActionValidationError::UnsupportedInput(src.dupe()).into());
            }
        };

        if outputs.len() != 1 {
            Err(CopyActionValidationError::WrongNumberOfOutputs(outputs.len()).into())
        } else {
            Ok(CopyAction {
                copy,
                inputs: BoxSliceSet::from(indexset![src]),
                outputs: BoxSliceSet::from(outputs),
            })
        }
    }

    fn input(&self) -> &ArtifactGroup {
        self.inputs
            .iter()
            .next()
            .expect("a single input by construction")
    }

    fn output(&self) -> &BuildArtifact {
        self.outputs
            .iter()
            .next()
            .expect("a single artifact by construction")
    }
}

#[async_trait]
impl Action for CopyAction {
    fn kind(&self) -> kuro_data::ActionKind {
        kuro_data::ActionKind::Copy
    }

    fn inputs(&self) -> kuro_error::Result<Cow<'_, [ArtifactGroup]>> {
        Ok(Cow::Borrowed(self.inputs.as_slice()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(self.outputs.as_slice())
    }

    fn first_output(&self) -> &BuildArtifact {
        self.output()
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("copy")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output().get_path().path().as_str())
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let (input, src_value) = ctx
            .artifact_values(self.input())
            .iter()
            .into_singleton()
            .buck_error_context("Input did not dereference to exactly one artifact")?;

        let artifact_fs = ctx.fs();
        let src = input.resolve_path(
            artifact_fs,
            if input.has_content_based_path() {
                Some(src_value.content_based_path_hash())
            } else {
                None
            }
            .as_ref(),
        )?;
        let tmp_dest = artifact_fs.resolve_build(
            self.output().get_path(),
            Some(&ContentBasedPathHash::for_output_artifact()),
        )?;

        let value = {
            let fs = artifact_fs.fs();
            let mut builder = ArtifactValueBuilder::new(fs, ctx.digest_config());
            match self.copy {
                CopyMode::Copy {
                    executable_bit_override,
                } => {
                    builder.add_copied(
                        src_value,
                        src.as_ref(),
                        tmp_dest.as_ref(),
                        executable_bit_override,
                    )?;
                }
                CopyMode::Symlink => {
                    builder.add_symlinked(src_value, src.clone(), tmp_dest.as_ref())?;
                }
            }

            builder.build(tmp_dest.as_ref())?
        };

        let dest = if self.output().get_path().is_content_based_path() {
            artifact_fs.resolve_build(
                self.output().get_path(),
                Some(&value.content_based_path_hash()),
            )?
        } else {
            tmp_dest
        };

        ctx.materializer()
            .declare_copy(
                dest.clone(),
                value.dupe(),
                // FIXME(JakobDegen): This is wrong in cases where the input artifact is a source
                // directory with ignored paths, as the materializer will incorrectly assume that
                // the source directory matches the artifact value when it doesn't.
                vec![CopiedArtifact::new(
                    src,
                    dest,
                    value.entry().dupe().map_dir(|d| d.as_immutable()),
                    match self.copy {
                        CopyMode::Copy {
                            executable_bit_override,
                        } => executable_bit_override,
                        CopyMode::Symlink => None,
                    },
                )],
            )
            .await?;

        Ok((
            ActionOutputs::from_single(self.output().get_path().dupe(), value),
            ActionExecutionMetadata {
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData::default(),
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

// ============================================================================
// SymlinkPathAction - Creates a symlink to a raw string path (Bazel target_path)
// ============================================================================

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum SymlinkPathActionError {
    #[error("Exactly one output must be specified for a symlink path action, got {0}")]
    WrongNumberOfOutputs(usize),
}

/// An unregistered action that creates a symlink pointing to a raw string path.
///
/// This implements Bazel's `ctx.actions.symlink(output=..., target_path=...)` where
/// the symlink target is a string rather than an artifact.
#[derive(Allocative)]
pub(crate) struct UnregisteredSymlinkPathAction {
    target_path: String,
}

impl UnregisteredSymlinkPathAction {
    pub(crate) fn new(target_path: String) -> Self {
        Self { target_path }
    }
}

impl UnregisteredAction for UnregisteredSymlinkPathAction {
    fn register(
        self: Box<Self>,
        outputs: IndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> kuro_error::Result<Box<dyn Action>> {
        if outputs.len() != 1 {
            return Err(SymlinkPathActionError::WrongNumberOfOutputs(outputs.len()).into());
        }
        let output = outputs.into_iter().next().unwrap();
        Ok(Box::new(SymlinkPathAction {
            target_path: self.target_path,
            output,
        }))
    }
}

#[derive(Debug, Allocative)]
struct SymlinkPathAction {
    target_path: String,
    output: BuildArtifact,
}

#[async_trait]
impl Action for SymlinkPathAction {
    fn kind(&self) -> kuro_data::ActionKind {
        kuro_data::ActionKind::SymlinkPath
    }

    fn inputs(&self) -> kuro_error::Result<Cow<'_, [ArtifactGroup]>> {
        // No artifact inputs — the target is a raw path string
        Ok(Cow::Borrowed(&[]))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(std::slice::from_ref(&self.output))
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.output
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("symlink")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output.get_path().path().as_str())
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let start = Instant::now();
        let artifact_fs = ctx.fs();

        // Resolve the output path in the build directory
        let dest = artifact_fs.resolve_build(self.output.get_path(), None)?;

        // Create the symlink entry: this will be a Symlink (relative) or ExternalSymlink (absolute)
        let symlink_entry = new_symlink(&self.target_path)?;
        let value = ArtifactValue::new(ActionDirectoryEntry::Leaf(symlink_entry), None);

        // Create parent directories and the actual symlink on disk
        let abs_dest = artifact_fs.fs().resolve(&dest);
        if let Some(parent) = abs_dest.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // Remove any existing file/symlink at the destination
        let _ = std::fs::remove_file(&abs_dest);
        kuro_fs::fs_util::symlink(&self.target_path, &abs_dest)?;

        // Inform the materializer that this artifact exists on disk
        ctx.materializer()
            .declare_existing(vec![DeclareArtifactPayload {
                path: dest,
                artifact: value.dupe(),
                persist_full_directory_structure: false,
            }])
            .await?;

        let wall_time = Instant::now() - start;

        Ok((
            ActionOutputs::from_single(self.output.get_path().dupe(), value),
            ActionExecutionMetadata {
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData { wall_time },
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    // TODO: This needs proper tests, but right now it's kind of a pain to get the
    //       action framework up and running to test actions
    #[test]
    fn copies_file() {}
}
