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
use slug_artifact::actions::key::ActionIndex;
use slug_artifact::actions::key::ActionKey;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_artifact::artifact::source_artifact::SourceArtifact;
use slug_build_api::actions::RegisteredAction;
use slug_build_api::actions::registry::RecordedActions;
use slug_build_api::analysis::AnalysisResult;
use slug_build_api::analysis::registry::FrozenAnalysisValueStorage;
use slug_build_api::analysis::registry::RecordedAnalysisValues;
use slug_build_api::artifact_groups::ArtifactGroup;
use slug_build_api::dynamic::storage::DYNAMIC_LAMBDA_PARAMS_STORAGES;
use slug_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use slug_build_api::interpreter::rule_defs::cc_common::CcInfoInstanceStub;
use slug_build_api::interpreter::rule_defs::cc_common::CcInfoProvider;
use slug_build_api::interpreter::rule_defs::platform_common::ConstraintSettingInfoProvider;
use slug_build_api::interpreter::rule_defs::provider::FrozenBuiltinProviderLike;
use slug_build_api::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;
use slug_build_api::interpreter::rule_defs::provider::builtin::constraint_setting_info::FrozenConstraintSettingInfo;
use slug_build_api::interpreter::rule_defs::provider::builtin::constraint_value_info::FrozenConstraintValueInfo;
use slug_build_api::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use slug_build_api::interpreter::rule_defs::provider::builtin::default_info::FrozenDefaultInfo;
use slug_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::FrozenExternalRunnerTestInfo;
use slug_build_api::interpreter::rule_defs::provider::builtin::external_runner_test_info::create_frozen_sh_test_info;
use slug_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;
use slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use slug_core::deferred::base_deferred_key::BaseDeferredKey;
use slug_core::deferred::key::DeferredHolderKey;
use slug_core::execution_types::executor_config::CommandExecutorConfig;
use slug_core::fs::buck_out_path::BuckOutPathKind;
use slug_core::fs::buck_out_path::BuildArtifactPath;
use slug_core::package::source_path::SourcePath;
use slug_core::provider::label::ProvidersLabel;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_core::target::label::label::TargetLabel;
use slug_error::BuckErrorContext;
use slug_error::internal_error;
use slug_execute::execute::request::OutputType;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use slug_node::attrs::attr_type::list::ListLiteral;
use slug_node::attrs::coerced_attr::CoercedAttr;
use slug_node::attrs::configured_attr::ConfiguredAttr;
use slug_node::attrs::inspect_options::AttrInspectOptions;
use slug_node::nodes::configured::ConfiguredTargetNodeRef;
use slug_node::rule_type::NativeRuleKind;
use slug_node::rule_type::RemovedNativeRule;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::OwnedFrozenValue;
use starlark::values::any_complex::StarlarkAnyComplex;
use starlark_map::small_map::SmallMap;

use crate::analysis::genrule_action::GenruleAction;
use crate::analysis::genrule_action::GenruleShell;

// ============================================================================
// Declared Toolchain Registry (for toolchain resolution)
// ============================================================================

/// Metadata about a `toolchain()` target, extracted during analysis.
/// Used by the toolchain resolution algorithm to match toolchains to platforms.
#[derive(Debug, Clone)]
pub struct DeclaredToolchainInfo {
    /// The toolchain_type this satisfies (e.g., "@bazel_tools//tools/cpp:toolchain_type")
    pub toolchain_type: String,
    /// The toolchain implementation target label
    pub toolchain_impl: String,
    /// For rule-based C++ toolchains, the generated cc_toolchain_config target.
    pub cc_toolchain_config: Option<String>,
    /// For rule-based C++ toolchains, the configured module map target, if any.
    pub cc_toolchain_module_map: Option<String>,
    /// Constraint values the execution platform must satisfy
    pub exec_compatible_with: Vec<String>,
    /// Constraint values the target platform must satisfy
    pub target_compatible_with: Vec<String>,
}

/// Global registry of declared toolchains, populated during analysis of `toolchain()` targets.
static DECLARED_TOOLCHAINS: std::sync::RwLock<Vec<(String, DeclaredToolchainInfo)>> =
    std::sync::RwLock::new(Vec::new());

