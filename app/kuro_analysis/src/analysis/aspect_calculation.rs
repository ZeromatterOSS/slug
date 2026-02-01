/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed licenses.
 */

//! DICE computation for aspect execution (Phase 8c).
//!
//! This module implements the `Key` trait for `AspectKey`, enabling incremental
//! computation and caching of aspect results through DICE.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dice::{CancellationContext, DiceComputations, Key};
use dupe::Dupe;
use futures::FutureExt;

use kuro_build_api::analysis::AnalysisResult;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::paths::module::StarlarkModulePath;
use kuro_interpreter::file_loader::LoadedModule;
use kuro_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use kuro_interpreter_for_build::aspect::FrozenStarlarkAspectCallable;
use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
use kuro_node::aspect_type::StarlarkAspectType;
use starlark::values::OwnedFrozenValueTyped;

use super::aspect_key::{AspectKey, AspectValue};
use super::calculation::AnalysisKey;

#[async_trait]
impl Key for AspectKey {
    type Value = kuro_error::Result<AspectValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        // 1. Get target's analysis result (ensures target is analyzed first)
        let target_result = ctx
            .compute(&AnalysisKey(self.target.dupe()))
            .await?
            .buck_error_context("Failed to get target analysis result for aspect")?
            .require_compatible()?;

        // 2. Load aspect module and extract aspect callable
        let module = load_aspect_module(ctx, &self.aspect_type).await?;
        let aspect = get_aspect_from_module(&module, &self.aspect_type.name)?;

        // 3. Check if aspect should apply to this target (required_providers filter)
        if !aspect_applies_to_target(&aspect, &target_result)? {
            // Target doesn't match required_providers - return empty providers
            // Note: AspectValue needs the target's default frozen provider collection
            // For now, return the target's providers (aspect just passes through)
            let providers_ref = target_result.providers()?;
            return Ok(AspectValue {
                providers: providers_ref.to_owned(),
            });
        }

        // 4. Compute aspects on dependencies (shadow graph - TODO for Phase 8d)
        let _dep_aspects = compute_dep_aspects(ctx, &self.target, &aspect).await?;

        // 5. Execute aspect implementation function
        let providers = execute_aspect(
            ctx,
            &self.target,
            &aspect,
            &target_result,
            cancellations,
        ).await?;

        Ok(AspectValue { providers })
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // Aspect values are not comparable (similar to AnalysisKey)
        false
    }
}

/// Load the module containing the aspect definition (Phase 8c).
/// Follows the same pattern as get_loaded_module() for rules.
async fn load_aspect_module(
    ctx: &mut DiceComputations<'_>,
    aspect_type: &Arc<StarlarkAspectType>,
) -> kuro_error::Result<LoadedModule> {
    let module = match &aspect_type.path {
        BzlOrBxlPath::Bzl(import_path) => {
            ctx.get_loaded_module_from_import_path(import_path).await?
        }
        BzlOrBxlPath::Bxl(bxl_path) => {
            ctx.get_loaded_module(StarlarkModulePath::BxlFile(&bxl_path))
                .await?
        }
    };
    Ok(module)
}

/// Extract the frozen aspect callable from a loaded module by name.
/// Follows the same pattern as get_rule_callable() for rules.
///
/// Returns an OwnedFrozenValueTyped that can be dereferenced to access the aspect.
fn get_aspect_from_module(
    module: &LoadedModule,
    name: &str,
) -> kuro_error::Result<starlark::values::OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>> {
    let aspect_value = module
        .env()
        .get_any_visibility(name)
        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))
        .with_buck_error_context(|| format!("Couldn't find aspect `{name}`"))?
        .0;

    aspect_value
        .downcast::<FrozenStarlarkAspectCallable>()
        .map_err(|v| kuro_error::Error::from(kuro_error::internal_error!(
            "Expected aspect callable, got: {}",
            v.value().to_repr()
        )))
}

