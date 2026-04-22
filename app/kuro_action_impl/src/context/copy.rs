/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::OptionDupedExt;
use indexmap::indexset;
use kuro_build_api::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use kuro_build_api::interpreter::rule_defs::artifact::output_artifact_like::OutputArtifactArg;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_execute::execute::request::OutputType;
use starlark::environment::MethodsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::ValueTyped;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::none::NoneOr;

use crate::actions::impls::copy::CopyMode;
use crate::actions::impls::copy::UnregisteredCopyAction;
use crate::actions::impls::copy::UnregisteredSymlinkPathAction;
use crate::actions::impls::symlinked_dir::UnregisteredSymlinkedDirAction;

fn create_dir_tree<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    this: &AnalysisActions<'v>,
    output: OutputArtifactArg<'v>,
    srcs: UnpackDictEntries<&'v str, ValueAsInputArtifactLike<'v>>,
    copy: CopyMode,
    uses_experimental_content_based_path_hashing: Option<bool>,
) -> kuro_error::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
    let action = UnregisteredSymlinkedDirAction::new(copy, srcs)?;
    let unioned_associated_artifacts = action.unioned_associated_artifacts();

    let mut this = this.state()?;
    let (declaration, output_artifact) = this.get_or_declare_output(
        eval,
        output,
        OutputType::Directory,
        uses_experimental_content_based_path_hashing,
    )?;
    this.register_action(indexset![output_artifact], action, None, None)?;

    Ok(declaration.into_declared_artifact(unioned_associated_artifacts))
}

fn copy_file_impl<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    this: &AnalysisActions<'v>,
    dest: OutputArtifactArg<'v>,
    src: ValueAsInputArtifactLike<'v>,
    copy: CopyMode,
    output_type: OutputType,
    uses_experimental_content_based_path_hashing: Option<bool>,
) -> kuro_error::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
    let src = src.0;

    let artifact = src.get_artifact_group()?;
    let associated_artifacts = src.get_associated_artifacts();
    let mut this = this.state()?;
    let (declaration, output_artifact) = this.get_or_declare_output(
        eval,
        dest,
        output_type,
        uses_experimental_content_based_path_hashing,
    )?;

    this.register_action(
        indexset![output_artifact],
        UnregisteredCopyAction::new(artifact, copy),
        None,
        None,
    )?;

    Ok(declaration.into_declared_artifact(
        associated_artifacts
            .duped()
            .unwrap_or_else(AssociatedArtifacts::new),
    ))
}

