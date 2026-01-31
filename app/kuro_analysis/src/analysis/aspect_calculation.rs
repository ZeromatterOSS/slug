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

use std::sync::Arc;
use std::collections::HashMap;

use async_trait::async_trait;
use dice::{CancellationContext, DiceComputations, Key};
use dupe::Dupe;
use futures::FutureExt;

use kuro_build_api::analysis::AnalysisResult;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;

use super::aspect_key::{AspectKey, AspectValue};
use super::calculation::AnalysisKey;

#[async_trait]
impl Key for AspectKey {
    type Value = kuro_error::Result<AspectValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        // 1. Get target's analysis result (ensures target is analyzed first)
        let target_result = ctx
            .compute(&AnalysisKey(self.target.dupe()))
            .await?
            .buck_error_context("Failed to get target analysis result for aspect")?
            .require_compatible()?;

        // 2. Load aspect callable from module registry
        // TODO(Phase 8c): Implement load_aspect_by_name()
        // let aspect = load_aspect_by_name(ctx, &self.aspect_name).await?;

        // 3. Check required_providers filter
        // TODO(Phase 8c): Implement aspect_applies_to_target()
        // if !aspect_applies_to_target(&aspect, &target_result)? {
        //     return Ok(AspectValue::empty());
        // }

        // 4. Recursively compute aspects on dependencies (depth-first)
        // TODO(Phase 8c): Implement compute_dep_aspects()
        // let dep_aspect_results = compute_dep_aspects(ctx, &self.target, &aspect).await?;

        // 5. Build shadow graph (replace deps in ctx.rule.attr with aspect results)
        // TODO(Phase 8c): Implement build_shadow_attrs()
        // let shadow_attrs = build_shadow_attrs(&target_result, &dep_aspect_results)?;

        // 6. Execute aspect using run_aspect_basic()
        // TODO(Phase 8c): Implement execute_aspect_impl()
        // let providers = execute_aspect_impl(
        //     ctx,
        //     &self.target,
        //     &aspect,
        //     &target_result,
        //     shadow_attrs,
        // ).await?;

        // For now, return a stub result (Phase 8c stub)
        // TODO(Phase 8c): Return actual aspect computation result
        let providers_ref = target_result.providers()?;

        Ok(AspectValue {
            providers: providers_ref.to_owned(),
        })
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // Aspect values are not comparable (similar to AnalysisKey)
        false
    }
}

/// Load an aspect callable by name from the module registry.
///
/// This finds the module where the aspect was defined and retrieves the
/// frozen aspect callable.
#[allow(dead_code)]
async fn load_aspect_by_name(
    _ctx: &mut DiceComputations<'_>,
    _aspect_name: &str,
) -> kuro_error::Result<()> {
    // TODO(Phase 8c): Implement aspect loading from module registry
    // This needs to:
    // 1. Find which module defined this aspect (requires module registry lookup)
    // 2. Load that module
    // 3. Get the frozen aspect callable by name
    todo!("Phase 8c: Implement aspect loading")
}

/// Check if an aspect applies to a target based on required_providers.
///
/// Returns true if:
/// - The aspect has no required_providers (applies to all targets), OR
/// - The target provides at least one of the required provider sets
///
/// The required_providers structure is: [[A], [B, C]] means A OR (B AND C)
#[allow(dead_code)]
fn aspect_applies_to_target(
    _aspect: &(), // TODO: Replace with FrozenStarlarkAspectCallable
    _target_result: &AnalysisResult,
) -> kuro_error::Result<bool> {
    // TODO(Phase 8c): Implement required_providers filtering
    // let required_providers = aspect.required_providers();
    //
    // // Empty required_providers = applies to all targets
    // if required_providers.is_empty() {
    //     return Ok(true);
    // }
    //
    // // Check any-of logic: [[A], [B, C]] means A OR (B AND C)
    // for provider_set in required_providers {
    //     let has_all = provider_set.iter().all(|provider_id| {
    //         target_result.providers().contains_provider(provider_id)
    //     });
    //     if has_all {
    //         return Ok(true);
    //     }
    // }
    //
    // Ok(false)
    Ok(true) // Stub: apply to all targets for now
}

/// Recursively compute aspects on dependencies.
///
/// This follows the aspect's attr_aspects to determine which dependency
/// attributes to propagate through, then computes the aspect on each
/// dependency in parallel via DICE.
#[allow(dead_code)]
async fn compute_dep_aspects(
    _ctx: &mut DiceComputations<'_>,
    _target: &ConfiguredTargetLabel,
    _aspect: &(), // TODO: Replace with FrozenStarlarkAspectCallable
) -> kuro_error::Result<HashMap<ConfiguredTargetLabel, AspectValue>> {
    // TODO(Phase 8c): Implement recursive aspect propagation
    // let node = ctx.get_configured_target_node(target).await?;
    //
    // let attr_aspects = aspect.attr_aspects(); // e.g., ["deps"] or ["*"]
    // let propagate_all = attr_aspects.iter().any(|a| a == "*");
    //
    // let mut futures = Vec::new();
    //
    // // For each attribute that matches attr_aspects
    // for attr in node.attrs() {
    //     if !propagate_all && !attr_aspects.iter().any(|a| a == attr.name()) {
    //         continue;
    //     }
    //
    //     // Extract dep labels from attribute value
    //     for dep in extract_dep_labels(&attr.value())? {
    //         let key = AspectKey::new(dep.dupe(), aspect.name().to_owned());
    //         futures.push(ctx.compute(&key));
    //     }
    // }
    //
    // // Execute all in parallel via DICE
    // let results = futures::future::try_join_all(futures).await?;
    //
    // // Collect into map
    // Ok(results
    //     .into_iter()
    //     .filter_map(|r| r.ok())
    //     .map(|v| (v.target.dupe(), v))
    //     .collect())
    Ok(HashMap::new()) // Stub: no dependencies for now
}
