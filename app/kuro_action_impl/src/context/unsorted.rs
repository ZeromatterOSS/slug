/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use indexmap::indexset;
use kuro_build_api::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkArtifactLike;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_output_artifact::StarlarkOutputArtifact;
use kuro_build_api::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use kuro_build_api::interpreter::rule_defs::cmd_args::StarlarkCmdArgs;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::CommandLineArg;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::digest_config::StarlarkDigestConfig;
use kuro_build_api::interpreter::rule_defs::transitive_set::FrozenTransitiveSetDefinition;
use kuro_build_api::interpreter::rule_defs::transitive_set::TransitiveSet;
use kuro_core::fs::buck_out_path::BuckOutPathKind;
use kuro_execute::execute::request::OutputType;
use starlark::environment::MethodsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenValueTyped;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueTyped;
use starlark::values::typing::StarlarkIter;

use crate::actions::impls::write::UnregisteredWriteAction;

/// Extract the parent directory from a sibling artifact to use as prefix.
///
/// In Bazel, `declare_file("foo.o", sibling=some_artifact)` places the new file
/// in the same output directory as `some_artifact`. We implement this by extracting
/// the parent directory from the sibling's short path and using it as the prefix
/// for `declare_output`.
fn sibling_to_prefix<'v>(sibling: Value<'v>) -> starlark::Result<Option<String>> {
    if sibling.is_none() {
        return Ok(None);
    }

    if let Some(artifact_like) = <&dyn StarlarkArtifactLike<'v>>::unpack_value(sibling)? {
        // Use Cell to extract the parent dir from within the with_short_path closure
        let parent_dir = std::cell::Cell::new(None);
        artifact_like
            .with_short_path(&|path| {
                if let Some(parent) = path.parent() {
                    let s = parent.as_str();
                    if !s.is_empty() {
                        parent_dir.set(Some(s.to_owned()));
                    }
                }
                // The return value is required by the trait but we don't use it.
                starlark::values::StringValue::default()
            })
            .map_err(starlark::Error::from)?;
        Ok(parent_dir.into_inner())
    } else if let Some(s) = sibling.unpack_str() {
        if let Some(idx) = s.rfind('/') {
            let parent = &s[..idx];
            if parent.is_empty() {
                Ok(None)
            } else {
                Ok(Some(parent.to_owned()))
            }
        } else {
            Ok(None)
        }
    } else {
        Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Expected File or declared artifact for `sibling`, got `{}`",
            sibling.get_type()
        )
        .into())
    }
}

