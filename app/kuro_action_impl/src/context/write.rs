/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use either::Either;
use fxhash::FxHashMap;
use indexmap::indexset;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::artifact_type::OutputArtifact;
use kuro_build_api::actions::impls::json::JsonUnpack;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_api::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use kuro_build_api::interpreter::rule_defs::artifact::output_artifact_like::OutputArtifactArg;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use kuro_build_api::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use kuro_build_api::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineArgLike;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use kuro_build_api::interpreter::rule_defs::cmd_args::CommandLineContext;
use kuro_build_api::interpreter::rule_defs::cmd_args::StarlarkCmdArgs;
use kuro_build_api::interpreter::rule_defs::cmd_args::StarlarkCommandLineValueUnpack;
use kuro_build_api::interpreter::rule_defs::cmd_args::WriteToFileMacroVisitor;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::CommandLineArg;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::resolved_macro::ResolvedMacro;
use kuro_execute::execute::request::OutputType;
use relative_path::RelativePathBuf;
use sha1::Digest;
use sha1::Sha1;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOf;
use starlark::values::ValueTyped;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::type_repr::StarlarkTypeRepr;
use starlark_map::small_set::SmallSet;

use crate::actions::impls::write::UnregisteredWriteAction;
use crate::actions::impls::write_json::UnregisteredWriteJsonAction;
use crate::actions::impls::write_macros::UnregisteredWriteMacrosToFileAction;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum WriteActionError {
    #[error(
        "Argument type attributes detected in a content to be written into a file, but support for arguments was not turned on. Use `allow_args` parameter to turn on the support for arguments."
    )]
    ArgAttrsDetectedButNotAllowed,
}

#[derive(UnpackValue, StarlarkTypeRepr)]
enum WriteContentArg<'v> {
    CommandLineArg(CommandLineArg<'v>),
    StarlarkCommandLineValueUnpack(StarlarkCommandLineValueUnpack<'v>),
}

/// We don't need to run this visitor in order to provide the inputs to the write actions,
/// because that is done lazily when we run the action.
/// However, we do need to always run this visitor, because it verifies that any content-based
/// inputs are bound. It will also collect "associated artifacts", if requested.
struct CommandLineInputVisitor {
    associated_artifacts: SmallSet<ArtifactGroup>,
    with_associated_artifacts: bool,
}

impl CommandLineInputVisitor {
    fn new(with_associated_artifacts: bool) -> Self {
        Self {
            associated_artifacts: Default::default(),
            with_associated_artifacts,
        }
    }
}

impl<'v> CommandLineArtifactVisitor<'v> for CommandLineInputVisitor {
    fn visit_input(&mut self, input: ArtifactGroup, _tags: Vec<&ArtifactTag>) {
        if self.with_associated_artifacts {
            self.associated_artifacts.insert(input.dupe());
        }
    }

    fn visit_declared_output(&mut self, _artifact: OutputArtifact<'v>, _tags: Vec<&ArtifactTag>) {}

    fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}

    fn visit_declared_artifact(
        &mut self,
        declared_artifact: kuro_artifact::artifact::artifact_type::DeclaredArtifact<'v>,
        tags: Vec<&ArtifactTag>,
    ) -> kuro_error::Result<()> {
        if self.with_associated_artifacts || declared_artifact.has_content_based_path() {
            let artifact = declared_artifact.ensure_bound()?.into_artifact();
            self.visit_input(ArtifactGroup::Artifact(artifact), tags);
        }

        Ok(())
    }
}

/// A TemplateDict for use with `ctx.actions.expand_template(computed_substitutions=...)`.
///
/// Created via `ctx.actions.template_dict()`. Accumulates key-value substitution pairs
/// that are lazily computed and applied when `expand_template` runs.
///
/// Example:
/// ```python
/// d = ctx.actions.template_dict()
/// d.add("{VERSION}", "1.2.3")
/// d.add_joined("{FLAGS}", feature_flags, join_with = ",")
/// ctx.actions.expand_template(
///     template = tmpl,
///     output = out,
///     computed_substitutions = d,
/// )
/// ```
#[derive(Debug, ProvidesStaticType, NoSerialize, allocative::Allocative)]
pub struct StarlarkTemplateDict {
    /// Accumulated substitution pairs: (pattern, replacement)
    entries: std::cell::RefCell<Vec<(String, String)>>,
}

unsafe impl<'v> starlark::values::Trace<'v> for StarlarkTemplateDict {
    fn trace(&mut self, _tracer: &starlark::values::Tracer<'v>) {}
}

/// Wrapper for unpacking StarlarkTemplateDict from a Value in starlark_module methods.
struct RefTemplateDict<'v>(&'v StarlarkTemplateDict);

impl<'v> StarlarkTypeRepr for RefTemplateDict<'v> {
    type Canonical = <StarlarkTemplateDict as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        StarlarkTemplateDict::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefTemplateDict<'v> {
    type Error = std::convert::Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(td) = value.downcast_ref::<StarlarkTemplateDict>() else {
            return Ok(None);
        };
        Ok(Some(RefTemplateDict(td)))
    }
}