/// Register a declared toolchain (called during toolchain() analysis).
///
/// Panics if the lock was poisoned by a concurrent writer panic. A poisoned
/// lock here means analysis state is already corrupt; silently recovering
/// would make build results non-deterministic.
pub fn register_declared_toolchain(toolchain_label: String, info: DeclaredToolchainInfo) {
    DECLARED_TOOLCHAINS
        .write()
        .expect("DECLARED_TOOLCHAINS poisoned")
        .push((toolchain_label, info));
}

/// Get all declared toolchains. Panics on poisoned lock (see
/// [`register_declared_toolchain`]).
pub fn get_declared_toolchains() -> Vec<(String, DeclaredToolchainInfo)> {
    DECLARED_TOOLCHAINS
        .read()
        .expect("DECLARED_TOOLCHAINS poisoned")
        .clone()
}

/// Clear declared toolchains (for fresh builds). Panics on poisoned lock (see
/// [`register_declared_toolchain`]).
pub fn clear_declared_toolchains() {
    DECLARED_TOOLCHAINS
        .write()
        .expect("DECLARED_TOOLCHAINS poisoned")
        .clear();
}

// ============================================================================
// Plan 13 Phase 3: Deferred toolchain pool (lazy load on resolution miss)
// ============================================================================

/// A deferred toolchain registration — captured during eager loading so a
/// later `resolve_toolchains` miss can lazy-load only the relevant subset.
#[derive(Debug, Clone)]
pub struct DeferredToolchain {
    /// Origin module name (matched against required `toolchain_type` repo
    /// names by the lazy-load heuristic).
    pub module: String,
    /// The full registered label (e.g., `@rules_swift//toolchains:all`).
    pub label: String,
}

/// Pool of toolchain registrations from non-root modules. Populated by
/// `ensure_registered_toolchains_loaded` instead of being eagerly loaded.
/// `ensure_deferred_toolchains_loaded` drains relevant entries on demand.
static DEFERRED_TOOLCHAINS: std::sync::RwLock<Vec<DeferredToolchain>> =
    std::sync::RwLock::new(Vec::new());

/// Marker set: keys (origin-module) of deferred entries already loaded so
/// repeated `resolve_toolchains` misses for the same type don't reload.
static LOADED_DEFERRED_KEYS: std::sync::LazyLock<
    std::sync::RwLock<std::collections::HashSet<String>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(std::collections::HashSet::new()));

