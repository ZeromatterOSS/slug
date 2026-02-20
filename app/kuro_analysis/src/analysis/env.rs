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

use dice::CancellationContext;
use dice::DiceComputations;
use dupe::Dupe;
use futures::Future;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::anon_promises_dyn::RunAnonPromisesAccessorPair;
use kuro_build_api::analysis::registry::AnalysisRegistry;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use kuro_build_api::interpreter::rule_defs::context::AnalysisContext;
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
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::execution_types::execution::ExecutionPlatformResolution;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::unsafe_send_future::UnsafeSendFuture;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_events::dispatch::get_dispatcher;
use kuro_execute::digest_config::HasDigestConfig;
use kuro_interpreter::dice::starlark_provider::StarlarkEvalKind;
use kuro_interpreter::factory::BuckStarlarkModule;
use kuro_interpreter::factory::StarlarkEvaluatorProvider;
use kuro_interpreter::print_handler::EventDispatcherPrintHandler;
use kuro_interpreter::soft_error::KuroStarlarkSoftErrorHandler;
use kuro_interpreter::types::rule::FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL;
use kuro_interpreter::types::rule::FROZEN_RULE_GET_IMPL;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
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
                // Create ExternalRunnerTestInfo with the executable as command
                let test_type = heap.alloc_str("custom").to_value();
                let command = heap.alloc(vec![exe]);
                let test_info = create_external_runner_test_info_for_bazel_test(test_type, command);
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

        let (attributes, plugins) = {
            let mut dep_analysis_results = get_deps_from_analysis_results(analysis_env.deps)?;

            // Phase 8h: Merge aspect providers into dependency provider collections.
            // When a rule's attribute has aspects (e.g., deps with aspects=[cc_proto_aspect]),
            // the aspect produces additional providers that should be accessible via dep[Provider].
            if !analysis_env.aspect_results.is_empty() {
                use kuro_build_api::interpreter::rule_defs::provider::collection::merge_provider_collections;
                for (dep_label, aspect_providers) in &analysis_env.aspect_results {
                    if let Some(base_providers) = dep_analysis_results.get(dep_label) {
                        let merged = merge_provider_collections(base_providers, aspect_providers);
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

            (
                node_to_attrs_struct(node, &mut &resolution_ctx)?,
                plugins_to_starlark_value(node, &mut &resolution_ctx)?,
            )
        };

        let registry = AnalysisRegistry::new_from_owner(
            BaseDeferredKey::TargetLabel(node.label().dupe()),
            analysis_env.execution_platform.dupe(),
        )?;

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
            );

            let list_res = analysis_env.rule_spec.invoke(&mut eval, ctx)?;

            Ok((ctx, list_res))
        })?;

        ctx.actions
            .run_promises(&mut RunAnonPromisesAccessorPair(&mut reentrant_eval, dice))
            .await?;

        // Pull the ctx object back out, and steal ctx.action's state back
        let analysis_registry = ctx.take_state();

        // For Bazel test rules (rule(test=True)), auto-inject ExternalRunnerTestInfo
        // if the implementation returned DefaultInfo(executable=...) but no ExternalRunnerTestInfo.
        let list_res = if node.is_test() {
            maybe_inject_test_info(env.heap(), list_res)?
        } else {
            list_res
        };

        // TODO: Convert the ValueError from `try_from_value` better than just printing its Debug
        // Use try_from_value_subtarget to auto-inject DefaultInfo when missing (Bazel compat:
        // build setting rules like error_format return only custom providers without DefaultInfo)
        let res_typed = ProviderCollection::try_from_value_subtarget(list_res, env.heap())?;
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
) -> impl RuleSpec + use<> {
    struct Impl {
        module: FrozenModule,
        name: String,
    }

    impl RuleSpec for Impl {
        fn invoke<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
            ctx: ValueTyped<'v, AnalysisContext<'v>>,
        ) -> kuro_error::Result<Value<'v>> {
            let rule_impl = get_rule_impl(eval, &self.module, &self.name)?;
            Ok(eval.eval_function(rule_impl.to_value(), &[ctx.to_value()], &[])?)
        }

        fn promise_artifact_mappings<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> kuro_error::Result<SmallMap<String, Value<'v>>> {
            promise_artifact_mappings(eval, &self.module, &self.name)
        }
    }

    Impl {
        module,
        name: rule_type.name.clone(),
    }
}
