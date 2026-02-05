/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Native rule analysis implementations for Bazel compatibility.
//!
//! This module provides analysis implementations for native rules like
//! constraint_setting and constraint_value which are required for BCR
//! packages like @platforms.

use std::collections::HashMap;

use kuro_build_api::actions::registry::RecordedActions;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::registry::FrozenAnalysisValueStorage;
use kuro_build_api::analysis::registry::RecordedAnalysisValues;
use kuro_build_api::dynamic::storage::DYNAMIC_LAMBDA_PARAMS_STORAGES;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::FrozenDefaultInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::internal_error;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::rule_type::NativeRuleKind;
use dupe::Dupe;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValueTyped;
use starlark::values::OwnedFrozenValue;
use starlark::values::any_complex::StarlarkAnyComplex;
use starlark_map::small_map::SmallMap;

/// Analyze a native rule target and return the analysis result.
pub fn analyze_native_rule(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    kind: &NativeRuleKind,
    _dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    match kind {
        NativeRuleKind::ConstraintSetting => {
            analyze_constraint_setting(target, configured_node)
        }
        NativeRuleKind::ConstraintValue => {
            analyze_constraint_value(target, configured_node)
        }
        NativeRuleKind::Filegroup => {
            Err(internal_error!(
                "Native filegroup analysis not implemented. Use Starlark filegroup instead."
            ))
        }
    }
}

/// Analyze a constraint_setting target.
/// For now, returns just DefaultInfo. Full ConstraintSettingInfo support
/// can be added later when the infrastructure is in place.
fn analyze_constraint_setting(
    target: &ConfiguredTargetLabel,
    _configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<AnalysisResult> {
    create_minimal_analysis_result(target)
}

/// Analyze a constraint_value target.
/// For now, returns just DefaultInfo. Full ConstraintValueInfo support
/// can be added later when the infrastructure is in place.
fn analyze_constraint_value(
    target: &ConfiguredTargetLabel,
    _configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<AnalysisResult> {
    create_minimal_analysis_result(target)
}

/// Create a minimal analysis result with just DefaultInfo.
/// This is used for native rules that don't need complex provider creation.
fn create_minimal_analysis_result(
    target: &ConfiguredTargetLabel,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Build provider collection with just DefaultInfo
    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    let provider_collection = FrozenValueTyped::<FrozenProviderCollection>::new_err(
        heap.alloc(FrozenProviderCollection::new(providers)),
    )?;

    // Create analysis storage
    let self_key = DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target.dupe()));

    let analysis_storage = heap.alloc_simple(StarlarkAnyComplex {
        value: FrozenAnalysisValueStorage::new_native(
            self_key.dupe(),
            DYNAMIC_LAMBDA_PARAMS_STORAGES
                .get()
                .unwrap()
                .new_frozen_dynamic_lambda_params_storage(),
            Some(provider_collection),
        ),
    });

    let heap_ref = heap.into_ref();
    let analysis_storage = unsafe {
        OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()?
    };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None, // No profiling data
        HashMap::new(), // No promise artifacts
        0, // No actions
        0, // No artifacts
        None, // No validations
    ))
}
