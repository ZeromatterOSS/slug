/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use dice::CancellationContext;
use dice::DiceComputations;
use dupe::Dupe;
use futures::Future;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::anon_promises_dyn::RunAnonPromisesAccessorPair;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::analysis::registry::AnalysisRegistry;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use kuro_build_api::interpreter::rule_defs::context::AnalysisContext;
use kuro_build_api::interpreter::rule_defs::context::ResolvedToolchains;
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
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::execution::GetExecutionPlatforms;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use kuro_node::rule_type::StarlarkRuleType;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
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
use crate::analysis::native_rule_analysis::register_declared_toolchain;
use crate::analysis::plugins::plugins_to_starlark_value;
use crate::attrs::resolve::ctx::AnalysisQueryResult;
use crate::attrs::resolve::ctx::AttrResolutionContext;
use crate::attrs::resolve::node_to_attrs_struct::node_to_attrs_struct;

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

/// Reset the eager loading flag (for fresh builds / daemon restart).
pub fn reset_toolchain_loading() {
    TOOLCHAINS_LOADING_DONE.store(false, Ordering::SeqCst);
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

    let registered = kuro_bzlmod::get_registered_toolchains();
    if registered.is_empty() {
        TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
        return;
    }

    tracing::debug!(
        "Eagerly loading {} registered toolchain package(s)",
        registered.len()
    );

    let cell_resolver = match dice.get_cell_resolver().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("Failed to get cell resolver for toolchain loading: {}", e);
            TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
            return;
        }
    };

    // Pre-filter packages whose paths can be resolved without touching DICE.
    // These filters were previously inside the serial loop; hoisting them out
    // lets the parallel dispatch below only do the expensive work.
    let mut to_load: Vec<(String, PackageLabel)> = Vec::new();
    let mut skipped_count = 0;
    for tc_label_str in &registered {
        let (repo_name, pkg_path) = match parse_registered_toolchain_label(tc_label_str) {
            Some(v) => v,
            None => {
                tracing::debug!("Could not parse toolchain label: {}", tc_label_str);
                continue;
            }
        };

        // Skip extension-generated repos (canonical names like "module+ext+repo").
        // These may not be materialized yet and loading them eagerly triggers
        // expensive extension execution (downloading SDKs, running repo rules, etc.).
        // They will be loaded on-demand when actually needed during analysis.
        if repo_name.contains('+') || repo_name.contains('~') {
            tracing::debug!(
                "Skipping extension-generated toolchain repo '{}' (loaded on-demand)",
                tc_label_str
            );
            skipped_count += 1;
            continue;
        }

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

        // Extension repos (paths containing '+' or '~') are loaded the same way
        // as other cells: via `dice.get_interpreter_results`. If the repo's
        // content is not yet on disk, the file-ops layer triggers
        // `ExtensionRepoExecutionKey::compute` and materialises it on demand.
        // Previously this path short-circuited with a skip, which meant
        // toolchain registrations coming from extension-generated repos such
        // as `rules_cc+cc_configure_extension+local_config_cc_toolchains`
        // were never fed into the DeclaredToolchainInfo registry — so CC
        // toolchain resolution failed with "No execution platform found that
        // provides all mandatory toolchain types". Fall through to the normal
        // `to_load.push(...)` path below for both in-tree and extension cells.
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

    // Load packages in parallel. Each load triggers its own transitive
    // `.bzl` chain; the Phase-7 spoke-materialization parallelization inside
    // each cascade gets to run concurrently across extensions too. DICE
    // dedups overlapping `get_interpreter_results` requests.
    //
    // Non-fatal errors (load failures, missing target nodes) are swallowed
    // to match the previous `continue` behaviour.
    use futures::FutureExt;
    let _: Vec<()> = match dice
        .try_compute_join(to_load, |ctx, (tc_label_str, package_label)| {
            async move {
                let eval_result = match ctx.get_interpreter_results(package_label.dupe()).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(
                            "Toolchain package '{}' load failed (non-fatal): {}",
                            tc_label_str,
                            e
                        );
                        return Ok::<(), kuro_error::Error>(());
                    }
                };

                let mut registered_count = 0;
                for (_target_name, target_node) in eval_result.targets().iter() {
                    if matches!(
                        target_node.rule_type(),
                        RuleType::Native(NativeRuleKind::Toolchain)
                    ) {
                        if let Some(info) = extract_toolchain_info_from_node(target_node) {
                            let label = target_node.label().to_string();
                            tracing::debug!(
                                "Eagerly registered toolchain '{}': type='{}', impl='{}'",
                                label,
                                info.toolchain_type,
                                info.toolchain_impl
                            );
                            register_declared_toolchain(label, info);
                            registered_count += 1;
                        }
                    }
                }

                if registered_count > 0 {
                    tracing::debug!(
                        "Loaded {} toolchain(s) from '{}'",
                        registered_count,
                        tc_label_str
                    );
                }
                Ok(())
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

    if skipped_count > 0 {
        tracing::debug!(
            "Skipped {} toolchain registration(s) (extension repos or unavailable)",
            skipped_count
        );
    }

    TOOLCHAINS_LOADING_DONE.store(true, Ordering::SeqCst);
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
        exec_compatible_with: exec_compat,
        target_compatible_with: target_compat,
    })
}

