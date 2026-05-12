/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use dice::CancellationContext;
use dice::DiceComputations;
use dupe::Dupe;
use futures::Future;
use futures::FutureExt;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::anon_promises_dyn::RunAnonPromisesAccessorPair;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::analysis::registry::AnalysisRegistry;
use kuro_build_api::interpreter::rule_defs::cc_common::CcExpandIfEqual;
use kuro_build_api::interpreter::rule_defs::cc_common::CcFeatureFlagSets;
use kuro_build_api::interpreter::rule_defs::cc_common::CcFlagGroup;
use kuro_build_api::interpreter::rule_defs::cc_common::CcFlagSet;
use kuro_build_api::interpreter::rule_defs::cc_common::CcToolchainFeatures;
use kuro_build_api::interpreter::rule_defs::cc_common::CcWithFeatureSet;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use kuro_build_api::interpreter::rule_defs::context::AnalysisContext;
use kuro_build_api::interpreter::rule_defs::context::ResolvedToolchains;
use kuro_build_api::interpreter::rule_defs::context::cc_toolchain_native_shim_provider_collection;
use kuro_build_api::interpreter::rule_defs::provider::FrozenBuiltinProviderLike;
use kuro_build_api::interpreter::rule_defs::provider::ValueAsProviderLike;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use kuro_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::FrozenExternalRunnerTestInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::create_external_runner_test_info_for_bazel_test;
use kuro_build_api::interpreter::rule_defs::provider::builtin::template_placeholder_info::FrozenTemplatePlaceholderInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::validation_info::FrozenValidationInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValueRef;
use kuro_build_api::interpreter::rule_defs::provider::collection::ProviderCollection;
use kuro_build_api::validation::transitive_validations::TransitiveValidations;
use kuro_build_api::validation::transitive_validations::TransitiveValidationsData;
use kuro_common::dice::cells::HasCellResolver;
use kuro_core::cells::cell_path::CellPathRef;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::execution_types::execution::ExecutionPlatform;
use kuro_core::execution_types::execution::ExecutionPlatformResolution;
use kuro_core::package::PackageLabel;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_core::unsafe_send_future::UnsafeSendFuture;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_events::dispatch::Span as DispatchSpan;
use kuro_events::dispatch::get_dispatcher;
use kuro_events::dispatch::span_simple as dispatch_span_simple;
use kuro_execute::digest_config::HasDigestConfig;
use kuro_interpreter::dice::starlark_provider::StarlarkEvalKind;
use kuro_interpreter::factory::BuckStarlarkModule;
use kuro_interpreter::factory::StarlarkEvaluatorProvider;
use kuro_interpreter::print_handler::EventDispatcherPrintHandler;
use kuro_interpreter::soft_error::KuroStarlarkSoftErrorHandler;
use kuro_interpreter::types::rule::FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL;
use kuro_interpreter::types::rule::FROZEN_RULE_GET_IMPL;
use kuro_interpreter_for_build::rule::FrozenStarlarkRuleCallable;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_attr::CoercedSelectorKeyRef;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::execution::GetExecutionPlatforms;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use kuro_node::rule_type::StarlarkRuleType;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::CallStackCheckpoint;
use starlark::eval::Evaluator;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::ValueTyped;
use starlark::values::ValueTypedComplex;
use starlark::values::list::ListRef;
use starlark_map::small_map::SmallMap;

use crate::analysis::native_rule_analysis::DeclaredToolchainInfo;
use crate::analysis::native_rule_analysis::DeferredToolchain;
use crate::analysis::native_rule_analysis::deferred_all_loaded;
use crate::analysis::native_rule_analysis::deferred_key_already_loaded;
use crate::analysis::native_rule_analysis::get_deferred_toolchains;
use crate::analysis::native_rule_analysis::mark_deferred_all_loaded;
use crate::analysis::native_rule_analysis::mark_deferred_key_loaded;
use crate::analysis::native_rule_analysis::register_declared_toolchain;
use crate::analysis::native_rule_analysis::set_deferred_toolchains;
use crate::analysis::plugins::plugins_to_starlark_value;
use crate::attrs::resolve::ctx::AnalysisQueryResult;
use crate::attrs::resolve::ctx::AttrResolutionContext;
use crate::attrs::resolve::node_to_attrs_struct::node_to_attrs_struct;

static ANALYSIS_ENV_VERBOSE_CHECKPOINT_COUNT: AtomicUsize = AtomicUsize::new(0);
const ANALYSIS_ENV_VERBOSE_CHECKPOINT_INTERVAL: usize = 1024;

/// For Bazel test rules (`rule(test=True)`) that return `DefaultInfo(executable=...)`
/// without an explicit `ExternalRunnerTestInfo`, auto-inject a synthetic
/// `ExternalRunnerTestInfo` so Kuro's test runner can execute them.
///
/// This bridges the gap between Bazel (where test rules are marked with `test=True`
/// and the executable comes from `DefaultInfo`) and Buck2/Kuro (where test targets
/// must provide `ExternalRunnerTestInfo`).
fn maybe_inject_test_info<'v>(
    heap: Heap<'v>,
    list_res: Value<'v>,
    tags: &[String],
) -> kuro_error::Result<Value<'v>> {
    // Handle struct(providers=[...]) pattern from legacy Bazel rules
    let (actual_list_res, is_struct) = if ListRef::from_value(list_res).is_none() {
        if let Some(pv) = kuro_build_api::interpreter::rule_defs::provider::collection::extract_providers_from_struct(list_res) {
            (pv, true)
        } else {
            return Ok(list_res);
        }
    } else {
        (list_res, false)
    };

    let list = match ListRef::from_value(actual_list_res) {
        Some(v) => v,
        None => return Ok(list_res),
    };

    let test_info_id = FrozenExternalRunnerTestInfo::builtin_provider_id();
    let default_info_id = DefaultInfoCallable::provider_id();

    let mut has_test_info = false;
    let mut default_info_value: Option<Value<'v>> = None;

    for value in list.iter() {
        if value.is_none() {
            continue;
        }
        if let Ok(Some(provider)) =
            <ValueAsProviderLike as starlark::values::UnpackValue>::unpack_value(value)
        {
            if provider.provider_id() == test_info_id {
                has_test_info = true;
                break;
            }
            if provider.provider_id() == default_info_id {
                default_info_value = Some(value);
            }
        }
    }

    if has_test_info {
        return Ok(list_res);
    }

    // Get executable from DefaultInfo.
    // DefaultInfo.executable returns a single File value (not a list), or None.
    if let Some(di_value) = default_info_value {
        if let Ok(Some(exe)) = di_value.get_attr("executable", heap) {
            if !exe.is_none() {
                // Create ExternalRunnerTestInfo with the executable as command.
                // Propagate Bazel tags as labels for test filtering (--include/--exclude).
                let test_type = heap.alloc_str("custom").to_value();
                let command = heap.alloc(vec![exe]);
                let labels: Vec<Value<'v>> = tags
                    .iter()
                    .map(|s| heap.alloc_str(s.as_str()).to_value())
                    .collect();
                let labels_value = heap.alloc(labels);
                let test_info = create_external_runner_test_info_for_bazel_test(
                    test_type,
                    command,
                    labels_value,
                );
                let test_info_value = heap.alloc(test_info);

                // Create new list with test_info appended
                let mut new_list: Vec<Value<'v>> = list.iter().collect();
                new_list.push(test_info_value);
                let new_list_val = heap.alloc(new_list);
                if is_struct {
                    // Re-wrap in struct(providers=[...]) to maintain pattern
                    return Ok(heap.alloc(starlark::values::structs::AllocStruct([(
                        "providers",
                        new_list_val,
                    )])));
                }
                return Ok(new_list_val);
            }
        }
    }

    // No executable found or no DefaultInfo, return original list
    Ok(list_res)
}

/// Bazel convention: if a rule declares outputs via `attr.output` /
/// `attr.output_list` and the implementation does not return a DefaultInfo
/// provider, the declared outputs become the target's `default_outputs`.
///
/// Kuro rule impls otherwise get an empty DefaultInfo auto-injected by
/// `ProviderCollection::try_from_value_subtarget`, which yields a target
/// with no outputs — cf. `bazel_skylib//rules:expand_template.bzl`, whose
/// impl is `ctx.actions.expand_template(... output = ctx.outputs.out ...)`
/// with no `return` statement.
///
/// This helper inspects `list_res`. If no DefaultInfo is present, it builds
/// one from the artifacts declared via `ctx.outputs.<name>` for each
/// `attr.output` attribute and appends it to the provider list. If
/// DefaultInfo is already present, the rule author's version wins (even if
/// it has empty `default_outputs`).
fn maybe_inject_implicit_default_info<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    ctx: starlark::values::ValueTyped<
        'v,
        kuro_build_api::interpreter::rule_defs::context::AnalysisContext<'v>,
    >,
    list_res: Value<'v>,
    output_attr_names: &[String],
) -> kuro_error::Result<Value<'v>> {
    if output_attr_names.is_empty() {
        return Ok(list_res);
    }

    let default_info_id = DefaultInfoCallable::provider_id();

    // Detect whether the impl already returned a DefaultInfo.
    let existing_providers: Vec<Value<'v>> = if list_res.is_none() {
        Vec::new()
    } else if let Some(list) = ListRef::from_value(list_res) {
        list.iter().collect()
    } else if <ValueAsProviderLike as starlark::values::UnpackValue>::unpack_value(list_res)
        .ok()
        .flatten()
        .is_some()
    {
        vec![list_res]
    } else {
        // struct(providers=[...]), unknown shape, etc. — let downstream error if needed.
        return Ok(list_res);
    };

    let heap = eval.heap();

    // Find the index of any DefaultInfo the impl already returned. We don't
    // short-circuit on its presence: Bazel lets rules write `return [DefaultInfo()]`
    // with no `files=` argument and still has the predeclared outputs become the
    // target's default outputs (see rules_cc's `gentbl_rule` at
    // llvm-project-overlay/mlir/tblgen.bzl, which returns `[DefaultInfo()]` and
    // relies on its `attr.output(name="out")` showing up as the target's files).
    // If the impl supplied a non-empty `default_outputs`, honour it verbatim.
    let existing_default_info_idx =
        existing_providers
            .iter()
            .enumerate()
            .find_map(|(i, v)| match (*v).is_none() {
                true => None,
                false => {
                    match <ValueAsProviderLike as starlark::values::UnpackValue>::unpack_value(*v) {
                        Ok(Some(p)) if p.provider_id() == default_info_id => Some(i),
                        _ => None,
                    }
                }
            });

    if let Some(idx) = existing_default_info_idx {
        let existing = existing_providers[idx];
        // If the existing DefaultInfo already has non-empty default_outputs,
        // trust the rule author.
        let has_outputs = existing
            .get_attr("default_outputs", heap)
            .ok()
            .flatten()
            .and_then(|v| v.length().ok())
            .is_some_and(|n| n > 0);
        if has_outputs {
            return Ok(list_res);
        }
    }

    let artifacts = ctx
        .as_ref()
        .collect_implicit_default_outputs(output_attr_names, heap);
    if artifacts.is_empty() {
        return Ok(list_res);
    }

    let default_info = kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfo::with_default_outputs(
        heap,
        artifacts,
    );
    let default_info_value = heap.alloc(default_info);

    let mut new_list = existing_providers;
    if let Some(idx) = existing_default_info_idx {
        // Replace the empty DefaultInfo with the populated one. Preserves
        // provider ordering so downstream consumers aren't surprised.
        new_list[idx] = default_info_value;
    } else {
        new_list.push(default_info_value);
    }
    Ok(heap.alloc(new_list))
}

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Tier0)]
enum AnalysisError {
    #[error(
        "Analysis context was missing a query result, this shouldn't be possible. Query was `{0}`"
    )]
    MissingQuery(String),
    #[error("required dependency `{0}` was not found")]
    MissingDep(ConfiguredProvidersLabel),
}

// Contains a `module` that things must live on, and various `FrozenProviderCollectionValue`s
// that are NOT tied to that module. Must claim ownership of them via `add_reference` before returning them.
pub struct RuleAnalysisAttrResolutionContext<'v> {
    pub module: &'v Module,
    pub dep_analysis_results: HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    pub query_results: HashMap<String, Arc<AnalysisQueryResult>>,
    pub execution_platform_resolution: ExecutionPlatformResolution,
}

impl<'v> AttrResolutionContext<'v> for &'_ RuleAnalysisAttrResolutionContext<'v> {
    fn starlark_module(&self) -> &'v Module {
        self.module
    }

    fn get_dep(
        &mut self,
        target: &ConfiguredProvidersLabel,
    ) -> kuro_error::Result<FrozenValueTyped<'v, FrozenProviderCollection>> {
        get_dep(&self.dep_analysis_results, target, self.module)
    }

    fn resolve_unkeyed_placeholder(
        &mut self,
        name: &str,
    ) -> kuro_error::Result<Option<FrozenCommandLineArg>> {
        Ok(resolve_unkeyed_placeholder(
            &self.dep_analysis_results,
            name,
            self.module,
        ))
    }

    fn resolve_query(&mut self, query: &str) -> kuro_error::Result<Arc<AnalysisQueryResult>> {
        resolve_query(&self.query_results, query, self.module)
    }

    fn execution_platform_resolution(&self) -> &ExecutionPlatformResolution {
        &self.execution_platform_resolution
    }
}

pub fn get_dep<'v>(
    dep_analysis_results: &HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    target: &ConfiguredProvidersLabel,
    module: &'v Module,
) -> kuro_error::Result<FrozenValueTyped<'v, FrozenProviderCollection>> {
    match dep_analysis_results.get(target.target()) {
        None => Err(AnalysisError::MissingDep(target.dupe()).into()),
        Some(x) => {
            let x = x.lookup_inner(target)?;
            // IMPORTANT: Anything given back to the user must be kept alive
            Ok(x.add_heap_ref(module.frozen_heap()))
        }
    }
}

pub fn resolve_unkeyed_placeholder<'v>(
    dep_analysis_results: &HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    name: &str,
    module: &'v Module,
) -> Option<FrozenCommandLineArg> {
    // TODO(cjhopman): Make it an error if two deps provide a value for the placeholder.
    for providers in dep_analysis_results.values() {
        if let Some(placeholder_info) = providers
            .provider_collection()
            .builtin_provider::<FrozenTemplatePlaceholderInfo>()
        {
            if let Some(value) = placeholder_info.unkeyed_variables().get(name) {
                // IMPORTANT: Anything given back to the user must be kept alive
                module
                    .frozen_heap()
                    .add_reference(providers.value().owner());
                return Some(*value);
            }
        }
    }
    None
}

pub fn resolve_query(
    query_results: &HashMap<String, Arc<AnalysisQueryResult>>,
    query: &str,
    module: &Module,
) -> kuro_error::Result<Arc<AnalysisQueryResult>> {
    match query_results.get(query) {
        None => Err(AnalysisError::MissingQuery(query.to_owned()).into()),
        Some(x) => {
            for (_, y) in x.result.iter() {
                // IMPORTANT: Anything given back to the user must be kept alive
                module.frozen_heap().add_reference(y.value().owner());
            }
            Ok(x.dupe())
        }
    }
}

pub trait RuleSpec: Sync {
    fn invoke<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
        ctx: ValueTyped<'v, AnalysisContext<'v>>,
    ) -> kuro_error::Result<Value<'v>>;

    fn promise_artifact_mappings<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<SmallMap<String, Value<'v>>>;

    /// Returns the implicit output patterns from `rule(outputs={...})`.
    /// Each pair is (name, pattern) where pattern may contain `%{name}`.
    fn rule_outputs(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Names of `attr.output()` / `attr.output_list()` attributes on this rule.
    /// Used at analysis time to auto-populate `DefaultInfo.default_outputs`
    /// from declared outputs when a rule impl does not return DefaultInfo
    /// (Bazel convention — see e.g. bazel_skylib's `expand_template` which
    /// returns `None` and relies on implicit default-output inference).
    fn output_attr_names(&self) -> Vec<String> {
        Vec::new()
    }

    /// Returns the toolchain types declared by this rule via `rule(toolchains=[...])`.
    /// Each entry is `(label, mandatory)`.
    fn toolchain_types(&self) -> Vec<(String, bool)> {
        Vec::new()
    }

    /// Returns the exec group definitions declared by this rule via `rule(exec_groups={...})`.
    fn exec_group_defs(&self) -> Vec<(String, kuro_node::rule::ExecGroupDef)> {
        Vec::new()
    }
}

/// Container for the environment that analysis implementation functions should run in
struct AnalysisEnv<'a> {
    rule_spec: &'a dyn RuleSpec,
    deps: Vec<(&'a ConfiguredTargetLabel, AnalysisResult)>,
    query_results: HashMap<String, Arc<AnalysisQueryResult>>,
    execution_platform: &'a ExecutionPlatformResolution,
    label: ConfiguredTargetLabel,
    cancellation: &'a CancellationContext,
    /// Aspect results to merge into dependency provider collections (Phase 8h).
    /// Maps dep label → aspect provider collection.
    aspect_results: HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
}

