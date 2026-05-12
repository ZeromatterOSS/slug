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
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use futures::FutureExt;
use slug_build_api::analysis::AnalysisResult;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_interpreter::file_loader::LoadedModule;
use slug_interpreter::load_module::InterpreterCalculation;
use slug_interpreter::paths::module::StarlarkModulePath;
use slug_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use slug_interpreter_for_build::aspect::FrozenStarlarkAspectCallable;
use slug_interpreter_for_build::attrs::resolve_configuration_field_to_label;
use slug_node::aspect_type::StarlarkAspectType;
use slug_node::bzl_or_bxl_path::BzlOrBxlPath;
use starlark::values::OwnedFrozenValueTyped;
use tracing::debug;

use super::aspect_key::AspectKey;
use super::aspect_key::AspectValue;
use super::calculation::AnalysisKey;

/// Parse a label string and analyze the target via DICE to get its providers.
///
/// Used to resolve aspect attribute defaults (e.g., configuration_field targets).
/// The label is resolved relative to the target's cell context.
async fn parse_and_analyze_label(
    ctx: &mut DiceComputations<'_>,
    label_str: &str,
    target: &ConfiguredTargetLabel,
) -> slug_error::Result<(
    ConfiguredTargetLabel,
    slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue,
)> {
    use slug_core::cells::name::CellName;
    use slug_core::cells::paths::CellRelativePath;
    use slug_core::package::PackageLabel;
    use slug_core::target::label::label::TargetLabel;
    use slug_core::target::name::TargetNameRef;

    // Parse label: @repo//pkg:target
    // Extract repo (cell), package, and target name
    let label_str = label_str.trim();

    // Strip @@ or @ prefix if present (Bazel 9 uses @@ for canonical labels)
    let label_no_at = label_str.trim_start_matches('@');

    // Split on //
    let (cell_str, path_and_target) = label_no_at.split_once("//").ok_or_else(|| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Invalid label format: {}",
            label_str
        )
    })?;

    // Split on :
    let (pkg_path, target_name) = path_and_target.split_once(':').ok_or_else(|| {
        // If no ':', target name = last path component (Bazel default)
        slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Invalid label format (no target): {}",
            label_str
        )
    })?;

    // Resolve cell name - use the cell from the label or fall back to target's cell
    let cell_name = if cell_str.is_empty() {
        target.pkg().cell_name()
    } else {
        CellName::unchecked_new(cell_str)?
    };

    let pkg = PackageLabel::new(cell_name, CellRelativePath::unchecked_new(pkg_path))?;

    let target_label = TargetLabel::new(pkg, TargetNameRef::unchecked_new(target_name));

    // Use the same configuration as the parent target
    let configured_label = target_label.configure(target.cfg().dupe());

    // Analyze via DICE
    let analysis_result = ctx
        .compute(&AnalysisKey(configured_label.dupe()))
        .await?
        .buck_error_context("Failed to analyze aspect attribute dependency")?
        .require_compatible()?;

    Ok((configured_label, analysis_result.providers()?.to_owned()))
}

#[async_trait]
impl Key for AspectKey {
    type Value = slug_error::Result<AspectValue>;

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
                analysis_result: None,
            });
        }

        // 4. Compute aspects on dependencies (shadow graph propagation)
        let dep_aspects =
            compute_dep_aspects(ctx, &self.target, &aspect, &self.aspect_type).await?;

        // Load @slug_builtins so the aspect dispatch can reach
        // `aspect_implementation_wrapper`. `None` for legacy workspaces
        // without the bundled cell.
        let builtins_module = super::calculation::get_slug_builtins_module(ctx).await?;

        // 5. Execute aspect implementation function with shadow graph
        let (providers, analysis_result) = execute_aspect(
            ctx,
            &self.target,
            &aspect,
            &self.aspect_type,
            &target_result,
            dep_aspects,
            builtins_module,
            cancellations,
        )
        .await?;

        Ok(AspectValue {
            providers,
            analysis_result: Some(analysis_result),
        })
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
) -> slug_error::Result<LoadedModule> {
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
) -> slug_error::Result<starlark::values::OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>> {
    let aspect_value = module
        .env()
        .get_any_visibility(name)
        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Tier0))
        .with_buck_error_context(|| format!("Couldn't find aspect `{name}`"))?
        .0;

    aspect_value
        .downcast::<FrozenStarlarkAspectCallable>()
        .map_err(|v| {
            slug_error::Error::from(slug_error::internal_error!(
                "Expected aspect callable, got: {}",
                v.value().to_repr()
            ))
        })
}