impl std::fmt::Display for StarlarkTemplateDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<template_dict>")
    }
}

impl StarlarkTemplateDict {
    pub fn new() -> Self {
        Self {
            entries: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Apply all accumulated substitutions to the given content.
    pub fn apply_to(&self, mut content: String) -> String {
        for (key, val) in self.entries.borrow().iter() {
            content = content.replace(key.as_str(), val.as_str());
        }
        content
    }

    /// Snapshot of accumulated `(pattern, replacement)` pairs for handing
    /// off to a deferred action (Plan 42). Returns owned strings since the
    /// action lives past the analysis-time scope this dict was created in.
    pub fn entries_for_action(&self) -> Vec<(String, String)> {
        self.entries.borrow().clone()
    }
}

impl<'v> AllocValue<'v> for StarlarkTemplateDict {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Methods for StarlarkTemplateDict (accessed via `template_dict.<method>`).
#[starlark_module]
fn template_dict_methods(builder: &mut MethodsBuilder) {
    /// Adds a static key-value substitution.
    ///
    /// `key` is the pattern to find in the template; `value` is the replacement.
    fn add<'v>(this: RefTemplateDict<'v>, key: &str, value: &str) -> starlark::Result<NoneType> {
        this.0
            .entries
            .borrow_mut()
            .push((key.to_owned(), value.to_owned()));
        Ok(NoneType)
    }

    /// Adds a substitution by joining multiple values from a list or depset.
    ///
    /// `key` is the pattern; `values` is a list/depset of items; `join_with` is the separator.
    /// Optional `map_each` transforms each item before joining.
    fn add_joined<'v>(
        this: RefTemplateDict<'v>,
        key: &str,
        values: Value<'v>,
        #[starlark(require = named)] join_with: &str,
        #[starlark(require = named, default = NoneType)] map_each: Value<'v>,
        #[starlark(require = named, default = false)] omit_if_empty: bool,
        #[starlark(require = named, default = false)] uniquify: bool,
        #[starlark(require = named, default = false)] allow_closure: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = allow_closure;
        // Collect items from the values (handles list, depset, or single value)
        let items: Vec<Value<'v>> = if let Ok(iter) = values.iterate(eval.heap()) {
            iter.collect()
        } else {
            vec![values]
        };

        let mut string_values: Vec<String> = Vec::new();
        for item in items {
            let item_str = if !map_each.is_none() {
                // Apply map_each callback to transform the item
                let result = eval.eval_function(map_each, &[item], &[])?;
                if result.is_none() {
                    continue;
                }
                result.to_str()
            } else {
                item.to_str()
            };
            string_values.push(item_str);
        }

        if uniquify {
            let mut seen = std::collections::HashSet::new();
            string_values.retain(|s| seen.insert(s.clone()));
        }

        if omit_if_empty && string_values.is_empty() {
            return Ok(NoneType);
        }

        let joined = string_values.join(join_with);
        this.0.entries.borrow_mut().push((key.to_owned(), joined));
        Ok(NoneType)
    }
}