struct AnalysisStarlarkProgress {
    target: String,
    started: Instant,
    call_count: Cell<usize>,
    bytecode_check_count: Cell<usize>,
    heartbeat_count: Cell<usize>,
    last_heartbeat_ms: Cell<usize>,
    interesting_samples: Cell<usize>,
    sampled_functions: RefCell<HashMap<String, usize>>,
}

impl AnalysisStarlarkProgress {
    fn new(label: &ConfiguredTargetLabel) -> Self {
        Self {
            target: label.to_string(),
            started: Instant::now(),
            call_count: Cell::new(0),
            bytecode_check_count: Cell::new(0),
            heartbeat_count: Cell::new(0),
            last_heartbeat_ms: Cell::new(0),
            interesting_samples: Cell::new(0),
            sampled_functions: RefCell::new(HashMap::new()),
        }
    }

    fn should_log_interesting(sample_count: usize) -> bool {
        sample_count.is_power_of_two()
    }

    fn is_interesting_starlark_file(filename: &str) -> bool {
        (filename.contains("rules_cc+0.2.17")
            && (filename.ends_with("cc/private/link/create_linker_input.bzl")
                || filename.ends_with("cc/private/link/create_library_to_link.bzl")
                || filename.ends_with(
                    "cc/private/link/create_linking_context_from_compilation_outputs.bzl",
                )
                || filename.ends_with("cc/private/cc_info.bzl")))
            || (filename.contains("rules_rust+0.69.0")
                && (filename.ends_with("rust/private/rust_allocator_libraries.bzl")
                    || filename.ends_with("rust/private/cc/cc_utils.bzl")
                    || filename.ends_with("rust/private/rust.bzl")
                    || filename.ends_with("rust/private/rustc.bzl")
                    || filename.ends_with("rust/private/utils.bzl")))
    }

    fn elapsed_ms(&self) -> usize {
        self.started.elapsed().as_millis().min(usize::MAX as u128) as usize
    }
}

impl<'e> CallStackCheckpoint<'e> for AnalysisStarlarkProgress {
    fn on_call_stack_push<'v>(&self, eval: &Evaluator<'v, '_, 'e>) {
        let call_count = self.call_count.get().saturating_add(1);
        self.call_count.set(call_count);

        let Some(location) = eval.call_stack_top_location() else {
            return;
        };
        let filename = location.filename();
        if !Self::is_interesting_starlark_file(filename) {
            return;
        }

        let resolved = location.resolve_span();
        let frame = eval.call_stack_top_frame();
        let function = frame
            .as_ref()
            .map(|frame| frame.name.as_str())
            .unwrap_or("<unknown>");
        let key = format!("{}:{}:{function}", filename, resolved.begin.line + 1);
        let interesting_samples = self.interesting_samples.get().saturating_add(1);
        self.interesting_samples.set(interesting_samples);
        let function_sample_count = {
            let mut sampled_functions = self.sampled_functions.borrow_mut();
            let count = sampled_functions.entry(key).or_insert(0);
            *count = count.saturating_add(1);
            *count
        };
        if !Self::should_log_interesting(interesting_samples) {
            return;
        }

        kuro_util::memory_checkpoint::checkpoint(
            "analysis_starlark_call_sample",
            [
                ("call_count", call_count),
                ("interesting_samples", interesting_samples),
                ("function_sample_count", function_sample_count),
                ("stack_depth", eval.call_stack_count()),
                ("line", resolved.begin.line + 1),
                ("column", resolved.begin.column + 1),
                ("filename_len", filename.len()),
                ("elapsed_ms", self.elapsed_ms()),
            ],
        );
        tracing::warn!(
            target: "kuro_memory",
            checkpoint = "analysis_starlark_call_sample",
            target_label = %self.target,
            starlark_file = filename,
            function,
            call_count,
            interesting_samples,
            function_sample_count,
            line = resolved.begin.line + 1,
            column = resolved.begin.column + 1,
            "analysis starlark call sample target={} file={} function={} line={} call_count={}",
            self.target,
            filename,
            function,
            resolved.begin.line + 1,
            call_count
        );
    }

    fn on_infrequent_instr_check<'v>(&self, eval: &Evaluator<'v, '_, 'e>) {
        let bytecode_check_count = self.bytecode_check_count.get().saturating_add(1);
        self.bytecode_check_count.set(bytecode_check_count);

        let elapsed_ms = self.elapsed_ms();
        if elapsed_ms < 1_000 {
            return;
        }

        let heartbeat_count = self.heartbeat_count.get();
        let interval_ms = if heartbeat_count < 4 { 1_000 } else { 5_000 };
        let last_heartbeat_ms = self.last_heartbeat_ms.get();
        if elapsed_ms.saturating_sub(last_heartbeat_ms) < interval_ms {
            return;
        }
        self.last_heartbeat_ms.set(elapsed_ms);
        let heartbeat_count = heartbeat_count.saturating_add(1);
        self.heartbeat_count.set(heartbeat_count);

        let location = eval.call_stack_top_location();
        let (filename, line, column) = if let Some(location) = location.as_ref() {
            let resolved = location.resolve_span();
            (
                location.filename(),
                resolved.begin.line + 1,
                resolved.begin.column + 1,
            )
        } else {
            ("<unknown>", 0, 0)
        };
        let frame = eval.call_stack_top_frame();
        let function = frame
            .as_ref()
            .map(|frame| frame.name.as_str())
            .unwrap_or("<unknown>");
        let interesting_file = Self::is_interesting_starlark_file(filename) as usize;

        kuro_util::memory_checkpoint::checkpoint(
            "analysis_starlark_eval_heartbeat",
            [
                ("heartbeat_count", heartbeat_count),
                ("bytecode_check_count", bytecode_check_count),
                ("call_count", self.call_count.get()),
                ("interesting_samples", self.interesting_samples.get()),
                ("stack_depth", eval.call_stack_count()),
                ("line", line),
                ("column", column),
                ("filename_len", filename.len()),
                ("interesting_file", interesting_file),
                ("elapsed_ms", elapsed_ms),
            ],
        );
        tracing::warn!(
            target: "kuro_memory",
            checkpoint = "analysis_starlark_eval_heartbeat",
            target_label = %self.target,
            starlark_file = filename,
            function,
            heartbeat_count,
            bytecode_check_count,
            line,
            column,
            interesting_file,
            "analysis starlark eval heartbeat target={} file={} function={} line={} bytecode_checks={}",
            self.target,
            filename,
            function,
            line,
            bytecode_check_count
        );
    }
}

fn should_emit_analysis_env_verbose_checkpoint(checkpoint: &'static str) -> bool {
    let high_volume = matches!(
        checkpoint,
        "analysis_evaluate_rule_phase"
            | "analysis_toolchain_resolution_substep"
            | "analysis_ctx_toolchain_provider"
    );
    if !high_volume {
        return true;
    }
    let count = ANALYSIS_ENV_VERBOSE_CHECKPOINT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    count.is_power_of_two() || count % ANALYSIS_ENV_VERBOSE_CHECKPOINT_INTERVAL == 0
}

fn analysis_eval_phase_checkpoint(
    checkpoint: &'static str,
    label: &ConfiguredTargetLabel,
    phase_id: usize,
    phase: &'static str,
    started: Instant,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    if !should_emit_analysis_env_verbose_checkpoint(checkpoint) {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis().min(usize::MAX as u128) as usize;
    kuro_util::memory_checkpoint::checkpoint(
        checkpoint,
        [
            ("phase_id", phase_id),
            ("elapsed_ms", elapsed_ms),
            ("target_len", label.to_string().len()),
        ],
    );
    tracing::warn!(
        target: "kuro_memory",
        checkpoint,
        target_label = %label,
        phase,
        phase_id,
        elapsed_ms,
        "analysis evaluate_rule phase target={} phase={} elapsed_ms={}",
        label,
        phase,
        elapsed_ms
    );
}

fn analysis_ctx_toolchain_provider_checkpoint(
    label: &ConfiguredTargetLabel,
    type_label: &str,
    impl_label: &str,
    configured: Option<&ConfiguredTargetLabel>,
    index: usize,
    total: usize,
    mandatory: bool,
    is_self_dependency: bool,
    status_id: usize,
    status: &'static str,
    started: Instant,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    if !should_emit_analysis_env_verbose_checkpoint("analysis_ctx_toolchain_provider") {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis().min(usize::MAX as u128) as usize;
    let configured_label = configured
        .map(|configured| configured.to_string())
        .unwrap_or_else(|| "<none>".to_owned());
    let configured_len = configured
        .map(|configured| configured.to_string().len())
        .unwrap_or(0);
    kuro_util::memory_checkpoint::checkpoint(
        "analysis_ctx_toolchain_provider",
        [
            ("status_id", status_id),
            ("elapsed_ms", elapsed_ms),
            ("index", index),
            ("total", total),
            ("mandatory", mandatory as usize),
            ("is_self_dependency", is_self_dependency as usize),
            ("type_len", type_label.len()),
            ("impl_len", impl_label.len()),
            ("configured_len", configured_len),
        ],
    );
    tracing::warn!(
        target: "kuro_memory",
        checkpoint = "analysis_ctx_toolchain_provider",
        target_label = %label,
        toolchain_type = type_label,
        toolchain_impl = impl_label,
        configured_toolchain = configured_label.as_str(),
        index,
        total,
        mandatory,
        is_self_dependency,
        status,
        elapsed_ms,
        "analysis ctx toolchain provider target={} type={} impl={} configured={} status={} mandatory={} self={} index={}/{} elapsed_ms={}",
        label,
        type_label,
        impl_label,
        configured_label,
        status,
        mandatory,
        is_self_dependency,
        index,
        total,
        elapsed_ms
    );
}

fn analysis_toolchain_resolution_checkpoint(
    label: &ConfiguredTargetLabel,
    step_id: usize,
    step: &'static str,
    started: Instant,
    fields: impl IntoIterator<Item = (&'static str, usize)>,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    if !should_emit_analysis_env_verbose_checkpoint("analysis_toolchain_resolution_substep") {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis().min(usize::MAX as u128) as usize;
    let mut checkpoint_fields = vec![
        ("step_id", step_id),
        ("elapsed_ms", elapsed_ms),
        ("target_len", label.to_string().len()),
    ];
    checkpoint_fields.extend(fields);
    kuro_util::memory_checkpoint::checkpoint(
        "analysis_toolchain_resolution_substep",
        checkpoint_fields,
    );
    tracing::warn!(
        target: "kuro_memory",
        checkpoint = "analysis_toolchain_resolution_substep",
        target_label = %label,
        step,
        step_id,
        elapsed_ms,
        "analysis toolchain resolution substep target={} step={} elapsed_ms={}",
        label,
        step,
        elapsed_ms
    );
}

fn is_cpp_toolchain_type_label(label: &str) -> bool {
    let label = label.trim_start_matches('@');
    if label.contains("tools/cpp:toolchain_type") {
        return true;
    }
    let Some((repo, package)) = label.split_once("//") else {
        return false;
    };
    (package == "tools/cpp:toolchain_type"
        && (repo == "bazel_tools" || repo.starts_with("bazel_tools+")))
        || package == "cc:toolchain_type"
            && (repo == "rules_cc"
                || repo.strip_prefix("rules_cc+").is_some_and(|rest| {
                    rest.split('+')
                        .next()
                        .is_some_and(|version| version.contains('.'))
                }))
}

pub(crate) async fn run_analysis<'a>(
    dice: &'a mut DiceComputations<'_>,
    label: &ConfiguredTargetLabel,
    results: Vec<(&'a ConfiguredTargetLabel, AnalysisResult)>,
    query_results: HashMap<String, Arc<AnalysisQueryResult>>,
    execution_platform: &'a ExecutionPlatformResolution,
    rule_spec: &'a dyn RuleSpec,
    node: ConfiguredTargetNodeRef<'a>,
    cancellation: &CancellationContext,
    aspect_results: HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
) -> kuro_error::Result<AnalysisResult> {
    let analysis_env = AnalysisEnv {
        rule_spec,
        deps: results,
        query_results,
        execution_platform,
        label: label.dupe(),
        cancellation,
        aspect_results,
    };
    run_analysis_with_env(dice, analysis_env, node).await
}

pub fn get_deps_from_analysis_results(
    results: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>> {
    results
        .into_iter()
        .map(|(label, result)| Ok((label.dupe(), result.providers()?.to_owned())))
        .collect::<kuro_error::Result<HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>>>()
}

// ============================================================================
// Eager Toolchain Loading (Phase 6)
// ============================================================================

/// Flag to ensure registered toolchain packages are loaded only once per session.
static TOOLCHAINS_LOADING_DONE: AtomicBool = AtomicBool::new(false);

/// Serializes the initial eager registered-toolchain load. Multiple analysis
/// keys can start concurrently before the done flag flips; only one should
/// materialize and scan the eager toolchain packages.
static EAGER_TOOLCHAIN_LOAD_LOCK: LazyLock<futures::lock::Mutex<()>> =
    LazyLock::new(|| futures::lock::Mutex::new(()));

/// Serializes deferred toolchain loading. Resolution can run concurrently for
/// many configured targets; without this gate, several misses can all decide to
/// drain the deferred pool before any one load has populated the global
/// `DeclaredToolchainInfo` registry.
static DEFERRED_TOOLCHAIN_LOAD_LOCK: LazyLock<futures::lock::Mutex<()>> =
    LazyLock::new(|| futures::lock::Mutex::new(()));

/// Reset the eager loading flag (for fresh builds / daemon restart).
pub fn reset_toolchain_loading() {
    TOOLCHAINS_LOADING_DONE.store(false, Ordering::SeqCst);
}

fn eager_toolchain_loading_checkpoint(
    step_id: usize,
    step: &'static str,
    started: Instant,
    fields: impl IntoIterator<Item = (&'static str, usize)>,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis().min(usize::MAX as u128) as usize;
    let mut checkpoint_fields = vec![("step_id", step_id), ("elapsed_ms", elapsed_ms)];
    checkpoint_fields.extend(fields);
    kuro_util::memory_checkpoint::checkpoint("analysis_eager_toolchain_loading", checkpoint_fields);
    tracing::warn!(
        target: "kuro_memory",
        checkpoint = "analysis_eager_toolchain_loading",
        step,
        step_id,
        elapsed_ms,
        "analysis eager toolchain loading step={} elapsed_ms={}",
        step,
        elapsed_ms
    );
}

/// Eagerly load all registered toolchain packages via DICE.
///
/// This triggers materialization of extension repos (e.g., `local_config_cc_toolchains`)
/// and populates the `DeclaredToolchainInfo` registry by extracting toolchain metadata
/// from each `toolchain()` target in the loaded packages.
///
/// Runs once per daemon session, guarded by `TOOLCHAINS_LOADING_DONE`.
pub async fn ensure_registered_toolchains_loaded(dice: &mut DiceComputations<'_>) {
    if TOOLCHAINS_LOADING_DONE.load(Ordering::SeqCst) {
        return;
    }

    let started = Instant::now();
    eager_toolchain_loading_checkpoint(1, "lock_wait_start", started, []);
    let _guard = EAGER_TOOLCHAIN_LOAD_LOCK.lock().await;
    eager_toolchain_loading_checkpoint(2, "lock_acquired", started, []);

    if TOOLCHAINS_LOADING_DONE.load(Ordering::SeqCst) {
        eager_toolchain_loading_checkpoint(3, "already_loaded_after_lock", started, []);
        return;
    }

    let registered = kuro_bzlmod::get_registered_toolchains();
    if registered.is_empty() {
        TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
        set_deferred_toolchains(Vec::new());
        eager_toolchain_loading_checkpoint(4, "no_registered_toolchains", started, []);
        return;
    }

    // Plan 13 Phase 3: split the registry into eager (root + bundled) and
    // deferred (non-root transitive). Only the eager set is loaded now; the
    // deferred set lives in `DEFERRED_TOOLCHAINS` until `resolve_toolchains`
    // misses on a required type and triggers
    // `ensure_deferred_toolchains_loaded` for a filtered subset.
    let (eager_registered, deferred): (Vec<_>, Vec<_>) = registered
        .into_iter()
        .partition(should_eager_load_registered_toolchain);
    let deferred_pool: Vec<DeferredToolchain> = deferred
        .iter()
        .map(|tc| DeferredToolchain {
            module: tc.module.clone(),
            label: tc.label.clone(),
        })
        .collect();
    set_deferred_toolchains(deferred_pool);
    eager_toolchain_loading_checkpoint(
        5,
        "registry_split",
        started,
        [
            ("eager_count", eager_registered.len()),
            ("deferred_count", deferred.len()),
        ],
    );

    tracing::debug!(
        "Eagerly loading {} root toolchain package(s); {} non-root deferred",
        eager_registered.len(),
        deferred.len()
    );

    let cell_resolver = match dice.get_cell_resolver().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("Failed to get cell resolver for toolchain loading: {}", e);
            TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
            eager_toolchain_loading_checkpoint(6, "cell_resolver_failed", started, []);
            return;
        }
    };

    // Pre-filter packages whose paths can be resolved without touching DICE.
    // These filters were previously inside the serial loop; hoisting them out
    // lets the parallel dispatch below only do the expensive work.
    let mut to_load: Vec<(String, PackageLabel)> = Vec::new();
    let mut skipped_count = 0;
    for tc in &eager_registered {
        let tc_label_str = &tc.label;
        let (repo_name, pkg_path) = match parse_registered_toolchain_label(tc_label_str) {
            Some(v) => v,
            None => {
                tracing::debug!("Could not parse toolchain label: {}", tc_label_str);
                continue;
            }
        };

        // Resolve cell name (triggers ExtensionRepoCellSetup → lazy materialization)
        let cell_name = match CellName::unchecked_new(&repo_name) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Invalid cell name '{}': {}", repo_name, e);
                continue;
            }
        };

        let cell_instance = match cell_resolver.get(cell_name) {
            Ok(c) => c,
            Err(_) => {
                tracing::debug!(
                    "Cell '{}' not found in resolver for toolchain '{}', skipping",
                    repo_name,
                    tc_label_str
                );
                skipped_count += 1;
                continue;
            }
        };

        // Extension repos (paths containing '+' or '~') are loaded the same
        // way as other cells: via `dice.get_interpreter_results`. If the
        // repo's content is not yet on disk, the file-ops layer triggers
        // `ExtensionRepoExecutionKey::compute` and materialises it on demand.
        // Root-module registrations must take this path even when they point
        // at extension-generated repos; Bazel gives root registrations higher
        // priority than transitive registrations, so deferring them can let a
        // lower-priority toolchain win before the root toolchain is visible.
        let _ = cell_instance;

        let cell_rel_path = CellRelativePath::unchecked_new(&pkg_path);
        let package_label =
            match PackageLabel::from_cell_path(CellPathRef::new(cell_name, cell_rel_path)) {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!(
                        "Failed to create package label for '{}': {}",
                        tc_label_str,
                        e
                    );
                    skipped_count += 1;
                    continue;
                }
            };

        to_load.push((tc_label_str.clone(), package_label));
    }

    eager_toolchain_loading_checkpoint(
        7,
        "load_packages_start",
        started,
        [
            ("to_load_count", to_load.len()),
            ("skipped_count", skipped_count),
        ],
    );
    load_and_register_toolchain_packages(dice, to_load).await;
    eager_toolchain_loading_checkpoint(
        8,
        "load_packages_complete",
        started,
        [("skipped_count", skipped_count)],
    );

    if skipped_count > 0 {
        tracing::debug!(
            "Skipped {} toolchain registration(s) (extension repos or unavailable)",
            skipped_count
        );
    }

    TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
    eager_toolchain_loading_checkpoint(9, "done", started, []);
}

