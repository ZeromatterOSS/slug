/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use allocative::Allocative;
use compact_str::CompactString;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocImmutableDict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark::values::tuple::UnpackTuple;

use crate::builtin_restriction::CustomAllowlistEntry;
use crate::builtin_restriction::check_custom_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisActions;

mod sequence;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcCommonModule;
starlark::starlark_simple_value!(CcCommonModule);

impl fmt::Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_common")
    }
}

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "do_not_use_tools_cpp_compiler_present").then_some(Value::new_none())
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_common_methods)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcInternalModule;
starlark::starlark_simple_value!(CcInternalModule);

impl fmt::Display for CcInternalModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_internal")
    }
}

#[starlark_value(type = "cc_internal")]
impl<'v> StarlarkValue<'v> for CcInternalModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_internal_methods)
    }
}

#[starlark_module]
fn cc_internal_methods(builder: &mut MethodsBuilder) {
    fn get_link_args<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] build_variables: Value<'v>,
        #[starlark(require = named)] parameter_file_type: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::link_args::construct(
            feature_configuration,
            action_name,
            build_variables,
            parameter_file_type,
            heap,
        )
    }

    fn intern_seq<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] seq: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        sequence::immutable_tuple(seq, heap)
    }

    fn dynamic_library_soname<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] path: &str,
        #[starlark(require = pos)] preserve_name: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::subrule_invocation::link_actions::soname(actions, path, preserve_name, eval)
    }

    fn wrap_link_actions<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] build_configuration: Option<Value<'v>>,
        #[starlark(require = pos, default = false)] sharable_artifacts: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::subrule_invocation::link_actions::wrap(
            actions,
            build_configuration,
            sharable_artifacts,
            eval,
        )
    }

    fn is_tree_artifact<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] artifact: Value<'v>,
    ) -> anyhow::Result<bool> {
        let artifact = crate::subrule_invocation::AnalysisArtifactValue::from_starlark(artifact)
            .ok_or_else(|| anyhow::anyhow!("is_tree_artifact requires a File"))?;
        Ok(matches!(
            artifact.artifact(),
            slug_build_api_v2::AnalysisArtifact::Derived { output, .. }
                if output.kind() == slug_build_api_v2::ActionOutputKind::Directory
        ))
    }

    fn create_cc_compile_action<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] action_construction_context: Value<'v>,
        #[starlark(require = named)] cc_compilation_context: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] configuration: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] source: Value<'v>,
        #[starlark(require = named)] compile_build_variables: Value<'v>,
        #[starlark(require = named)] toolchain_type: &'v str,
        #[starlark(require = named)] copts_filter: Option<Value<'v>>,
        #[starlark(require = named)] additional_compilation_inputs: Option<Value<'v>>,
        #[starlark(require = named)] additional_compilation_inputs_set: Option<Value<'v>>,
        #[starlark(require = named)] additional_include_scanning_roots: Option<Value<'v>>,
        #[starlark(require = named, default = NoneType)] output_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dotd_file: Value<'v>,
        #[starlark(require = named)] diagnostics_file: Option<Value<'v>>,
        #[starlark(require = named)] gcno_file: Option<Value<'v>>,
        #[starlark(require = named)] dwo_file: Option<Value<'v>>,
        #[starlark(require = named)] lto_indexing_file: Option<Value<'v>>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named)] cache_key_inputs: Option<Value<'v>>,
        #[starlark(require = named)] build_info_header_files: Option<Value<'v>>,
        #[starlark(require = named)] action_name: Option<Value<'v>>,
        #[starlark(require = named)] should_scan_includes: Option<Value<'v>>,
        #[starlark(require = named)] shareable: Option<Value<'v>>,
        #[starlark(require = named)] module_files: Option<Value<'v>>,
        #[starlark(require = named)] modmap_file: Option<Value<'v>>,
        #[starlark(require = named)] modmap_input_file: Option<Value<'v>>,
        #[starlark(require = named)] additional_outputs: Option<Value<'v>>,
        #[starlark(require = named, default = false)] needs_include_validation: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        use crate::cc_compile_action::CcCompileRequest;
        use crate::cc_compile_action::empty_sequence;
        use crate::cc_compile_action::none;
        let copts_filter = crate::cc_compile_action::optional_string(copts_filter, "copts_filter")?;
        let action_name = crate::cc_compile_action::optional_string(action_name, "action_name")?;
        let should_scan_includes =
            crate::cc_compile_action::optional_bool(should_scan_includes, "should_scan_includes")?;
        let shareable = crate::cc_compile_action::optional_bool(shareable, "shareable")?;
        anyhow::ensure!(
            copts_filter.is_none_or(str::is_empty),
            "CppCompile nonempty copts_filter is unsupported"
        );
        anyhow::ensure!(
            shareable != Some(true),
            "CppCompile shareable actions are unsupported"
        );
        for (value, name) in [
            (diagnostics_file, "diagnostics_file"),
            (gcno_file, "gcno_file"),
            (dwo_file, "dwo_file"),
            (lto_indexing_file, "lto_indexing_file"),
            (module_files, "module_files"),
            (modmap_file, "modmap_file"),
            (modmap_input_file, "modmap_input_file"),
        ] {
            none(value, name)?;
        }
        empty_sequence(
            additional_include_scanning_roots,
            "additional_include_scanning_roots",
        )?;
        empty_sequence(additional_outputs, "additional_outputs")?;
        anyhow::ensure!(
            crate::cc_variables::cc_variables_value(compile_build_variables).is_some(),
            "compile_build_variables requires native CcToolchainVariables"
        );
        let context = action_construction_context
            .request_value::<&dyn crate::cc_context::CcCompileActionContext>()
            .ok_or_else(|| {
                anyhow::anyhow!("create_cc_compile_action requires a genuine rule ctx")
            })?;
        let request = CcCompileRequest {
            configuration: crate::cc_compile_outputs::configuration::unpack(configuration)?,
            compilation_context: cc_compilation_context,
            toolchain: cc_toolchain,
            features: feature_configuration,
            variables: compile_build_variables,
            source,
            output: output_file,
            dotd: dotd_file,
            additional_inputs: additional_compilation_inputs,
            additional_inputs_set: additional_compilation_inputs_set,
            cache_key_inputs,
            build_info_headers: build_info_header_files,
            action_name,
            should_scan_includes,
            use_pic,
            needs_include_validation,
            toolchain_type,
        };
        let pending = context.prepare_cc_compile(&request)?;
        crate::cc_compile_action::record(
            eval.module(),
            pending,
            compile_build_variables,
            cc_compilation_context,
        );
        Ok(NoneType)
    }

    fn compute_output_name_prefix_dir<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] configuration: Value<'v>,
        #[starlark(require = named)] purpose: &str,
    ) -> anyhow::Result<&'static str> {
        crate::cc_compile_outputs::output_name_prefix(configuration, purpose)
    }

    fn declare_compile_output_file<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] label: Value<'v>,
        #[starlark(require = named)] output_name: &str,
        #[starlark(require = named)] configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_compile_outputs::declare(ctx, label, output_name, configuration, eval)
    }

    fn actions2ctx_cheat<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] actions: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::subrule_invocation::actions_to_context(actions, eval)
    }

    fn get_artifact_name_for_category<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] category: &str,
        #[starlark(require = named)] output_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        Ok(crate::cc_artifact_naming::artifact_name_for_category(
            cc_toolchain,
            category,
            output_name,
            eval,
        )?
        .to_string())
    }

    fn per_file_copts<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] cpp_configuration: Value<'v>,
        #[starlark(require = pos)] source_file: Value<'v>,
        #[starlark(require = pos)] label: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let cpp = crate::analysis_fragments::cpp_fragment_value(cpp_configuration)
            .ok_or_else(|| anyhow::anyhow!("per_file_copts requires a Cpp configuration"))?;
        crate::subrule_invocation::AnalysisArtifactValue::from_starlark(source_file)
            .ok_or_else(|| anyhow::anyhow!("per_file_copts requires a File"))?;
        crate::provider::starlark_label(label)
            .ok_or_else(|| anyhow::anyhow!("per_file_copts requires a Label"))?;
        cpp.projection().per_file_copts_empty()?;
        Ok(heap.alloc(AllocImmutableList(std::iter::empty::<Value<'v>>())))
    }

    fn cc_toolchain_variables<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] vars: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::construct(vars, eval.heap())
    }
    fn combine_cc_toolchain_variables<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] parent: Value<'v>,
        #[starlark(args)] variables: UnpackTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::combine(parent, &variables.items, eval.heap())
    }
    fn intern_string_sequence_variable_value<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] string_sequence: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::string_sequence(string_sequence, eval.heap())
    }
    fn exec_os<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] ctx: Value<'v>,
    ) -> anyhow::Result<&'static str> {
        ctx.request_value::<&dyn crate::cc_context::CcExecutionContext>()
            .ok_or_else(|| anyhow::anyhow!("exec_os requires an active rule ctx"))?
            .execution_os()
    }

    fn cc_toolchain_features<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] toolchain_config_info: Value<'v>,
        #[starlark(require = named)] tools_directory: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_features::construct(toolchain_config_info, tools_directory, eval)
    }

    fn dynamic_library_symlink<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] library: Value<'v>,
        #[starlark(require = pos)] solib_directory: &str,
        #[starlark(require = pos)] preserve_name: bool,
        #[starlark(require = pos)] prefix_consumer: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let actions = AnalysisActions::from_starlark(actions)
            .ok_or_else(|| anyhow::anyhow!("dynamic_library_symlink requires ctx.actions"))?;
        Ok(eval
            .heap()
            .alloc_simple(actions.register_dynamic_library_symlink(
                library,
                solib_directory,
                preserve_name,
                prefix_consumer,
            )?))
    }

    fn absolute_symlink<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] output: Value<'v>,
        #[starlark(require = named)] target_path: &str,
        #[starlark(require = named)] progress_message: Option<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let actions = ctx
            .get_attr("actions", eval.heap())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .and_then(AnalysisActions::from_starlark)
            .ok_or_else(|| anyhow::anyhow!("absolute_symlink requires a rule or subrule ctx"))?;
        actions.register_absolute_symlink(output, target_path, progress_message)?;
        Ok(NoneType)
    }

    fn check_private_api<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] allowlist: Value<'v>,
        #[starlark(require = named, default = 1)] depth: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let depth = usize::try_from(depth)
            .map_err(|_| anyhow::anyhow!("check_private_api depth must be nonnegative"))?;
        let entries = custom_allowlist(allowlist)?;
        let identities = source_identities_for_evaluator(eval)?;
        check_custom_allowlist(eval, &identities, &entries, depth)?;
        Ok(NoneType)
    }

    fn freeze<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        if let Some(dict) = DictRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableDict(dict.iter())));
        }
        if let Some(list) = ListRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableList(list.iter())));
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableList(tuple.iter())));
        }
        // Pinned CcStarlarkInternal.freeze leaves non-iterable values untouched.
        if value.unpack_str().is_some() || value.unpack_bool().is_some() {
            anyhow::bail!("cc_internal.freeze requires a Starlark value, not string or bool");
        }
        if matches!(value.get_type(), "range" | "set") {
            anyhow::bail!(
                "cc_internal.freeze {} conversion is not supported",
                value.get_type()
            );
        }
        Ok(value)
    }

    fn create_header_info<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_header_module: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        modular_public_headers: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        modular_private_headers: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        textual_headers: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_pic_module: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        use crate::cc_header_info::headers;
        use crate::cc_header_info::module;
        crate::cc_header_info::create(
            slug_build_api_v2::CcHeaderInfoInput {
                header_module: module(header_module, "header_module")?,
                pic_header_module: module(pic_header_module, "pic_header_module")?,
                modular_public_headers: headers(
                    modular_public_headers,
                    "modular_public_headers",
                    eval.heap(),
                )?,
                modular_private_headers: headers(
                    modular_private_headers,
                    "modular_private_headers",
                    eval.heap(),
                )?,
                textual_headers: headers(textual_headers, "textual_headers", eval.heap())?,
                separate_module_headers: headers(
                    separate_module_headers,
                    "separate_module_headers",
                    eval.heap(),
                )?,
                separate_module: module(separate_module, "separate_module")?,
                separate_pic_module: module(separate_pic_module, "separate_pic_module")?,
                deps: Box::default(),
            },
            eval,
        )
    }
    fn create_header_info_with_deps<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_info: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::list::AllocList::EMPTY)]
        merged_deps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_header_info::with_deps(header_info, deps, merged_deps, eval)
    }
}