#[starlark_value(type = "template_dict")]
impl<'v> StarlarkValue<'v> for StarlarkTemplateDict {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(template_dict_methods)
    }
}

#[starlark_module]
pub(crate) fn analysis_actions_methods_write(methods: &mut MethodsBuilder) {
    /// Creates a new TemplateDict for use with `expand_template(computed_substitutions=...)`.
    ///
    /// A TemplateDict accumulates key-value substitution pairs via `add()` and `add_joined()`.
    ///
    /// Example:
    /// ```python
    /// d = ctx.actions.template_dict()
    /// d.add("{VERSION}", "1.2.3")
    /// ctx.actions.expand_template(
    ///     template = tmpl,
    ///     output = out,
    ///     computed_substitutions = d,
    /// )
    /// ```
    fn template_dict<'v>(
        this: &AnalysisActions<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(eval.heap().alloc(StarlarkTemplateDict::new()))
    }

    /// Returns an `artifact` whose contents are `content` written as a JSON value.
    ///
    /// * `output`: can be a string, or an existing artifact created with `declare_output`
    /// * `content`:  must be composed of the basic json types (boolean, number, string, list/tuple,
    ///   dictionary) plus artifacts and command lines
    ///     * An artifact will be written as a string containing the path
    ///     * A command line will be written as a list of strings, unless `joined=True` is set, in
    ///       which case it will be a string
    /// * If you pass `with_inputs = True`, you'll get back a `cmd_args` that expands to the JSON
    ///   file but carries all the underlying inputs as dependencies (so you don't have to use, for
    ///   example, `hidden` for them to be added to an action that already receives the JSON file)
    /// * `pretty` (optional): write formatted JSON (defaults to `False`)
    /// * `absolute` (optional): if set, this action will produce absolute paths in its output when
    ///   rendering artifact paths. You generally shouldn't use this if you plan to use this action
    ///   as the input for anything else, as this would effectively result in losing all shared
    ///   caching. (defaults to `False`)
    fn write_json<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] output: OutputArtifactArg<'v>,
        #[starlark(require = pos)] content: ValueOf<'v, JsonUnpack<'v>>,
        #[starlark(require = named, default = false)] with_inputs: bool,
        #[starlark(require = named, default = false)] pretty: bool,
        #[starlark(require = named, default = false)] absolute: bool,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        #[starlark(require = named, default = false)]
        use_dep_files_placeholder_for_content_based_paths: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<impl AllocValue<'v> + use<'v>> {
        let mut this = this.state()?;
        let (declaration, output_artifact) = this.get_or_declare_output(
            eval,
            output,
            OutputType::File,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?;

        let value = declaration.into_declared_artifact(AssociatedArtifacts::new());
        let cli = UnregisteredWriteJsonAction::cli(value.to_value(), content.value)?;

        let mut visitor = CommandLineInputVisitor::new(false);
        cli.visit_contents(&mut visitor)?;

        this.register_action(
            indexset![output_artifact],
            UnregisteredWriteJsonAction::new(
                pretty,
                absolute,
                use_dep_files_placeholder_for_content_based_paths,
            ),
            Some(content.value),
            None,
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;

        // TODO(cjhopman): The with_inputs thing can go away once we have artifact dependencies (we'll still
        // need the UnregisteredWriteJsonAction::cli() to represent the dependency though).
        if with_inputs {
            // TODO(nga): we use `AllocValue`, so this function return type for this branch
            //   is `write_json_cli_args`. We want just `cmd_args`,
            //   because users don't care about precise type.
            //   Do it when we migrate to new types not based on strings.
            let cli = UnregisteredWriteJsonAction::cli(value.to_value(), content.value)?;
            Ok(Either::Right(cli))
        } else {
            Ok(Either::Left(value))
        }
    }

    /// Returns an `artifact` whose contents are `content`
    ///
    /// * `is_executable` (optional): indicates whether the resulting file should be marked with
    ///   executable permissions
    /// * `allow_args` (optional): must be set to `True` if you want to write parameter arguments to
    ///   the file (in particular, macros that write to file)
    ///     * If it is true, the result will be a pair of the `artifact` containing content and a
    ///       list of artifact values that were written by macros, which should be used in hidden
    ///       fields or similar
    /// * `with_inputs` (optional): if set, add artifacts in `content` as associated artifacts of the return `artifact`.
    /// * `absolute` (optional): if set, this action will produce absolute paths in its output when
    ///   rendering artifact paths. You generally shouldn't use this if you plan to use this action
    ///   as the input for anything else, as this would effectively result in losing all shared
    ///   caching.
    ///
    /// The content is often a string, but can be any `ArgLike` value. This is occasionally useful
    /// for generating scripts to run as a part of another action. `cmd_args` in the content are
    /// newline separated unless another delimiter is explicitly specified.
    fn write<'v>(
        this: &AnalysisActions<'v>,
        output: OutputArtifactArg<'v>,
        content: WriteContentArg<'v>,
        // Bazel allows is_executable as positional: ctx.actions.write(output, content, is_executable)
        #[starlark(default = false)] is_executable: bool,
        #[starlark(require = named, default = false)] allow_args: bool,
        // If set, add artifacts in content as associated artifacts of the output. This will only work for bound artifacts.
        #[starlark(require = named, default = false)] with_inputs: bool,
        #[starlark(require = named, default = false)] absolute: bool,
        #[starlark(require = named, default = NoneOr::None)]
        uses_experimental_content_based_path_hashing: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] has_content_based_path: NoneOr<bool>,
        #[starlark(require = named, default = false)]
        use_dep_files_placeholder_for_content_based_paths: bool,
        // Bazel 9 compatibility: accepted but ignored
        #[starlark(require = named, default = NoneOr::None)] mnemonic: NoneOr<&str>,
        #[starlark(require = named, default = NoneOr::None)] execution_requirements: NoneOr<
            Value<'v>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<
        Either<
            ValueTyped<'v, StarlarkDeclaredArtifact<'v>>,
            (
                ValueTyped<'v, StarlarkDeclaredArtifact<'v>>,
                Vec<StarlarkDeclaredArtifact<'v>>,
            ),
        >,
    > {
        // Bazel 9 params accepted for compatibility
        let _ = (mnemonic, execution_requirements);

        fn count_write_to_file_macros(
            args_allowed: bool,
            cli: &dyn CommandLineArgLike,
        ) -> kuro_error::Result<u32> {
            if !args_allowed && cli.contains_arg_attr() {
                return Err(WriteActionError::ArgAttrsDetectedButNotAllowed.into());
            }

            struct WriteToFileMacrosCounter {
                count: u32,
            }

            impl WriteToFileMacroVisitor for WriteToFileMacrosCounter {
                fn visit_write_to_file_macro(
                    &mut self,
                    _m: &ResolvedMacro,
                    _artifact_path_mapping: &dyn ArtifactPathMapper,
                ) -> kuro_error::Result<()> {
                    self.count += 1;
                    Ok(())
                }

                fn set_current_relative_to_path(
                    &mut self,
                    _gen: &dyn Fn(
                        &dyn CommandLineContext,
                    )
                        -> kuro_error::Result<Option<RelativePathBuf>>,
                ) -> kuro_error::Result<()> {
                    Ok(())
                }
            }

            let mut counter = WriteToFileMacrosCounter { count: 0 };
            // At this point the mapping doesn't matter because we're only doing a count
            cli.visit_write_to_file_macros(&mut counter, &FxHashMap::default())?;
            Ok(counter.count)
        }

        fn get_cli_inputs(
            with_inputs: bool,
            cli: &dyn CommandLineArgLike,
        ) -> kuro_error::Result<SmallSet<ArtifactGroup>> {
            let mut visitor = CommandLineInputVisitor::new(with_inputs);
            cli.visit_artifacts(&mut visitor)?;
            Ok(visitor.associated_artifacts)
        }

        let mut this = this.state()?;
        let (declaration, output_artifact) = this.get_or_declare_output(
            eval,
            output,
            OutputType::File,
            uses_experimental_content_based_path_hashing
                .into_option()
                .or(has_content_based_path.into_option()),
        )?;

        let (content_cli, written_macro_count, mut associated_artifacts) = match content {
            WriteContentArg::CommandLineArg(content) => {
                let content_arg = content.as_command_line_arg();
                let count = count_write_to_file_macros(allow_args, content_arg)?;
                let associated_artifacts = get_cli_inputs(with_inputs, content_arg)?;
                (content, count, associated_artifacts)
            }
            WriteContentArg::StarlarkCommandLineValueUnpack(content) => {
                let cli = StarlarkCmdArgs::try_from_value_typed(content)?;
                let count = count_write_to_file_macros(allow_args, &cli)?;
                let associated_artifacts = get_cli_inputs(with_inputs, &cli)?;
                (
                    CommandLineArg::from_cmd_args(eval.heap().alloc_typed(cli)),
                    count,
                    associated_artifacts,
                )
            }
        };

        let path_resolution_method = output_artifact.path_resolution_method();

        let written_macro_files = if written_macro_count > 0 {
            let macro_directory_path = {
                // There might be several write actions at once, use write action output hash to deterministically avoid collisions for .macro files.
                let digest = output_artifact
                    .get_path()
                    .with_full_path(|path| Sha1::digest(path.as_str().as_bytes()));
                let sha = hex::encode(digest);
                format!("__macros/{sha}")
            };

            let mut written_macro_files = indexset![];
            for i in 0..written_macro_count {
                let macro_file = this.declare_output(
                    None,
                    &format!("{}/{}.macro", &macro_directory_path, i),
                    OutputType::File,
                    eval.call_stack_top_location(),
                    path_resolution_method,
                    eval.heap(),
                )?;
                written_macro_files.insert(macro_file);
            }

            let state = &mut *this;
            let action = UnregisteredWriteMacrosToFileAction::new(
                output_artifact
                    .get_path()
                    .with_short_path(|p| p.to_string()),
                use_dep_files_placeholder_for_content_based_paths,
            );
            state.register_action(
                written_macro_files.iter().map(|a| a.as_output()).collect(),
                action,
                Some(content_cli.to_value()),
                None,
                None,
                std::sync::Arc::new(std::collections::BTreeMap::new()),
            )?;

            written_macro_files
        } else {
            indexset![]
        };

        let action = {
            let maybe_macro_files = if allow_args {
                let mut macro_files = indexset![];
                for a in &written_macro_files {
                    let artifact = a.dupe().ensure_bound()?.into_artifact();
                    macro_files.insert(artifact.dupe());
                }
                Some(macro_files)
            } else {
                None
            };
            UnregisteredWriteAction {
                is_executable,
                macro_files: maybe_macro_files,
                absolute,
                use_dep_files_placeholder_for_content_based_paths,
            }
        };
        this.register_action(
            indexset![output_artifact],
            action,
            Some(content_cli.to_value()),
            None,
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;

        if allow_args {
            for a in &written_macro_files {
                associated_artifacts.insert(ArtifactGroup::Artifact(
                    a.dupe().ensure_bound()?.into_artifact(),
                ));
            }
        }

        let value =
            declaration.into_declared_artifact(AssociatedArtifacts::from(associated_artifacts));
        if allow_args {
            let macro_files: Vec<StarlarkDeclaredArtifact> = written_macro_files
                .into_iter()
                .map(|a| StarlarkDeclaredArtifact::new(None, a, AssociatedArtifacts::new()))
                .collect();
            Ok(Either::Right((value, macro_files)))
        } else {
            // Prefer simpler API when there is no possibility for write-to-file macros to be present in a content
            Ok(Either::Left(value))
        }
    }

    /// Bazel-compatible alias for `write`.
    ///
    /// `ctx.actions.write_file(output, content, is_executable)` is the Bazel name
    /// for what Buck2 calls `ctx.actions.write(output, content, is_executable)`.
    fn write_file<'v>(
        this: &AnalysisActions<'v>,
        output: OutputArtifactArg<'v>,
        content: WriteContentArg<'v>,
        #[starlark(default = false)] is_executable: bool,
        #[starlark(require = named, default = false)] allow_args: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<
        Either<
            ValueTyped<'v, StarlarkDeclaredArtifact<'v>>,
            (
                ValueTyped<'v, StarlarkDeclaredArtifact<'v>>,
                Vec<StarlarkDeclaredArtifact<'v>>,
            ),
        >,
    > {
        let mut state = this.state()?;
        let (declaration, output_artifact) =
            state.get_or_declare_output(eval, output, OutputType::File, None)?;

        let (content_cli, mut associated_artifacts) = match content {
            WriteContentArg::CommandLineArg(content) => {
                let content_arg = content.as_command_line_arg();
                if !allow_args && content_arg.contains_arg_attr() {
                    let e: kuro_error::Error =
                        WriteActionError::ArgAttrsDetectedButNotAllowed.into();
                    return Err(e.into());
                }
                let mut visitor = CommandLineInputVisitor::new(false);
                content_arg.visit_artifacts(&mut visitor)?;
                (content, visitor.associated_artifacts)
            }
            WriteContentArg::StarlarkCommandLineValueUnpack(content) => {
                let cli = StarlarkCmdArgs::try_from_value_typed(content)?;
                if !allow_args && cli.contains_arg_attr() {
                    let e: kuro_error::Error =
                        WriteActionError::ArgAttrsDetectedButNotAllowed.into();
                    return Err(e.into());
                }
                let mut visitor = CommandLineInputVisitor::new(false);
                cli.visit_artifacts(&mut visitor)?;
                let associated = visitor.associated_artifacts;
                (
                    CommandLineArg::from_cmd_args(eval.heap().alloc_typed(cli)),
                    associated,
                )
            }
        };

        let action = UnregisteredWriteAction {
            is_executable,
            macro_files: None,
            absolute: false,
            use_dep_files_placeholder_for_content_based_paths: false,
        };
        state.register_action(
            indexset![output_artifact],
            action,
            Some(content_cli.to_value()),
            None,
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;

        let value =
            declaration.into_declared_artifact(AssociatedArtifacts::from(associated_artifacts));
        Ok(Either::Left(value))
    }

    /// Expands a template file with substitutions (Bazel-compatible).
    ///
    /// Creates a new file by reading a template and replacing substitution patterns.
    ///
    /// * `template`: The template file (input artifact)
    /// * `output`: The output file to create
    /// * `substitutions`: Dictionary of string replacements (key -> value)
    /// * `is_executable`: Whether the output should be executable (default: False)
    /// * `computed_substitutions`: Optional computed substitutions (not yet supported)
    #[allow(unused_variables)]
    fn expand_template<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] template: starlark::values::Value<'v>,
        #[starlark(require = named)] output: OutputArtifactArg<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        substitutions: starlark::values::Value<'v>,
        #[starlark(require = named, default = false)] is_executable: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        computed_substitutions: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<starlark::values::none::NoneType> {
        // Plan 42: register a deferred `ExpandTemplateAction` so the template
        // file is materialized as an action input before we read it at
        // execution time. Source-file and build-artifact templates flow
        // through the same path; for source files, the action input
        // dependency is essentially free.

        let mut this = this.state()?;
        let (declaration, output_artifact) =
            this.get_or_declare_output(eval, output, OutputType::File, None)?;

        let artifact_like = <&dyn kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike>::unpack_value(template)?
            .ok_or_else(|| kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "expand_template: `template` must be an input artifact, got {}",
                template.to_repr()
            ))?;
        let template_group = artifact_like.get_artifact_group()?;

        // Collect substitutions in declaration order. Bazel applies them
        // sequentially as substring replacements.
        let mut subs: Vec<(String, String)> = Vec::new();
        if !substitutions.is_none() {
            if let Some(dict) = starlark::values::dict::DictRef::from_value(substitutions) {
                for (key, value) in dict.iter() {
                    if let (Some(k), Some(v)) = (key.unpack_str(), value.unpack_str()) {
                        subs.push((k.to_owned(), v.to_owned()));
                    }
                }
            }
        }
        if !computed_substitutions.is_none() {
            if let Some(tdict) = computed_substitutions.downcast_ref::<StarlarkTemplateDict>() {
                for (k, v) in tdict.entries_for_action() {
                    subs.push((k, v));
                }
            }
        }

        let action = crate::actions::impls::expand_template::UnregisteredExpandTemplateAction {
            template: template_group,
            substitutions: subs,
            is_executable,
        };
        this.register_action(
            indexset![output_artifact],
            action,
            None,
            None,
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;

        let _ = declaration;
        Ok(starlark::values::none::NoneType)
    }

    /// Bazel build stamping: transform version file contents.
    ///
    /// In Bazel, this reads the volatile workspace status file, applies `transform_func`,
    /// and writes the result using the template. Kuro stubs this to create an empty
    /// output file since build stamping is not yet implemented.
    #[allow(unused_variables)]
    fn transform_version_file<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] transform_func: starlark::values::Value<'v>,
        #[starlark(require = named)] template: starlark::values::Value<'v>,
        #[starlark(require = named)] output_file_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        let name = eval.heap().alloc_str(output_file_name);
        stub_transform_file(this, name.as_str(), eval)
    }

    /// Bazel build stamping: transform info file contents.
    ///
    /// In Bazel, this reads the stable workspace status file, applies `transform_func`,
    /// and writes the result using the template. Kuro stubs this to create an empty
    /// output file since build stamping is not yet implemented.
    #[allow(unused_variables)]
    fn transform_info_file<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] transform_func: starlark::values::Value<'v>,
        #[starlark(require = named)] template: starlark::values::Value<'v>,
        #[starlark(require = named)] output_file_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
        let name = eval.heap().alloc_str(output_file_name);
        stub_transform_file(this, name.as_str(), eval)
    }
}

/// Shared implementation for `transform_version_file` and `transform_info_file` stubs.
/// Creates an output file with placeholder content.
fn stub_transform_file<'v>(
    this: &AnalysisActions<'v>,
    output_file_name: &'v str,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<ValueTyped<'v, StarlarkDeclaredArtifact<'v>>> {
    let mut state = this.state()?;
    let (declaration, output_artifact) = state.get_or_declare_output(
        eval,
        OutputArtifactArg::Str(output_file_name),
        OutputType::File,
        None,
    )?;
    let content_str = eval.heap().alloc_str("// Build stamping not implemented\n");
    let cmd_args = StarlarkCmdArgs::try_from_value(content_str.to_value())?;
    let content_cli = CommandLineArg::from_cmd_args(eval.heap().alloc_typed(cmd_args));
    let action = UnregisteredWriteAction {
        is_executable: false,
        macro_files: None,
        absolute: false,
        use_dep_files_placeholder_for_content_based_paths: false,
    };
    state.register_action(
        indexset![output_artifact],
        action,
        Some(content_cli.to_value()),
        None,
        None,
        std::sync::Arc::new(std::collections::BTreeMap::new()),
    )?;
    Ok(declaration.into_declared_artifact(AssociatedArtifacts::new()))
}
