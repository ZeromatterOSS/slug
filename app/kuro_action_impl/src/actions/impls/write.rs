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
use std::slice;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::artifact_type::OutputArtifact;
use kuro_artifact::artifact::build_artifact::BuildArtifact;
use kuro_build_api::actions::Action;
use kuro_build_api::actions::ActionExecutionCtx;
use kuro_build_api::actions::UnregisteredAction;
use kuro_build_api::actions::execute::action_executor::ActionExecutionKind;
use kuro_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use kuro_build_api::actions::execute::action_executor::ActionOutputs;
use kuro_build_api::actions::execute::error::ExecuteError;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_api::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use kuro_build_api::interpreter::rule_defs::cmd_args::AbsCommandLineContext;
use kuro_build_api::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use kuro_build_api::interpreter::rule_defs::cmd_args::DefaultCommandLineContext;
use kuro_build_api::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use kuro_build_signals::env::WaitingData;
use kuro_common::file_ops::metadata::TrackedFileDigest;
use kuro_core::category::CategoryRef;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_error::BuckErrorContext;
use kuro_execute::artifact::fs::ExecutorFs;
use kuro_execute::execute::command_executor::ActionExecutionTimingData;
use kuro_execute::materialize::materializer::WriteRequest;
use dupe::Dupe;
use indexmap::IndexMap;
use indexmap::IndexSet;
use indexmap::indexmap;
use starlark::values::OwnedFrozenValue;
use starlark::values::UnpackValue;

use crate::actions::impls::run::DepFilesPlaceholderArtifactPathMapper;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Tier0)]
enum WriteActionValidationError {
    #[error("WriteAction received no outputs")]
    NoOutputs,
    #[error("WriteAction received more than one output")]
    TooManyOutputs,
    #[error("Expected command line value, got {0}")]
    ContentsNotCommandLineValue(String),
}

pub(crate) struct CommandLineContentBasedInputVisitor {
    pub(crate) content_based_inputs: IndexSet<ArtifactGroup>,
}

impl CommandLineContentBasedInputVisitor {
    pub(crate) fn new() -> Self {
        Self {
            content_based_inputs: Default::default(),
        }
    }
}

impl<'v> CommandLineArtifactVisitor<'v> for CommandLineContentBasedInputVisitor {
    fn visit_input(&mut self, input: ArtifactGroup, _tags: Vec<&ArtifactTag>) {
        if input.uses_content_based_path() {
            self.content_based_inputs.insert(input);
        }
    }

    fn visit_declared_output(&mut self, _artifact: OutputArtifact<'v>, _tags: Vec<&ArtifactTag>) {}

    fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}

    fn visit_declared_artifact(
        &mut self,
        declared_artifact: kuro_artifact::artifact::artifact_type::DeclaredArtifact<'v>,
        tags: Vec<&ArtifactTag>,
    ) -> kuro_error::Result<()> {
        if declared_artifact.has_content_based_path() {
            let artifact = declared_artifact.ensure_bound()?.into_artifact();
            self.visit_input(ArtifactGroup::Artifact(artifact), tags);
        }

        Ok(())
    }

    fn skip_hidden(&self) -> bool {
        true
    }
}

#[derive(Allocative, Debug)]
pub(crate) struct UnregisteredWriteAction {
    pub(crate) is_executable: bool,
    pub(crate) absolute: bool,
    pub(crate) macro_files: Option<IndexSet<Artifact>>,
    pub(crate) use_dep_files_placeholder_for_content_based_paths: bool,
}

impl UnregisteredAction for UnregisteredWriteAction {
    fn register(
        self: Box<Self>,
        outputs: IndexSet<BuildArtifact>,
        starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> kuro_error::Result<Box<dyn Action>> {
        let contents = starlark_data.expect("module data to be present");

        let write_action = WriteAction::new(contents, outputs, *self)?;
        Ok(Box::new(write_action))
    }
}

#[derive(Debug, Allocative)]
struct WriteAction {
    contents: OwnedFrozenValue, // StarlarkCmdArgs
    output: BuildArtifact,
    inner: UnregisteredWriteAction,
}

impl WriteAction {
    fn new(
        contents: OwnedFrozenValue,
        outputs: IndexSet<BuildArtifact>,
        inner: UnregisteredWriteAction,
    ) -> kuro_error::Result<Self> {
        let mut outputs = outputs.into_iter();

        let output = match (outputs.next(), outputs.next()) {
            (Some(o), None) => o,
            (None, ..) => return Err(WriteActionValidationError::NoOutputs.into()),
            (Some(..), Some(..)) => return Err(WriteActionValidationError::TooManyOutputs.into()),
        };

        if ValueAsCommandLineLike::unpack_value(contents.value())?.is_none() {
            return Err(WriteActionValidationError::ContentsNotCommandLineValue(
                contents.value().to_repr(),
            )
            .into());
        }

        Ok(WriteAction {
            contents,
            output,
            inner,
        })
    }