/// Load a list of `(label_str, package_label)` toolchain packages in
/// parallel, registering each `toolchain()` target into
/// `DECLARED_TOOLCHAINS` in the original registration order. Errors are
/// non-fatal — swallowed with a warn.
async fn load_and_register_toolchain_packages(
    dice: &mut DiceComputations<'_>,
    to_load: Vec<(String, PackageLabel)>,
) {
    if to_load.is_empty() {
        return;
    }
    use futures::FutureExt;
    let loaded = match dice
        .try_compute_join(to_load, |ctx, (tc_label_str, package_label)| {
            async move {
                let eval_result = match ctx.get_interpreter_results(package_label.dupe()).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(
                            "Toolchain package '{}' load failed (non-fatal): {}",
                            tc_label_str,
                            diagnostic_summary(&e)
                        );
                        return Ok::<_, kuro_error::Error>((tc_label_str, Vec::new()));
                    }
                };

                let mut toolchains = Vec::new();
                let mut alias_cache: HashMap<String, String> = HashMap::new();
                for (_target_name, target_node) in eval_result.targets().iter() {
                    if matches!(
                        target_node.rule_type(),
                        RuleType::Native(NativeRuleKind::Toolchain)
                    ) {
                        if let Some(mut info) = extract_toolchain_info_from_node(target_node) {
                            let label = target_node.label().to_string();
                            info.toolchain_impl =
                                canonicalize_extension_sibling_label(&label, &info.toolchain_impl);
                            for (_impl_name, impl_node) in eval_result.targets().iter() {
                                if impl_node.label().to_string() == info.toolchain_impl {
                                    for attr in impl_node.attrs(AttrInspectOptions::All) {
                                        match attr.name {
                                            "toolchain_config" => {
                                                let config =
                                                    extract_label_from_coerced_attr(&attr.value);
                                                if !config.is_empty() {
                                                    info.cc_toolchain_config =
                                                        Some(canonicalize_extension_sibling_label(
                                                            &info.toolchain_impl,
                                                            &config,
                                                        ));
                                                }
                                            }
                                            "module_map" => {
                                                let module_map =
                                                    extract_label_from_coerced_attr(&attr.value);
                                                if !module_map.is_empty() {
                                                    info.cc_toolchain_module_map =
                                                        Some(canonicalize_extension_sibling_label(
                                                            &info.toolchain_impl,
                                                            &module_map,
                                                        ));
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    break;
                                }
                            }
                            let canonical = canonicalize_toolchain_type_label(
                                ctx,
                                &info.toolchain_type,
                                &mut alias_cache,
                            )
                            .await;
                            if canonical != info.toolchain_type {
                                tracing::debug!(
                                    "Resolved toolchain_type alias '{}' -> '{}'",
                                    info.toolchain_type,
                                    canonical
                                );
                                info.toolchain_type = canonical;
                            }
                            toolchains.push((label, info));
                        }
                    }
                }

                Ok((tc_label_str, toolchains))
            }
            .boxed()
        })
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Toolchain loading join failed (non-fatal): {}", e);
            Vec::new()
        }
    };

    // Publish after the parallel package loads complete. Toolchain resolution
    // is priority-ordered by `register_toolchains()` order; appending from
    // worker completion order would make "first match wins" nondeterministic.
    for (tc_label_str, toolchains) in loaded {
        for (label, info) in &toolchains {
            tracing::debug!(
                "Registered toolchain '{}': type='{}', impl='{}'",
                label,
                info.toolchain_type,
                info.toolchain_impl
            );
        }
        let registered_count = toolchains.len();
        for (label, info) in toolchains {
            register_declared_toolchain(label, info);
        }
        if registered_count > 0 {
            tracing::debug!(
                "Loaded {} toolchain(s) from '{}'",
                registered_count,
                tc_label_str
            );
        }
    }
}

fn diagnostic_summary(error: &kuro_error::Error) -> String {
    const MAX_CHARS: usize = 500;
    let rendered = error.to_string();
    truncate_diagnostic(rendered, MAX_CHARS)
}

fn truncate_diagnostic(rendered: String, max_chars: usize) -> String {
    let mut iter = rendered.char_indices();
    let Some((idx, _)) = iter.nth(max_chars) else {
        return rendered;
    };
    let omitted = rendered[idx..].chars().count();
    format!(
        "{} ... (truncated; {} chars omitted)",
        &rendered[..idx],
        omitted
    )
}

/// Plan 13 Phase 3: build a `(label_str, PackageLabel)` list from a slice of
/// raw label strings, applying the same parse + cell-resolve filters the
/// eager loader uses. Returns `None` for entries that can't be resolved.
async fn prepare_toolchain_load_list(
    dice: &mut DiceComputations<'_>,
    labels: &[String],
) -> Vec<(String, PackageLabel)> {
    let cell_resolver = match dice.get_cell_resolver().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("Cell resolver unavailable for deferred load: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for tc_label_str in labels {
        let (repo_name, pkg_path) = match parse_registered_toolchain_label(tc_label_str) {
            Some(v) => v,
            None => continue,
        };
        let cell_name = match CellName::unchecked_new(&repo_name) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if cell_resolver.get(cell_name).is_err() {
            continue;
        }
        let cell_rel_path = CellRelativePath::unchecked_new(&pkg_path);
        let package_label =
            match PackageLabel::from_cell_path(CellPathRef::new(cell_name, cell_rel_path)) {
                Ok(p) => p,
                Err(_) => continue,
            };
        out.push((tc_label_str.clone(), package_label));
    }
    out
}

/// Plan 13 Phase 3: lazy-load deferred toolchain registrations whose
/// origin module or label repo name plausibly matches the required
/// `toolchain_type` labels. Falls back to loading the full deferred pool
/// once if a heuristic-filtered pass still leaves the resolution wanting.
///
/// Returns `true` iff any deferred entry was loaded (caller should retry
/// `resolve_toolchains` if so).
pub async fn ensure_deferred_toolchains_loaded(
    dice: &mut DiceComputations<'_>,
    required_types: &[String],
) -> bool {
    let _guard = DEFERRED_TOOLCHAIN_LOAD_LOCK.lock().await;
    let pool = get_deferred_toolchains();
    if pool.is_empty() {
        return false;
    }

    // Build the heuristic match set: repo names extracted from each
    // required `toolchain_type` label. Most rules_X repos register
    // toolchains for their own `:toolchain_type` target, so a deferred
    // entry whose origin module or label repo name appears here is the
    // likely candidate.
    let needle_repos: std::collections::HashSet<String> = required_types
        .iter()
        .filter_map(|t| {
            let s = t.trim_start_matches('@');
            s.find("//").map(|p| s[..p].to_owned())
        })
        .collect();

    let mut filtered: Vec<String> = Vec::new();
    let mut filtered_keys: Vec<String> = Vec::new();
    for entry in &pool {
        let label_repo = entry
            .label
            .trim_start_matches('@')
            .split("//")
            .next()
            .unwrap_or("")
            .to_owned();
        let key = format!("{}::{}", entry.module, entry.label);
        if deferred_key_already_loaded(&key) {
            continue;
        }
        if needle_repos.contains(&entry.module) || needle_repos.contains(&label_repo) {
            filtered.push(entry.label.clone());
            filtered_keys.push(key);
        }
    }

    if !filtered.is_empty() {
        tracing::debug!(
            "Lazy-loading {} deferred toolchain(s) for required types {:?}",
            filtered.len(),
            required_types
        );
        let to_load = prepare_toolchain_load_list(dice, &filtered).await;
        load_and_register_toolchain_packages(dice, to_load).await;
        for key in filtered_keys {
            mark_deferred_key_loaded(key);
        }
        return true;
    }

    // Heuristic missed. As a last-resort fallback (Plan 13 Phase 3 design),
    // load the entire remaining deferred pool once. This guarantees we
    // never silently fail to find a toolchain that was registered.
    if deferred_all_loaded() {
        return false;
    }
    let mut all_remaining: Vec<String> = Vec::new();
    let mut all_remaining_keys: Vec<String> = Vec::new();
    for entry in &pool {
        let key = format!("{}::{}", entry.module, entry.label);
        if deferred_key_already_loaded(&key) {
            continue;
        }
        all_remaining.push(entry.label.clone());
        all_remaining_keys.push(key);
    }
    if all_remaining.is_empty() {
        mark_deferred_all_loaded();
        return false;
    }
    tracing::debug!(
        "Heuristic miss for {:?}; loading all {} remaining deferred toolchain(s)",
        required_types,
        all_remaining.len()
    );
    let to_load = prepare_toolchain_load_list(dice, &all_remaining).await;
    load_and_register_toolchain_packages(dice, to_load).await;
    for key in all_remaining_keys {
        mark_deferred_key_loaded(key);
    }
    mark_deferred_all_loaded();
    true
}

/// Plan 13 Phase 3: bundled cells that must always be eager-loaded even
/// when registered transitively. These back kuro's bundled toolchains
/// (`@bazel_tools`, `@local_config_*`, `@platforms`) — code paths assume
/// they're materialized before resolution runs.
fn is_bundled_eager_toolchain(label: &str) -> bool {
    let stripped = label.trim_start_matches('@');
    let Some(slash_pos) = stripped.find("//") else {
        return false;
    };
    let repo = &stripped[..slash_pos];
    matches!(
        repo,
        "bazel_tools"
            | "platforms"
            | "local_config_platform"
            | "local_config_cc"
            | "local_config_python"
    ) || repo.starts_with("local_config_")
}

fn should_eager_load_registered_toolchain(tc: &kuro_bzlmod::RegisteredToolchain) -> bool {
    tc.is_root || is_bundled_eager_toolchain(&tc.label)
}

/// Parse a registered toolchain label like `@repo//pkg:target` or `@repo//:all`.
/// Returns `(repo_name, pkg_path)`.
fn parse_registered_toolchain_label(label: &str) -> Option<(String, String)> {
    let stripped = label
        .strip_prefix("@@")
        .or_else(|| label.strip_prefix('@'))?;
    let slash_pos = stripped.find("//")?;
    let repo_name = &stripped[..slash_pos];
    if repo_name.is_empty() {
        return None;
    }
    let after_slashes = &stripped[slash_pos + 2..];
    // Extract package path (before the colon, if any)
    let pkg_path = if let Some(colon_pos) = after_slashes.find(':') {
        &after_slashes[..colon_pos]
    } else {
        after_slashes
    };
    Some((repo_name.to_owned(), pkg_path.to_owned()))
}

/// Plan 24 Phase 2: read the configured target's `exec_properties`
/// attribute and return it as a sorted `BTreeMap` for the actions
/// registry. Empty when the attribute was unset (the default) or when
/// every entry has a non-string key/value (which should be unreachable
/// because the internal attribute type is `dict(string, string)`).
fn collect_target_exec_properties(
    node: kuro_node::nodes::configured::ConfiguredTargetNodeRef<'_>,
) -> Arc<std::collections::BTreeMap<String, String>> {
    use kuro_node::attrs::configured_attr::ConfiguredAttr;

    let mut out = std::collections::BTreeMap::new();
    if let Some(attr) = node.get("exec_properties", AttrInspectOptions::All) {
        if let ConfiguredAttr::Dict(dict) = &attr.value {
            for (k, v) in dict.0.iter() {
                if let (ConfiguredAttr::String(kstr), ConfiguredAttr::String(vstr)) = (k, v) {
                    out.insert(kstr.0.to_string(), vstr.0.to_string());
                }
            }
        }
    }
    Arc::new(out)
}

/// Extract DeclaredToolchainInfo from an unconfigured toolchain() target node.
///
/// Reads the `toolchain_type`, `toolchain`, `exec_compatible_with`, and
/// `target_compatible_with` attributes from the CoercedAttr values.
fn extract_toolchain_info_from_node(
    target_node: kuro_node::nodes::unconfigured::TargetNodeRef<'_>,
) -> Option<DeclaredToolchainInfo> {
    let mut toolchain_type = String::new();
    let mut toolchain_impl = String::new();
    let mut exec_compat = Vec::new();
    let mut target_compat = Vec::new();

    // Read all attributes (including internal ones for constraint lists)
    for attr in target_node.attrs(AttrInspectOptions::All) {
        match attr.name {
            "toolchain_type" => {
                toolchain_type = extract_label_from_coerced_attr(&attr.value);
            }
            "toolchain" => {
                toolchain_impl = extract_label_from_coerced_attr(&attr.value);
            }
            "exec_compatible_with" => {
                exec_compat = extract_label_list_from_coerced_attr(&attr.value);
            }
            "target_compatible_with" => {
                target_compat = extract_label_list_from_coerced_attr(&attr.value);
            }
            _ => {}
        }
    }

    if toolchain_type.is_empty() {
        return None;
    }

    Some(DeclaredToolchainInfo {
        toolchain_type,
        toolchain_impl,
        cc_toolchain_config: None,
        cc_toolchain_module_map: None,
        exec_compatible_with: exec_compat,
        target_compatible_with: target_compat,
    })
}