/// Extract a label string from a CoercedAttr (dep or label).
fn extract_label_from_coerced_attr(attr: &CoercedAttr) -> String {
    match attr {
        CoercedAttr::Dep(providers_label) => providers_label.target().to_string(),
        CoercedAttr::Label(providers_label) => providers_label.target().to_string(),
        CoercedAttr::String(s) => s.0.as_str().to_owned(),
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
        let registered_exec_platforms = if needs_candidate_list {
            dice.get_execution_platforms().await?
        } else {
            None
        };
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
        let (toolchain_resolution_result, exec_group_resolution_results) =
            resolve_toolchain_types(
                analysis_env.rule_spec.toolchain_types(),
                exec_group_defs,
                node,
                &candidate_constraints,
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
        let resolved_toolchains_for_ctx = if let Some(result) = &toolchain_resolution_result {
            let any_resolved = result.resolved_toolchains.values().any(|v| v.is_some());
            if any_resolved {
                let target_cfg = node.label().cfg().dupe();
                let mut toolchain_providers = std::collections::HashMap::<
                    String,
                    Option<FrozenProviderCollectionValue>,
                >::new();

                for (type_label, resolved) in &result.resolved_toolchains {
                    if let Some(tc) = resolved {
                        tracing::debug!(
                            "  {} → impl='{}', analyzing...",
                            type_label,
                            tc.toolchain_impl
                        );

                        // Try to analyze the toolchain impl target
                        let provider_value =
                            match parse_impl_label_to_target_label(&tc.toolchain_impl) {
                                Some(target_label) => {
                                    let configured =
                                        target_label.configure(target_cfg.dupe());
                                    match dice.get_analysis_result(&configured).await {
                                        Ok(maybe_compat) => match maybe_compat {
                                            kuro_core::configuration::compatibility::MaybeCompatible::Compatible(analysis) => {
                                                match analysis.providers() {
                                                    Ok(providers) => {
                                                        let owned = providers.to_owned();
                                                        tracing::debug!(
                                                            "  {} → analyzed impl '{}' successfully",
                                                            type_label,
                                                            tc.toolchain_impl
                                                        );
                                                        Some(owned)
                                                    }
                                                    Err(e) => {
                                                        tracing::debug!(
                                                            "  {} → providers extraction failed: {}",
                                                            type_label,
                                                            e
                                                        );
                                                        None
                                                    }
                                                }
                                            }
                                            _ => {
                                                tracing::debug!(
                                                    "  {} → impl '{}' is incompatible",
                                                    type_label,
                                                    tc.toolchain_impl
                                                );
                                                None
                                            }
                                        },
                                        Err(e) => {
                                            tracing::debug!(
                                                "  {} → analysis of impl '{}' failed: {:#}",
                                                type_label,
                                                tc.toolchain_impl,
                                                e
                                            );
                                            None
                                        }
                                    }
                                }
                                None => {
                                    tracing::debug!(
                                        "  {} → could not parse impl label '{}'",
                                        type_label,
                                        tc.toolchain_impl
                                    );
                                    None
                                }
                            };

                        toolchain_providers.insert(type_label.clone(), provider_value);
                    } else {
                        toolchain_providers.insert(type_label.clone(), None);
                    }
                }

                Some(ResolvedToolchains {
                    toolchains: toolchain_providers,
                    exec_platform: result.exec_platform.clone(),
                })
            } else {
                None
            }
        } else {
            None
        };

        configure_span.end(kuro_data::AnalysisStageEnd {});

        let eval_kind = StarlarkEvalKind::Analysis(node.label().dupe());
        let eval_provider = StarlarkEvaluatorProvider::new(dice, eval_kind).await?;
        let mut reentrant_eval =
            eval_provider.make_reentrant_evaluator(&env, analysis_env.cancellation.into())?;

        let (ctx, list_res) = reentrant_eval.with_evaluator(|mut eval| {
            eval.set_print_handler(&print);
            eval.set_soft_error_handler(&KuroStarlarkSoftErrorHandler);

            let ctx = AnalysisContext::prepare(
                eval.heap(),
                Some(attributes),
                Some(analysis_env.label),
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

            Ok((ctx, list_res))
        })?;

        ctx.actions
            .run_promises(&mut RunAnonPromisesAccessorPair(&mut reentrant_eval, dice))
            .await?;

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

        let finished_eval = reentrant_eval.finish_evaluation();

        let declared_actions = analysis_registry.num_declared_actions();
        let declared_artifacts = analysis_registry.num_declared_artifacts();
        let registry_finalizer = analysis_registry.finalize(&env)?;
        let (token, frozen_env, profile_data) = finished_eval.freeze_and_finish(env)?;
        let recorded_values = registry_finalizer(&frozen_env)?;

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
fn resolve_toolchain_types(
    toolchain_types: Vec<(String, bool)>,
    exec_group_defs: Vec<(String, kuro_node::rule::ExecGroupDef)>,
    node: ConfiguredTargetNodeRef<'_>,
    candidate_platforms: &[crate::analysis::toolchain_resolution::PlatformConstraints],
) -> (
    Option<crate::analysis::toolchain_resolution::ToolchainResolutionResult>,
    std::collections::HashMap<
        String,
        crate::analysis::toolchain_resolution::ToolchainResolutionResult,
    >,
) {
    tracing::debug!(
        "Toolchain types for '{}': {:?} (count={}), exec_groups: {}",
        node.label(),
        toolchain_types,
        toolchain_types.len(),
        exec_group_defs.len()
    );
    if toolchain_types.is_empty() && exec_group_defs.is_empty() {
        return (None, std::collections::HashMap::new());
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
    let mut requests = vec![ExecGroupResolutionRequest {
        group_name: "default".to_owned(),
        required_types: toolchain_types
            .iter()
            .map(|(label, mandatory)| RequiredToolchainType {
                type_label: label.clone(),
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
    // sees the same list as default-group resolution does. `target_platform`
    // stays as host — `target_compatible_with` filtering of toolchains is
    // unchanged by Plan 24.
    let target = PlatformConstraints::host_platform();
    let candidates: Vec<PlatformConstraints> = if candidate_platforms.is_empty() {
        vec![target.clone()]
    } else {
        candidate_platforms.to_vec()
    };

    match resolve_toolchains_multi_group(&requests, &target, &candidates) {
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
            (default_result, exec_groups)
        }
        Err(e) => {
            tracing::debug!("Toolchain resolution failed for '{}': {}", node.label(), e);
            (None, std::collections::HashMap::new())
        }
    }
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
    use super::*;

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
}