/// Check if an aspect should be applied to a target based on required_providers
/// and required_aspect_providers filtering.
///
/// Both filters implement any-of logic:
/// - Empty list = no filtering (passes)
/// - [[A], [B, C]] means: target must have A OR (B AND C)
///
/// If both required_providers AND required_aspect_providers are specified,
/// the target must satisfy BOTH (AND logic between the two filters).
///
/// Phase 8e: required_aspect_providers checks against the target's own providers.
/// Phase 8f will check against providers from `requires` aspects.
fn aspect_applies_to_target(
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
) -> slug_error::Result<bool> {
    let aspect_ref = aspect.as_ref();
    let required_providers = aspect_ref.required_providers();
    let required_aspect_providers = aspect_ref.required_aspect_providers();

    // If both are empty, applies to all targets
    if required_providers.is_empty() && required_aspect_providers.is_empty() {
        return Ok(true);
    }

    let target_providers = target_result.providers()?;

    // Check required_providers filter
    let satisfies_required_providers = if required_providers.is_empty() {
        true
    } else {
        check_any_of_providers(required_providers, &target_providers)?
    };

    // Check required_aspect_providers filter
    // Phase 8e: Check against target's providers (simplified implementation)
    // Phase 8f: Will check against providers from `requires` aspects
    let satisfies_required_aspect_providers = if required_aspect_providers.is_empty() {
        true
    } else {
        check_any_of_providers(required_aspect_providers, &target_providers)?
    };

    // Both must be satisfied (Bazel semantics)
    Ok(satisfies_required_providers && satisfies_required_aspect_providers)
}

