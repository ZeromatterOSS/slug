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
        if preserve_name || prefix_consumer {
            anyhow::bail!(
                "dynamic_library_symlink supports only preserve_name=False and prefix_consumer=False"
            );
        }
        let actions = AnalysisActions::from_value(actions)
            .ok_or_else(|| anyhow::anyhow!("dynamic_library_symlink requires ctx.actions"))?;
        Ok(eval
            .heap()
            .alloc_simple(actions.register_dynamic_library_symlink(library, solib_directory)?))
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
            .and_then(AnalysisActions::from_value)
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
