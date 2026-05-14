/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `ctx.actions.expand_template` as a deferred action (Plan 42).
//!
//! Reads a template file (source or build artifact) at execution time,
//! applies a fixed list of `(key, value)` substring substitutions, and
//! writes the result to the output. The template is declared as an
//! action input so build-artifact templates are materialized before
//! the read.

use std::borrow::Cow;
use std::slice;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use gazebo::prelude::*;
use indexmap::IndexSet;
use indexmap::indexmap;
use indexmap::indexset;
use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_build_api::actions::Action;
use slug_build_api::actions::ActionExecutionCtx;
use slug_build_api::actions::UnregisteredAction;
use slug_build_api::actions::box_slice_set::BoxSliceSet;
use slug_build_api::actions::execute::action_executor::ActionExecutionKind;
use slug_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use slug_build_api::actions::execute::action_executor::ActionOutputs;
use slug_build_api::actions::execute::error::ExecuteError;
use slug_build_api::artifact_groups::ArtifactGroup;
use slug_build_signals::env::WaitingData;
use slug_core::category::CategoryRef;
use slug_error::BuckErrorContext;
use slug_execute::artifact::artifact_dyn::ArtifactDyn;
use slug_execute::execute::command_executor::ActionExecutionTimingData;
use slug_execute::materialize::materializer::WriteRequest;
use starlark::values::OwnedFrozenValue;

use crate::actions::impls::common::first_input_artifact;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum ExpandTemplateActionError {
    #[error("expand_template requires exactly one output, got {0}")]
    WrongNumberOfOutputs(usize),
    #[error("expand_template input did not resolve to a single artifact")]
    AmbiguousInput,
}

#[derive(Allocative, Debug)]
pub(crate) struct UnregisteredExpandTemplateAction {
    pub(crate) template: ArtifactGroup,
    /// Substitutions in the order they were declared. Bazel applies them
    /// sequentially as plain substring replacements.
    pub(crate) substitutions: Vec<(String, String)>,
    pub(crate) is_executable: bool,
}

impl UnregisteredAction for UnregisteredExpandTemplateAction {
    fn register(
        self: Box<Self>,
        outputs: IndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> slug_error::Result<Box<dyn Action>> {
        Ok(Box::new(ExpandTemplateAction::new(*self, outputs)?))
    }
}

#[derive(Debug, Allocative)]
struct ExpandTemplateAction {
    inputs: BoxSliceSet<ArtifactGroup>,
    output: BuildArtifact,
    substitutions: Vec<(String, String)>,
    is_executable: bool,
}

impl ExpandTemplateAction {
    fn new(
        unreg: UnregisteredExpandTemplateAction,
        outputs: IndexSet<BuildArtifact>,
    ) -> slug_error::Result<Self> {
        if outputs.len() != 1 {
            return Err(ExpandTemplateActionError::WrongNumberOfOutputs(outputs.len()).into());
        }
        let output = outputs.into_iter().next().ok_or_else(|| {
            ExpandTemplateActionError::WrongNumberOfOutputs(0)
        })?;
        Ok(ExpandTemplateAction {
            inputs: BoxSliceSet::from(indexset![unreg.template]),
            output,
            substitutions: unreg.substitutions,
            is_executable: unreg.is_executable,
        })
    }

    fn input(&self) -> &ArtifactGroup {
        first_input_artifact(&self.inputs)
    }
}

#[async_trait]
impl Action for ExpandTemplateAction {
    fn kind(&self) -> slug_data::ActionKind {
        slug_data::ActionKind::Write
    }

    fn inputs(&self) -> slug_error::Result<Cow<'_, [ArtifactGroup]>> {
        Ok(Cow::Borrowed(self.inputs.as_slice()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(slice::from_ref(&self.output))
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.output
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("expand_template")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output.get_path().path().as_str())
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let artifact_fs = ctx.fs();

        // Resolve template to absolute on-disk path.
        let (template_artifact, template_value) = ctx
            .artifact_values(self.input())
            .iter()
            .into_singleton()
            .ok_or_else(|| slug_error::Error::from(ExpandTemplateActionError::AmbiguousInput))?;

        let template_rel = template_artifact.resolve_path(
            artifact_fs,
            if template_artifact.has_content_based_path() {
                Some(template_value.content_based_path_hash())
            } else {
                None
            }
            .as_ref(),
        )?;
        let template_abs = artifact_fs.fs().resolve(&template_rel);
        let template_content =
            std::fs::read_to_string(&template_abs).map_err(|e| ExecuteError::Error {
                error: slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "expand_template: failed to read template at {}: {}",
                    template_abs.display(),
                    e
                ),
            })?;

        let mut content = template_content;
        for (k, v) in &self.substitutions {
            content = content.replace(k, v);
        }
        let content_bytes = content.into_bytes();

        let mut execution_start = None;
        let value = ctx
            .materializer()
            .declare_write(Box::new(|| {
                execution_start = Some(Instant::now());
                let path = artifact_fs.resolve_build(self.output.get_path(), None)?;
                Ok(vec![WriteRequest {
                    path,
                    content: content_bytes,
                    is_executable: self.is_executable,
                }])
            }))
            .await?
            .into_iter()
            .next()
            .buck_error_context("expand_template did not execute")?;

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