#[starlark_module]
pub(crate) fn analysis_actions_methods_unsorted(builder: &mut MethodsBuilder) {
    /// Returns an unbound `artifact`, representing where a file will go, which must be bound before analysis terminates.
    /// The usual way of binding an artifact is with `ctx.actions.run`. As an example:
    ///
    /// ```python
    /// my_output = ctx.actions.declare_output("output.o")
    /// ctx.actions.run(["gcc", "-c", my_source, "-o", my_output.as_output()], category = "compile")
    /// ```
    ///
    /// This snippet declares an output with the filename `output.o` (it will be located in the output directory
    /// for this target). Note the use of `as_output` to tag this artifact as being an output in
    /// the action. After binding the artifact you can subsequently use `my_output` as either an
    /// input for subsequent actions, or as the result in a provider.
    ///
    /// Artifacts from a single target may not have the same name, so if you then want a second
    /// artifact also named `output.o` you need to supply a prefix, e.g.
    /// `ctx.actions.declare_output("directory", "output.o")`. The artifact will still report having
    /// name `output.o`, but will be located at `directory/output.o`.
    ///
    /// The `dir` argument should be set to `True` if the binding will be a directory.
    fn declare_output<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] prefix: &str,
        #[starlark(require = pos)] filename: Option<&str>,
        #[starlark(require = named, default = false)] dir: bool,
        #[starlark(require = named, default = false)] uses_experimental_content_based_path_hashing: bool,
        #[starlark(require = named, default = false)] has_content_based_path: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        // We take either one or two positional arguments, namely (filename) or (prefix, filename).
        // The prefix argument is optional, but first, so we pretend the filename is optional
        // and fix them up here.
        let (prefix, filename) = match filename {
            None => (None, prefix),
            Some(filename) => (Some(prefix), filename),
        };

        let output_type = if dir {
            OutputType::Directory
        } else {
            OutputType::FileOrDirectory
        };
        let path_resolution_method =
            if uses_experimental_content_based_path_hashing || has_content_based_path {
                BuckOutPathKind::ContentHash
            } else {
                BuckOutPathKind::Configuration
            };
        let artifact = this.state()?.declare_output(
            prefix,
            filename,
            output_type,
            eval.call_stack_top_location(),
            path_resolution_method,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible alias for `declare_output`.
    /// Declares an output file that will be created by an action.
    ///
    /// When `sibling` is provided, the new file is placed in the same directory
    /// as the sibling artifact. This is useful for rules that need to create
    /// output files alongside other outputs (e.g., .o files next to .cc files).
    fn declare_file<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::FileOrDirectory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible alias for declaring directory outputs.
    /// Declares an output directory that will be created by an action.
    ///
    /// When `sibling` is provided, the new directory is placed in the same
    /// directory as the sibling artifact.
    fn declare_directory<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::Directory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible: declares a shareable artifact.
    ///
    /// In Bazel, shareable artifacts can be shared across configurations (e.g.,
    /// Android split configurations). In Kuro, this is an alias for `declare_file`
    /// since we don't yet implement cross-configuration artifact sharing.
    /// Used by rules_cc for virtual includes, linkstamp objects, and LTO artifacts.
    fn declare_shareable_artifact<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::FileOrDirectory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible: declares a shareable directory artifact.
    ///
    /// Like `declare_shareable_artifact` but for directory outputs.
    /// Used by rules_cc's LTO backend for creating tree artifacts.
    fn declare_shareable_directory<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::Directory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible: declares a symlink artifact.
    ///
    /// In Bazel, this is only available with `--experimental_allow_unresolved_symlinks`.
    /// The returned artifact represents a symlink that will be created by
    /// `ctx.actions.symlink()`. We implement this as a regular file artifact
    /// since Buck2/Kuro handles symlinks transparently during execution.
    fn declare_symlink<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        // Symlink artifacts are treated as regular file artifacts in Kuro.
        // The actual symlink creation happens via ctx.actions.symlink().
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::FileOrDirectory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Bazel-compatible: declare a shareable artifact.
    /// In Bazel, this creates an artifact that can be shared across targets.
    /// We implement it as a simple declare_output alias.
    fn declare_shareable_artifact<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        sibling: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let prefix = sibling_to_prefix(sibling)?;
        let artifact = this.state()?.declare_output(
            prefix.as_deref(),
            filename,
            OutputType::FileOrDirectory,
            eval.call_stack_top_location(),
            BuckOutPathKind::Configuration,
            eval.heap(),
        )?;

        Ok(StarlarkDeclaredArtifact::new(
            eval.call_stack_top_location(),
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    /// Creates a new transitive set. For details, see https://kuro.build/docs/rule_authors/transitive_sets/.
    fn tset<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] definition: FrozenValueTyped<'v, FrozenTransitiveSetDefinition>,
        #[starlark(require = named)] value: Option<Value<'v>>,
        #[starlark(require = named)] children: Option<
            ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, TransitiveSet<'v>>> {
        let mut this = this.state()?;
        this.create_transitive_set(definition, value, children.map(|v| v.get()), eval)
    }

    /// Create a unique artifact tag for tracking input dependencies with dependency files.
    ///
    /// An `ArtifactTag` is used to associate inputs and outputs in a build action with a dependency
    /// file (depfile), enabling more accurate incremental builds by tracking which inputs were
    /// actually used during action execution.
    ///
    /// ### How Dependency Files Work
    ///
    /// Traditional build systems face a dilemma: if an action depends on 1000 header files but only
    /// uses 10, should changing any of the 1000 trigger a rebuild? Being conservative (always rebuild)
    /// is slow, while being optimistic (never rebuild) risks correctness.
    ///
    /// Dependency files solve this by letting the build tool (e.g., GCC, Clang, Swift compiler) report
    /// which inputs it actually used:
    ///
    /// 1. **First build**: The action runs with all potential inputs available
    /// 2. **Tool generates depfile**: The compiler writes a file listing only the inputs it actually read
    /// 3. **Subsequent builds**: Kuro only triggers rebuilds when files listed in the depfile change
    ///
    /// ### Usage Pattern
    ///
    /// The typical workflow involves three steps:
    ///
    /// 1. Create a unique tag with `ctx.actions.artifact_tag()`
    /// 2. Use the tag's `tag_artifacts()` method to mark:
    ///    - All potential input dependencies
    ///    - The depfile output
    /// 3. Associate the tag with a label in the `dep_files` parameter of `ctx.actions.run()`
    ///
    /// ### Example: C++ Compilation
    ///
    /// ```python
    /// def _compile_impl(ctx):
    ///     # Step 1: Create a unique tag for tracking header dependencies
    ///     headers_tag = ctx.actions.artifact_tag()
    ///
    ///     # Prepare inputs and outputs
    ///     headers_dir = ctx.actions.copied_dir("headers", {...})
    ///     dep_file = ctx.actions.declare_output("depfile")
    ///     output = ctx.actions.declare_output("output.o")
    ///
    ///     # Step 2: Tag the inputs and depfile output
    ///     cmd = cmd_args([
    ///         "gcc", "-c", "main.cpp",
    ///         "-I", headers_tag.tag_artifacts(headers_dir),  # Mark potential inputs
    ///         "-o", output.as_output(),
    ///         "-MMD",                                        # Tell GCC to generate depfile
    ///         "-MF", headers_tag.tag_artifacts(dep_file.as_output()),  # Mark depfile output
    ///     ])
    ///
    ///     # Step 3: Associate the tag with the "headers" label
    ///     ctx.actions.run(
    ///         cmd,
    ///         category = "cxx_compile",
    ///         dep_files = {"headers": headers_tag}
    ///     )
    /// ```
    ///
    /// In this example:
    /// - `headers_dir` contains 1000 header files
    /// - GCC generates `depfile` listing only the 10 headers actually used
    /// - On subsequent builds, only changes to those 10 headers trigger recompilation
    ///
    ///
    /// ### Depfile Format
    ///
    /// Dependency files use Makefile syntax:
    ///
    /// ```makefile
    /// output.o: main.cpp foo.h bar.h internal.h
    /// ```
    ///
    /// This tells Kuro that `output.o` depends on these specific files. Kuro reads this file
    /// after the action completes and uses it to determine which tagged inputs to track for
    /// future incremental builds.
    ///
    /// ### Return Value
    ///
    /// Returns a new `ArtifactTag` instance. Each call creates a unique tag that can be compared
    /// for equality, allowing Kuro to match tagged inputs with their corresponding depfiles.
    ///
    /// ### See Also
    ///
    /// - [`ArtifactTag.tag_artifacts()`](../ArtifactTag#artifacttagtag_artifacts): Tag both inputs and outputs
    /// - [`ArtifactTag.tag_inputs()`](../ArtifactTag#artifacttagtag_inputs): Tag only inputs
    /// - [`ctx.actions.run()`](../AnalysisActions#analysisactionsrun): The `dep_files` parameter documentation
    fn artifact_tag<'v>(this: &AnalysisActions<'v>) -> starlark::Result<ArtifactTag> {
        let _ = this;
        Ok(ArtifactTag::new())
    }

    /// Obtain this daemon's digest configuration. This allows rules to discover what digests the
    /// daemon may be able to e.g. defer download because they conform to its RE backend's expected
    /// digest format.
    fn digest_config<'v>(this: &AnalysisActions<'v>) -> starlark::Result<StarlarkDigestConfig> {
        Ok(StarlarkDigestConfig {
            digest_config: this.digest_config,
        })
    }

    /// Bazel-compatible args() for building command lines.
    ///
    /// Creates a new Args object for building command line arguments.
    /// This is equivalent to Kuro's cmd_args() global function.
    ///
    /// The Args object supports methods like:
    /// - `add(value)`: Add a value to the args
    /// - `add_all(values)`: Add all values from a list
    /// - `add_joined(values, join_with)`: Add values joined with a separator
    ///
    /// Example:
    /// ```python
    /// args = ctx.actions.args()
    /// args.add("--output", output_file)
    /// args.add_all(input_files)
    /// ctx.actions.run(arguments = args, ...)
    /// ```
    fn args<'v>(
        this: &AnalysisActions<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkCmdArgs<'v>>> {
        let _ = this;
        Ok(eval.heap().alloc_typed(StarlarkCmdArgs::default()))
    }

    /// Bazel-compatible: create a no-op placeholder action.
    ///
    /// `ctx.actions.do_nothing(mnemonic, inputs=[], outputs=[])` creates an action
    /// that does nothing but binds the specified outputs to the action graph.
    /// Used in rules that conditionally produce outputs.
    fn do_nothing<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = named)] mnemonic: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        inputs: starlark::values::Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        outputs: starlark::values::Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<starlark::values::none::NoneType> {
        let _ = (mnemonic, inputs);

        // Bind each output by registering a write action with empty content.
        // This ensures outputs aren't left unbound, which would cause analysis errors.
        if !outputs.is_none() {
            if let Ok(iter) = outputs.iterate(eval.heap()) {
                let mut state = this.state()?;
                for output_val in iter {
                    let output_artifact = if let Some(declared) =
                        output_val.downcast_ref::<StarlarkDeclaredArtifact>()
                    {
                        declared.output_artifact()
                    } else if let Some(output) = output_val.downcast_ref::<StarlarkOutputArtifact>()
                    {
                        output.artifact()
                    } else {
                        continue;
                    };
                    {
                        let action = UnregisteredWriteAction {
                            is_executable: false,
                            macro_files: None,
                            absolute: false,
                            use_dep_files_placeholder_for_content_based_paths: false,
                        };
                        // Write empty content to bind the output
                        let empty_args = StarlarkCmdArgs::default();
                        let cli = eval.heap().alloc_typed(empty_args);
                        let content = CommandLineArg::from_cmd_args(cli);
                        state.register_action(
                            indexset![output_artifact],
                            action,
                            Some(content.to_value()),
                            None,
                        )?;
                    }
                }
            }
        }
        Ok(starlark::values::none::NoneType)
    }

    /// Bazel-compatible: register an action that always fails with the given message.
    ///
    /// `ctx.actions.fail(message)` creates an action that, when executed, fails with
    /// the provided error message. This is used by rules to signal unsupported
    /// configurations or platforms at build time (rather than analysis time).
    ///
    /// In Bazel, this creates an action whose execution always fails.
    /// In Kuro, we fail immediately at analysis time since there's no benefit
    /// to deferring the error.
    fn fail<'v>(
        this: &AnalysisActions<'v>,
        #[starlark(require = pos)] message: &str,
    ) -> starlark::Result<starlark::values::none::NoneType> {
        let _ = this;
        Err(starlark::Error::new_other(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("action fail: {}", message),
        )))
    }
}
