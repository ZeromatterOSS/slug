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
use std::sync::Arc;

use dupe::Dupe;
use kuro_artifact::actions::key::ActionIndex;
use kuro_artifact::actions::key::ActionKey;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::build_artifact::BuildArtifact;
use kuro_artifact::artifact::source_artifact::SourceArtifact;
use kuro_build_api::actions::RegisteredAction;
use kuro_build_api::actions::registry::RecordedActions;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::registry::FrozenAnalysisValueStorage;
use kuro_build_api::analysis::registry::RecordedAnalysisValues;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_api::dynamic::storage::DYNAMIC_LAMBDA_PARAMS_STORAGES;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use kuro_build_api::interpreter::rule_defs::cc_common::CcInfoInstanceStub;
use kuro_build_api::interpreter::rule_defs::cc_common::CcInfoProvider;
use kuro_build_api::interpreter::rule_defs::platform_common::ConstraintValueInfoInstance;
use kuro_build_api::interpreter::rule_defs::platform_common::ConstraintValueInfoProvider;
use kuro_build_api::interpreter::rule_defs::provider::FrozenBuiltinProviderLike;
use kuro_build_api::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::FrozenDefaultInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::FrozenExternalRunnerTestInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::create_frozen_sh_test_info;
use kuro_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::execution_types::executor_config::CommandExecutorConfig;
use kuro_core::fs::buck_out_path::BuckOutPathKind;
use kuro_core::fs::buck_out_path::BuildArtifactPath;
use kuro_core::package::source_path::SourcePath;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_error::internal_error;
use kuro_execute::execute::request::OutputType;
use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use kuro_node::attrs::attr_type::list::ListLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::configured_attr::ConfiguredAttr;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::rule_type::NativeRuleKind;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::OwnedFrozenValue;
use starlark::values::any_complex::StarlarkAnyComplex;
use starlark_map::small_map::SmallMap;

use crate::analysis::genrule_action::GenruleAction;

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
        NativeRuleKind::Filegroup => analyze_filegroup(target, configured_node, dep_analysis),
        NativeRuleKind::Alias => analyze_alias(target, dep_analysis),
        NativeRuleKind::LabelFlag => analyze_label_flag(target, dep_analysis), // dep_analysis is empty (build_setting_default is a string, not a dep)
        NativeRuleKind::ConfigSetting => {
            analyze_config_setting(target, configured_node, dep_analysis)
        }
        NativeRuleKind::ToolchainType => create_minimal_analysis_result(target),
        NativeRuleKind::PackageGroup => create_minimal_analysis_result(target),
        NativeRuleKind::Genrule => analyze_genrule(target, configured_node, dep_analysis),
        NativeRuleKind::Platform => analyze_platform(target, dep_analysis),
        NativeRuleKind::CcLibrary => create_cc_analysis_result(target),
        NativeRuleKind::CcBinary => create_cc_analysis_result(target),
        NativeRuleKind::CcTest => create_cc_analysis_result(target),
        NativeRuleKind::TestSuite => analyze_test_suite(target, dep_analysis),
        NativeRuleKind::Toolchain => create_minimal_analysis_result(target),
        NativeRuleKind::ShLibrary => analyze_sh_library(target, configured_node, dep_analysis),
        NativeRuleKind::ShBinary => analyze_sh_binary(target, configured_node),
        NativeRuleKind::ShTest => analyze_sh_test(target, configured_node),
        NativeRuleKind::CcLibcTopAlias => create_minimal_analysis_result(target),
        NativeRuleKind::AnalysisTest => analyze_analysis_test(target),
        NativeRuleKind::Genquery => analyze_genquery(target),
    }
}

