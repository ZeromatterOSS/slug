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
///
/// NOTE: For the initial implementation, this returns empty providers.
/// Full execution with Starlark evaluation context setup will be added next.
#[allow(dead_code)]
async fn execute_aspect(
    _ctx: &mut DiceComputations<'_>,
    _target: &ConfiguredTargetLabel,
    _aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
    _cancellations: &CancellationContext,
) -> kuro_error::Result<kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue> {
    // TODO(Phase 8d): Implement full aspect execution
    // This requires:
    // 1. Creating a Starlark Heap and Evaluator
    // 2. Building AnalysisRegistry for action registration
    // 3. Extracting rule kind and attributes from target
    // 4. Calling run_aspect_basic() with all parameters
    // 5. Freezing the result providers
    //
    // For now, return the target's providers as a placeholder.
    // This allows aspects to pass through without execution.
    let providers = target_result.providers()?;
    Ok(providers.to_owned())
}
