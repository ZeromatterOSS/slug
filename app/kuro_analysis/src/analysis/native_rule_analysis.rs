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

use dupe::Dupe;
use kuro_build_api::actions::registry::RecordedActions;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::registry::FrozenAnalysisValueStorage;
use kuro_build_api::analysis::registry::RecordedAnalysisValues;
use kuro_build_api::dynamic::storage::DYNAMIC_LAMBDA_PARAMS_STORAGES;
use kuro_build_api::interpreter::rule_defs::cc_common::CcInfoInstanceStub;
use kuro_build_api::interpreter::rule_defs::cc_common::CcInfoProvider;
use kuro_build_api::interpreter::rule_defs::platform_common::ConstraintValueInfoInstance;
use kuro_build_api::interpreter::rule_defs::platform_common::ConstraintValueInfoProvider;
use kuro_build_api::interpreter::rule_defs::provider::FrozenBuiltinProviderLike;
use kuro_build_api::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::FrozenDefaultInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::internal_error;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::rule_type::NativeRuleKind;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::OwnedFrozenValue;
use starlark::values::any_complex::StarlarkAnyComplex;
use starlark_map::small_map::SmallMap;

/// Analyze a native rule target and return the analysis result.
pub fn analyze_native_rule(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    kind: &NativeRuleKind,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    match kind {
        NativeRuleKind::ConstraintSetting => analyze_constraint_setting(target, configured_node),
        NativeRuleKind::ConstraintValue => {
            analyze_constraint_value(target, configured_node, dep_analysis)
        }
        NativeRuleKind::Filegroup => analyze_filegroup(target, dep_analysis),
        NativeRuleKind::Alias => analyze_alias(target, dep_analysis),
        NativeRuleKind::LabelFlag => analyze_label_flag(target, dep_analysis),
        NativeRuleKind::ConfigSetting => analyze_config_setting(target, dep_analysis),
        NativeRuleKind::ToolchainType => create_minimal_analysis_result(target),
        NativeRuleKind::PackageGroup => create_minimal_analysis_result(target),
        NativeRuleKind::Genrule => create_minimal_analysis_result(target),
        NativeRuleKind::Platform => create_minimal_analysis_result(target),
        NativeRuleKind::CcLibrary => create_cc_analysis_result(target),
        NativeRuleKind::CcBinary => create_cc_analysis_result(target),
        NativeRuleKind::CcTest => create_cc_analysis_result(target),
        NativeRuleKind::TestSuite => create_minimal_analysis_result(target),
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
/// Returns DefaultInfo, ConstraintValueInfo, and ConfigurationInfo so that
/// `target[platform_common.ConstraintValueInfo]` works and config_setting
/// can extract constraint data from deps.
fn analyze_constraint_value(
    target: &ConfiguredTargetLabel,
    _configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Extract constraint_setting label from dep_analysis.
    // constraint_value has exactly one dep: its constraint_setting.
    let constraint_setting_label = if !dep_analysis.is_empty() {
        dep_analysis[0].0.unconfigured().to_string()
    } else {
        String::new()
    };

    // Create ConstraintValueInfo with the target's label
    let constraint_value_info = heap.alloc(ConstraintValueInfoInstance {
        constraint_setting_label,
        label: target.unconfigured().to_string(),
    });

    // Create ConfigurationInfo with one constraint pair (cs→cv)
    // so that config_setting can merge constraints from deps.
    let mut providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            ConstraintValueInfoProvider::provider_id().dupe(),
            constraint_value_info,
        ),
    ]);

    if !dep_analysis.is_empty() {
        let cs_label = dep_analysis[0].0.unconfigured().dupe();
        let cv_label = ProvidersLabel::default_for(target.unconfigured().dupe());
        let config_info =
            FrozenConfigurationInfo::for_native_config_setting(&[(cs_label, cv_label)], &heap);
        providers.insert(
            FrozenConfigurationInfo::builtin_provider_id().dupe(),
            config_info,
        );
    }

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
    let analysis_storage =
        unsafe { OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()? };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        0,
        0,
        None,
    ))
}

/// Analyze a label_flag target.
/// A label_flag forwards all providers from its `build_setting_default` target.
/// This is similar to alias - it acts as a configurable indirection.
fn analyze_label_flag(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    // label_flag has exactly one dep (build_setting_default), forward its providers
    if dep_analysis.len() == 1 {
        let (_default_label, default_result) = dep_analysis.into_iter().next().unwrap();
        Ok(default_result)
    } else if dep_analysis.is_empty() {
        // No deps resolved - return minimal DefaultInfo
        create_minimal_analysis_result(target)
    } else {
        Err(internal_error!(
            "label_flag target {} has {} dependencies. Expected exactly one 'build_setting_default' dependency.",
            target,
            dep_analysis.len()
        ))
    }
}