/// Analyze a genrule target.
///
/// Genrules run a shell command to produce output files. This function:
/// 1. Reads the `cmd`, `outs`, and `srcs` attributes from the target
/// 2. Creates `BuildArtifact`s for each output file
/// 3. Collects input `ArtifactGroup`s from source files and dep analysis
/// 4. Registers a `GenruleAction` that executes the shell command
/// 5. Returns `DefaultInfo` with the output artifacts
fn analyze_genrule(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let target_node = configured_node.to_owned();
    let target_ref = target_node.target_node().as_ref();
    let pkg = target.pkg();

    // Read cmd and cmd_bash from CONFIGURED node (resolves select() expressions).
    // On Unix, prefer cmd_bash if non-empty (Bazel-compatible).
    let read_configured_string = |attr_name: &str| -> String {
        if let Some(attr) = configured_node.get(attr_name, AttrInspectOptions::All) {
            match attr.value {
                ConfiguredAttr::String(s) => s.0.as_str().to_owned(),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    };
    let cmd_raw = read_configured_string("cmd");
    let cmd_bash_raw = read_configured_string("cmd_bash");
    let cmd = if !cmd_bash_raw.is_empty() && cfg!(unix) {
        cmd_bash_raw
    } else {
        cmd_raw
    };

    // Read outs attribute → list of output file name strings
    let out_names: Vec<String> =
        if let Some(outs_attr) = target_ref.attr_or_none("outs", AttrInspectOptions::All) {
            match outs_attr.value {
                CoercedAttr::List(ListLiteral(items)) => items
                    .iter()
                    .filter_map(|item| {
                        if let CoercedAttr::String(s) = item {
                            Some(s.0.as_str().to_owned())
                        } else {
                            None
                        }
                    })
                    .collect(),
                CoercedAttr::String(s) => vec![s.0.as_str().to_owned()],
                _ => vec![],
            }
        } else {
            vec![]
        };

    // Create a single ActionKey for this genrule (action index 0)
    let self_key = DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target.dupe()));
    let action_key = ActionKey::new(self_key.dupe(), ActionIndex::new(0));

    // Create BuildArtifact for each output file
    let mut output_artifacts: Vec<BuildArtifact> = Vec::with_capacity(out_names.len());
    let heap = FrozenHeap::new();
    let mut output_starlark: Vec<FrozenValue> = Vec::with_capacity(out_names.len());

    for out_name in &out_names {
        let path = BuildArtifactPath::new(
            BaseDeferredKey::TargetLabel(target.dupe()),
            ForwardRelativePathBuf::new(out_name.clone())
                .with_buck_error_context(|| format!("Invalid genrule output path: {}", out_name))?,
            BuckOutPathKind::Configuration,
        );
        let ba = BuildArtifact::new(path, action_key.dupe(), OutputType::File)?;
        let starlark_ba = heap.alloc_simple(StarlarkArtifact::new(Artifact::from(ba.dupe())));
        output_starlark.push(starlark_ba);
        output_artifacts.push(ba);
    }

    // Collect input ArtifactGroups:
    // 1) Source files from the `srcs` attr
    // 2) DefaultInfo.default_outputs from dep_analysis entries
    let mut inputs: Vec<ArtifactGroup> = Vec::new();

    // Use configured node to resolve select() expressions in srcs/tools.
    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        collect_artifact_groups_from_configured_attr(&srcs_attr.value, &pkg, &mut inputs);
    }

    // Also collect from tools attr (these are executables needed by the command)
    if let Some(tools_attr) = configured_node.get("tools", AttrInspectOptions::All) {
        collect_artifact_groups_from_configured_attr(&tools_attr.value, &pkg, &mut inputs);
    }

    // Add DefaultInfo outputs from dep_analysis (label deps in srcs/tools)
    for (_dep_label, dep_result) in &dep_analysis {
        if let Ok(providers_ref) = dep_result.providers() {
            let collection: &FrozenProviderCollection = providers_ref.value().as_ref();
            if let Some(default_info) = collection.builtin_provider::<FrozenDefaultInfo>() {
                for starlark_artifact in default_info.default_outputs() {
                    inputs.push(ArtifactGroup::Artifact(starlark_artifact.artifact()));
                }
            }
        }
    }

    // Build location mappings for $(location label) / $(execpath label) expansion.
    // These map each referenced label to its dep's output artifacts.
    let mut location_mappings = build_location_mappings(&cmd, &dep_analysis);

    // Also build location mappings for source files referenced in $(location ...).
    // Source files (e.g. "defs.bzl" in srcs) can be referenced as $(location :defs.bzl).
    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        let source_mappings = build_source_file_location_mappings(&srcs_attr.value, &pkg);
        // Add source mappings for labels not already resolved via dep_analysis.
        // If a key exists in location_mappings but with empty artifacts, replace it.
        for (label, artifacts) in source_mappings {
            if artifacts.is_empty() {
                continue;
            }
            let existing = location_mappings.iter_mut().find(|(k, _)| k == &label);
            if let Some((_, existing_arts)) = existing {
                // Replace empty dep_analysis entry with source file artifacts
                if existing_arts.is_empty() {
                    *existing_arts = artifacts;
                }
            } else {
                location_mappings.push((label, artifacts));
            }
        }
    }

    // Create the genrule action
    let genrule_action = GenruleAction::new(cmd, inputs, output_artifacts, location_mappings);

    // Register the action
    let registered_action = Arc::new(RegisteredAction::new(
        action_key.dupe(),
        Box::new(genrule_action),
        CommandExecutorConfig::testing_local(),
    ));
    let mut recorded_actions = RecordedActions::new(1);
    recorded_actions.insert(action_key, registered_action);

    // Build DefaultInfo with output artifacts
    let default_info = FrozenDefaultInfo::with_outputs(&heap, output_starlark);

    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    let provider_collection = FrozenValueTyped::<FrozenProviderCollection>::new_err(
        heap.alloc(FrozenProviderCollection::new(providers)),
    )?;

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

    let recorded_values =
        RecordedAnalysisValues::new_native(self_key, Some(analysis_storage), recorded_actions);

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        1, // 1 action registered
        out_names.len() as u64,
        None,
    ))
}