/// Helper to check any-of provider filtering.
///
/// Implements the logic: [[A], [B, C]] means A OR (B AND C)
/// - Outer list: OR (any set must match)
/// - Inner list: AND (all providers in set must be present)
fn check_any_of_providers(
    required: &[Vec<starlark::values::FrozenValue>],
    target_providers: &slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValueRef<'_>,
) -> slug_error::Result<bool> {
    for provider_set in required {
        // Check if target has all providers in this set (AND logic within set)
        let mut has_all = true;
        for provider_val in provider_set {
            // Extract provider ID from the frozen value
            // Provider values are frozen provider callable objects
            let provider_callable = provider_val
                .as_provider_callable()
                .internal_error("required providers must contain provider callables")?;
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

/// Recursively compute aspects on dependencies via DICE.
///
/// This follows the aspect's attr_aspects to determine which dependency
/// attributes to propagate through. For each dependency found:
/// 1. Check if attribute name matches attr_aspects (or "*" matches all)
/// 2. Extract dependency labels using ConfiguredAttrTraversal
/// 3. Recursively compute AspectKey for each (dep, aspect_type) pair
/// 4. Collect results into a HashMap for shadow graph injection
///
/// The recursive DICE computation ensures depth-first execution order:
/// dependencies' aspects complete before the parent's aspect executes.
async fn compute_dep_aspects(
    ctx: &mut DiceComputations<'_>,
    target: &ConfiguredTargetLabel,
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    aspect_type: &Arc<StarlarkAspectType>,
) -> slug_error::Result<HashMap<ConfiguredTargetLabel, AspectValue>> {
    use slug_core::plugins::PluginKind;
    use slug_core::plugins::PluginKindSet;
    use slug_core::provider::label::ConfiguredProvidersLabel;
    use slug_core::target::label::label::TargetLabel;
    use slug_node::attrs::configured_traversal::ConfiguredAttrTraversal;
    use slug_node::attrs::inspect_options::AttrInspectOptions;
    use slug_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;

    // 1. Get the configured target node
    let node = ctx
        .get_configured_target_node(target)
        .await?
        .require_compatible()?;

    // 2. Get attr_aspects from the aspect (which attributes to propagate through)
    let attr_aspects = aspect.as_ref().attr_aspects();
    let propagate_all = attr_aspects.iter().any(|a| a == "*");

    // If no attr_aspects specified, no propagation
    if attr_aspects.is_empty() {
        return Ok(HashMap::new());
    }

    // 3. Collector for dependency labels
    struct AspectDepsCollector {
        deps: Vec<ConfiguredTargetLabel>,
    }

    impl ConfiguredAttrTraversal for AspectDepsCollector {
        fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> slug_error::Result<()> {
            self.deps.push(dep.target().dupe());
            Ok(())
        }

        fn dep_with_plugins(
            &mut self,
            dep: &ConfiguredProvidersLabel,
            _plugin_kinds: &PluginKindSet,
        ) -> slug_error::Result<()> {
            self.deps.push(dep.target().dupe());
            Ok(())
        }

        // Exec deps and toolchain deps do not propagate aspects (Bazel semantics)
        fn exec_dep(&mut self, _dep: &ConfiguredProvidersLabel) -> slug_error::Result<()> {
            Ok(())
        }

        fn toolchain_dep(&mut self, _dep: &ConfiguredProvidersLabel) -> slug_error::Result<()> {
            Ok(())
        }

        fn plugin_dep(&mut self, _dep: &TargetLabel, _kind: &PluginKind) -> slug_error::Result<()> {
            Ok(())
        }
    }

    // 4. Traverse configured attributes matching attr_aspects
    // ConfiguredTargetNode::attrs() returns already-configured attributes (ConfiguredAttrFull)
    let mut aspect_keys = Vec::new();

    for a in node.attrs(AttrInspectOptions::All) {
        // Check if this attribute should propagate the aspect
        let should_propagate = propagate_all || attr_aspects.iter().any(|aa| aa == a.name);

        if !should_propagate {
            continue;
        }

        // Only propagate through label and label_list attributes
        // (Other attribute types cannot have dependencies)
        if !a.attr.coercer().is_label_type() {
            continue;
        }

        // Traverse the configured attribute to collect dependencies
        let mut collector = AspectDepsCollector { deps: Vec::new() };
        a.traverse(node.label().pkg(), &mut collector)?;

        // Create AspectKey for each dependency
        for dep_label in collector.deps {
            aspect_keys.push(AspectKey::new(dep_label, aspect_type.dupe()));
        }
    }

    // 5. Compute all aspects in parallel via DICE
    if aspect_keys.is_empty() {
        return Ok(HashMap::new());
    }

    let dep_aspect_results = ctx
        .compute_join(aspect_keys.iter(), |ctx, key| {
            async move { ctx.compute(key).await }.boxed()
        })
        .await;

    // 6. Collect results into HashMap
    let mut result = HashMap::new();
    for (key, res) in aspect_keys.into_iter().zip(dep_aspect_results) {
        match res {
            Ok(Ok(aspect_value)) => {
                result.insert(key.target.dupe(), aspect_value);
            }
            Ok(Err(e)) => {
                // Propagate aspect computation errors
                return Err(e);
            }
            Err(e) => {
                // Convert DICE errors
                return Err(e.into());
            }
        }
    }

    Ok(result)
}

/// Execute an aspect on a target, returning the provider collection.
///
/// This sets up the Starlark evaluation context and calls run_aspect_basic()
/// to execute the aspect implementation function.
///
/// The `dep_aspects` parameter contains shadow graph results: aspect providers
/// for dependencies that have been processed by this aspect. When resolving
/// `ctx.rule.attr.deps`, these aspect providers take precedence over the
/// target's regular providers.
async fn execute_aspect(
    ctx: &mut DiceComputations<'_>,
    target: &ConfiguredTargetLabel,
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    aspect_type: &Arc<StarlarkAspectType>,
    target_result: &AnalysisResult,
    dep_aspects: HashMap<ConfiguredTargetLabel, AspectValue>,
    builtins_module: Option<starlark::environment::FrozenModule>,
    cancellations: &CancellationContext,
) -> slug_error::Result<(
    slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue,
    AnalysisResult,
)> {
    use slug_build_api::analysis::registry::AnalysisRegistry;
    use slug_build_api::interpreter::rule_defs::aspect::AspectContext;
    use slug_build_api::interpreter::rule_defs::aspect::AspectRuleInfo;
    use slug_build_api::interpreter::rule_defs::aspect::AspectTargetProviders;
    use slug_build_api::interpreter::rule_defs::provider::collection::ProviderCollection;
    use slug_core::deferred::base_deferred_key::BaseDeferredKey;
    use slug_core::unsafe_send_future::UnsafeSendFuture;
    use slug_events::dispatch::get_dispatcher;
    use slug_execute::digest_config::HasDigestConfig;
    use slug_interpreter::dice::starlark_provider::StarlarkEvalKind;
    use slug_interpreter::factory::BuckStarlarkModule;
    use slug_interpreter::factory::StarlarkEvaluatorProvider;
    use slug_interpreter::print_handler::EventDispatcherPrintHandler;
    use slug_interpreter::soft_error::SlugStarlarkSoftErrorHandler;
    use slug_node::attrs::inspect_options::AttrInspectOptions;
    use slug_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
    use starlark::values::ValueOfUnchecked;
    use starlark::values::structs::AllocStruct;

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

    // Build dep_analysis_results: aspect results take precedence (shadow graph)
    // Only fetch regular analysis for deps that don't have aspect results
    let dep_analysis_results: std::collections::HashMap<
        ConfiguredTargetLabel,
        slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue,
    > = {
        // Determine which deps need regular analysis (no aspect result available)
        let deps_needing_analysis: Vec<_> = dep_labels
            .iter()
            .filter(|label| !dep_aspects.contains_key(*label))
            .cloned()
            .collect();

        // Fetch regular analysis results only for deps without aspect results
        let regular_analysis: HashMap<ConfiguredTargetLabel, slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue> = if !deps_needing_analysis.is_empty() {
            let results = ctx.compute_join(deps_needing_analysis.iter(), |ctx, label| {
                async move {
                    ctx.compute(&AnalysisKey(label.dupe())).await
                }.boxed()
            }).await;

            let mut map = HashMap::new();
            for (label, result) in deps_needing_analysis.iter().zip(results) {
                if let Ok(Ok(analysis_result)) = result {
                    if let Ok(compatible) = analysis_result.require_compatible() {
                        if let Ok(providers) = compatible.providers() {
                            map.insert(label.dupe(), providers.to_owned());
                        }
                    }
                }
            }
            map
        } else {
            HashMap::new()
        };

        // Build combined map: aspect results take precedence
        let mut combined = HashMap::new();

        // First, add aspect results (shadow graph - these take precedence)
        for (label, aspect_value) in &dep_aspects {
            combined.insert(label.dupe(), aspect_value.providers.dupe());
        }

        // Then add regular analysis results for deps without aspects
        for (label, providers) in regular_analysis {
            if !combined.contains_key(&label) {
                combined.insert(label, providers);
            }
        }

        combined
    };

    // Resolve aspect attribute dependencies (async - before Starlark eval)
    // For each aspect attr with a configuration_field() default or a regular
    // label default, analyze the target and collect providers for ctx.attr resolution.
    let aspect_attr_deps: HashMap<
        String,
        (
            ConfiguredTargetLabel,
            slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue,
        ),
    > = {
        use slug_node::attrs::coerced_attr::CoercedAttr;

        let mut attr_deps = HashMap::new();
        let aspect_attrs = aspect.as_ref().attrs();

        for (attr_name, starlark_attr) in aspect_attrs {
            if let Some((fragment, name)) = starlark_attr.configuration_field() {
                if let Some(label_str) = resolve_configuration_field_to_label(fragment, name) {
                    // Parse the label and analyze the target
                    match parse_and_analyze_label(ctx, label_str, target).await {
                        Ok((resolved_label, providers)) => {
                            attr_deps.insert(attr_name.clone(), (resolved_label, providers));
                        }
                        Err(e) => {
                            debug!(attr = %attr_name, label = %label_str, error = ?e,
                                "aspect configuration_field attr resolution failed; aspect may not use this attr");
                        }
                    }
                }
            } else if let Some(default) = starlark_attr.default() {
                // Also resolve regular label (CoercedAttr::Dep) defaults.
                // e.g., _aspect_proto_toolchain = attr.label(default = "//python:python_toolchain")
                if let CoercedAttr::Dep(providers_label) = default.as_ref() {
                    let configured_label =
                        providers_label.target().configure(target.cfg().dupe());
                    match ctx.compute(&AnalysisKey(configured_label.dupe())).await {
                        Ok(Ok(result)) => {
                            if let Ok(analysis) = result.require_compatible() {
                                if let Ok(providers) = analysis.providers() {
                                    attr_deps.insert(
                                        attr_name.clone(),
                                        (configured_label, providers.to_owned()),
                                    );
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            debug!(attr = %attr_name, label = %configured_label, error = ?e,
                                "aspect label default analysis failed; aspect may not use this attr");
                        }
                        Err(e) => {
                            debug!(attr = %attr_name, label = %configured_label, error = ?e,
                                "aspect label default DICE compute failed; aspect may not use this attr");
                        }
                    }
                } else {
                    // Default is not a label dep, skip
                }
            }
        }
        attr_deps
    };

    // Clone aspect_type for use inside the async block
    let aspect_type_for_registry = aspect_type.dupe();

    // Execute aspect in a Starlark module environment (similar to rule analysis)
    let fut = async move {
        let result = BuckStarlarkModule::with_profiling_async(|env| async move {
            let print = EventDispatcherPrintHandler(get_dispatcher());

            // 3. Build attribute resolution context for resolving rule attributes
            struct AspectAttrResolutionContext<'v> {
                module: &'v starlark::environment::Module,
                dep_analysis_results: std::collections::HashMap<ConfiguredTargetLabel, slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue>,
                execution_platform: slug_core::execution_types::execution::ExecutionPlatformResolution,
            }

            impl<'v> AttrResolutionContext<'v> for &'_ AspectAttrResolutionContext<'v> {
                fn starlark_module(&self) -> &'v starlark::environment::Module {
                    self.module
                }

                fn get_dep(
                    &mut self,
                    target: &slug_core::provider::label::ConfiguredProvidersLabel,
                ) -> slug_error::Result<starlark::values::FrozenValueTyped<'v, slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection>> {
                    // Look up the dep's providers from our collected map
                    // Shadow graph: if dep has aspect result, return aspect providers
                    // Otherwise, return target's regular providers
                    match self.dep_analysis_results.get(target.target()) {
                        Some(providers) => {
                            let providers_ref = providers.lookup_inner(target)?;
                            Ok(providers_ref.add_heap_ref(self.module.frozen_heap()))
                        }
                        None => Err(slug_error::slug_error!(
                            slug_error::ErrorTag::Tier0,
                            "Dependency {} not found in aspect resolution context",
                            target
                        )),
                    }
                }

                fn resolve_unkeyed_placeholder(
                    &mut self,
                    _name: &str,
                ) -> slug_error::Result<Option<slug_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg>> {
                    Ok(None)
                }

                fn resolve_query(
                    &mut self,
                    _query: &str,
                ) -> slug_error::Result<Arc<crate::attrs::resolve::ctx::AnalysisQueryResult>> {
                    Err(slug_error::internal_error!(
                        "Aspect attribute resolution does not support queries yet"
                    ))
                }

                fn execution_platform_resolution(&self) -> &slug_core::execution_types::execution::ExecutionPlatformResolution {
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
            // Use BaseDeferredKey::Aspect so that action lookups route to the
            // aspect's own AnalysisResult (not the target's).
            let aspect_deferred_key = Arc::new(
                super::aspect_deferred_key::AspectDeferredKey {
                    target: target.dupe(),
                    aspect_type: aspect_type_for_registry.dupe(),
                },
            );
            let registry = AnalysisRegistry::new_from_owner(
                BaseDeferredKey::Aspect(aspect_deferred_key),
                execution_platform,
            )?;

            // 6. Set up Starlark evaluator
            let eval_kind = StarlarkEvalKind::Analysis(target.dupe());
            let eval_provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;
            let mut reentrant_eval =
                eval_provider.make_reentrant_evaluator(&env, cancellations.into())?;

            // 7. Execute aspect implementation function (inlined from run_aspect_basic)
            let (aspect_context, provider_collection) = reentrant_eval.with_evaluator(|eval| {
                eval.set_print_handler(&print);
                eval.set_soft_error_handler(&SlugStarlarkSoftErrorHandler);

                // Get target providers for aspect execution (as a reference)
                let target_providers = target_providers_frozen.as_ref();

                // Get aspect implementation function
                let aspect_impl = aspect.as_ref().implementation();

                // Resolve aspect-specific attributes (ctx.attr)
                let aspect_attrs = aspect.as_ref().attrs();
                let has_attrs = !aspect_attrs.is_empty();

                let aspect_attr = if has_attrs {
                    // Build attribute struct with resolved values
                    let mut attr_pairs: Vec<(&str, starlark::values::Value)> = Vec::new();

                    for (attr_name, _starlark_attr) in aspect_attrs {
                        // Check if we resolved this attr as a configuration_field dep
                        if let Some((resolved_label, providers)) = aspect_attr_deps.get(attr_name) {
                            // Wrap as a dependency value (target[ProviderInfo] syntax)
                            let dep_providers_label = slug_core::provider::label::ConfiguredProvidersLabel::new(
                                resolved_label.dupe(),
                                slug_core::provider::label::ProvidersName::Default,
                            );
                            let providers_ref = providers.lookup_inner(&dep_providers_label);
                            match providers_ref {
                                Ok(frozen_collection) => {
                                    let collection_ref = frozen_collection.add_heap_ref(eval.module().frozen_heap());
                                    let dep = eval.heap().alloc(
                                        slug_build_api::interpreter::rule_defs::provider::dependency::Dependency::new(
                                            eval.heap(),
                                            dep_providers_label,
                                            collection_ref,
                                            None,
                                        ),
                                    );
                                    attr_pairs.push((attr_name, dep));
                                }
                                Err(_) => {
                                    attr_pairs.push((attr_name, starlark::values::Value::new_none()));
                                }
                            }
                        } else {
                            // No resolved value - use None
                            attr_pairs.push((attr_name, starlark::values::Value::new_none()));
                        }
                    }

                    let attrs_struct = eval.heap().alloc(AllocStruct(attr_pairs));
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

                // Route aspect impls through
                // `aspect_implementation_wrapper(impl, target, ctx)`
                // when @slug_builtins exposes one. Wrapper absent (no
                // `@slug_builtins` registered, or no aspect wrapper
                // exposed) → direct invocation.
                let wrapper_value = match &builtins_module {
                    Some(module) => module
                        .get_option("aspect_implementation_wrapper")
                        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Tier0))?
                        .map(|w| w.owned_value(eval.frozen_heap())),
                    None => None,
                };
                let result = match wrapper_value {
                    Some(wrapper) => eval.eval_function(
                        wrapper,
                        &[aspect_impl.to_value(), target_val, ctx.to_value()],
                        &[],
                    ),
                    None => eval.eval_function(
                        aspect_impl.to_value(),
                        &[target_val, ctx.to_value()],
                        &[],
                    ),
                }
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

            // 10. Get the frozen provider collection and build AnalysisResult
            let recorded_values = registry_finalizer(&frozen_env)?;
            let frozen_providers = recorded_values.provider_collection()?.to_owned();

            let analysis_result = AnalysisResult::new(
                recorded_values,
                None, // no profiling data for aspects
                std::collections::HashMap::new(), // no promise artifacts
                0, // num_declared_actions (not tracked for aspects)
                0, // num_declared_artifacts
                None, // no validations
            );

            Ok((token, (frozen_providers, analysis_result)))
        })
        .await;

        // Return the FrozenProviderCollectionValue
        // with_profiling_async automatically handles the profiling token
        result
    };

    unsafe { UnsafeSendFuture::new_encapsulates_starlark(fut) }.await
}

/// Initialize the `EVAL_ASPECT_DEFERRED` late binding.
///
/// This is called during program startup to register the aspect deferred key
/// resolution handler. When the build system encounters `BaseDeferredKey::Aspect`,
/// it dispatches through this handler to look up the aspect's DICE-cached
/// `AnalysisResult`.
pub fn init_eval_aspect_deferred() {
    use slug_build_api::deferred::calculation::EVAL_ASPECT_DEFERRED;

    EVAL_ASPECT_DEFERRED.init(|ctx, key| {
        Box::pin(async move {
            let aspect_key = key
                .into_any()
                .downcast::<super::aspect_deferred_key::AspectDeferredKey>()
                .ok()
                .internal_error("Expecting AspectDeferredKey")?;

            // Create the DICE key and compute the aspect result
            let dice_key = AspectKey::new(aspect_key.target.dupe(), aspect_key.aspect_type.dupe());

            let aspect_value = ctx
                .compute(&dice_key)
                .await?
                .buck_error_context("Failed to compute aspect for deferred key")?;

            aspect_value
                .analysis_result
                .internal_error("Aspect analysis result missing for deferred key")
        })
    });
}