#[starlark_module]
pub(crate) fn analysis_actions_methods_copy(methods: &mut MethodsBuilder) {
    /// Copies the source `artifact` to the destination (which can be a string representing a
    /// filename or an output `artifact`) and returns the output `artifact`. The copy works for
    /// files or directories.
    fn copy_file<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] dest: OutputArtifactArg<'v>,
        #[starlark(require = pos)] src: ValueAsInputArtifactLike<'v>,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] executable_bit_override: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        // `copy_file` can copy either a file or a directory, even though its name has the word
        // `file` in it
        Ok(copy_file_impl(
            eval,
            this,
            dest,
            src,
            CopyMode::Copy {
                executable_bit_override: executable_bit_override.into_option(),
            },
            OutputType::FileOrDirectory,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?)
    }

    /// Creates a symlink to the source `artifact` at the destination (which can be a string
    /// representing a filename or an output `artifact`) and returns the output `artifact`. The
    /// symlink works for files or directories.
    fn symlink_file<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] dest: OutputArtifactArg<'v>,
        #[starlark(require = pos)] src: ValueAsInputArtifactLike<'v>,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        // `copy_file` can copy either a file or a directory, even though its name has the word
        // `file` in it
        Ok(copy_file_impl(
            eval,
            this,
            dest,
            src,
            CopyMode::Symlink,
            OutputType::FileOrDirectory,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?)
    }

    /// Creates a symlink action (Bazel-compatible).
    ///
    /// Accepts either `target_file` (an artifact) or `target_path` (a string).
    /// Exactly one must be provided.
    ///
    /// When `target_file` is used, creates a symlink tracked by the artifact's content.
    /// When `target_path` is used, creates an unresolved symlink pointing to the given path
    /// (the output should be declared via `ctx.actions.declare_symlink()`).
    ///
    /// See: https://bazel.build/rules/lib/actions#symlink
    #[allow(clippy::too_many_arguments)]
    fn symlink<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] output: OutputArtifactArg<'v>,
        #[starlark(require = named, default = NoneOr::None)] target_file: NoneOr<
            ValueAsInputArtifactLike<'v>,
        >,
        #[starlark(require = named, default = NoneOr::None)] target_path: NoneOr<&str>,
        #[starlark(require = named, default = NoneOr::None)] target_type: NoneOr<&str>,
        #[starlark(require = named, default = false)] is_executable: bool,
        #[starlark(require = named, default = NoneOr::None)] progress_message: NoneOr<&str>,
        #[starlark(require = named, default = false)] use_exec_root_for_source: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (
            is_executable,
            progress_message,
            use_exec_root_for_source,
            target_type,
        );

        match (target_file.into_option(), target_path.into_option()) {
            (Some(target_file), None) => {
                // Artifact-backed symlink (content-tracked)
                let result = copy_file_impl(
                    eval,
                    this,
                    output,
                    target_file,
                    CopyMode::Symlink,
                    OutputType::FileOrDirectory,
                    None,
                )?;
                Ok(result.to_value())
            }
            (None, Some(path)) => {
                // String path symlink (unresolved, for declare_symlink outputs)
                let action = UnregisteredSymlinkPathAction::new(path.to_owned());
                let mut this = this.state()?;
                let (declaration, output_artifact) =
                    this.get_or_declare_output(eval, output, OutputType::FileOrDirectory, None)?;
                this.register_action(indexset![output_artifact], action, None, None)?;
                Ok(declaration
                    .into_declared_artifact(AssociatedArtifacts::new())
                    .to_value())
            }
            (Some(_), Some(_)) => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "ctx.actions.symlink(): exactly one of target_file or target_path must be specified, got both"
            )
            .into()),
            (None, None) => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "ctx.actions.symlink(): exactly one of target_file or target_path must be specified, got neither"
            )
            .into()),
        }
    }

    /// Make a copy of a directory.
    fn copy_dir<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] dest: OutputArtifactArg<'v>,
        #[starlark(require = pos)] src: ValueAsInputArtifactLike<'v>,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] executable_bit_override: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        Ok(copy_file_impl(
            eval,
            this,
            dest,
            src,
            CopyMode::Copy {
                executable_bit_override: executable_bit_override.into_option(),
            },
            OutputType::Directory,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?)
    }

    /// Returns an `artifact` that is a directory containing symlinks.
    /// The srcs must be a dictionary of path (as string, relative to the result directory) to bound `artifact`, which will be laid out in the directory.
    fn symlinked_dir<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] output: OutputArtifactArg<'v>,
        #[starlark(require = pos)] srcs: UnpackDictEntries<&'v str, ValueAsInputArtifactLike<'v>>,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        Ok(create_dir_tree(
            eval,
            this,
            output,
            srcs,
            CopyMode::Symlink,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?)
    }

    /// Returns an `artifact` which is a directory containing copied files.
    /// The srcs must be a dictionary of path (as string, relative to the result directory) to the bound `artifact`, which will be laid out in the directory.
    fn copied_dir<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] output: OutputArtifactArg<'v>,
        #[starlark(require = pos)] srcs: UnpackDictEntries<&'v str, ValueAsInputArtifactLike<'v>>,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] executable_bit_override: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        Ok(create_dir_tree(
            eval,
            this,
            output,
            srcs,
            CopyMode::Copy {
                executable_bit_override: executable_bit_override.into_option(),
            },
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?)
    }
}