/// Extract all unique labels from $(location label), $(locations label),
/// $(execpath label), and $(execpaths label) patterns in a genrule cmd.
fn extract_location_labels(cmd: &str) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    let mut remaining = cmd;
    while let Some(start) = remaining.find("$(") {
        let after_paren = &remaining[start + 2..];
        let keyword_len =
            if after_paren.starts_with("locations ") || after_paren.starts_with("execpaths ") {
                10usize
            } else if after_paren.starts_with("location ") || after_paren.starts_with("execpath ") {
                9usize
            } else {
                remaining = &remaining[start + 2..];
                continue;
            };
        let label_rest = &after_paren[keyword_len..];
        if let Some(end) = label_rest.find(')') {
            let label = label_rest[..end].trim().to_owned();
            if !labels.contains(&label) {
                labels.push(label);
            }
            remaining = &remaining[start + 2 + keyword_len + end + 1..];
        } else {
            remaining = &remaining[start + 2..];
        }
    }
    labels
}

/// Build location mappings for $(location label) expansion in genrule.
///
/// For each label referenced by a `$(location ...)` pattern in the cmd,
/// finds the matching dep in dep_analysis and collects its output artifacts.
/// Returns `Vec<(label_key, Vec<ArtifactGroup>)>` for use in GenruleAction.
fn build_location_mappings(
    cmd: &str,
    dep_analysis: &[(&ConfiguredTargetLabel, AnalysisResult)],
) -> Vec<(String, Vec<ArtifactGroup>)> {
    let labels = extract_location_labels(cmd);
    if labels.is_empty() {
        return Vec::new();
    }
    let mut mappings: Vec<(String, Vec<ArtifactGroup>)> = Vec::new();
    for label in &labels {
        let label_name = label.rsplit(':').next().unwrap_or(label.as_str());
        let mut found_artifacts: Vec<ArtifactGroup> = Vec::new();
        for (dep_label, dep_result) in dep_analysis {
            let dep_str = dep_label.unconfigured().to_string();
            let dep_name = dep_str.rsplit(':').next().unwrap_or(dep_str.as_str());
            // Match on exact string (e.g. "//pkg:target") or name suffix (e.g. ":target")
            if dep_str == *label || dep_name == label_name {
                if let Ok(providers_ref) = dep_result.providers() {
                    let collection: &FrozenProviderCollection = providers_ref.value().as_ref();
                    if let Some(default_info) = collection.builtin_provider::<FrozenDefaultInfo>() {
                        for starlark_artifact in default_info.default_outputs() {
                            found_artifacts
                                .push(ArtifactGroup::Artifact(starlark_artifact.artifact()));
                        }
                    }
                }
                break;
            }
        }
        mappings.push((label.clone(), found_artifacts));
    }
    mappings
}