/// Extract a label string from a CoercedAttr (dep, label, or configuration_dep).
///
/// `exec_compatible_with` / `target_compatible_with` use
/// `attrs.configuration_dep(...)` and land as `CoercedAttr::ConfigurationDep`
/// after coercion — we have to handle that variant or the constraint
/// list reads empty and toolchain resolution falls back to "first
/// registration wins" regardless of host OS/CPU.
fn extract_label_from_coerced_attr(attr: &CoercedAttr) -> String {
    match attr {
        CoercedAttr::Dep(providers_label) => providers_label.target().to_string(),
        CoercedAttr::Label(providers_label) => providers_label.target().to_string(),
        CoercedAttr::SourceLabel(providers_label) => providers_label.target().to_string(),
        CoercedAttr::ConfigurationDep(providers_label) => providers_label.target().to_string(),
        CoercedAttr::SplitTransitionDep(providers_label) => providers_label.target().to_string(),
        CoercedAttr::PluginDep(label) => label.to_string(),
        CoercedAttr::String(s) => s.0.as_str().to_owned(),
        CoercedAttr::OneOf(inner, _) => extract_label_from_coerced_attr(inner),
        CoercedAttr::Selector(selector) => selector
            .all_entries()
            .find_map(|(_key, value)| {
                let label = extract_label_from_coerced_attr(value);
                (!label.is_empty()).then_some(label)
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Extract a list of label strings from a CoercedAttr (list of deps/labels).
fn extract_label_list_from_coerced_attr(attr: &CoercedAttr) -> Vec<String> {
    match attr {
        CoercedAttr::List(list) => list
            .iter()
            .filter_map(|item| {
                let s = extract_label_from_coerced_attr(item);
                if s.is_empty() { None } else { Some(s) }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse a toolchain impl label string into a `TargetLabel`.
///
/// Accepts formats: `@repo//pkg:target`, `@@repo//pkg:target`, `repo//pkg:target`
/// Returns `None` if the label cannot be parsed (missing components, etc.).
fn parse_impl_label_to_target_label(label: &str) -> Option<TargetLabel> {
    // Strip any leading @ characters
    let stripped = label.trim_start_matches('@');
    let slash_pos = stripped.find("//")?;
    let repo_name = &stripped[..slash_pos];
    if repo_name.is_empty() {
        return None;
    }
    let after_slashes = &stripped[slash_pos + 2..];

    // Split into package path and target name
    let (pkg_path, target_name) = if let Some(colon_pos) = after_slashes.find(':') {
        (&after_slashes[..colon_pos], &after_slashes[colon_pos + 1..])
    } else {
        // No colon means target name = last path component (Bazel shorthand)
        let last_slash = after_slashes.rfind('/').map(|p| p + 1).unwrap_or(0);
        (after_slashes, &after_slashes[last_slash..])
    };

    if target_name.is_empty() {
        return None;
    }

    let cell_name = CellName::unchecked_new(repo_name).ok()?;
    let cell_rel_path = CellRelativePath::unchecked_new(pkg_path);
    let package_label =
        PackageLabel::from_cell_path(CellPathRef::new(cell_name, cell_rel_path)).ok()?;
    let target_name_ref = TargetNameRef::new(target_name).ok()?;
    Some(TargetLabel::new(package_label, target_name_ref))
}

/// Canonicalize a label in an extension-generated repository that points at a
/// sibling repo by its internal apparent name.
///
/// Repository rules generated by a module extension commonly write labels like
/// `@coreutils_linux_amd64//:coreutils_toolchain` from inside
/// `@aspect_bazel_lib+toolchains+coreutils_toolchains`. Bazel resolves that
/// through the generated repo's repository mapping to
/// `@aspect_bazel_lib+toolchains+coreutils_linux_amd64`. Kuro does not yet carry
/// per-repository mapping metadata into these extracted strings, so apply the
/// same canonical-name convention while registering toolchain wrappers.
fn canonicalize_extension_sibling_label(owner_label: &str, dep_label: &str) -> String {
    let Some((owner_repo, _)) = owner_label.trim_start_matches('@').split_once("//") else {
        return dep_label.to_owned();
    };
    let Some((module, extension, _owner_internal)) = kuro_bzlmod::parse_canonical_name(owner_repo)
    else {
        return dep_label.to_owned();
    };

    let dep_stripped = dep_label.trim_start_matches('@');
    let Some((dep_repo, rest)) = dep_stripped.split_once("//") else {
        return dep_label.to_owned();
    };
    if dep_repo.is_empty()
        || dep_repo.contains('+')
        || dep_repo == module
        || dep_repo.ends_with('+')
    {
        return dep_label.to_owned();
    }

    format!("@{}+{}+{}//{}", module, extension, dep_repo, rest)
}

/// Plan 11 Phase 8: Resolve a `toolchain_type` label through any `alias()`
/// chain to its canonical `toolchain_type()` rule label.
///
/// rules_rust's `BUILD_for_toolchain` template emits
/// `toolchain_type = "@rules_rust//rust:toolchain"` literally, but
/// `@rules_rust//rust:toolchain` is an `alias()` whose `actual` is
/// `:toolchain_type` (the real `toolchain_type()` rule). Rules' Starlark uses
/// `Label("//rust:toolchain_type")` for resolution, so without alias
/// resolution exact-string matching in `resolve_toolchains` fails and the
/// rust toolchain never resolves.
///
/// On any failure (parse error, package load error, missing target), returns
/// the input label unchanged — toolchain registration should never block on
/// a label we can't resolve.
async fn canonicalize_toolchain_type_label(
    ctx: &mut DiceComputations<'_>,
    label_str: &str,
    cache: &mut HashMap<String, String>,
) -> String {
    if let Some(canonical) = cache.get(label_str) {
        return canonical.clone();
    }

    let mut current = label_str.to_owned();
    let mut visited: Vec<String> = vec![current.clone()];
    const MAX_DEPTH: usize = 8;

    for _ in 0..MAX_DEPTH {
        let target_label = match parse_impl_label_to_target_label(&current) {
            Some(t) => t,
            None => break,
        };
        let pkg = target_label.pkg();
        let eval_result = match ctx.get_interpreter_results(pkg.dupe()).await {
            Ok(e) => e,
            Err(_) => break,
        };
        let node = match eval_result.get_target(target_label.name()) {
            Some(n) => n,
            None => break,
        };
        if !matches!(node.rule_type(), RuleType::Native(NativeRuleKind::Alias)) {
            break;
        }

        // Read 'actual' attr from the alias.
        let mut next: Option<String> = None;
        for attr in node.attrs(AttrInspectOptions::All) {
            if attr.name == "actual" {
                let s = extract_label_from_coerced_attr(&attr.value);
                if !s.is_empty() {
                    next = Some(s);
                }
                break;
            }
        }
        let next = match next {
            Some(s) => s,
            None => break,
        };
        if visited.iter().any(|v| v == &next) {
            break; // cycle
        }
        visited.push(next.clone());
        current = next;
    }

    cache.insert(label_str.to_owned(), current.clone());
    current
}

struct CcToolchainFeaturesMetadata {
    features: CcToolchainFeatures,
    data_labels: Vec<CcToolchainDataLabelMetadata>,
}

struct CcToolchainDataLabelMetadata {
    actions: Vec<String>,
    label: TargetLabel,
}

async fn extract_cc_toolchain_features_metadata(
    ctx: &mut DiceComputations<'_>,
    toolchain_config_label: &str,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Option<CcToolchainFeaturesMetadata> {
    let config_target = parse_impl_label_to_target_label(toolchain_config_label)?;
    let config_node = target_node_for_metadata(ctx, &config_target).await?;

    let mut feature_names = Vec::new();
    let mut default_enabled_features = Vec::new();
    let mut feature_flag_sets = Vec::new();
    let mut seen_features = HashSet::new();
    let mut data_labels = Vec::new();

    let mut action_config_flag_sets = Vec::new();
    if let Some(args_attr) = config_node.attr_or_none("args", AttrInspectOptions::All) {
        for args_label in labels_from_coerced_attr(&args_attr.value, target_cfg) {
            action_config_flag_sets.extend(
                metadata_flag_sets_for_args_label(
                    ctx,
                    args_label,
                    target_cfg,
                    &mut HashSet::new(),
                    &mut data_labels,
                )
                .await,
            );
        }
    }

    for attr_name in ["known_features", "enabled_features"] {
        if let Some(features_attr) = config_node.attr_or_none(attr_name, AttrInspectOptions::All) {
            for feature_set_label in labels_from_coerced_attr(&features_attr.value, target_cfg) {
                for feature_label in metadata_feature_labels_for_set(
                    ctx,
                    feature_set_label,
                    target_cfg,
                    &mut HashSet::new(),
                )
                .await
                {
                    let Some(feature_name) = metadata_feature_name(ctx, feature_label.dupe()).await
                    else {
                        continue;
                    };
                    if seen_features.insert(feature_name.clone()) {
                        feature_names.push(feature_name.clone());
                        let flag_sets = metadata_feature_flag_sets(
                            ctx,
                            feature_label,
                            target_cfg,
                            &mut HashSet::new(),
                            &mut data_labels,
                        )
                        .await;
                        if !flag_sets.is_empty() {
                            feature_flag_sets
                                .push(CcFeatureFlagSets::new(feature_name.clone(), flag_sets));
                        }
                    }
                    if attr_name == "enabled_features"
                        && !default_enabled_features.iter().any(|f| f == &feature_name)
                    {
                        default_enabled_features.push(feature_name);
                    }
                }
            }
        }
    }

    let mut seen_data = HashSet::new();
    data_labels.retain(|data| seen_data.insert(metadata_label_key(&data.label)));

    Some(CcToolchainFeaturesMetadata {
        features: CcToolchainFeatures::new(
            feature_names,
            default_enabled_features,
            action_config_flag_sets,
            feature_flag_sets,
            Vec::new(),
            String::new(),
        ),
        data_labels,
    })
}

async fn target_node_for_metadata(
    ctx: &mut DiceComputations<'_>,
    label: &TargetLabel,
) -> Option<TargetNode> {
    ctx.get_target_node(label).await.ok()
}

fn labels_from_coerced_attr(
    attr: &CoercedAttr,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Vec<TargetLabel> {
    fn push_one(
        labels: &mut Vec<TargetLabel>,
        item: &CoercedAttr,
        target_cfg: &kuro_core::configuration::data::ConfigurationData,
    ) {
        match item {
            CoercedAttr::Selector(selector) => {
                if let Some(value) = metadata_selected_attr(selector.all_entries(), target_cfg) {
                    push_one(labels, value, target_cfg);
                }
            }
            CoercedAttr::Concat(items) => {
                for item in items.iter() {
                    push_one(labels, item, target_cfg);
                }
            }
            CoercedAttr::Dep(label)
            | CoercedAttr::Label(label)
            | CoercedAttr::SourceLabel(label)
            | CoercedAttr::ConfigurationDep(label)
            | CoercedAttr::SplitTransitionDep(label) => labels.push(label.target().dupe()),
            CoercedAttr::PluginDep(label) => labels.push(label.dupe()),
            CoercedAttr::List(list) => {
                for item in list.iter() {
                    push_one(labels, item, target_cfg);
                }
            }
            CoercedAttr::Tuple(list) => {
                for item in list.iter() {
                    push_one(labels, item, target_cfg);
                }
            }
            CoercedAttr::Dict(dict) => {
                for (key, value) in dict.iter() {
                    push_one(labels, key, target_cfg);
                    push_one(labels, value, target_cfg);
                }
            }
            CoercedAttr::OneOf(inner, _) => push_one(labels, inner, target_cfg),
            _ => {}
        }
    }
    let mut labels = Vec::new();
    push_one(&mut labels, attr, target_cfg);
    labels
}

fn metadata_selected_attr<'a>(
    entries: impl IntoIterator<Item = (CoercedSelectorKeyRef<'a>, &'a CoercedAttr)>,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Option<&'a CoercedAttr> {
    let mut default = None;
    for (key, value) in entries {
        match key {
            CoercedSelectorKeyRef::Default => default = Some(value),
            CoercedSelectorKeyRef::Target(key) => {
                if metadata_select_key_matches(&key.to_string(), target_cfg) {
                    return Some(value);
                }
            }
        }
    }
    default
}

fn metadata_select_key_matches(
    key: &str,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> bool {
    let short_name = target_cfg.short_name();
    if (key.contains("@platforms//os:linux") || key.contains("platforms//os:linux"))
        && short_name.contains("linux")
    {
        return true;
    }
    if (key.contains("@platforms//cpu:x86_64") || key.contains("platforms//cpu:x86_64"))
        && (short_name.contains("x86_64") || short_name.contains("amd64"))
    {
        return true;
    }

    let Ok(data) = target_cfg.data() else {
        return false;
    };
    let key_without_at = key.trim_start_matches('@');
    data.constraints.values().any(|value| {
        let value = value.to_string();
        value == key
            || value.trim_start_matches('@') == key_without_at
            || key_without_at.ends_with(value.trim_start_matches('@'))
    }) || metadata_composite_config_setting_name_matches(key, target_cfg)
}

fn metadata_composite_config_setting_name_matches(
    key: &str,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> bool {
    let config_setting_name = key.rsplit(':').next().unwrap_or(key);
    if config_setting_name.is_empty() {
        return false;
    }

    let mut platform_tokens = HashSet::new();
    metadata_add_platform_tokens(&mut platform_tokens, target_cfg.short_name());
    let Ok(data) = target_cfg.data() else {
        return false;
    };
    for value in data.constraints.values() {
        let value = value.to_string();
        let value_name = value.rsplit(':').next().unwrap_or(&value);
        metadata_add_platform_tokens(&mut platform_tokens, value_name);
    }

    let required_tokens = metadata_platform_tokens(config_setting_name);
    !required_tokens.is_empty()
        && required_tokens
            .iter()
            .all(|token| platform_tokens.contains(token))
}

fn metadata_add_platform_tokens(tokens: &mut HashSet<String>, value: &str) {
    for token in metadata_platform_tokens(value) {
        if token.starts_with("gnu.") {
            tokens.insert("gnu".to_owned());
        }
        tokens.insert(token);
    }
}

fn metadata_platform_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if value.contains("x86_64") || value.contains("x86-64") {
        tokens.push("x86_64".to_owned());
    }
    tokens
}

fn string_from_coerced_attr(attr: &CoercedAttr) -> Option<String> {
    match attr {
        CoercedAttr::String(s) | CoercedAttr::EnumVariant(s) => Some(s.as_str().to_owned()),
        CoercedAttr::OneOf(inner, _) => string_from_coerced_attr(inner),
        _ => None,
    }
}

fn strings_from_coerced_attr(
    attr: &CoercedAttr,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Vec<String> {
    match attr {
        CoercedAttr::Selector(selector) => {
            metadata_selected_attr(selector.all_entries(), target_cfg)
                .into_iter()
                .flat_map(|value| strings_from_coerced_attr(value, target_cfg))
                .collect()
        }
        CoercedAttr::Concat(items) => items
            .iter()
            .flat_map(|item| strings_from_coerced_attr(item, target_cfg))
            .collect(),
        CoercedAttr::List(list) => list
            .iter()
            .flat_map(|item| strings_from_coerced_attr(item, target_cfg))
            .collect(),
        CoercedAttr::Tuple(list) => list
            .iter()
            .flat_map(|item| strings_from_coerced_attr(item, target_cfg))
            .collect(),
        CoercedAttr::OneOf(inner, _) => strings_from_coerced_attr(inner, target_cfg),
        _ => string_from_coerced_attr(attr).into_iter().collect(),
    }
}

fn metadata_rule_name(node: &TargetNode) -> Option<&str> {
    match node.rule_type() {
        RuleType::Starlark(rule_type) => Some(rule_type.name.as_str()),
        _ => None,
    }
}

fn metadata_label_key(label: &TargetLabel) -> String {
    label.to_string()
}

fn metadata_path_for_label(
    label: &TargetLabel,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> String {
    let buck_out_root = kuro_execute::path::artifact_path::get_artifact_path_buck_out_root();
    let cell_name = label.pkg().cell_name().as_str();
    let external_cell_name = kuro_core::cells::canonical_dynamic_extension_cell_name(cell_name)
        .unwrap_or_else(|| cell_name.to_owned());
    let cfg_hash = target_cfg.output_hash().as_str();
    let cell_relative_path = label.pkg().cell_relative_path().as_str();
    let target_name = label.name().as_str();
    if kuro_core::cells::is_root_cell_name(cell_name) {
        if cell_relative_path.is_empty() {
            format!(
                "{}/gen/{}/{}/{}",
                buck_out_root, external_cell_name, cfg_hash, target_name
            )
        } else {
            format!(
                "{}/gen/{}/{}/{}/{}",
                buck_out_root, external_cell_name, cfg_hash, cell_relative_path, target_name
            )
        }
    } else if cell_relative_path.is_empty() {
        format!(
            "{}/gen/{}/{}/external/{}/{}",
            buck_out_root, external_cell_name, cfg_hash, external_cell_name, target_name
        )
    } else {
        format!(
            "{}/gen/{}/{}/external/{}/{}/{}",
            buck_out_root,
            external_cell_name,
            cfg_hash,
            external_cell_name,
            cell_relative_path,
            target_name
        )
    }
}

fn metadata_variable_name_for_label(label: &TargetLabel) -> String {
    label.name().as_str().to_owned()
}

enum MetadataFormatSubstitution {
    Variable(String),
    Literal(String),
}

async fn metadata_format_substitution_for_label(
    ctx: &mut DiceComputations<'_>,
    label: TargetLabel,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> MetadataFormatSubstitution {
    let key = metadata_label_key(&label);
    if key.contains("/variables:") {
        return MetadataFormatSubstitution::Variable(metadata_variable_name_for_label(&label));
    }
    if let Some(node) = target_node_for_metadata(ctx, &label).await {
        if matches!(
            metadata_rule_name(&node),
            Some("cc_variable") | Some("_cc_variable")
        ) {
            return MetadataFormatSubstitution::Variable(metadata_variable_name_for_label(&label));
        }
    }
    MetadataFormatSubstitution::Literal(metadata_path_for_label(&label, target_cfg))
}

fn metadata_format_map_from_attr(
    attr: &CoercedAttr,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Vec<(String, TargetLabel)> {
    let mut result = Vec::new();
    let CoercedAttr::Dict(dict) = attr else {
        return result;
    };
    for (key, value) in dict.iter() {
        let key_string = string_from_coerced_attr(key);
        let value_string = string_from_coerced_attr(value);
        let key_label = labels_from_coerced_attr(key, target_cfg).into_iter().next();
        let value_label = labels_from_coerced_attr(value, target_cfg)
            .into_iter()
            .next();
        match (key_string, value_string, key_label, value_label) {
            (Some(name), _, _, Some(label)) => result.push((name, label)),
            (_, Some(name), Some(label), _) => result.push((name, label)),
            _ => {}
        }
    }
    result
}

fn metadata_apply_format(
    arg: &str,
    substitutions: &[(String, MetadataFormatSubstitution)],
) -> String {
    let mut rendered = arg.to_owned();
    for (placeholder, substitution) in substitutions {
        let value = match substitution {
            MetadataFormatSubstitution::Variable(name) => format!("%{{{name}}}"),
            MetadataFormatSubstitution::Literal(value) => value.clone(),
        };
        rendered = rendered.replace(&format!("{{{placeholder}}}"), &value);
    }
    rendered
}

fn metadata_attr_label(
    node: &TargetNode,
    attr_name: &str,
    target_cfg: &kuro_core::configuration::data::ConfigurationData,
) -> Option<TargetLabel> {
    node.attr_or_none(attr_name, AttrInspectOptions::All)
        .and_then(|attr| {
            labels_from_coerced_attr(&attr.value, target_cfg)
                .into_iter()
                .next()
        })
}

fn metadata_attr_string(node: &TargetNode, attr_name: &str) -> Option<String> {
    node.attr_or_none(attr_name, AttrInspectOptions::All)
        .and_then(|attr| string_from_coerced_attr(&attr.value))
}

fn metadata_feature_name_from_node(node: &TargetNode) -> Option<String> {
    metadata_attr_string(node, "feature_name").or_else(|| {
        let label = node
            .attr_or_none("overrides", AttrInspectOptions::All)
            .map(|attr| extract_label_from_coerced_attr(&attr.value))?;
        if label.is_empty() {
            None
        } else {
            label.rsplit(':').next().map(str::to_owned)
        }
    })
}

fn metadata_feature_name<'a>(
    ctx: &'a mut DiceComputations<'_>,
    feature_label: TargetLabel,
) -> futures::future::BoxFuture<'a, Option<String>> {
    async move {
        let node = target_node_for_metadata(ctx, &feature_label).await?;
        metadata_feature_name_from_node(&node)
            .or_else(|| Some(feature_label.name().as_str().to_owned()))
    }
    .boxed()
}

fn metadata_feature_labels_for_set<'a>(
    ctx: &'a mut DiceComputations<'_>,
    feature_set_label: TargetLabel,
    target_cfg: &'a kuro_core::configuration::data::ConfigurationData,
    seen: &'a mut HashSet<String>,
) -> futures::future::BoxFuture<'a, Vec<TargetLabel>> {
    async move {
        let key = metadata_label_key(&feature_set_label);
        if !seen.insert(key) {
            return Vec::new();
        }
        let Some(node) = target_node_for_metadata(ctx, &feature_set_label).await else {
            return Vec::new();
        };
        match metadata_rule_name(&node) {
            Some("cc_feature") | Some("_cc_feature") => vec![feature_set_label],
            Some("cc_feature_set") | Some("_cc_feature_set") => {
                let mut features = Vec::new();
                if let Some(all_of) = node.attr_or_none("all_of", AttrInspectOptions::All) {
                    for label in labels_from_coerced_attr(&all_of.value, target_cfg) {
                        features.extend(
                            metadata_feature_labels_for_set(ctx, label, target_cfg, seen).await,
                        );
                    }
                }
                features
            }
            _ => vec![feature_set_label],
        }
    }
    .boxed()
}

fn metadata_with_features_for_constraints<'a>(
    ctx: &'a mut DiceComputations<'_>,
    labels: Vec<TargetLabel>,
    target_cfg: &'a kuro_core::configuration::data::ConfigurationData,
) -> futures::future::BoxFuture<'a, Vec<CcWithFeatureSet>> {
    async move {
        let mut result = Vec::new();
        for label in labels {
            let features =
                metadata_feature_labels_for_set(ctx, label, target_cfg, &mut HashSet::new()).await;
            let mut names = Vec::new();
            for feature in features {
                if let Some(name) = metadata_feature_name(ctx, feature).await {
                    names.push(name);
                }
            }
            if !names.is_empty() {
                result.push(CcWithFeatureSet::new(names, Vec::new()));
            }
        }
        result
    }
    .boxed()
}

fn metadata_action_names_for_label<'a>(
    ctx: &'a mut DiceComputations<'_>,
    action_label: TargetLabel,
    target_cfg: &'a kuro_core::configuration::data::ConfigurationData,
    seen: &'a mut HashSet<String>,
) -> futures::future::BoxFuture<'a, Vec<String>> {
    async move {
        let key = metadata_label_key(&action_label);
        if !seen.insert(key) {
            return Vec::new();
        }
        let Some(node) = target_node_for_metadata(ctx, &action_label).await else {
            return Vec::new();
        };
        match metadata_rule_name(&node) {
            Some("cc_action_type") | Some("_cc_action_type") => {
                metadata_attr_string(&node, "action_name")
                    .into_iter()
                    .collect()
            }
            Some("cc_action_type_set") | Some("_cc_action_type_set") => {
                let mut names = Vec::new();
                if let Some(actions_attr) = node.attr_or_none("actions", AttrInspectOptions::All) {
                    for label in labels_from_coerced_attr(&actions_attr.value, target_cfg) {
                        names.extend(
                            metadata_action_names_for_label(ctx, label, target_cfg, seen).await,
                        );
                    }
                }
                names
            }
            _ => Vec::new(),
        }
    }
    .boxed()
}

fn metadata_actions_include_link(actions: &[String]) -> bool {
    actions.iter().any(|action| {
        action == "c++-link-executable"
            || action == "c++-link-dynamic-library"
            || action == "c++-link-nodeps-dynamic-library"
            || action == "c++-link-static-library"
            || action.contains("cpp_link")
            || action.contains("link-executable")
            || action.contains("link-dynamic-library")
            || action.contains("link-nodeps-dynamic-library")
            || action.contains("link-static-library")
    })
}

fn metadata_flag_sets_for_args_label<'a>(
    ctx: &'a mut DiceComputations<'_>,
    args_label: TargetLabel,
    target_cfg: &'a kuro_core::configuration::data::ConfigurationData,
    seen: &'a mut HashSet<String>,
    data_labels: &'a mut Vec<CcToolchainDataLabelMetadata>,
) -> futures::future::BoxFuture<'a, Vec<CcFlagSet>> {
    async move {
        let key = metadata_label_key(&args_label);
        if !seen.insert(key) {
            return Vec::new();
        }
        let Some(node) = target_node_for_metadata(ctx, &args_label).await else {
            return Vec::new();
        };
        match metadata_rule_name(&node) {
            Some("cc_args_list") | Some("_cc_args_list") => {
                let mut flag_sets = Vec::new();
                if let Some(args_attr) = node.attr_or_none("args", AttrInspectOptions::All) {
                    for label in labels_from_coerced_attr(&args_attr.value, target_cfg) {
                        flag_sets.extend(
                            metadata_flag_sets_for_args_label(
                                ctx,
                                label,
                                target_cfg,
                                seen,
                                data_labels,
                            )
                            .await,
                        );
                    }
                }
                flag_sets
            }
            Some("cc_args")
            | Some("_cc_args")
            | Some("cc_nested_args")
            | Some("_cc_nested_args") => {
                let mut actions = Vec::new();
                if let Some(actions_attr) = node.attr_or_none("actions", AttrInspectOptions::All) {
                    for action_label in labels_from_coerced_attr(&actions_attr.value, target_cfg) {
                        actions.extend(
                            metadata_action_names_for_label(
                                ctx,
                                action_label,
                                target_cfg,
                                &mut HashSet::new(),
                            )
                            .await,
                        );
                    }
                }

                let mut substitutions = Vec::new();
                if let Some(format_attr) = node.attr_or_none("format", AttrInspectOptions::All) {
                    for (placeholder, label) in
                        metadata_format_map_from_attr(&format_attr.value, target_cfg)
                    {
                        let substitution =
                            metadata_format_substitution_for_label(ctx, label, target_cfg).await;
                        substitutions.push((placeholder, substitution));
                    }
                }

                if let Some(data_attr) = node.attr_or_none("data", AttrInspectOptions::All) {
                    for label in labels_from_coerced_attr(&data_attr.value, target_cfg) {
                        data_labels.push(CcToolchainDataLabelMetadata {
                            actions: actions.clone(),
                            label,
                        });
                    }
                }

                let mut flags = Vec::new();
                if let Some(args_attr) = node.attr_or_none("args", AttrInspectOptions::All) {
                    for arg in strings_from_coerced_attr(&args_attr.value, target_cfg) {
                        flags.push(metadata_apply_format(&arg, &substitutions));
                    }
                }

                let mut nested_groups = Vec::new();
                if let Some(nested_attr) = node.attr_or_none("nested", AttrInspectOptions::All) {
                    for nested_label in labels_from_coerced_attr(&nested_attr.value, target_cfg) {
                        for nested_set in metadata_flag_sets_for_args_label(
                            ctx,
                            nested_label,
                            target_cfg,
                            &mut HashSet::new(),
                            data_labels,
                        )
                        .await
                        {
                            nested_groups.extend(nested_set.into_flag_groups());
                        }
                    }
                }

                let iterate_over = metadata_attr_label(&node, "iterate_over", target_cfg)
                    .map(|label| metadata_variable_name_for_label(&label));
                let expand_if_available =
                    metadata_attr_label(&node, "requires_not_none", target_cfg)
                        .map(|label| metadata_variable_name_for_label(&label));
                let expand_if_not_available =
                    metadata_attr_label(&node, "requires_none", target_cfg)
                        .map(|label| metadata_variable_name_for_label(&label));
                let expand_if_true = metadata_attr_label(&node, "requires_true", target_cfg)
                    .map(|label| metadata_variable_name_for_label(&label));
                let expand_if_false = metadata_attr_label(&node, "requires_false", target_cfg)
                    .map(|label| metadata_variable_name_for_label(&label));
                let expand_if_equal = metadata_attr_label(&node, "requires_equal", target_cfg)
                    .and_then(|label| {
                        metadata_attr_string(&node, "requires_equal_value").map(|value| {
                            CcExpandIfEqual::new(metadata_variable_name_for_label(&label), value)
                        })
                    });
                let group = CcFlagGroup::new(
                    flags,
                    nested_groups,
                    iterate_over,
                    expand_if_available,
                    expand_if_not_available,
                    expand_if_true,
                    expand_if_false,
                    expand_if_equal,
                );
                let with_features = if let Some(attr) =
                    node.attr_or_none("requires_any_of", AttrInspectOptions::All)
                {
                    metadata_with_features_for_constraints(
                        ctx,
                        labels_from_coerced_attr(&attr.value, target_cfg),
                        target_cfg,
                    )
                    .await
                } else {
                    Vec::new()
                };
                vec![CcFlagSet::new(actions, vec![group], with_features)]
            }
            _ => Vec::new(),
        }
    }
    .boxed()
}

fn metadata_feature_flag_sets<'a>(
    ctx: &'a mut DiceComputations<'_>,
    feature_label: TargetLabel,
    target_cfg: &'a kuro_core::configuration::data::ConfigurationData,
    seen: &'a mut HashSet<String>,
    data_labels: &'a mut Vec<CcToolchainDataLabelMetadata>,
) -> futures::future::BoxFuture<'a, Vec<CcFlagSet>> {
    async move {
        let key = metadata_label_key(&feature_label);
        if !seen.insert(key) {
            return Vec::new();
        }
        let Some(node) = target_node_for_metadata(ctx, &feature_label).await else {
            return Vec::new();
        };
        let mut flag_sets = Vec::new();
        if let Some(args_attr) = node.attr_or_none("args", AttrInspectOptions::All) {
            for args_label in labels_from_coerced_attr(&args_attr.value, target_cfg) {
                flag_sets.extend(
                    metadata_flag_sets_for_args_label(
                        ctx,
                        args_label,
                        target_cfg,
                        &mut HashSet::new(),
                        data_labels,
                    )
                    .await,
                );
            }
        }
        flag_sets
    }
    .boxed()
}

// Used to express that the impl Future below captures multiple named lifetimes.
// See https://github.com/rust-lang/rust/issues/34511#issuecomment-373423999 for more details.
trait Captures<'x> {}
impl<T: ?Sized> Captures<'_> for T {}

fn run_analysis_with_env<'a, 'd: 'a>(
    dice: &'a mut DiceComputations<'d>,
    analysis_env: AnalysisEnv<'a>,
    node: ConfiguredTargetNodeRef<'a>,
) -> impl Future<Output = kuro_error::Result<AnalysisResult>> + 'a + Captures<'d> {
    let fut = async move { run_analysis_with_env_underlying(dice, analysis_env, node).await };
    unsafe { UnsafeSendFuture::new_encapsulates_starlark(fut) }
}

async fn run_analysis_with_env_underlying(
    dice: &mut DiceComputations<'_>,
    analysis_env: AnalysisEnv<'_>,
    node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<AnalysisResult> {
    BuckStarlarkModule::with_profiling_async(|env| async move {
        let print = EventDispatcherPrintHandler(get_dispatcher());
        let evaluate_rule_started = Instant::now();

        let validations_from_deps = analysis_env
            .deps
            .iter()
            .filter_map(|(label, analysis_result)| {
                analysis_result
                    .validations
                    .dupe()
                    .map(|v| ((*label).dupe(), v))
            })
            .collect::<SmallMap<_, _>>();

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            1,
            "attr_eval_start",
            evaluate_rule_started,
        );
        // Plan 16.6: attr_eval sub-span — attribute coercion + plugin resolution.
        let (attributes, plugins) = dispatch_span_simple::<
            _,
            kuro_data::AnalysisStageEnd,
            _,
            kuro_error::Result<_>,
        >(
            kuro_data::AnalysisStageStart {
                stage: Some(kuro_data::analysis_stage_start::Stage::AttrEval(())),
            },
            || {
                let mut dep_analysis_results = get_deps_from_analysis_results(analysis_env.deps)?;

                // Phase 8h: Merge aspect providers into dependency provider collections.
                // When a rule's attribute has aspects (e.g., deps with aspects=[cc_proto_aspect]),
                // the aspect produces additional providers that should be accessible via dep[Provider].
                if !analysis_env.aspect_results.is_empty() {
                    use kuro_build_api::interpreter::rule_defs::provider::collection::merge_provider_collections;
                    for (dep_label, aspect_providers) in &analysis_env.aspect_results {
                        if let Some(base_providers) = dep_analysis_results.get(dep_label) {
                            let merged =
                                merge_provider_collections(base_providers, aspect_providers);
                            dep_analysis_results.insert(dep_label.dupe(), merged);
                        }
                    }
                }

                let resolution_ctx = RuleAnalysisAttrResolutionContext {
                    module: &env,
                    dep_analysis_results,
                    query_results: analysis_env.query_results,
                    execution_platform_resolution: node.execution_platform_resolution().clone(),
                };

                Ok((
                    node_to_attrs_struct(node, &mut &resolution_ctx)?,
                    plugins_to_starlark_value(node, &mut &resolution_ctx)?,
                ))
            },
            kuro_data::AnalysisStageEnd {},
        )?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            2,
            "attr_eval_complete",
            evaluate_rule_started,
        );

        // Plan 16.6: configure sub-span — toolchain resolution + exec-group
        // resolution + recursive DICE analysis of toolchain impl targets. Uses
        // a guard (Span::start + .end) because the body has `.await` which
        // rules out span_simple, and the captures make an async-block
        // refactor here more invasive than it's worth.
        let configure_span = DispatchSpan::start(
            get_dispatcher(),
            kuro_data::AnalysisStageStart {
                stage: Some(kuro_data::analysis_stage_start::Stage::Configure(())),
            },
        );

        // Plan 24 Phase 2: read the configured target's `exec_properties`
        // attribute and hand it to the actions registry so each action's
        // RE Platform message gets the keys merged on top of the resolved
        // exec platform's own `exec_properties`. Empty dict (the default)
        // is a no-op — the actions registry skips the merge in that case.
        let target_exec_properties = collect_target_exec_properties(node);

        // Plan 24 Phase 4: extract the names of exec groups declared via
        // `rule(exec_groups={...})` so `actions.run(exec_group=…)` can
        // validate against the list. Empty for rules with no exec groups.
        let exec_group_defs = analysis_env.rule_spec.exec_group_defs();
        let valid_exec_group_names: Arc<[String]> = exec_group_defs
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
            .into();

        // Plan 24 Phase 8: surface registered exec platform candidates to
        // the multi-group resolver so each named exec_group can pick its
        // own platform from the same candidate list `compute_execution_platforms`
        // produced. Without this, every group would resolve against the host
        // alone — `exec_compatible_with` constraints on a named group could
        // never select between registered candidates.
        //
        // Skip `get_execution_platforms()` when the rule needs neither toolchain
        // resolution nor named exec groups. `compute_execution_platforms`
        // analyzes the `[build] execution_platforms = ...` target, whose own
        // analysis (and that of its transitive deps — `platform()`,
        // `constraint_value()`, etc.) re-enters this code path. Without the
        // short-circuit, those rules deadlock waiting on the same DICE key
        // they were dispatched from.
        let needs_candidate_list =
            !analysis_env.rule_spec.toolchain_types().is_empty() || !exec_group_defs.is_empty();
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            3,
            "execution_platforms_start",
            evaluate_rule_started,
        );
        let registered_exec_platforms = if needs_candidate_list {
            dice.get_execution_platforms().await?
        } else {
            None
        };
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            4,
            "execution_platforms_complete",
            evaluate_rule_started,
        );
        let (candidate_constraints, candidate_by_label): (
            Vec<crate::analysis::toolchain_resolution::PlatformConstraints>,
            HashMap<String, ExecutionPlatform>,
        ) = match registered_exec_platforms.as_ref() {
            Some(eps) => {
                let constraints = eps
                    .candidates()
                    .map(crate::analysis::toolchain_resolution::PlatformConstraints::from_execution_platform)
                    .collect();
                let by_label: HashMap<String, ExecutionPlatform> =
                    eps.candidates().map(|p| (p.id(), p.dupe())).collect();
                (constraints, by_label)
            }
            None => (
                vec![crate::analysis::toolchain_resolution::PlatformConstraints::host_platform()],
                HashMap::new(),
            ),
        };

        // Run toolchain resolution BEFORE entering the Starlark evaluator.
        // Note: ensure_registered_toolchains_loaded() is called from
        // calculation.rs::get_analysis_result_inner() which covers all rule types.
        // The resolution reads from the DeclaredToolchainInfo registry which is
        // populated by eager loading above and by toolchain() target analysis.
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            5,
            "toolchain_resolution_start",
            evaluate_rule_started,
        );
        let (toolchain_resolution_result, exec_group_resolution_results) =
            resolve_toolchain_types(
                dice,
                analysis_env.rule_spec.toolchain_types(),
                exec_group_defs,
                node,
                &candidate_constraints,
            )
            .await?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            6,
            "toolchain_resolution_complete",
            evaluate_rule_started,
        );

        // Plan 24 Phase 8: turn each named group's resolved exec_platform
        // label into a full `ExecutionPlatformResolution` (carrying the
        // executor config + RE properties) by looking it up in the
        // registered candidate list. Default group is omitted — the
        // existing `analysis_env.execution_platform` already covers it.
        let group_platforms: HashMap<String, ExecutionPlatformResolution> =
            exec_group_resolution_results
                .iter()
                .filter_map(|(name, result)| {
                    candidate_by_label.get(&result.exec_platform).map(|plat| {
                        (
                            name.clone(),
                            ExecutionPlatformResolution::new(Some(plat.dupe()), Vec::new()),
                        )
                    })
                })
                .collect();

        let registry = AnalysisRegistry::new_from_owner_and_deferred_with_attrs(
            analysis_env.execution_platform.dupe(),
            DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(node.label().dupe())),
            target_exec_properties,
            valid_exec_group_names,
            Arc::new(group_platforms),
        )?;

        // Build ResolvedToolchains from the resolution result.
        // For each resolved toolchain type, analyze the impl target via DICE
        // to get its real providers. Types that fail analysis get None (the
        // ResolvedToolchains.at() method returns an error for unresolved types).
        let mandatory_types: std::collections::HashSet<String> = analysis_env
            .rule_spec
            .toolchain_types()
            .into_iter()
            .filter_map(|(label, mandatory)| if mandatory { Some(label) } else { None })
            .collect();

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            7,
            "ctx_toolchain_provider_analysis_start",
            evaluate_rule_started,
        );
        let resolved_toolchains_for_ctx = if let Some(result) = &toolchain_resolution_result {
            let any_resolved = result.resolved_toolchains.values().any(|v| v.is_some());
            if any_resolved {
                let target_cfg = node.label().cfg().dupe();
                let mut toolchain_providers = std::collections::HashMap::<
                    String,
                    Option<FrozenProviderCollectionValue>,
                >::new();
                let toolchain_count = result.resolved_toolchains.len();

                for (toolchain_index, (type_label, resolved)) in
                    result.resolved_toolchains.iter().enumerate()
                {
                    if let Some(tc) = resolved {
                        tracing::debug!(
                            "  {} → impl='{}', analyzing...",
                            type_label,
                            tc.toolchain_impl
                        );

                        let is_mandatory = mandatory_types.contains(type_label);

                        // Analyze the toolchain impl target. For mandatory
                        // toolchains, propagate analysis errors so the user
                        // sees the real failure (rather than a cryptic
                        // "NoneType has no attribute X" at the call site).
                        let provider_value: Option<FrozenProviderCollectionValue> =
                            match parse_impl_label_to_target_label(&tc.toolchain_impl) {
                                Some(target_label) => {
                                    let configured = target_label.configure(target_cfg.dupe());
                                    let is_self_dependency = configured.eq(node.label());
                                    analysis_ctx_toolchain_provider_checkpoint(
                                        &analysis_env.label,
                                        type_label,
                                        &tc.toolchain_impl,
                                        Some(&configured),
                                        toolchain_index,
                                        toolchain_count,
                                        is_mandatory,
                                        is_self_dependency,
                                        1,
                                        "analysis_start",
                                        evaluate_rule_started,
                                    );
                                    let use_cpp_native_shim =
                                        is_cpp_toolchain_type_label(type_label);
                                    if use_cpp_native_shim {
                                        let toolchain_config_info = None;
                                        let toolchain_metadata =
                                            if let Some(toolchain_config) = &tc.cc_toolchain_config {
                                                extract_cc_toolchain_features_metadata(
                                                    dice,
                                                    toolchain_config,
                                                    &target_cfg,
                                                )
                                                .await
                                            } else {
                                                None
                                            };
                                        let toolchain_features = toolchain_metadata
                                            .as_ref()
                                            .map(|metadata| metadata.features.clone());
                                        let toolchain_data = toolchain_metadata
                                            .as_ref()
                                            .map(|metadata| {
                                                metadata
                                                    .data_labels
                                                    .iter()
                                                    .filter(|data| {
                                                        metadata_actions_include_link(&data.actions)
                                                    })
                                                    .map(|data| {
                                                        let label = &data.label;
                                                        (
                                                            label.configure(target_cfg.dupe()),
                                                            metadata_path_for_label(
                                                                label,
                                                                &target_cfg,
                                                            )
                                                            .into(),
                                                        )
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        let module_map_path = tc
                                            .cc_toolchain_module_map
                                            .as_deref()
                                            .and_then(parse_impl_label_to_target_label)
                                            .map(|label| metadata_path_for_label(&label, &target_cfg));
                                        analysis_ctx_toolchain_provider_checkpoint(
                                            &analysis_env.label,
                                            type_label,
                                            &tc.toolchain_impl,
                                            Some(&configured),
                                            toolchain_index,
                                            toolchain_count,
                                            is_mandatory,
                                            is_self_dependency,
                                            8,
                                            "cc_toolchain_native_shim",
                                            evaluate_rule_started,
                                        );
                                        Some(cc_toolchain_native_shim_provider_collection(
                                            &tc.toolchain_impl,
                                            target_cfg.short_name(),
                                            toolchain_config_info,
                                            toolchain_features,
                                            module_map_path,
                                            toolchain_data,
                                        ))
                                    } else {
                                        let analysis_result =
                                            dice.get_analysis_result(&configured).await;
                                        match analysis_result {
                                            Ok(kuro_core::configuration::compatibility::MaybeCompatible::Compatible(analysis)) => {
                                                analysis_ctx_toolchain_provider_checkpoint(
                                                    &analysis_env.label,
                                                    type_label,
                                                    &tc.toolchain_impl,
                                                    Some(&configured),
                                                    toolchain_index,
                                                    toolchain_count,
                                                    is_mandatory,
                                                    is_self_dependency,
                                                    2,
                                                    "analysis_compatible",
                                                    evaluate_rule_started,
                                                );
                                                match analysis.providers() {
                                                    Ok(providers) => Some(providers.to_owned()),
                                                    Err(e) => {
                                                        analysis_ctx_toolchain_provider_checkpoint(
                                                            &analysis_env.label,
                                                            type_label,
                                                            &tc.toolchain_impl,
                                                            Some(&configured),
                                                            toolchain_index,
                                                            toolchain_count,
                                                            is_mandatory,
                                                            is_self_dependency,
                                                            4,
                                                            "provider_extract_error",
                                                            evaluate_rule_started,
                                                        );
                                                        if is_mandatory {
                                                            return Err(e).with_buck_error_context(|| {
                                                                format!(
                                                                    "Failed to extract providers from \
                                                                     mandatory toolchain impl '{}' for \
                                                                     toolchain type '{}'",
                                                                    tc.toolchain_impl, type_label
                                                                )
                                                            });
                                                        }
                                                        tracing::debug!(
                                                            "  {} → providers extraction failed (optional): {}",
                                                            type_label,
                                                            e
                                                        );
                                                        None
                                                    }
                                                }
                                            }
                                            Ok(_) => {
                                                analysis_ctx_toolchain_provider_checkpoint(
                                                    &analysis_env.label,
                                                    type_label,
                                                    &tc.toolchain_impl,
                                                    Some(&configured),
                                                    toolchain_index,
                                                    toolchain_count,
                                                    is_mandatory,
                                                    is_self_dependency,
                                                    3,
                                                    "analysis_incompatible",
                                                    evaluate_rule_started,
                                                );
                                                if is_mandatory {
                                                    return Err(kuro_error::kuro_error!(
                                                        kuro_error::ErrorTag::Input,
                                                        "Mandatory toolchain impl '{}' for type '{}' \
                                                         is incompatible with target configuration",
                                                        tc.toolchain_impl,
                                                        type_label
                                                    ));
                                                }
                                                tracing::debug!(
                                                    "  {} → impl '{}' incompatible (optional)",
                                                    type_label,
                                                    tc.toolchain_impl
                                                );
                                                None
                                            }
                                            Err(e) => {
                                                analysis_ctx_toolchain_provider_checkpoint(
                                                    &analysis_env.label,
                                                    type_label,
                                                    &tc.toolchain_impl,
                                                    Some(&configured),
                                                    toolchain_index,
                                                    toolchain_count,
                                                    is_mandatory,
                                                    is_self_dependency,
                                                    5,
                                                    "analysis_error",
                                                    evaluate_rule_started,
                                                );
                                                if is_mandatory {
                                                    return Err(e).with_buck_error_context(|| {
                                                        format!(
                                                            "Failed to analyze mandatory toolchain \
                                                             impl '{}' for toolchain type '{}'",
                                                            tc.toolchain_impl, type_label
                                                        )
                                                    });
                                                }
                                                tracing::debug!(
                                                    "  {} → analysis of impl '{}' failed (optional): {:#}",
                                                    type_label,
                                                    tc.toolchain_impl,
                                                    e
                                                );
                                                None
                                            }
                                        }
                                    }
                                }
                                None => {
                                    analysis_ctx_toolchain_provider_checkpoint(
                                        &analysis_env.label,
                                        type_label,
                                        &tc.toolchain_impl,
                                        None,
                                        toolchain_index,
                                        toolchain_count,
                                        is_mandatory,
                                        false,
                                        6,
                                        "parse_error",
                                        evaluate_rule_started,
                                    );
                                    if is_mandatory {
                                        return Err(kuro_error::kuro_error!(
                                            kuro_error::ErrorTag::Input,
                                            "Could not parse mandatory toolchain impl label '{}' \
                                             for toolchain type '{}'",
                                            tc.toolchain_impl,
                                            type_label
                                        ));
                                    }
                                    tracing::debug!(
                                        "  {} → could not parse impl label '{}' (optional)",
                                        type_label,
                                        tc.toolchain_impl
                                    );
                                    None
                                }
                            };

                        toolchain_providers.insert(type_label.clone(), provider_value);
                    } else {
                        analysis_ctx_toolchain_provider_checkpoint(
                            &analysis_env.label,
                            type_label,
                            "<none>",
                            None,
                            toolchain_index,
                            toolchain_count,
                            mandatory_types.contains(type_label),
                            false,
                            7,
                            "unresolved_optional",
                            evaluate_rule_started,
                        );
                        toolchain_providers.insert(type_label.clone(), None);
                    }
                }

                Some(ResolvedToolchains {
                    toolchains: toolchain_providers,
                    exec_platform: result.exec_platform.clone(),
                    target_platform: target_cfg.short_name().to_owned(),
                })
            } else {
                None
            }
        } else {
            None
        };
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            8,
            "ctx_toolchain_provider_analysis_complete",
            evaluate_rule_started,
        );

        configure_span.end(kuro_data::AnalysisStageEnd {});

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            9,
            "starlark_provider_start",
            evaluate_rule_started,
        );
        let eval_kind = StarlarkEvalKind::Analysis(node.label().dupe());
        let eval_provider = StarlarkEvaluatorProvider::new(dice, eval_kind).await?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            10,
            "starlark_provider_complete",
            evaluate_rule_started,
        );
        let mut reentrant_eval =
            eval_provider.make_reentrant_evaluator(&env, analysis_env.cancellation.into())?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            11,
            "reentrant_evaluator_ready",
            evaluate_rule_started,
        );

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            12,
            "rule_impl_evaluator_start",
            evaluate_rule_started,
        );
        let (ctx, list_res) = reentrant_eval.with_evaluator(|mut eval| {
            eval.set_print_handler(&print);
            eval.set_soft_error_handler(&KuroStarlarkSoftErrorHandler);
            if kuro_util::memory_checkpoint::enabled() {
                eval.set_call_stack_checkpoint(Box::new(AnalysisStarlarkProgress::new(
                    &analysis_env.label,
                )));
            }

            let ctx = AnalysisContext::prepare(
                eval.heap(),
                Some(attributes),
                Some(analysis_env.label.dupe()),
                Some(plugins.into()),
                registry,
                dice.global_data().get_digest_config(),
                analysis_env.rule_spec.rule_outputs(),
            );

            // Set resolved toolchains on the context so ctx.toolchains returns
            // real resolution results instead of empty ResolvedToolchains.
            if let Some(resolved) = resolved_toolchains_for_ctx {
                let real_count = resolved.toolchains.values().filter(|v| v.is_some()).count();
                let total = resolved.toolchains.len();
                tracing::debug!(
                    "Setting ResolvedToolchains on ctx for '{}': {}/{} with real providers (exec='{}')",
                    node.label(),
                    real_count,
                    total,
                    resolved.exec_platform
                );
                let resolved_value = eval.heap().alloc(resolved);
                ctx.set_resolved_toolchains(resolved_value);
            }

            // Set resolved exec groups on the context so ctx.exec_groups returns
            // real per-group toolchain resolution results.
            if !exec_group_resolution_results.is_empty() {
                use kuro_build_api::interpreter::rule_defs::context::ResolvedExecGroups;

                let valid_names: Vec<String> = exec_group_resolution_results.keys().cloned().collect();
                // For now, we store the exec group names. Full per-group DICE analysis
                // of toolchain impl targets would mirror the default group flow above.
                // For the initial implementation, store empty provider maps (the resolution
                // result exists but providers are not yet analyzed).
                let groups = exec_group_resolution_results
                    .into_iter()
                    .map(|(name, _result)| {
                        // TODO: analyze per-group toolchain impl targets via DICE
                        (name, std::collections::HashMap::new())
                    })
                    .collect();

                let exec_groups_value = eval.heap().alloc(ResolvedExecGroups {
                    groups,
                    valid_names,
                });
                ctx.set_resolved_exec_groups(exec_groups_value);
            }

            // Plan 16.6: run_impl sub-span — the rule's Starlark impl
            // invocation itself, excluding toolchain resolution and attr
            // coercion. This is what "rule took too long" actually measures.
            analysis_eval_phase_checkpoint(
                "analysis_evaluate_rule_phase",
                &analysis_env.label,
                13,
                "rule_impl_invoke_start",
                evaluate_rule_started,
            );
            let list_res = dispatch_span_simple::<
                _,
                kuro_data::AnalysisStageEnd,
                _,
                kuro_error::Result<_>,
            >(
                kuro_data::AnalysisStageStart {
                    stage: Some(kuro_data::analysis_stage_start::Stage::RunImpl(())),
                },
                || analysis_env.rule_spec.invoke(&mut eval, ctx),
                kuro_data::AnalysisStageEnd {},
            )?;
            analysis_eval_phase_checkpoint(
                "analysis_evaluate_rule_phase",
                &analysis_env.label,
                14,
                "rule_impl_invoke_complete",
                evaluate_rule_started,
            );

            // Bazel convention: when a rule declares outputs via `attr.output`
            // / `attr.output_list` and the impl does not return DefaultInfo
            // explicitly, those declared files become the `default_outputs`.
            // Example: bazel_skylib's `expand_template` returns `None` and
            // relies on this to make its generated file the default output.
            //
            // Compute implicit outputs now (while the actions registry is
            // still live) and, if the impl did not supply DefaultInfo,
            // inject one carrying them.
            let list_res = maybe_inject_implicit_default_info(
                &mut eval,
                ctx,
                list_res,
                &analysis_env.rule_spec.output_attr_names(),
            )?;
            analysis_eval_phase_checkpoint(
                "analysis_evaluate_rule_phase",
                &analysis_env.label,
                15,
                "provider_value_ready",
                evaluate_rule_started,
            );

            Ok((ctx, list_res))
        })?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            16,
            "rule_impl_evaluator_complete",
            evaluate_rule_started,
        );

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            20,
            "promises_start",
            evaluate_rule_started,
        );
        ctx.actions
            .run_promises(&mut RunAnonPromisesAccessorPair(&mut reentrant_eval, dice))
            .await?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            21,
            "promises_complete",
            evaluate_rule_started,
        );

        // Pull the ctx object back out, and steal ctx.action's state back
        let analysis_registry = ctx.take_state();

        // For Bazel test rules (rule(test=True)), auto-inject ExternalRunnerTestInfo
        // if the implementation returned DefaultInfo(executable=...) but no ExternalRunnerTestInfo.
        // Propagate the Bazel `tags` attribute as ExternalRunnerTestInfo labels so that
        // `kuro test --include=small` works for Bazel tests tagged with `tags = ["small"]`.
        let list_res = if node.is_test() {
            use kuro_node::attrs::configured_attr::ConfiguredAttr;
            use kuro_node::attrs::inspect_options::AttrInspectOptions;
            let tags: Vec<String> = if let Some(attr) = node.get("tags", AttrInspectOptions::All) {
                if let ConfiguredAttr::List(list) = attr.value {
                    list.iter()
                        .filter_map(|item| {
                            if let ConfiguredAttr::String(s) = item {
                                Some(s.0.as_str().to_owned())
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            maybe_inject_test_info(env.heap(), list_res, &tags)?
        } else {
            list_res
        };

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            30,
            "provider_collection_start",
            evaluate_rule_started,
        );
        // TODO: Convert the ValueError from `try_from_value` better than just printing its Debug
        // Use try_from_value_subtarget to auto-inject DefaultInfo when missing (Bazel compat:
        // build setting rules like error_format return only custom providers without DefaultInfo)
        let res_typed = ProviderCollection::try_from_value_subtarget(list_res, env.heap())?;

        // Validate rule(provides=[...]) contract: check that the implementation
        // returned all declared provider types.
        let required_provides = node.provides();
        if !required_provides.is_empty() {
            let returned_names = res_typed.provider_names_generic();
            for required in required_provides {
                if !returned_names.iter().any(|name| name == required) {
                    return Err(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "rule '{}' is declared to provide [{}] but implementation only returns [{}]",
                        node.label().name(),
                        required_provides.join(", "),
                        returned_names.join(", "),
                    ));
                }
            }
        }

        {
            let provider_collection = ValueTypedComplex::new_err(env.heap().alloc(res_typed))
                .internal_error("Just allocated provider collection")?;
            analysis_registry
                .analysis_value_storage
                .set_result_value(provider_collection)?;
        }
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            31,
            "provider_collection_complete",
            evaluate_rule_started,
        );

        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            40,
            "freeze_start",
            evaluate_rule_started,
        );
        let finished_eval = reentrant_eval.finish_evaluation();

        let declared_actions = analysis_registry.num_declared_actions();
        let declared_artifacts = analysis_registry.num_declared_artifacts();
        let registry_finalizer = analysis_registry.finalize(&env)?;
        let (token, frozen_env, profile_data) = finished_eval.freeze_and_finish(env)?;
        let recorded_values = registry_finalizer(&frozen_env)?;
        analysis_eval_phase_checkpoint(
            "analysis_evaluate_rule_phase",
            &analysis_env.label,
            41,
            "freeze_complete",
            evaluate_rule_started,
        );

        let validations = transitive_validations(
            validations_from_deps,
            recorded_values.provider_collection()?,
        );

        Ok((
            token,
            AnalysisResult::new(
                recorded_values,
                profile_data,
                HashMap::new(),
                declared_actions,
                declared_artifacts,
                validations,
            ),
        ))
    })
    .await
}

