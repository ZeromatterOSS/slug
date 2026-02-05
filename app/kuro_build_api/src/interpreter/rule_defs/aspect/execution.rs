/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Basic aspect execution for Phase 8b.
//!
//! This provides a minimal execution path for testing aspect implementations
//! without full DICE integration. Phase 8c will add shadow graph propagation
//! and caching.

use dupe::Dupe;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_execute::digest_config::DigestConfig;
use starlark::eval::Evaluator;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::ValueOfUnchecked;
use starlark::values::structs::AllocStruct;
use starlark::values::structs::StructRef;

use crate::analysis::registry::AnalysisRegistry;
use crate::interpreter::rule_defs::aspect::AspectContext;
use crate::interpreter::rule_defs::aspect::AspectRuleInfo;
use crate::interpreter::rule_defs::aspect::AspectTargetProviders;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValueRef;
use crate::interpreter::rule_defs::provider::collection::ProviderCollection;

/// Run an aspect on a single target (Phase 8b - basic execution, no propagation).
///
/// This is a minimal execution path for testing that:
/// - Creates AspectContext with all required members
/// - Invokes the aspect implementation function with (target, ctx)
/// - Validates the returned providers
///
/// What this does NOT do (deferred to Phase 8c):
/// - Shadow graph propagation via attr_aspects
/// - DICE caching
/// - Resolving ctx.rule.attr dependencies to aspect results
/// - required_providers filtering
///
/// # Arguments
///
/// * `heap` - Starlark heap for allocations
/// * `target_providers` - The target's provider collection
/// * `target_label` - The target's configured label
/// * `rule_kind` - The rule type name (e.g., "cc_library")
/// * `rule_attrs` - The rule's attributes as a struct
/// * `aspect_impl` - The aspect implementation function
/// * `aspect_has_attrs` - Whether the aspect defines custom attributes
/// * `eval` - Starlark evaluator
/// * `registry` - Analysis registry for action registration
/// * `digest_config` - Digest configuration
///
/// # Returns
///
/// The provider collection returned by the aspect implementation.
///
/// # Errors
///
/// Returns an error if:
/// - The aspect implementation function fails
/// - The aspect returns DefaultInfo (not allowed)
/// - The returned value is not a valid provider collection
#[allow(dead_code)] // Used in Phase 8c
pub fn run_aspect_basic<'v>(
    heap: Heap<'v>,
    target_providers: FrozenProviderCollectionValueRef<'v>,
    target_label: ConfiguredTargetLabel,
    rule_kind: String,
    rule_attrs: ValueOfUnchecked<'v, StructRef<'static>>,
    aspect_impl: FrozenValue,
    aspect_has_attrs: bool,
    eval: &mut Evaluator<'v, '_, '_>,
    registry: AnalysisRegistry<'v>,
    digest_config: DigestConfig,
) -> kuro_error::Result<ProviderCollection<'v>> {
    // 1. Resolve aspect-specific attributes
    let aspect_attr = if aspect_has_attrs {
        // For Phase 8b, create an empty struct for aspect attrs
        // Full attribute resolution will be added in Phase 8c
        let attrs_struct = heap.alloc(AllocStruct::EMPTY);
        Some(ValueOfUnchecked::new(attrs_struct))
    } else {
        None
    };

    // 2. Create AspectRuleInfo
    let rule_info = heap.alloc_typed(AspectRuleInfo::new(rule_kind, rule_attrs));

    // 3. Create AspectContext
    let ctx = AspectContext::prepare(
        heap,
        aspect_attr,
        target_label.dupe(),
        rule_info,
        registry,
        digest_config,
    );

    // 4. Wrap target providers for `target[SomeInfo]` syntax
    let target = heap.alloc(AspectTargetProviders::new(target_providers, target_label));

    // 5. Invoke implementation: impl(target, ctx)
    let result = eval
        .eval_function(aspect_impl.to_value(), &[target, ctx.to_value()], &[])
        .buck_error_context("Aspect implementation failed")?;

    // 6. Validate and return providers (aspects cannot return DefaultInfo)
    ProviderCollection::try_from_aspect_value(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_execution_compiles() {
        // This test verifies that the function signature compiles correctly
        // Full integration testing requires Starlark module setup and will be
        // done via manual verification in Phase 8c.
    }

    #[test]
    fn test_run_aspect_basic_exists() {
        // Verify the function exists and has the expected signature.
        // The actual signature is:
        // pub fn run_aspect_basic<'v>(
        //     heap: Heap<'v>,
        //     target_providers: FrozenProviderCollectionValueRef<'v>,
        //     target_label: ConfiguredTargetLabel,
        //     rule_kind: String,
        //     rule_attrs: ValueOfUnchecked<'v, StructRef<'static>>,
        //     aspect_impl: FrozenValue,
        //     aspect_has_attrs: bool,
        //     eval: &mut Evaluator<'v, '_, '_>,
        //     registry: AnalysisRegistry<'v>,
        //     digest_config: DigestConfig,
        // ) -> kuro_error::Result<ProviderCollection<'v>>
        //
        // This test simply ensures the function is exported and compiles.
        // Full integration tests will be added in Phase 8c after manual verification.
        let _ = run_aspect_basic as fn(_, _, _, _, _, _, _, _, _, _) -> _;
    }

    // TODO(Phase 8c): Add integration tests once manual verification is complete:
    // - test_aspect_executes_and_receives_context: Verify aspect impl is called
    // - test_aspect_ctx_rule_kind: Verify ctx.rule.kind returns correct value
    // - test_aspect_ctx_rule_attr: Verify ctx.rule.attr is accessible
    // - test_aspect_ctx_label: Verify ctx.label returns target label
    // - test_aspect_empty_providers: Verify empty provider list is valid
    // - test_aspect_rejects_default_info: Verify DefaultInfo is rejected
    // - test_target_provider_access: Verify target[SomeInfo] syntax works
}