/// Build location mappings for source files in genrule srcs.
///
/// For `$(location :file.txt)` patterns, the label `:file.txt` refers to a source file
/// rather than a dep target. This function extracts such source files from the `srcs`
/// ConfiguredAttr and maps them by their file name (e.g. `":defs.bzl"` → SourceArtifact).
fn build_source_file_location_mappings(
    attr: &ConfiguredAttr,
    pkg: &kuro_core::package::PackageLabel,
) -> Vec<(String, Vec<ArtifactGroup>)> {
    let mut result: Vec<(String, Vec<ArtifactGroup>)> = Vec::new();
    collect_source_file_location_mappings_recursive(attr, pkg, &mut result);
    result
}

fn collect_source_file_location_mappings_recursive(
    attr: &ConfiguredAttr,
    pkg: &kuro_core::package::PackageLabel,
    out: &mut Vec<(String, Vec<ArtifactGroup>)>,
) {
    match attr {
        ConfiguredAttr::List(ListLiteral(items)) => {
            for item in items.iter() {
                collect_source_file_location_mappings_recursive(item, pkg, out);
            }
        }
        ConfiguredAttr::OneOf(inner, _) => {
            collect_source_file_location_mappings_recursive(inner, pkg, out);
        }
        ConfiguredAttr::SourceFile(coerced_path) => {
            // The "label" for a source file is ":filename" (colon + last path component)
            let file_path = coerced_path.path();
            let filename = file_path
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or(file_path.as_str());
            let label_key = format!(":{}", filename);
            let source_artifact =
                SourceArtifact::new(SourcePath::new(pkg.dupe(), file_path.clone()));
            out.push((
                label_key,
                vec![ArtifactGroup::Artifact(Artifact::from(source_artifact))],
            ));
        }
        _ => {}
    }
}

/// Collect ArtifactGroups from a CoercedAttr that may contain source files.
/// Source file entries become `ArtifactGroup::Artifact(SourceArtifact)`.
/// Label deps are NOT collected here (they come from dep_analysis instead).
fn collect_artifact_groups_from_attr(
    attr: &CoercedAttr,
    pkg: &kuro_core::package::PackageLabel,
    out: &mut Vec<ArtifactGroup>,
) {
    match attr {
        CoercedAttr::List(ListLiteral(items)) => {
            for item in items.iter() {
                collect_artifact_groups_from_attr(item, pkg, out);
            }
        }
        CoercedAttr::OneOf(inner, _) => {
            collect_artifact_groups_from_attr(inner, pkg, out);
        }
        CoercedAttr::SourceFile(coerced_path) => {
            for path in coerced_path.inputs() {
                let source_artifact = SourceArtifact::new(SourcePath::new(pkg.dupe(), path.dupe()));
                out.push(ArtifactGroup::Artifact(Artifact::from(source_artifact)));
            }
        }
        // Label deps are resolved via dep_analysis, not here
        _ => {}
    }
}