/// Resolve a rule's declared toolchain types and exec groups.
///
/// Returns `(default_group_result, named_exec_group_results)`. The default
/// group corresponds to the rule-level `toolchains=[...]` declaration; named
/// exec groups come from `exec_groups={...}`. Each group is resolved
/// independently and may pick a different exec platform.
async fn resolve_toolchain_types(
    dice: &mut DiceComputations<'_>,
    toolchain_types: Vec<(String, bool)>,
    exec_group_defs: Vec<(String, kuro_node::rule::ExecGroupDef)>,
    node: ConfiguredTargetNodeRef<'_>,
    candidate_platforms: &[crate::analysis::toolchain_resolution::PlatformConstraints],
) -> kuro_error::Result<(
    Option<crate::analysis::toolchain_resolution::ToolchainResolutionResult>,
    std::collections::HashMap<
        String,
        crate::analysis::toolchain_resolution::ToolchainResolutionResult,
    >,
)> {
    let resolution_started = Instant::now();
    let target_label = node.label();
    tracing::debug!(
        "Toolchain types for '{}': {:?} (count={}), exec_groups: {}",
        target_label,
        toolchain_types,
        toolchain_types.len(),
        exec_group_defs.len()
    );
    if toolchain_types.is_empty() && exec_group_defs.is_empty() {
        return Ok((None, std::collections::HashMap::new()));
    }

    use crate::analysis::toolchain_resolution::ExecGroupResolutionRequest;
    use crate::analysis::toolchain_resolution::PlatformConstraints;
    use crate::analysis::toolchain_resolution::RequiredToolchainType;
    use crate::analysis::toolchain_resolution::resolve_toolchains_multi_group;

    // Build resolution requests: default group + named exec groups.
    // The mandatory flag comes from the rule's per-toolchain declaration
    // (`config_common.toolchain_type(..., mandatory=False)`). Optional
    // toolchains with no matching registration resolve to None; the ctx
    // still exposes the entry so `ctx.toolchains[type]` returns the
    // collection (None-typed), not a lookup error.
    //
    // Pre-resolve aliases for every required type so the
    // resolver can compare canonically against registered toolchain types
    // (which were canonicalized at registration time). The original label
    // stays on `RequiredToolchainType.type_label` (used as the result map
    // key, where `ctx.toolchains[X].at(X)` looks it up); the canonical
    // form is stashed in `canonical_type_label` for the comparison side.
    // Without this, rules_python's `@rules_python//python:toolchain_type`
    // (an alias to `@bazel_tools//tools/python:toolchain_type`) never
    // matches the registered `local_config_python//:host_toolchain` whose
    // type was canonicalized to the bazel_tools form.
    let mut alias_cache: HashMap<String, String> = HashMap::new();
    let mut all_type_labels: Vec<String> = Vec::new();
    for (label, _) in &toolchain_types {
        all_type_labels.push(label.clone());
    }
    for (_, def) in &exec_group_defs {
        for t in &def.toolchain_types {
            all_type_labels.push(t.clone());
        }
    }
    let unique_type_count = all_type_labels
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    analysis_toolchain_resolution_checkpoint(
        target_label,
        1,
        "canonicalize_type_labels_start",
        resolution_started,
        [
            ("default_type_count", toolchain_types.len()),
            ("exec_group_count", exec_group_defs.len()),
            ("all_type_count", all_type_labels.len()),
            ("unique_type_count", unique_type_count),
        ],
    );
    for (index, label) in all_type_labels.iter().enumerate() {
        if !alias_cache.contains_key(label) {
            analysis_toolchain_resolution_checkpoint(
                target_label,
                2,
                "canonicalize_type_label_start",
                resolution_started,
                [
                    ("index", index),
                    ("total", all_type_labels.len()),
                    ("label_len", label.len()),
                    ("alias_cache_size", alias_cache.len()),
                ],
            );
            let canonical = canonicalize_toolchain_type_label(dice, label, &mut alias_cache).await;
            let canonical_len = canonical.len();
            let changed = (canonical != *label) as usize;
            alias_cache.insert(label.clone(), canonical);
            analysis_toolchain_resolution_checkpoint(
                target_label,
                3,
                "canonicalize_type_label_complete",
                resolution_started,
                [
                    ("index", index),
                    ("total", all_type_labels.len()),
                    ("label_len", label.len()),
                    ("canonical_len", canonical_len),
                    ("changed", changed),
                    ("alias_cache_size", alias_cache.len()),
                ],
            );
        }
    }
    analysis_toolchain_resolution_checkpoint(
        target_label,
        4,
        "canonicalize_type_labels_complete",
        resolution_started,
        [
            ("all_type_count", all_type_labels.len()),
            ("unique_type_count", unique_type_count),
            ("alias_cache_size", alias_cache.len()),
        ],
    );
    let resolve_alias = |label: &str| -> String {
        alias_cache
            .get(label)
            .cloned()
            .unwrap_or_else(|| label.to_owned())
    };

    let mut requests = vec![ExecGroupResolutionRequest {
        group_name: "default".to_owned(),
        required_types: toolchain_types
            .iter()
            .map(|(label, mandatory)| RequiredToolchainType {
                type_label: label.clone(),
                canonical_type_label: resolve_alias(label),
                mandatory: *mandatory,
            })
            .collect(),
        exec_constraints: vec![],
    }];
    for (name, def) in &exec_group_defs {
        requests.push(ExecGroupResolutionRequest {
            group_name: name.clone(),
            required_types: def
                .toolchain_types
                .iter()
                .map(|t| RequiredToolchainType {
                    type_label: t.clone(),
                    canonical_type_label: resolve_alias(t),
                    // Exec group toolchains default to optional — many rules
                    // declare them with mandatory=False (e.g., cc_test's
                    // test_runner_toolchain_type). The mandatory flag should
                    // be extracted from the rule definition, but for now we
                    // default to false to avoid breaking builds.
                    mandatory: false,
                })
                .collect(),
            exec_constraints: def.exec_compatible_with.clone(),
        });
    }

    // Plan 24 Phase 8: pass the registered candidate platforms (sourced from
    // `compute_execution_platforms` at the call site) so per-group resolution
    // sees the same list as default-group resolution does. Check
    // `target_compatible_with` against the configured target platform, whose
    // constraints may include user/toolchain constraints beyond OS and CPU.
    let target = PlatformConstraints::from_configuration_data(node.label().cfg());
    let candidates: Vec<PlatformConstraints> = if candidate_platforms.is_empty() {
        vec![target.clone()]
    } else {
        candidate_platforms.to_vec()
    };

    // Plan 13 Phase 3: collect every required toolchain type label across
    // all groups. If the first resolve pass fails (Err) or leaves a
    // mandatory type unresolved, lazy-load the deferred pool filtered by
    // those types and retry once. Optional misses are intentionally not
    // retry drivers: Bazel preserves `mandatory = False` requirements as
    // unresolved optional entries rather than making them block analysis.
    let all_required_type_labels: Vec<String> = requests
        .iter()
        .flat_map(|r| r.required_types.iter().map(|t| t.type_label.clone()))
        .collect();

    analysis_toolchain_resolution_checkpoint(
        target_label,
        5,
        "first_resolve_start",
        resolution_started,
        [
            ("request_count", requests.len()),
            ("required_type_count", all_required_type_labels.len()),
            ("candidate_count", candidates.len()),
        ],
    );
    let first = resolve_toolchains_multi_group(&requests, &target, &candidates);
    let (
        first_error,
        first_group_count,
        first_resolved_count,
        first_unresolved_mandatory,
        first_unresolved_optional,
        first_missing_group_count,
    ) = summarize_toolchain_resolution_result(&first, &requests);
    analysis_toolchain_resolution_checkpoint(
        target_label,
        6,
        "first_resolve_complete",
        resolution_started,
        [
            ("error", first_error),
            ("group_count", first_group_count),
            ("resolved_count", first_resolved_count),
            ("unresolved_mandatory", first_unresolved_mandatory),
            ("unresolved_optional", first_unresolved_optional),
            ("missing_group_count", first_missing_group_count),
        ],
    );
    let mandatory_unresolved = needs_deferred_toolchain_retry(&first, &requests);
    analysis_toolchain_resolution_checkpoint(
        target_label,
        7,
        "deferred_retry_decision",
        resolution_started,
        [
            ("retry", mandatory_unresolved as usize),
            ("required_type_count", all_required_type_labels.len()),
            ("unresolved_mandatory", first_unresolved_mandatory),
            ("unresolved_optional", first_unresolved_optional),
            ("first_error", first_error),
        ],
    );
    let resolved_result = if mandatory_unresolved && !all_required_type_labels.is_empty() {
        // Retry even when this call did not perform the load itself. Another
        // concurrent analysis may have populated the global declared-toolchain
        // registry while this call waited on the deferred-load gate.
        analysis_toolchain_resolution_checkpoint(
            target_label,
            8,
            "ensure_deferred_toolchains_loaded_start",
            resolution_started,
            [
                ("required_type_count", all_required_type_labels.len()),
                ("candidate_count", candidates.len()),
            ],
        );
        let loaded = ensure_deferred_toolchains_loaded(dice, &all_required_type_labels).await;
        analysis_toolchain_resolution_checkpoint(
            target_label,
            9,
            "ensure_deferred_toolchains_loaded_complete",
            resolution_started,
            [
                ("loaded", loaded as usize),
                ("required_type_count", all_required_type_labels.len()),
                ("candidate_count", candidates.len()),
            ],
        );
        analysis_toolchain_resolution_checkpoint(
            target_label,
            10,
            "retry_resolve_start",
            resolution_started,
            [
                ("loaded", loaded as usize),
                ("request_count", requests.len()),
                ("required_type_count", all_required_type_labels.len()),
                ("candidate_count", candidates.len()),
            ],
        );
        let retry = resolve_toolchains_multi_group(&requests, &target, &candidates);
        let (
            retry_error,
            retry_group_count,
            retry_resolved_count,
            retry_unresolved_mandatory,
            retry_unresolved_optional,
            retry_missing_group_count,
        ) = summarize_toolchain_resolution_result(&retry, &requests);
        analysis_toolchain_resolution_checkpoint(
            target_label,
            11,
            "retry_resolve_complete",
            resolution_started,
            [
                ("error", retry_error),
                ("group_count", retry_group_count),
                ("resolved_count", retry_resolved_count),
                ("unresolved_mandatory", retry_unresolved_mandatory),
                ("unresolved_optional", retry_unresolved_optional),
                ("missing_group_count", retry_missing_group_count),
            ],
        );
        retry
    } else {
        first
    };

    match resolved_result {
        Ok(multi_result) => {
            let default_result = multi_result.groups.get("default").cloned();
            if let Some(ref result) = default_result {
                let resolved_count = result
                    .resolved_toolchains
                    .values()
                    .filter(|v| v.is_some())
                    .count();
                tracing::debug!(
                    "Toolchain resolution for '{}': {}/{} type(s) resolved",
                    node.label(),
                    resolved_count,
                    result.resolved_toolchains.len()
                );
                for (type_label, resolved) in &result.resolved_toolchains {
                    tracing::debug!(
                        "  {} → {:?}",
                        type_label,
                        resolved.as_ref().map(|r| &r.toolchain_impl)
                    );
                }
            }
            // Collect named exec group results (everything except "default")
            let exec_groups: std::collections::HashMap<String, _> = multi_result
                .groups
                .into_iter()
                .filter(|(name, _)| name != "default")
                .collect();
            Ok((default_result, exec_groups))
        }
        Err(e) => Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Toolchain resolution failed for '{}': {}",
            node.label(),
            e
        )),
    }
}

