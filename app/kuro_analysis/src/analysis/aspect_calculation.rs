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
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::paths::module::StarlarkModulePath;
use kuro_interpreter::file_loader::LoadedModule;
use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
use kuro_node::aspect_type::StarlarkAspectType;

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

        // 2. Load and validate aspect module exists (Phase 8c - follows rule loading pattern)
        let _module = load_aspect_module(ctx, &self.aspect_type).await?;
        // TODO(Phase 8c): Extract aspect callable and check required_providers
        // let aspect = get_aspect_from_module(&module, &self.aspect_type.name)?;
        // if !aspect_applies_to_target(aspect, &target_result)? { ... }

        // 3-6. Execute aspect (Phase 8c - TODO)
        // TODO(Phase 8c): Implement recursive propagation, shadow graph, and execution
        // For now, just return the target's providers as a stub

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

// TODO(Phase 8c): Implement get_aspect_from_module() and aspect_applies_to_target()
// These require proper Starlark value handling which needs more infrastructure.
// For now, we just validate that the module loads successfully.

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