/// Collect ArtifactGroups from a CONFIGURED attr (with select() already resolved).
fn collect_artifact_groups_from_configured_attr(
    attr: &ConfiguredAttr,
    pkg: &kuro_core::package::PackageLabel,
    out: &mut Vec<ArtifactGroup>,
) {
    match attr {
        ConfiguredAttr::List(ListLiteral(items)) => {
            for item in items.iter() {
                collect_artifact_groups_from_configured_attr(item, pkg, out);
            }
        }
        ConfiguredAttr::OneOf(inner, _) => {
            collect_artifact_groups_from_configured_attr(inner, pkg, out);
        }
        ConfiguredAttr::SourceFile(coerced_path) => {
            for path in coerced_path.inputs() {
                let source_artifact = SourceArtifact::new(SourcePath::new(pkg.dupe(), path.dupe()));
                out.push(ArtifactGroup::Artifact(Artifact::from(source_artifact)));
            }
        }
        // Label deps are resolved via dep_analysis, not here
        _ => {}
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
/// A label_flag is a Bazel build setting that holds a label value.
/// Its build_setting_default is stored as a plain string (not a dep), so there are no
/// deps to forward.
///
/// We return DefaultInfo + minimal CcInfo so that when rules (like rules_rust's
/// rust_binary_without_process_wrapper) resolve their `_import_macro_dep` through
/// an alias chain ending at a label_flag, the dep is recognized as a cc_library-like
/// dep (via CcInfo) rather than triggering "rust targets can only depend on rust_library
/// or cc_library" errors in collect_deps. The CcInfo has an empty linking context,
/// so it contributes no linker flags.
fn analyze_label_flag(
    target: &ConfiguredTargetLabel,
    _dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    // Minimal CcInfo with empty linking context - required so that label_flag
    // deps (via alias chains) pass the rules_rust collect_deps provider check.
    let cc_info = heap.alloc(CcInfoInstanceStub);
    let providers = SmallMap::from_iter([
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

/// Returns the default value for known Bazel command-line flags.
/// Used for evaluating config_setting with `values` attribute.
fn bazel_flag_default(flag: &str) -> Option<&'static str> {
    match flag {
        "strict_public_imports" => Some("off"),
        "strict_proto_deps" => Some("off"),
        // Add more known flag defaults as needed
        _ => None,
    }
}

/// Check if a config_setting's `values` attribute entries all match known Bazel defaults.
/// Returns true if either:
/// - The `values` attribute is empty (no flag-based constraints)
/// - All flag values match their known Bazel defaults
/// Returns false if any flag is unknown or its value doesn't match the default.
fn check_values_match_defaults(configured_node: ConfiguredTargetNodeRef<'_>) -> bool {
    // Try to read the `values` attribute from the unconfigured target node
    let target_node = configured_node.to_owned();
    let target_ref = target_node.target_node().as_ref();

    if let Some(values_attr) = target_ref.attr_or_none("values", AttrInspectOptions::All) {
        match values_attr.value {
            CoercedAttr::Dict(dict_lit) => {
                if dict_lit.0.is_empty() {
                    // Empty values dict - no flag constraints
                    return false;
                }
                // Check each flag:value pair against known defaults
                for (key, value) in dict_lit.0.iter() {
                    if let (CoercedAttr::String(k), CoercedAttr::String(v)) = (key, value) {
                        match bazel_flag_default(k.0.as_str()) {
                            Some(default_val) if v.0.as_str().eq_ignore_ascii_case(default_val) => {
                                // This entry matches the default
                            }
                            _ => {
                                // Unknown flag or value doesn't match default
                                return false;
                            }
                        }
                    } else {
                        return false;
                    }
                }
                // All entries matched their defaults
                true
            }
            _ => false,
        }
    } else {
        false
    }
}

/// Analyze a config_setting target.
/// Creates a ConfigurationInfo provider by merging constraint data from all
/// constraint_value deps. This allows `select()` to match against the config_setting.
fn analyze_config_setting(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
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

    // If no real constraint pairs found (flag_values/values only), check
    // whether the `values` attribute references Bazel flags with known defaults.
    // If all values match their defaults, the config_setting should match
    // (empty constraint set = matches everything). Otherwise, add a sentinel
    // that will never match any real platform configuration.
    if all_constraint_pairs.is_empty() {
        let values_match_defaults = check_values_match_defaults(configured_node);

        if !values_match_defaults {
            let sentinel_setting = target.unconfigured().dupe();
            let sentinel_value = ProvidersLabel::default_for(target.unconfigured().dupe());
            all_constraint_pairs.push((sentinel_setting, sentinel_value));
        }
        // If values match defaults, leave constraint pairs empty → matches everything
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

/// Collect source file artifacts from a CONFIGURED attr (with select() already resolved).
fn collect_source_files_from_configured_attr(
    attr: &ConfiguredAttr,
    pkg: &kuro_core::package::PackageLabel,
    heap: &FrozenHeap,
    out: &mut Vec<FrozenValue>,
) {
    match attr {
        ConfiguredAttr::List(ListLiteral(items)) => {
            for item in items.iter() {
                collect_source_files_from_configured_attr(item, pkg, heap, out);
            }
        }
        ConfiguredAttr::OneOf(inner, _) => {
            collect_source_files_from_configured_attr(inner, pkg, heap, out);
        }
        ConfiguredAttr::SourceFile(coerced_path) => {
            for path in coerced_path.inputs() {
                let source_artifact = SourceArtifact::new(SourcePath::new(pkg.dupe(), path.dupe()));
                let artifact = Artifact::from(source_artifact);
                let starlark_artifact = heap.alloc_simple(StarlarkArtifact::new(artifact));
                out.push(starlark_artifact);
            }
        }
        _ => {}
    }
}

/// Analyze a filegroup target.
/// Filegroups collect files from their srcs and data deps.
/// For filegroups with source files in srcs, returns DefaultInfo with those source artifacts.
/// For filegroups with no srcs (like empty sentinel targets), returns empty DefaultInfo.
/// For filegroups with deps, merges DefaultInfo.default_outputs from all deps.
fn analyze_filegroup(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    // Check if this filegroup has source files in its srcs attr (from exports_files).
    // Source files are not deps, so they don't appear in dep_analysis.
    let heap = FrozenHeap::new();
    let mut source_outputs: Vec<FrozenValue> = Vec::new();

    let pkg = target.pkg();

    // Use configured node to resolve select() expressions in srcs.
    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        collect_source_files_from_configured_attr(
            &srcs_attr.value,
            &pkg,
            &heap,
            &mut source_outputs,
        );
    }

    if dep_analysis.is_empty() {
        if source_outputs.is_empty() {
            // Empty filegroup - return minimal DefaultInfo
            return create_minimal_analysis_result(target);
        }
        // Source-only filegroup (from exports_files)
        let default_info = FrozenDefaultInfo::with_outputs(&heap, source_outputs);
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
        let analysis_storage = unsafe {
            OwnedFrozenValue::new(heap_ref.dupe(), analysis_storage).downcast_starlark()?
        };
        let recorded_values = RecordedAnalysisValues::new_native(
            self_key,
            Some(analysis_storage),
            RecordedActions::new(0),
        );
        return Ok(AnalysisResult::new(
            recorded_values,
            None,
            HashMap::new(),
            0,
            0,
            None,
        ));
    }

    // Fast path: single dep with no source files, just forward its result directly
    if dep_analysis.len() == 1 && source_outputs.is_empty() {
        let (_label, result) = dep_analysis.into_iter().next().unwrap();
        return Ok(result);
    }

    // Collect default_outputs from all deps into a single merged list, plus source_outputs.
    // Use the heap already created above.
    let mut all_outputs: Vec<FrozenValue> = source_outputs;
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

    let providers = SmallMap::from_iter([
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

/// Analyze a `platform()` target.
///
/// Collects `ConfigurationInfo` providers from all `constraint_values` deps and parent
/// platform deps, merges their constraint pairs, and produces a `PlatformInfo` provider
/// containing the platform's label and the merged configuration.
///
/// The `PlatformInfo` provider is what Bazel uses to resolve toolchain selection and
/// `select()` matching against platform constraints.
fn analyze_platform(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Collect constraint pairs from all deps.
    // constraint_value deps expose ConfigurationInfo with their single constraint pair.
    // parent platform deps expose PlatformInfo whose configuration also has constraint pairs.
    let mut all_constraint_pairs = Vec::new();
    for (_dep_label, dep_result) in &dep_analysis {
        if let Ok(providers) = dep_result.providers() {
            // Collect from ConfigurationInfo (provided by constraint_value and config_setting deps)
            if let Some(config_info) = providers
                .value()
                .builtin_provider::<FrozenConfigurationInfo>()
            {
                let config_data = config_info.to_config_setting_data();
                for (ck, cv) in config_data.constraints {
                    all_constraint_pairs.push((ck.key.dupe(), cv.0.dupe()));
                }
            }
            // Also collect from PlatformInfo (provided by parent platform deps)
            if let Some(platform_info) = providers.value().builtin_provider::<FrozenPlatformInfo>()
            {
                if let Ok(config_data) = platform_info.to_configuration() {
                    if let Ok(data) = config_data.data() {
                        for (ck, cv) in &data.constraints {
                            all_constraint_pairs.push((ck.key.dupe(), cv.0.dupe()));
                        }
                    }
                }
            }
        }
    }

    // The platform label is the unconfigured target label string.
    let label_str = target.unconfigured().to_string();

    // Create PlatformInfo with the merged constraint configuration.
    let platform_info =
        FrozenPlatformInfo::for_native_platform(&label_str, &all_constraint_pairs, &heap);

    let mut providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);
    providers.insert(
        FrozenPlatformInfo::builtin_provider_id().dupe(),
        platform_info,
    );

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

/// Analyze an `sh_library` target.
///
/// Returns DefaultInfo with all `srcs` source files as default_outputs.
/// Behaves like filegroup but for shell scripts.
fn analyze_sh_library(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let mut source_outputs: Vec<FrozenValue> = Vec::new();
    let pkg = target.pkg();

    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        collect_source_files_from_configured_attr(
            &srcs_attr.value,
            &pkg,
            &heap,
            &mut source_outputs,
        );
    }

    // Also merge outputs from dep analysis
    let mut all_outputs = source_outputs;
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

    let default_info = if all_outputs.is_empty() {
        FrozenDefaultInfo::testing_empty(&heap)
    } else {
        FrozenDefaultInfo::with_outputs(&heap, all_outputs)
    };

    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    make_native_analysis_result(target, heap, providers, 0)
}

/// Analyze a `test_suite` target.
///
/// A test_suite groups multiple test targets under a single label.
/// The constituent tests are stored in the internal TESTS_ATTRIBUTE (ID 8) as labels,
/// so node.tests() returns them for expansion by the test runner (kuro test).
/// The test_suite itself produces no build artifacts.
fn analyze_test_suite(
    target: &ConfiguredTargetLabel,
    _dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> kuro_error::Result<AnalysisResult> {
    create_minimal_analysis_result(target)
}

/// Analyze an `sh_binary` target.
///
/// Returns DefaultInfo with the first source file as both a default output and the executable.
/// The shell script is used directly as the executable (it must have +x bits set).
fn analyze_sh_binary(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let pkg = target.pkg();
    let mut source_outputs: Vec<FrozenValue> = Vec::new();

    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        collect_source_files_from_configured_attr(
            &srcs_attr.value,
            &pkg,
            &heap,
            &mut source_outputs,
        );
    }

    let default_info = if let Some(&first_src) = source_outputs.first() {
        FrozenDefaultInfo::with_executable(&heap, first_src)
    } else {
        FrozenDefaultInfo::testing_empty(&heap)
    };

    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    make_native_analysis_result(target, heap, providers, 0)
}

/// Analyze an `sh_test` target.
///
/// Like `sh_binary` but also includes `ExternalRunnerTestInfo` so that
/// `kuro test //:foo_sh_test` works.
fn analyze_sh_test(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let pkg = target.pkg();
    let mut source_outputs: Vec<FrozenValue> = Vec::new();

    if let Some(srcs_attr) = configured_node.get("srcs", AttrInspectOptions::All) {
        collect_source_files_from_configured_attr(
            &srcs_attr.value,
            &pkg,
            &heap,
            &mut source_outputs,
        );
    }

    let (default_info, test_command_fv) = if let Some(&first_src) = source_outputs.first() {
        let di = FrozenDefaultInfo::with_executable(&heap, first_src);
        // Command uses bash as interpreter so the script doesn't need +x bits.
        let bash_str = heap.alloc("bash");
        let cmd_list =
            heap.alloc(starlark::values::list::AllocList([bash_str, first_src]));
        (di, cmd_list)
    } else {
        let di = FrozenDefaultInfo::testing_empty(&heap);
        let empty_list = heap.alloc(starlark::values::list::AllocList::EMPTY);
        (di, empty_list)
    };

    let test_info = create_frozen_sh_test_info(&heap, test_command_fv);
    let test_info_fv = heap.alloc(test_info);

    let providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            FrozenExternalRunnerTestInfo::builtin_provider_id().dupe(),
            test_info_fv,
        ),
    ]);

    make_native_analysis_result(target, heap, providers, 0)
}

/// Analyze an `analysis_test` target created by `testing.analysis_test()`.
///
/// Analysis tests have no build actions - they pass by virtue of completing
/// analysis without errors. We produce an empty DefaultInfo and a
/// ExternalRunnerTestInfo with an empty command, which the test runner treats
/// as a passing test.
fn analyze_analysis_test(target: &ConfiguredTargetLabel) -> kuro_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    let empty_list = heap.alloc(starlark::values::list::AllocList::EMPTY);
    let test_info = create_frozen_sh_test_info(&heap, empty_list);
    let test_info_fv = heap.alloc(test_info);

    let providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            FrozenExternalRunnerTestInfo::builtin_provider_id().dupe(),
            test_info_fv,
        ),
    ]);

    make_native_analysis_result(target, heap, providers, 0)
}