fn needs_deferred_toolchain_retry(
    first: &Result<crate::analysis::toolchain_resolution::MultiGroupResolutionResult, String>,
    requests: &[crate::analysis::toolchain_resolution::ExecGroupResolutionRequest],
) -> bool {
    let multi = match first {
        Ok(multi) => multi,
        Err(_) => return true,
    };

    requests.iter().any(|request| {
        let Some(result) = multi.groups.get(&request.group_name) else {
            return true;
        };
        request.required_types.iter().any(|required| {
            required.mandatory
                && result
                    .resolved_toolchains
                    .get(&required.type_label)
                    .map(|resolved| resolved.is_none())
                    .unwrap_or(true)
        })
    })
}

fn summarize_toolchain_resolution_result(
    result: &Result<crate::analysis::toolchain_resolution::MultiGroupResolutionResult, String>,
    requests: &[crate::analysis::toolchain_resolution::ExecGroupResolutionRequest],
) -> (usize, usize, usize, usize, usize, usize) {
    let required_count: usize = requests.iter().map(|r| r.required_types.len()).sum();
    let optional_count = requests
        .iter()
        .flat_map(|r| &r.required_types)
        .filter(|t| !t.mandatory)
        .count();
    let mandatory_count = required_count - optional_count;

    let multi = match result {
        Ok(multi) => multi,
        Err(_) => return (1, 0, 0, mandatory_count, optional_count, requests.len()),
    };

    let resolved_count = multi
        .groups
        .values()
        .flat_map(|group| group.resolved_toolchains.values())
        .filter(|resolved| resolved.is_some())
        .count();
    let mut unresolved_mandatory = 0;
    let mut unresolved_optional = 0;
    let mut missing_group_count = 0;
    for request in requests {
        let Some(group) = multi.groups.get(&request.group_name) else {
            missing_group_count += 1;
            for required in &request.required_types {
                if required.mandatory {
                    unresolved_mandatory += 1;
                } else {
                    unresolved_optional += 1;
                }
            }
            continue;
        };
        for required in &request.required_types {
            let unresolved = group
                .resolved_toolchains
                .get(&required.type_label)
                .map(|resolved| resolved.is_none())
                .unwrap_or(true);
            if unresolved {
                if required.mandatory {
                    unresolved_mandatory += 1;
                } else {
                    unresolved_optional += 1;
                }
            }
        }
    }

    (
        0,
        multi.groups.len(),
        resolved_count,
        unresolved_mandatory,
        unresolved_optional,
        missing_group_count,
    )
}