fn custom_allowlist(value: Value<'_>) -> anyhow::Result<Vec<CustomAllowlistEntry>> {
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("check_private_api allowlist must be a sequence of tuples")
    };
    values
        .into_iter()
        .map(|value| {
            let pair = TupleRef::from_value(value).ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            let [apparent_repo, package_prefix] = pair.iter().collect::<Vec<_>>()[..] else {
                anyhow::bail!("check_private_api allowlist entries must be two-string tuples")
            };
            let apparent_repo = apparent_repo.unpack_str().ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            let package_prefix = package_prefix.unpack_str().ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            Ok(CustomAllowlistEntry {
                apparent_repo: CompactString::new(apparent_repo),
                package_prefix: CompactString::new(package_prefix),
            })
        })
        .collect()
}

#[starlark_module]
fn cc_common_methods(builder: &mut MethodsBuilder) {
    fn get_tool_for_action<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::dispatch::tool(feature_configuration, action_name, eval.heap())
    }
    fn get_execution_requirements<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::dispatch::requirements(feature_configuration, action_name, eval.heap())
    }
    fn action_is_enabled<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
    ) -> anyhow::Result<bool> {
        crate::cc_variables::dispatch::action_enabled(feature_configuration, action_name)
    }
    fn get_memory_inefficient_command_line<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::dispatch::command_line(
            feature_configuration,
            action_name,
            variables,
            eval,
        )
    }
    fn get_environment_variables<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        crate::cc_variables::dispatch::environment(
            feature_configuration,
            action_name,
            variables,
            eval,
        )
    }
    fn empty_variables<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(crate::cc_variables::dispatch::empty(eval.heap()))
    }
    #[allow(non_snake_case)]
    fn internal_DO_NOT_USE<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<CcInternalModule> {
        let context = BzlEvaluationContext::from_evaluator(eval)?;
        let source = context.source_identity_for_call(eval)?;
        if !source
            .label
            .package()
            .repo()
            .as_str()
            .starts_with("rules_cc+")
        {
            let canonical = source.label.to_string();
            let canonical = if source.label.package().repo().is_root() {
                canonical.strip_prefix("@@").unwrap_or(&canonical)
            } else {
                &canonical
            };
            anyhow::bail!("file '{canonical}' cannot use private API");
        }
        Ok(CcInternalModule)
    }
}

pub(crate) fn cc_common_globals(builder: &mut GlobalsBuilder) {
    builder.set("cc_common", CcCommonModule);
}

#[cfg(test)]
pub(crate) mod solib_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcModuleApi {
    Public,
    Internal,
}
pub fn cc_common_api(value: Value<'_>) -> Option<CcModuleApi> {
    if CcCommonModule::from_value(value).is_some() {
        Some(CcModuleApi::Public)
    } else if CcInternalModule::from_value(value).is_some() {
        Some(CcModuleApi::Internal)
    } else {
        None
    }
}