/// Analyze a config_setting target.
/// Creates a ConfigurationInfo provider by merging constraint data from all
/// constraint_value deps. This allows `select()` to match against the config_setting.
fn analyze_config_setting(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Collect constraint pairs from each constraint_value dep's ConfigurationInfo.
    let mut all_constraint_pairs = Vec::new();
    for (_dep_label, dep_result) in &dep_analysis {
        if let Ok(providers) = dep_result.providers() {
            if let Some(config_info) = providers
                .value()
                .builtin_provider::<FrozenConfigurationInfo>()
            {
                let config_data = config_info.to_config_setting_data();
                for (ck, cv) in config_data.constraints {
                    all_constraint_pairs.push((ck.key.dupe(), cv.0.dupe()));
                }
            }
        }
    }

    // If no real constraint pairs found (flag_values/values only), add a sentinel
    // constraint pair that will never match any real platform configuration.
    // This ensures the config_setting is valid as a select() key but won't match.
    if all_constraint_pairs.is_empty() {
        let sentinel_setting = target.unconfigured().dupe();
        let sentinel_value = ProvidersLabel::default_for(target.unconfigured().dupe());
        all_constraint_pairs.push((sentinel_setting, sentinel_value));
    }

    // Create merged ConfigurationInfo with all constraint pairs
    let config_info =
        FrozenConfigurationInfo::for_native_config_setting(&all_constraint_pairs, &heap);

    let mut providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);
    providers.insert(
        FrozenConfigurationInfo::builtin_provider_id().dupe(),
        config_info,
    );

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
    let analysis_storage =
        unsafe { OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()? };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        0,
        0,
        None,
    ))
}

/// Analyze an alias target.
/// An alias forwards all providers from its `actual` target.
fn analyze_alias(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    // The alias should have exactly one dependency - the `actual` target.
    // Find it in the dep_analysis and return its providers.
    if dep_analysis.len() == 1 {
        // Clone the actual target's analysis result
        let (_actual_label, actual_result) = dep_analysis.into_iter().next().unwrap();
        Ok(actual_result)
    } else if dep_analysis.is_empty() {
        Err(internal_error!(
            "Alias target {} has no dependencies. It should have exactly one 'actual' dependency.",
            target
        ))
    } else {
        Err(internal_error!(
            "Alias target {} has {} dependencies. It should have exactly one 'actual' dependency.",
            target,
            dep_analysis.len()
        ))
    }
}

/// Analyze a filegroup target.
/// Filegroups collect files from their srcs and data deps.
/// For filegroups with no srcs (like empty sentinel targets), returns empty DefaultInfo.
/// For filegroups with deps, merges DefaultInfo.default_outputs from all deps.
fn analyze_filegroup(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    if dep_analysis.is_empty() {
        // Empty filegroup - return minimal DefaultInfo
        return create_minimal_analysis_result(target);
    }

    // Fast path: single dep, just forward its result directly
    if dep_analysis.len() == 1 {
        let (_label, result) = dep_analysis.into_iter().next().unwrap();
        return Ok(result);
    }

    let heap = FrozenHeap::new();

    // Collect default_outputs from all deps into a single merged list.
    // We alloc each StarlarkArtifact on our heap so they live long enough.
    let mut all_outputs: Vec<FrozenValue> = Vec::new();
    for (_dep_label, dep_result) in &dep_analysis {
        if let Ok(providers_ref) = dep_result.providers() {
            let collection: &FrozenProviderCollection = providers_ref.value().as_ref();
            if let Some(default_info) = collection.builtin_provider::<FrozenDefaultInfo>() {
                for artifact in default_info.default_outputs() {
                    all_outputs.push(heap.alloc(artifact));
                }
            }
        }
    }

    let default_info = FrozenDefaultInfo::with_outputs(&heap, all_outputs);

    // Build provider collection with merged DefaultInfo
    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    let provider_collection = FrozenValueTyped::<FrozenProviderCollection>::new_err(
        heap.alloc(FrozenProviderCollection::new(providers)),
    )?;

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
    let analysis_storage =
        unsafe { OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()? };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        0,
        0,
        None,
    ))
}

/// Create an analysis result with DefaultInfo + CcInfo for native cc rules.
fn create_cc_analysis_result(target: &ConfiguredTargetLabel) -> kuro_error::Result<AnalysisResult> {
    // Register the repo root as an include directory for native cc_library stubs.
    // This ensures that when other targets compile against this stub dep,
    // headers from this repo can be found (e.g., "absl/base/macros.h" from abseil-cpp).
    let cell_name = target.pkg().cell_name().as_str();
    if !kuro_core::cells::is_root_cell_name(cell_name) {
        let include_dir = format!("external/{}", cell_name);
        kuro_build_api::interpreter::rule_defs::cc_common::register_external_include_dir(
            &include_dir,
        );
    }

    let heap = FrozenHeap::new();

    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    let cc_info = heap.alloc(CcInfoInstanceStub);

    let mut providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (CcInfoProvider::provider_id().dupe(), cc_info),
    ]);

    let provider_collection = FrozenValueTyped::<FrozenProviderCollection>::new_err(
        heap.alloc(FrozenProviderCollection::new(providers)),
    )?;

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
    let analysis_storage =
        unsafe { OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()? };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        0,
        0,
        None,
    ))
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
    let analysis_storage =
        unsafe { OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()? };

    let recorded_values = RecordedAnalysisValues::new_native(
        self_key,
        Some(analysis_storage),
        RecordedActions::new(0),
    );

    Ok(AnalysisResult::new(
        recorded_values,
        None,           // No profiling data
        HashMap::new(), // No promise artifacts
        0,              // No actions
        0,              // No artifacts
        None,           // No validations
    ))
}