pub fn transitive_validations(
    deps: SmallMap<ConfiguredTargetLabel, TransitiveValidations>,
    provider_collection: FrozenProviderCollectionValueRef,
) -> Option<TransitiveValidations> {
    let provider_collection = provider_collection.to_owned();
    let info = provider_collection
        .value
        .maybe_map(|c| c.as_ref().builtin_provider_value::<FrozenValidationInfo>())
        .map(|v| v.into_owned_frozen_ref());
    if info.is_some() || deps.len() > 1 {
        Some(TransitiveValidations(Arc::new(TransitiveValidationsData {
            info,
            children: deps.into_keys().collect(),
        })))
    } else {
        assert!(
            deps.len() <= 1,
            "Reuse the single element if any from one of the deps for current node."
        );
        deps.into_values().next()
    }
}

fn get_rule_callable(
    eval: &mut Evaluator<'_, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> kuro_error::Result<FrozenValue> {
    let rule_callable = module
        .get_any_visibility(name)
        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))
        .with_buck_error_context(|| format!("Couldn't find rule `{name}`"))?
        .0;
    let rule_callable = rule_callable.owned_value(eval.frozen_heap());
    let rule_callable = rule_callable
        .unpack_frozen()
        .internal_error("Must be frozen")?;
    Ok(rule_callable)
}