    fn get_contents(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> kuro_error::Result<String> {
        let mut cli = Vec::<String>::new();

        let macro_files = self.inner.macro_files.as_ref().map(|macro_files| {
            macro_files
                .iter()
                .map(|a| (a, artifact_path_mapping.get(a)))
                .collect()
        });

        let mut ctx = if let Some(ref macro_files) = macro_files {
            DefaultCommandLineContext::new_with_write_to_file_macros_support(fs, macro_files)
        } else {
            DefaultCommandLineContext::new(fs)
        };

        let mut abs;

        let ctx = if self.inner.absolute {
            abs = AbsCommandLineContext::wrap(ctx);
            &mut abs as _
        } else {
            &mut ctx as _
        };

        ValueAsCommandLineLike::unpack_value_err(self.contents.value())
            .unwrap()
            .0
            .add_to_command_line(&mut cli, ctx, artifact_path_mapping)?;

        Ok(cli.join("\n"))
    }
}

#[async_trait]
impl Action for WriteAction {
    fn kind(&self) -> kuro_data::ActionKind {
        kuro_data::ActionKind::Write
    }

    fn inputs(&self) -> kuro_error::Result<Cow<'_, [ArtifactGroup]>> {
        if self.inner.use_dep_files_placeholder_for_content_based_paths {
            return Ok(Cow::Borrowed(&[]));
        }

        let mut visitor = CommandLineContentBasedInputVisitor::new();
        ValueAsCommandLineLike::unpack_value_err(self.contents.value())?
            .0
            .visit_artifacts(&mut visitor)?;
        let mut content_based_inputs = visitor.content_based_inputs;
        if let Some(macro_files) = &self.inner.macro_files {
            for artifact in macro_files {
                if artifact.has_content_based_path() {
                    content_based_inputs.insert(ArtifactGroup::Artifact(artifact.dupe()));
                }
            }
        }
        Ok(Cow::Owned(content_based_inputs.into_iter().collect()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(slice::from_ref(&self.output))
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.output
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("write")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output.get_path().path().as_str())
    }

    fn aquery_attributes(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> IndexMap<String, String> {
        // TODO(cjhopman): We should change this api to support returning a Result.
        indexmap! {
            "contents".to_owned() => match self.get_contents(fs, artifact_path_mapping) {
                Ok(v) => v,
                Err(e) => format!("ERROR: constructing contents ({e})")
            },
            "absolute".to_owned() => self.inner.absolute.to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let fs = ctx.fs();

        let mut execution_start = None;

        let value = ctx
            .materializer()
            .declare_write(Box::new(|| {
                execution_start = Some(Instant::now());
                let content = if self.inner.use_dep_files_placeholder_for_content_based_paths {
                    self.get_contents(
                        &ctx.executor_fs(),
                        &DepFilesPlaceholderArtifactPathMapper {},
                    )?
                } else {
                    self.get_contents(&ctx.executor_fs(), &ctx.artifact_path_mapping(None))?
                }
                .into_bytes();
                let path = fs.resolve_build(
                    self.output.get_path(),
                    if self.output.get_path().is_content_based_path() {
                        let digest = TrackedFileDigest::from_content(
                            &content,
                            ctx.digest_config().cas_digest_config(),
                        );
                        Some(ContentBasedPathHash::new(digest.raw_digest().as_bytes())?)
                    } else {
                        None
                    }
                    .as_ref(),
                )?;
                Ok(vec![WriteRequest {
                    path,
                    content,
                    is_executable: self.inner.is_executable,
                }])
            }))
            .await?
            .into_iter()
            .next()
            .buck_error_context("Write did not execute")?;

        let wall_time = Instant::now()
            - execution_start.buck_error_context("Action did not set execution_start")?;

        Ok((
            ActionOutputs::new(indexmap![self.output.get_path().dupe() => value]),
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
    fn writes_file() {}
}