/// Marker: when the "load-everything-deferred" fallback has fired, set true
/// so subsequent misses don't re-iterate the full pool.
static LOADED_ALL_DEFERRED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Replace the deferred pool. Called by `ensure_registered_toolchains_loaded`.
pub fn set_deferred_toolchains(items: Vec<DeferredToolchain>) {
    *DEFERRED_TOOLCHAINS
        .write()
        .expect("DEFERRED_TOOLCHAINS poisoned") = items;
    LOADED_DEFERRED_KEYS
        .write()
        .expect("LOADED_DEFERRED_KEYS poisoned")
        .clear();
    LOADED_ALL_DEFERRED.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// Snapshot the deferred pool (cheap clone of label strings).
pub fn get_deferred_toolchains() -> Vec<DeferredToolchain> {
    DEFERRED_TOOLCHAINS
        .read()
        .expect("DEFERRED_TOOLCHAINS poisoned")
        .clone()
}

/// True iff the (module, label) pair was already drained by a previous
/// lazy-load pass.
pub fn deferred_key_already_loaded(key: &str) -> bool {
    LOADED_DEFERRED_KEYS
        .read()
        .expect("LOADED_DEFERRED_KEYS poisoned")
        .contains(key)
}

/// Mark a deferred key as loaded.
pub fn mark_deferred_key_loaded(key: String) {
    LOADED_DEFERRED_KEYS
        .write()
        .expect("LOADED_DEFERRED_KEYS poisoned")
        .insert(key);
}

/// True iff the load-everything fallback has already fired.
pub fn deferred_all_loaded() -> bool {
    LOADED_ALL_DEFERRED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Mark that the load-everything fallback fired.
pub fn mark_deferred_all_loaded() {
    LOADED_ALL_DEFERRED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Analyze a native rule target and return the analysis result.
pub fn analyze_native_rule(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef<'_>,
    kind: &NativeRuleKind,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
    flag_values_match: bool,
    values_match: bool,
) -> slug_error::Result<AnalysisResult> {
    match kind {
        NativeRuleKind::ConstraintSetting => analyze_constraint_setting(target, configured_node),
        NativeRuleKind::ConstraintValue => {
            analyze_constraint_value(target, configured_node, dep_analysis)
        }
        NativeRuleKind::Filegroup => analyze_filegroup(target, configured_node, dep_analysis),
        NativeRuleKind::Alias => analyze_alias(target, dep_analysis),
        NativeRuleKind::LabelFlag => analyze_label_flag(target, dep_analysis), // dep_analysis is empty (build_setting_default is a string, not a dep)
        NativeRuleKind::ConfigSetting => {
            analyze_config_setting(target, dep_analysis, flag_values_match, values_match)
        }
        NativeRuleKind::ToolchainType => create_minimal_analysis_result(target),
        NativeRuleKind::PackageGroup => analyze_package_group(target),
        NativeRuleKind::Genrule => analyze_genrule(target, configured_node, dep_analysis),
        NativeRuleKind::Platform => analyze_platform(target, configured_node, dep_analysis),
        NativeRuleKind::TestSuite => analyze_test_suite(target, dep_analysis),
        NativeRuleKind::Toolchain => analyze_toolchain(target, configured_node, dep_analysis),
        NativeRuleKind::CcLibcTopAlias => create_minimal_analysis_result(target),
        NativeRuleKind::AnalysisTest => analyze_analysis_test(target),
        NativeRuleKind::Genquery => analyze_genquery(target),
        NativeRuleKind::StarlarkDocExtract => analyze_genquery(target), // stub: empty output file
        NativeRuleKind::XcodeConfig => analyze_xcode_config(target),
        NativeRuleKind::Removed(removed) => analyze_removed_rule(target, *removed),
    }
}

/// Analyze a Bazel-9-removed native rule. Returns a Bazel-shaped diagnostic.
/// See Plan 27 (`thoughts/shared/plans/slug-bazel-subplans/27-...md`).
fn analyze_removed_rule(
    target: &ConfiguredTargetLabel,
    removed: RemovedNativeRule,
) -> slug_error::Result<AnalysisResult> {
    Err(slug_error::slug_error!(
        slug_error::ErrorTag::Input,
        "{message}\n  in target {target}",
        message = removed.diagnostic_message(),
        target = target,
    ))
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
) -> slug_error::Result<AnalysisResult> {
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

    // Select command and shell interpreter based on platform and available attrs.
    // On Windows, Bazel priority is: cmd_ps > cmd_bat > cmd_bash > cmd
    // On Unix, priority is: cmd_bash > cmd
    #[cfg(windows)]
    let (cmd, shell) = {
        let cmd_ps_raw = read_configured_string("cmd_ps");
        let cmd_bat_raw = read_configured_string("cmd_bat");
        if !cmd_ps_raw.is_empty() {
            (cmd_ps_raw, GenruleShell::PowerShell)
        } else if !cmd_bat_raw.is_empty() {
            (cmd_bat_raw, GenruleShell::CmdExe)
        } else if !cmd_bash_raw.is_empty() {
            (cmd_bash_raw, GenruleShell::Bash)
        } else {
            (cmd_raw, GenruleShell::Bash)
        }
    };
    #[cfg(not(windows))]
    let (cmd, shell) = if !cmd_bash_raw.is_empty() {
        (cmd_bash_raw, GenruleShell::Bash)
    } else {
        (cmd_raw, GenruleShell::Bash)
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
    let genrule_action = GenruleAction::new(
        cmd,
        inputs,
        output_artifacts,
        out_names.clone(),
        location_mappings,
        shell,
    );

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

    let num_outputs = out_names.len() as u64;
    make_native_analysis_result(target, heap, providers, 1, num_outputs, recorded_actions)
}

/// Extract all unique labels from $(location label), $(locations label),
/// $(execpath label), $(execpaths label), $(rootpath label), $(rootpaths label),
/// $(rlocationpath label), and $(rlocationpaths label) patterns in a genrule cmd.
fn extract_location_labels(cmd: &str) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    let mut remaining = cmd;
    while let Some(start) = remaining.find("$(") {
        let after_paren = &remaining[start + 2..];
        let keyword_len = if after_paren.starts_with("locations ")
            || after_paren.starts_with("execpaths ")
            || after_paren.starts_with("rootpaths ")
        {
            10usize
        } else if after_paren.starts_with("location ")
            || after_paren.starts_with("execpath ")
            || after_paren.starts_with("rootpath ")
        {
            9usize
        } else if after_paren.starts_with("rlocationpaths ") {
            15usize
        } else if after_paren.starts_with("rlocationpath ") {
            14usize
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
    pkg: &slug_core::package::PackageLabel,
) -> Vec<(String, Vec<ArtifactGroup>)> {
    let mut result: Vec<(String, Vec<ArtifactGroup>)> = Vec::new();
    collect_source_file_location_mappings_recursive(attr, pkg, &mut result);
    result
}

fn collect_source_file_location_mappings_recursive(
    attr: &ConfiguredAttr,
    pkg: &slug_core::package::PackageLabel,
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

/// Collect ArtifactGroups from a CONFIGURED attr (with select() already resolved).
fn collect_artifact_groups_from_configured_attr(
    attr: &ConfiguredAttr,
    pkg: &slug_core::package::PackageLabel,
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
/// Returns DefaultInfo and ConstraintSettingInfo so that
/// `refs.x[ConstraintSettingInfo]` works in configuration transitions.
fn analyze_constraint_setting(
    target: &ConfiguredTargetLabel,
    _configured_node: ConfiguredTargetNodeRef<'_>,
) -> slug_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();
    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    let constraint_setting_info =
        FrozenConstraintSettingInfo::create_on_frozen_heap(target.unconfigured().dupe(), &heap);

    let providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            ConstraintSettingInfoProvider::provider_id().dupe(),
            constraint_setting_info,
        ),
    ]);

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

/// Analyze a constraint_value target.
/// Returns DefaultInfo, ConstraintValueInfo, and ConfigurationInfo so that
/// `target[platform_common.ConstraintValueInfo]` works and config_setting
/// can extract constraint data from deps.
fn analyze_constraint_value(
    target: &ConfiguredTargetLabel,
    _configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> slug_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Extract constraint_setting label from dep_analysis.
    // constraint_value has exactly one dep: its constraint_setting.
    let cs_target_label = if !dep_analysis.is_empty() {
        dep_analysis[0].0.unconfigured().dupe()
    } else {
        target.unconfigured().dupe()
    };

    // Create the real FrozenConstraintSettingInfo and FrozenConstraintValueInfo so that
    // transition functions can access `.setting.label` on the ConstraintValueInfo instance.
    let frozen_cs_info = FrozenConstraintSettingInfo::create_on_frozen_heap(cs_target_label, &heap);
    let cv_providers_label = ProvidersLabel::default_for(target.unconfigured().dupe());
    let constraint_value_info =
        FrozenConstraintValueInfo::create_on_frozen_heap(frozen_cs_info, cv_providers_label, &heap);

    // Create ConfigurationInfo with one constraint pair (cs→cv)
    // so that config_setting can merge constraints from deps.
    let mut providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            FrozenConstraintValueInfo::builtin_provider_id().dupe(),
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

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
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
) -> slug_error::Result<AnalysisResult> {
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
    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

/// Returns the default value for known Bazel command-line flags.

/// Analyze a config_setting target.
/// Creates a ConfigurationInfo provider by merging constraint data from all
/// constraint_value deps. This allows `select()` to match against the config_setting.
///
/// `flag_values_match` indicates whether all `flag_values` entries match their
/// `build_setting_default` values (pre-computed asynchronously in `calculation.rs`).
/// `values_match` indicates whether all `values` entries (buckconfig key-value pairs) match
/// the current buckconfig (pre-computed asynchronously in `calculation.rs`).
fn analyze_config_setting(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
    flag_values_match: bool,
    values_match: bool,
) -> slug_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Collect constraint pairs from each constraint_value dep's ConfigurationInfo.
    // Note: flag_values deps also appear in dep_analysis, but they don't contribute
    // ConfigurationInfo (they're build setting targets, not constraint_value targets),
    // so they are naturally skipped by this loop.
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

    // If flag_values or values don't match, this config_setting should not match.
    // Add a sentinel constraint that never matches any real platform.
    if !flag_values_match || !values_match {
        let sentinel_setting = target.unconfigured().dupe();
        let sentinel_value = ProvidersLabel::default_for(target.unconfigured().dupe());
        all_constraint_pairs.push((sentinel_setting, sentinel_value));
    }
    // Otherwise (flag_values and values both match), leave all_constraint_pairs as-is.
    // If empty → config_setting matches everything (no constraints).
    // If non-empty → config_setting matches when the platform has those constraints.

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

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

/// Analyze an alias target.
/// An alias forwards all providers from its `actual` target.
fn analyze_alias(
    target: &ConfiguredTargetLabel,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> slug_error::Result<AnalysisResult> {
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
    pkg: &slug_core::package::PackageLabel,
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
) -> slug_error::Result<AnalysisResult> {
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
        return make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0));
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

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
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
    configured_node: ConfiguredTargetNodeRef<'_>,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> slug_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Collect constraint pairs from all deps.
    // constraint_value deps expose ConfigurationInfo with their single constraint pair.
    // parent platform deps expose PlatformInfo whose configuration also has constraint pairs.
    let mut all_constraint_pairs = Vec::new();
    let mut inherited_exec_properties: Vec<(String, String)> = Vec::new();
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
                // Parent platforms contribute their exec_properties; child
                // `exec_properties` will override any shared keys below.
                for (k, v) in platform_info.exec_properties_entries() {
                    inherited_exec_properties.push((k, v));
                }
            }
        }
    }

    // Read this platform's own exec_properties and merge over the inherited set.
    let mut exec_properties: std::collections::BTreeMap<String, String> =
        inherited_exec_properties.into_iter().collect();
    if let Some(attr) = configured_node.get("exec_properties", AttrInspectOptions::All) {
        if let ConfiguredAttr::Dict(dict) = &attr.value {
            for (k, v) in dict.0.iter() {
                if let (ConfiguredAttr::String(kstr), ConfiguredAttr::String(vstr)) = (k, v) {
                    exec_properties.insert(kstr.0.to_string(), vstr.0.to_string());
                }
            }
        }
    }
    let exec_properties_vec: Vec<(String, String)> = exec_properties.into_iter().collect();

    // Bazel labels name the main repo without a cell prefix (`//pkg:tgt`).
    // Slug's internal Display keeps the root cell (`zeromatter//pkg:tgt`),
    // which is not parseable by the Bazel-shaped validation path.
    let label_str = bazel_platform_label(target.unconfigured());

    // Create PlatformInfo with the merged constraint configuration.
    let platform_info = FrozenPlatformInfo::for_native_platform(
        &label_str,
        &all_constraint_pairs,
        &exec_properties_vec,
        &heap,
    );

    let mut providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);
    providers.insert(
        FrozenPlatformInfo::builtin_provider_id().dupe(),
        platform_info,
    );

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

fn bazel_platform_label(target: &TargetLabel) -> String {
    if slug_core::cells::is_root_cell_name(target.pkg().cell_name().as_str()) {
        format!("//{}:{}", target.pkg().cell_relative_path(), target.name())
    } else {
        target.to_string()
    }
}

/// Analyze a `test_suite` target.
///
/// A test_suite groups multiple test targets under a single label.
/// The constituent tests are stored in the internal TESTS_ATTRIBUTE (ID 8) as labels,
/// so node.tests() returns them for expansion by the test runner (slug test).
/// The test_suite itself produces no build artifacts.
fn analyze_test_suite(
    target: &ConfiguredTargetLabel,
    _dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> slug_error::Result<AnalysisResult> {
    create_minimal_analysis_result(target)
}

/// Analyze an `analysis_test` target created by `testing.analysis_test()`.
///
/// Analysis tests have no build actions - they pass by virtue of completing
/// analysis without errors. We produce an empty DefaultInfo and a
/// ExternalRunnerTestInfo with an empty command, which the test runner treats
/// as a passing test.
fn analyze_analysis_test(target: &ConfiguredTargetLabel) -> slug_error::Result<AnalysisResult> {
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

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

/// Analyze a `genquery` target.
///
/// genquery runs a Bazel query expression and writes the results to an output file.
/// This is a stub implementation: it declares an output file (named after the target)
/// and registers an action that creates an empty file. Full query execution would
/// require integrating with the Slug query engine at build time.
///
/// In Bazel: `genquery(name="deps", expression="deps(//foo:bar)", scope=["//foo:bar"])`
/// produces a file `deps` containing one label per line.
fn analyze_genquery(target: &ConfiguredTargetLabel) -> slug_error::Result<AnalysisResult> {
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
        vec![],
        GenruleShell::Bash,
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

    make_native_analysis_result(target, heap, providers, 1, 1, recorded_actions)
}

/// Build an `AnalysisResult` from a FrozenHeap + providers map.
/// Consolidates the ~25-line tail shared across all native `analyze_X` functions.
///
/// Parameters:
/// - `num_actions` and `num_declared_artifacts` are forwarded into `AnalysisResult::new`.
/// - `recorded_actions` carries any actions registered by the caller (or
///   `RecordedActions::new(0)` for action-less rules).
fn make_native_analysis_result(
    target: &ConfiguredTargetLabel,
    heap: FrozenHeap,
    providers: SmallMap<std::sync::Arc<slug_core::provider::id::ProviderId>, FrozenValue>,
    num_actions: u64,
    num_declared_artifacts: u64,
    recorded_actions: RecordedActions,
) -> slug_error::Result<AnalysisResult> {
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

    let recorded_values =
        RecordedAnalysisValues::new_native(self_key, Some(analysis_storage), recorded_actions);

    Ok(AnalysisResult::new(
        recorded_values,
        None,
        HashMap::new(),
        num_actions,
        num_declared_artifacts,
        None,
    ))
}

/// Analyze a toolchain() target.
///
/// Extracts the toolchain_type, toolchain implementation, and platform
/// constraints, then registers the metadata in the global declared
/// toolchains registry for use by the toolchain resolution algorithm.
fn analyze_toolchain(
    target: &ConfiguredTargetLabel,
    configured_node: ConfiguredTargetNodeRef,
    dep_analysis: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> slug_error::Result<AnalysisResult> {
    let label_str = target.unconfigured().to_string();

    // Extract toolchain_type and toolchain impl labels from deps.
    // The toolchain() rule has exactly two user attrs: "toolchain_type" and "toolchain".
    // These are deps, so their labels appear in dep_analysis.
    let mut toolchain_type_label = String::new();
    let mut toolchain_impl_label = String::new();

    // Read the attrs to get the declared dep labels
    for attr_full in configured_node.attrs(AttrInspectOptions::DefinedOnly) {
        if attr_full.name == "toolchain_type" {
            if let ConfiguredAttr::Dep(dep) = &attr_full.value {
                toolchain_type_label = dep.label.target().unconfigured().to_string();
            }
        } else if attr_full.name == "toolchain" {
            if let ConfiguredAttr::Dep(dep) = &attr_full.value {
                toolchain_impl_label = dep.label.target().unconfigured().to_string();
            }
        }
    }

    // Extract exec_compatible_with and target_compatible_with from internal attrs.
    // These are list-of-label attrs. We extract the label strings.
    let mut exec_compat = Vec::new();
    let mut target_compat = Vec::new();

    for attr_full in configured_node.attrs(AttrInspectOptions::All) {
        if attr_full.name == "exec_compatible_with" || attr_full.name == "target_compatible_with" {
            let labels = extract_label_strings_from_attr(&attr_full.value);
            if attr_full.name == "exec_compatible_with" {
                exec_compat = labels;
            } else {
                target_compat = labels;
            }
        }
    }

    // Register in the global declared toolchains registry
    if !toolchain_type_label.is_empty() {
        let info = DeclaredToolchainInfo {
            toolchain_type: toolchain_type_label.clone(),
            toolchain_impl: toolchain_impl_label.clone(),
            cc_toolchain_config: None,
            cc_toolchain_module_map: None,
            exec_compatible_with: exec_compat,
            target_compatible_with: target_compat,
        };
        tracing::debug!(
            "Registered toolchain '{}': type='{}', impl='{}'",
            label_str,
            info.toolchain_type,
            info.toolchain_impl
        );
        register_declared_toolchain(label_str, info);
    }

    // Return minimal analysis result (toolchain() targets don't produce
    // actions or providers that other rules consume directly)
    create_minimal_analysis_result(target)
}

/// Extract label strings from a configured attribute that's a list of deps.
fn extract_label_strings_from_attr(attr: &ConfiguredAttr) -> Vec<String> {
    let mut labels = Vec::new();
    fn push_one(labels: &mut Vec<String>, item: &ConfiguredAttr) {
        match item {
            ConfiguredAttr::Dep(dep) => {
                labels.push(dep.label.target().unconfigured().to_string());
            }
            ConfiguredAttr::Label(label) => {
                labels.push(label.target().unconfigured().to_string());
            }
            // `exec_compatible_with` / `target_compatible_with` and other
            // compatibility attrs use `attrs.configuration_dep(...)`,
            // which lands as `ConfigurationDep` after coercion. Without
            // this branch, `extract_label_strings_from_attr` silently
            // drops every constraint and toolchain resolution sees an
            // empty `exec_compatible_with` list — every toolchain
            // matches every platform and the first one wins.
            ConfiguredAttr::ConfigurationDep(label) => {
                labels.push(label.target().to_string());
            }
            ConfiguredAttr::OneOf(inner, _) => push_one(labels, inner),
            _ => {}
        }
    }
    if let ConfiguredAttr::List(list) = attr {
        for item in list.iter() {
            push_one(&mut labels, item);
        }
    }
    labels
}

/// Create a minimal analysis result with just DefaultInfo.
/// This is used for native rules that don't need complex provider creation.
/// Analyze package_group native rule.
///
/// Provides PackageSpecificationInfo (required by cc_toolchain's allow_list attributes)
/// alongside DefaultInfo.
fn analyze_package_group(target: &ConfiguredTargetLabel) -> slug_error::Result<AnalysisResult> {
    use slug_build_api::interpreter::rule_defs::cc_common::PackageSpecificationInfoInstanceGen;
    use slug_build_api::interpreter::rule_defs::cc_common::PackageSpecificationInfoProvider;
    use starlark::values::FrozenValue;

    let heap = FrozenHeap::new();

    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    let pkg_spec = heap.alloc_simple(PackageSpecificationInfoInstanceGen {
        packages: FrozenValue::new_empty_list(),
    });

    let providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            PackageSpecificationInfoProvider::provider_id().dupe(),
            pkg_spec,
        ),
    ]);

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

/// Analyze xcode_config native rule.
///
/// Provides XcodeVersionConfig with stub values for non-Apple platforms.
/// The generated cc_toolchain_config.bzl accesses this unconditionally via
/// `ctx.attr._xcode_config[apple_common.XcodeVersionConfig]`.
fn analyze_xcode_config(target: &ConfiguredTargetLabel) -> slug_error::Result<AnalysisResult> {
    use slug_build_api::interpreter::rule_defs::apple_common::XcodeVersionConfigInstance;
    use slug_build_api::interpreter::rule_defs::apple_common::XcodeVersionConfigProvider;

    let heap = FrozenHeap::new();

    let default_info = FrozenDefaultInfo::testing_empty(&heap);
    let xcode_config = heap.alloc_simple(XcodeVersionConfigInstance);

    let providers = SmallMap::from_iter([
        (
            DefaultInfoCallable::provider_id().dupe(),
            default_info.to_frozen_value(),
        ),
        (
            XcodeVersionConfigProvider::provider_id().dupe(),
            xcode_config,
        ),
    ]);

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}

fn create_minimal_analysis_result(
    target: &ConfiguredTargetLabel,
) -> slug_error::Result<AnalysisResult> {
    let heap = FrozenHeap::new();

    // Create DefaultInfo (empty)
    let default_info = FrozenDefaultInfo::testing_empty(&heap);

    // Build provider collection with just DefaultInfo
    let providers = SmallMap::from_iter([(
        DefaultInfoCallable::provider_id().dupe(),
        default_info.to_frozen_value(),
    )]);

    make_native_analysis_result(target, heap, providers, 0, 0, RecordedActions::new(0))
}