pub fn get_rule_impl(
    eval: &mut Evaluator<'_, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> kuro_error::Result<FrozenValue> {
    let rule_callable = get_rule_callable(eval, module, name)?;
    let rule_impl = (FROZEN_RULE_GET_IMPL.get()?)(rule_callable)?;
    Ok(rule_impl)
}

pub fn promise_artifact_mappings<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> kuro_error::Result<SmallMap<String, Value<'v>>> {
    let rule_callable = get_rule_callable(eval, module, name)?;
    let frozen_promise_artifact_mappings =
        (FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL.get()?)(rule_callable)?;

    Ok(frozen_promise_artifact_mappings
        .iter()
        .map(|(frozen_string, frozen_func)| (frozen_string.to_string(), frozen_func.to_value()))
        .collect::<SmallMap<_, _>>())
}

pub fn get_user_defined_rule_spec(
    module: FrozenModule,
    rule_type: &StarlarkRuleType,
    builtins_module: Option<FrozenModule>,
) -> impl RuleSpec + use<> {
    struct Impl {
        module: FrozenModule,
        name: String,
        implicit_rule_outputs: Vec<(String, String)>,
        output_attr_names: Vec<String>,
        toolchain_types: Vec<(String, bool)>,
        exec_group_defs: Vec<(String, kuro_node::rule::ExecGroupDef)>,
        /// Bundled `@kuro_builtins//:exports.bzl`. If it exposes a
        /// `rule_implementation_wrapper`, every Starlark rule impl gets
        /// called as `wrapper(impl, ctx)` instead of `impl(ctx)`. The
        /// wrapper installs the Starlark `ctx` facade that serves the
        /// migrated `ctx`-method bodies. None means @kuro_builtins
        /// isn't registered in this workspace; analysis falls back to
        /// direct invocation.
        builtins_module: Option<FrozenModule>,
    }

    impl RuleSpec for Impl {
        fn invoke<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
            ctx: ValueTyped<'v, AnalysisContext<'v>>,
        ) -> kuro_error::Result<Value<'v>> {
            let rule_impl = get_rule_impl(eval, &self.module, &self.name)?;

            // Store the ctx in thread-local for subrule invocations to use.
            // SAFETY: ctx.to_value() is on the evaluator's heap which outlives eval_function.
            // The TLS slot is cleared in `CtxGuard::drop`, so an error or panic from
            // eval_function cannot leave a stale pointer behind for a later caller.
            let ctx_val = ctx.to_value();
            let ctx_bits: usize = unsafe { std::mem::transmute(ctx_val) };
            kuro_interpreter_for_build::subrule::set_current_rule_ctx_raw(ctx_bits);

            // Stash the bundled `subrule_implementation_wrapper` for
            // the duration of this rule's eval so subrule invocations
            // from inside the rule impl route through the same
            // Starlark facade. Same safety contract as the ctx slot.
            if let Some(wrapper) = self.lookup_subrule_implementation_wrapper(eval)? {
                let wrapper_bits: usize = unsafe { std::mem::transmute(wrapper) };
                kuro_build_api::interpreter::rule_ctx_storage::set_current_subrule_wrapper_raw(
                    wrapper_bits,
                );
            }

            struct CtxGuard;
            impl Drop for CtxGuard {
                fn drop(&mut self) {
                    kuro_interpreter_for_build::subrule::clear_current_rule_ctx();
                    kuro_build_api::interpreter::rule_ctx_storage::clear_current_subrule_wrapper();
                }
            }
            let _guard = CtxGuard;

            // Route Starlark rule impl calls through the bundled
            // `rule_implementation_wrapper(impl, ctx)` when it's
            // available. The wrapper installs a Starlark `ctx` facade
            // that serves migrated `ctx`-method bodies (see
            // `_make_rule_facade` in `@kuro_builtins//:exports.bzl`).
            let result = if let Some(wrapper) = self.lookup_rule_implementation_wrapper(eval)? {
                eval.eval_function(wrapper, &[rule_impl.to_value(), ctx_val], &[])
            } else {
                eval.eval_function(rule_impl.to_value(), &[ctx_val], &[])
            };

            Ok(result?)
        }

        fn promise_artifact_mappings<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> kuro_error::Result<SmallMap<String, Value<'v>>> {
            promise_artifact_mappings(eval, &self.module, &self.name)
        }

        fn rule_outputs(&self) -> Vec<(String, String)> {
            self.implicit_rule_outputs.clone()
        }

        fn output_attr_names(&self) -> Vec<String> {
            self.output_attr_names.clone()
        }

        fn toolchain_types(&self) -> Vec<(String, bool)> {
            self.toolchain_types.clone()
        }

        fn exec_group_defs(&self) -> Vec<(String, kuro_node::rule::ExecGroupDef)> {
            self.exec_group_defs.clone()
        }
    }

    impl Impl {
        /// Look up `rule_implementation_wrapper` in the bundled
        /// `@kuro_builtins//:exports.bzl`. Returns `None` when the
        /// workspace doesn't have `@kuro_builtins` registered (legacy
        /// non-bzlmod), or when the bundled module doesn't expose the
        /// wrapper (e.g. test exports.bzl without the hook).
        fn lookup_rule_implementation_wrapper<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> kuro_error::Result<Option<Value<'v>>> {
            self.lookup_wrapper(eval, "rule_implementation_wrapper")
        }

        /// Look up `subrule_implementation_wrapper` in the bundled
        /// module. Same fallback semantics as the rule wrapper.
        fn lookup_subrule_implementation_wrapper<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> kuro_error::Result<Option<Value<'v>>> {
            self.lookup_wrapper(eval, "subrule_implementation_wrapper")
        }

        fn lookup_wrapper<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
            name: &str,
        ) -> kuro_error::Result<Option<Value<'v>>> {
            let Some(builtins) = &self.builtins_module else {
                return Ok(None);
            };
            let owned = builtins
                .get_option(name)
                .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))?;
            Ok(owned.map(|w| w.owned_value(eval.frozen_heap())))
        }
    }

    // Extract rule(outputs={...}) patterns, attr.output names, toolchain_types,
    // and exec_group_defs from the frozen callable.
    let (implicit_rule_outputs, output_attr_names, toolchain_types, exec_group_defs) =
        if let Ok((val, _)) = module.get_any_visibility(&rule_type.name) {
            if let Ok(typed) = val.downcast::<FrozenStarlarkRuleCallable>() {
                (
                    typed.as_ref().rule_outputs().to_vec(),
                    typed.as_ref().output_attr_names().to_vec(),
                    typed.as_ref().toolchain_types().to_vec(),
                    typed.as_ref().exec_group_defs().to_vec(),
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new())
            }
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

    Impl {
        module,
        name: rule_type.name.clone(),
        implicit_rule_outputs,
        output_attr_names,
        toolchain_types,
        exec_group_defs,
        builtins_module,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::analysis::toolchain_resolution::ExecGroupResolutionRequest;
    use crate::analysis::toolchain_resolution::MultiGroupResolutionResult;
    use crate::analysis::toolchain_resolution::RequiredToolchainType;
    use crate::analysis::toolchain_resolution::ResolvedToolchain;
    use crate::analysis::toolchain_resolution::ToolchainResolutionResult;

    fn required_toolchain(label: &str, mandatory: bool) -> RequiredToolchainType {
        RequiredToolchainType {
            type_label: label.to_owned(),
            canonical_type_label: label.to_owned(),
            mandatory,
        }
    }

    fn resolved_toolchain(label: &str) -> ResolvedToolchain {
        ResolvedToolchain {
            toolchain_target: format!("{label}_target"),
            toolchain_impl: format!("{label}_impl"),
            cc_toolchain_config: None,
            cc_toolchain_module_map: None,
            toolchain_type: label.to_owned(),
        }
    }

    #[test]
    fn test_parse_registered_toolchain_label() {
        // Standard @repo//:all pattern
        assert_eq!(
            parse_registered_toolchain_label("@local_config_cc_toolchains//:all"),
            Some(("local_config_cc_toolchains".to_owned(), "".to_owned()))
        );

        // Double @@ prefix
        assert_eq!(
            parse_registered_toolchain_label("@@rules_rust//rust/toolchain:all"),
            Some(("rules_rust".to_owned(), "rust/toolchain".to_owned()))
        );

        // Package path with target
        assert_eq!(
            parse_registered_toolchain_label("@rules_cc//cc:toolchain_type"),
            Some(("rules_cc".to_owned(), "cc".to_owned()))
        );

        // No target (implicit)
        assert_eq!(
            parse_registered_toolchain_label("@repo//pkg"),
            Some(("repo".to_owned(), "pkg".to_owned()))
        );

        // Root package
        assert_eq!(
            parse_registered_toolchain_label("@repo//:target"),
            Some(("repo".to_owned(), "".to_owned()))
        );

        // Invalid: no @ prefix
        assert_eq!(parse_registered_toolchain_label("//foo:bar"), None);

        // Invalid: empty repo name
        assert_eq!(parse_registered_toolchain_label("@//:all"), None);

        // Invalid: no //
        assert_eq!(parse_registered_toolchain_label("@repo"), None);
    }

    #[test]
    fn test_parse_impl_label_to_target_label() {
        // Standard @repo//pkg:target
        let label = parse_impl_label_to_target_label("@rules_foreign_cc//toolchains:cmake_impl");
        assert!(label.is_some());
        let label = label.unwrap();
        assert_eq!(label.name().as_str(), "cmake_impl");

        // Root package @repo//:target
        let label = parse_impl_label_to_target_label("@local_config_cc//:cc-compiler-k8");
        assert!(label.is_some());
        let label = label.unwrap();
        assert_eq!(label.name().as_str(), "cc-compiler-k8");

        // Double @@ prefix
        let label = parse_impl_label_to_target_label("@@rules_rust//rust:toolchain_impl");
        assert!(label.is_some());

        // No target (implicit - last component of path)
        let label = parse_impl_label_to_target_label("@repo//pkg/subpkg");
        assert!(label.is_some());
        let label = label.unwrap();
        assert_eq!(label.name().as_str(), "subpkg");

        // No @ prefix (Kuro internal format)
        let label = parse_impl_label_to_target_label("local_config_cc//:cc-compiler-k8");
        assert!(label.is_some());
        let label = label.unwrap();
        assert_eq!(label.name().as_str(), "cc-compiler-k8");

        // Relative label (no repo) - should fail
        assert!(parse_impl_label_to_target_label("//foo:bar").is_none());

        // Invalid: empty target
        assert!(parse_impl_label_to_target_label("@repo//:").is_none());
    }

    #[test]
    fn test_root_extension_toolchains_are_eager() {
        let root_extension = kuro_bzlmod::RegisteredToolchain {
            module: "zeromatter".to_owned(),
            label: "@rules_rs+toolchains+default_rust_toolchains//:all".to_owned(),
            is_root: true,
        };
        assert!(should_eager_load_registered_toolchain(&root_extension));

        let transitive_extension = kuro_bzlmod::RegisteredToolchain {
            module: "rules_rust".to_owned(),
            label: "@rules_rust+rust+rust_toolchains//:all".to_owned(),
            is_root: false,
        };
        assert!(!should_eager_load_registered_toolchain(
            &transitive_extension
        ));

        let bundled_transitive = kuro_bzlmod::RegisteredToolchain {
            module: "bazel_tools".to_owned(),
            label: "@local_config_cc//:all".to_owned(),
            is_root: false,
        };
        assert!(should_eager_load_registered_toolchain(&bundled_transitive));
    }

    #[test]
    fn test_deferred_retry_ignores_optional_miss() {
        let requests = vec![ExecGroupResolutionRequest {
            group_name: "default".to_owned(),
            required_types: vec![
                required_toolchain("@rules_rust//rust:toolchain_type", true),
                required_toolchain("@bazel_tools//tools/cpp:toolchain_type", false),
            ],
            exec_constraints: Vec::new(),
        }];
        let mut resolved_toolchains = HashMap::new();
        resolved_toolchains.insert(
            "@rules_rust//rust:toolchain_type".to_owned(),
            Some(resolved_toolchain("@rules_rust//rust:toolchain_type")),
        );
        resolved_toolchains.insert("@bazel_tools//tools/cpp:toolchain_type".to_owned(), None);
        let mut groups = HashMap::new();
        groups.insert(
            "default".to_owned(),
            ToolchainResolutionResult {
                exec_platform: "@local_config_platform//:host".to_owned(),
                resolved_toolchains,
            },
        );

        assert!(!needs_deferred_toolchain_retry(
            &Ok(MultiGroupResolutionResult { groups }),
            &requests
        ));
    }

    #[test]
    fn test_deferred_retry_keeps_mandatory_miss() {
        let requests = vec![ExecGroupResolutionRequest {
            group_name: "default".to_owned(),
            required_types: vec![required_toolchain("@rules_rust//rust:toolchain_type", true)],
            exec_constraints: Vec::new(),
        }];
        let mut resolved_toolchains = HashMap::new();
        resolved_toolchains.insert("@rules_rust//rust:toolchain_type".to_owned(), None);
        let mut groups = HashMap::new();
        groups.insert(
            "default".to_owned(),
            ToolchainResolutionResult {
                exec_platform: "@local_config_platform//:host".to_owned(),
                resolved_toolchains,
            },
        );

        assert!(needs_deferred_toolchain_retry(
            &Ok(MultiGroupResolutionResult { groups }),
            &requests
        ));
    }
}