/// Check if an aspect should be applied to a target based on required_providers filtering.
///
/// The required_providers field implements any-of logic:
/// - Empty required_providers = applies to all targets
/// - [[A], [B, C]] means: target must have A OR (B AND C)
fn aspect_applies_to_target(
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
) -> kuro_error::Result<bool> {
    let aspect_ref = aspect.as_ref();
    let required_providers = aspect_ref.required_providers();

    // Empty required_providers = applies to all targets
    if required_providers.is_empty() {
        return Ok(true);
    }

    let target_providers = target_result.providers()?;

    // Check any-of logic: [[A], [B, C]] means A OR (B AND C)
    for provider_set in required_providers {
        // Check if target has all providers in this set (AND logic within set)
        let mut has_all = true;
        for provider_val in provider_set {
            // Extract provider ID from the frozen value
            // Provider values in required_providers are frozen provider callable objects
            let provider_callable = provider_val
                .as_provider_callable()
                .internal_error("required_providers must contain provider callables")?;
            let provider_id = provider_callable.id()?;

            // Access the inner FrozenProviderCollection through the value() method
            if !target_providers.value().contains_provider(provider_id) {
                has_all = false;
                break;
            }
        }
        if has_all {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Recursively compute aspects on dependencies.
///
/// This follows the aspect's attr_aspects to determine which dependency
/// attributes to propagate through, then computes the aspect on each
/// dependency in parallel via DICE.
///
/// NOTE: For the initial implementation, this returns an empty map.
/// The gather_deps() function in kuro_configured/nodes.rs already handles
/// aspect triggering based on attributes with aspects=[...] attached.
/// Full shadow graph propagation via attr_aspects will be implemented later.
#[allow(dead_code)]
async fn compute_dep_aspects(
    _ctx: &mut DiceComputations<'_>,
    _target: &ConfiguredTargetLabel,
    _aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
) -> kuro_error::Result<HashMap<ConfiguredTargetLabel, AspectValue>> {
    // TODO(Phase 8d): Implement full shadow graph propagation
    // This requires:
    // 1. Getting the configured target node from DICE
    // 2. Extracting attributes matching aspect.attr_aspects()
    // 3. Recursively computing aspects on those dependencies
    // 4. Building shadow graph (ctx.rule.attr.deps contains aspect results)
    //
    // For now, aspects execute on individual targets without recursive propagation.
    // gather_deps() in kuro_configured/nodes.rs handles initial aspect triggering.
    Ok(HashMap::new())
}

/// Execute an aspect on a target, returning the provider collection.
///
/// This sets up the Starlark evaluation context and calls run_aspect_basic()
/// to execute the aspect implementation function.
#[allow(dead_code)]
async fn execute_aspect(
    ctx: &mut DiceComputations<'_>,
    target: &ConfiguredTargetLabel,
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
    cancellations: &CancellationContext,
) -> kuro_error::Result<kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue> {
    use kuro_build_api::analysis::registry::AnalysisRegistry;
    use kuro_build_api::interpreter::rule_defs::aspect::AspectContext;
    use kuro_build_api::interpreter::rule_defs::aspect::AspectRuleInfo;
    use kuro_build_api::interpreter::rule_defs::aspect::AspectTargetProviders;
    use kuro_build_api::interpreter::rule_defs::provider::collection::ProviderCollection;
    use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
    use kuro_core::unsafe_send_future::UnsafeSendFuture;
    use kuro_events::dispatch::get_dispatcher;
    use kuro_execute::digest_config::HasDigestConfig;
    use kuro_interpreter::dice::starlark_provider::StarlarkEvalKind;
    use kuro_interpreter::factory::BuckStarlarkModule;
    use kuro_interpreter::factory::StarlarkEvaluatorProvider;
    use kuro_interpreter::print_handler::EventDispatcherPrintHandler;
    use kuro_interpreter::soft_error::KuroStarlarkSoftErrorHandler;
    use kuro_node::attrs::inspect_options::AttrInspectOptions;
    use kuro_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
    use starlark::values::structs::AllocStruct;
    use starlark::values::ValueOfUnchecked;

    use crate::attrs::resolve::configured_attr::ConfiguredAttrExt;
    use crate::attrs::resolve::ctx::AttrResolutionContext;

    // 1. Get the configured target node to access rule kind and attributes
    let node = ctx
        .get_configured_target_node(target)
        .await?
        .require_compatible()?;

    // 2. Extract rule kind (name of the rule, e.g., "cc_library")
    let rule_kind = node.rule_type().name().to_owned();

    // Extract execution platform before moving node into async block
    let execution_platform = node.execution_platform_resolution().dupe();

    // Get target providers before moving into async block
    let target_providers_frozen = target_result.providers()?.to_owned();

    // Collect attributes before moving into async block
    let attrs_to_resolve: Vec<_> = node.attrs(AttrInspectOptions::All).collect();

    // Collect dependency labels from the node's deps
    let dep_labels: Vec<ConfiguredTargetLabel> = node.deps().map(|d| d.label().dupe()).collect();

    // Fetch dependency analysis results in parallel via DICE
    let dep_analysis_results: std::collections::HashMap<ConfiguredTargetLabel, kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue> = {
        let dep_results = ctx.compute_join(dep_labels.iter(), |ctx, label| {
            async move {
                ctx.compute(&AnalysisKey(label.dupe())).await
            }.boxed()
        }).await;

        let mut map = std::collections::HashMap::new();
        for (label, result) in dep_labels.iter().zip(dep_results) {
            if let Ok(Ok(analysis_result)) = result {
                if let Ok(compatible) = analysis_result.require_compatible() {
                    if let Ok(providers) = compatible.providers() {
                        map.insert(label.dupe(), providers.to_owned());
                    }
                }
            }
        }
        map
    };

    // Execute aspect in a Starlark module environment (similar to rule analysis)
    let fut = async move {
        let result = BuckStarlarkModule::with_profiling_async(|env| async move {
            let print = EventDispatcherPrintHandler(get_dispatcher());

            // 3. Build attribute resolution context for resolving rule attributes
            struct AspectAttrResolutionContext<'v> {
                module: &'v starlark::environment::Module,
                dep_analysis_results: std::collections::HashMap<ConfiguredTargetLabel, kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue>,
                execution_platform: kuro_core::execution_types::execution::ExecutionPlatformResolution,
            }

            impl<'v> AttrResolutionContext<'v> for &'_ AspectAttrResolutionContext<'v> {
                fn starlark_module(&self) -> &'v starlark::environment::Module {
                    self.module
                }

                fn get_dep(
                    &mut self,
                    target: &kuro_core::provider::label::ConfiguredProvidersLabel,
                ) -> kuro_error::Result<starlark::values::FrozenValueTyped<'v, kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection>> {
                    // Look up the dep's analysis result from our collected map
                    // For Phase 8c, this returns the target's providers (not aspect shadow graph)
                    match self.dep_analysis_results.get(target.target()) {
                        Some(providers) => {
                            let providers_ref = providers.lookup_inner(target)?;
                            Ok(providers_ref.add_heap_ref(self.module.frozen_heap()))
                        }
                        None => Err(kuro_error::kuro_error!(
                            kuro_error::ErrorTag::Tier0,
                            "Dependency {} not found in aspect resolution context",
                            target
                        )),
                    }
                }

                fn resolve_unkeyed_placeholder(
                    &mut self,
                    _name: &str,
                ) -> kuro_error::Result<Option<kuro_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg>> {
                    Ok(None)
                }

                fn resolve_query(
                    &mut self,
                    _query: &str,
                ) -> kuro_error::Result<Arc<crate::attrs::resolve::ctx::AnalysisQueryResult>> {
                    Err(kuro_error::internal_error!(
                        "Aspect attribute resolution does not support queries yet"
                    ))
                }

                fn execution_platform_resolution(&self) -> &kuro_core::execution_types::execution::ExecutionPlatformResolution {
                    &self.execution_platform
                }
            }

            // 4. Extract rule attributes as a Starlark struct (ctx.rule.attr)
            let rule_attrs = {
                let resolution_ctx = AspectAttrResolutionContext {
                    module: &env,
                    dep_analysis_results,
                    execution_platform: execution_platform.clone(),
                };

                let mut resolved_attrs = Vec::with_capacity(attrs_to_resolve.len());
                for a in attrs_to_resolve {
                    // Resolve each attribute value
                    resolved_attrs.push((
                        a.name,
                        a.value.resolve_single(target.pkg(), &mut &resolution_ctx)?,
                    ));
                }
                env.heap().alloc_typed_unchecked(AllocStruct(resolved_attrs))
            };

            // 5. Create AnalysisRegistry for action registration
            let registry = AnalysisRegistry::new_from_owner(
                BaseDeferredKey::TargetLabel(target.dupe()),
                execution_platform,
            )?;

            // 6. Set up Starlark evaluator
            let eval_kind = StarlarkEvalKind::Analysis(target.dupe());
            let eval_provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;
            let mut reentrant_eval =
                eval_provider.make_reentrant_evaluator(&env, cancellations.into())?;

            // 7. Execute aspect implementation function (inlined from run_aspect_basic)
            let (aspect_context, provider_collection) = reentrant_eval.with_evaluator(|mut eval| {
                eval.set_print_handler(&print);
                eval.set_soft_error_handler(&KuroStarlarkSoftErrorHandler);

                // Get target providers for aspect execution (as a reference)
                let target_providers = target_providers_frozen.as_ref();

                // Get aspect implementation function
                let aspect_impl = aspect.as_ref().implementation();

                // Check if aspect has custom attributes
                let has_attrs = !aspect.as_ref().attrs().is_empty();

                // Create aspect-specific attributes
                let aspect_attr = if has_attrs {
                    let attrs_struct = eval.heap().alloc(AllocStruct::EMPTY);
                    Some(ValueOfUnchecked::new(attrs_struct))
                } else {
                    None
                };

                // Create AspectRuleInfo
                let rule_info = eval.heap().alloc_typed(AspectRuleInfo::new(rule_kind.clone(), rule_attrs.cast()));

                // Create AspectContext
                let ctx = AspectContext::prepare(
                    eval.heap(),
                    aspect_attr,
                    target.dupe(),
                    rule_info,
                    registry,
                    ctx.global_data().get_digest_config(),
                );

                // Wrap target providers for target[SomeInfo] syntax
                let target_val = eval.heap().alloc(AspectTargetProviders::new(
                    target_providers,
                    target.dupe(),
                ));

                // Invoke aspect implementation: impl(target, ctx)
                let result = eval
                    .eval_function(
                        aspect_impl.to_value(),
                        &[target_val, ctx.to_value()],
                        &[],
                    )
                    .buck_error_context("Aspect implementation failed")?;

                // Validate and convert to ProviderCollection
                let providers = ProviderCollection::try_from_aspect_value(result)?;

                Ok((ctx, providers))
            })?;

            // 8. Store the provider collection in the analysis registry
            use starlark::values::ValueTypedComplex;
            let provider_collection_value = ValueTypedComplex::new_err(env.heap().alloc(provider_collection))
                .internal_error("Just allocated provider collection")?;

            let analysis_registry = aspect_context.take_state();
            analysis_registry.analysis_value_storage.set_result_value(provider_collection_value)?;

            // Finalize the registry before freezing
            let registry_finalizer = analysis_registry.finalize(&env)?;

            // 9. Freeze the environment
            // Note: provider_collection was moved into alloc(), aspect_context was consumed by take_state()
            let finished_eval = reentrant_eval.finish_evaluation();
            let (token, frozen_env, _) = finished_eval.freeze_and_finish(env)?;

            // 10. Get the frozen provider collection
            let recorded_values = registry_finalizer(&frozen_env)?;
            let frozen_providers = recorded_values.provider_collection()?;

            Ok((token, frozen_providers.to_owned()))
        })
        .await;

        // Return the FrozenProviderCollectionValue
        // with_profiling_async automatically handles the profiling token
        result
    };

    unsafe { UnsafeSendFuture::new_encapsulates_starlark(fut) }.await
}