/// Analyze a `genquery` target.
///
/// genquery runs a Bazel query expression and writes the results to an output file.
/// This is a stub implementation: it declares an output file (named after the target)
/// and registers an action that creates an empty file. Full query execution would
/// require integrating with the Kuro query engine at build time.
///
/// In Bazel: `genquery(name="deps", expression="deps(//foo:bar)", scope=["//foo:bar"])`
/// produces a file `deps` containing one label per line.
fn analyze_genquery(target: &ConfiguredTargetLabel) -> kuro_error::Result<AnalysisResult> {
    let self_key = DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target.dupe()));
    let action_key = ActionKey::new(self_key.dupe(), ActionIndex::new(0));

    // The output file is named after the rule (same as the target name)
    let output_name = target.name().as_str().to_owned();
    let path = BuildArtifactPath::new(
        BaseDeferredKey::TargetLabel(target.dupe()),
        ForwardRelativePathBuf::new(output_name.clone())
            .with_buck_error_context(|| format!("Invalid genquery output path: {}", output_name))?,
        BuckOutPathKind::Configuration,
    );
    let output_artifact = BuildArtifact::new(path, action_key.dupe(), OutputType::File)?;

    let heap = FrozenHeap::new();
    let starlark_output = heap.alloc_simple(StarlarkArtifact::new(Artifact::from(
        output_artifact.dupe(),
    )));

    // Register an action that creates an empty output file (stub implementation).
    // A real implementation would run the query and write results.
    let genrule_action = GenruleAction::new(
        "touch \"$@\"".to_owned(),
        vec![],
        vec![output_artifact],
        vec![],
    );
    let registered_action = Arc::new(RegisteredAction::new(
        action_key.dupe(),
        Box::new(genrule_action),
        CommandExecutorConfig::testing_local(),
    ));
    let mut recorded_actions = RecordedActions::new(1);
    recorded_actions.insert(action_key, registered_action);

    let default_info = FrozenDefaultInfo::with_outputs(&heap, vec![starlark_output]);
    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    let provider_collection = FrozenValueTyped::<FrozenProviderCollection>::new_err(
        heap.alloc(FrozenProviderCollection::new(providers)),
    )?;

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

    let recorded_values =
        RecordedAnalysisValues::new_native(self_key, Some(analysis_storage), recorded_actions);

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        1, // 1 action registered
        1, // 1 declared artifact
        None,
    ))
}

/// Build a `AnalysisResult` from a FrozenHeap + providers map.
/// Avoids boilerplate duplication across sh_library, sh_binary, sh_test.
fn make_native_analysis_result(
    target: &ConfiguredTargetLabel,
    heap: FrozenHeap,
    providers: SmallMap<std::sync::Arc<kuro_core::provider::id::ProviderId>, FrozenValue>,
    num_actions: u64,
) -> kuro_error::Result<AnalysisResult> {
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
        num_actions,
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
