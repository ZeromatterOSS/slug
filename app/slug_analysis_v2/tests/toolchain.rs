/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use slug_analysis_v2::ConstraintSet;
use slug_analysis_v2::ConstraintSetting;
use slug_analysis_v2::ConstraintValue;
use slug_analysis_v2::ExecGroup;
use slug_analysis_v2::ExecGroupCollection;
use slug_analysis_v2::ExecutionPlatform;
use slug_analysis_v2::RegisteredToolchains;
use slug_analysis_v2::RegisteredToolchainsKey;
use slug_analysis_v2::ResolvedToolchainContext;
use slug_analysis_v2::ToolchainResolutionError;
use slug_analysis_v2::ToolchainResolutionRequest;
use slug_analysis_v2::ToolchainTarget;
use slug_analysis_v2::ToolchainType;
use slug_identity_v2::CanonicalLabel;

fn label(value: &str) -> CanonicalLabel {
    CanonicalLabel::parse(value).unwrap()
}

fn os_setting() -> ConstraintSetting {
    ConstraintSetting::new(label("@@platforms//os:os"))
}

fn os_value(name: &str) -> ConstraintValue {
    ConstraintValue::new(os_setting(), label(&format!("@@platforms//os:{name}")))
}

fn platform(name: &str, os: &str) -> ExecutionPlatform {
    ExecutionPlatform::new(
        label(&format!("@@//platforms:{name}")),
        ConstraintSet::new(vec![os_value(os)]),
    )
}

fn toolchain_type() -> ToolchainType {
    ToolchainType::new(label("@@//toolchains:demo_type"))
}

fn toolchain(name: &str, exec_os: &str) -> ToolchainTarget {
    ToolchainTarget::new(
        label(&format!("@@//toolchains:{name}")),
        toolchain_type(),
        ConstraintSet::default(),
        ConstraintSet::new(vec![os_value(exec_os)]),
    )
}

#[test]
fn toolchain_resolution_selects_first_compatible_execution_platform() {
    let registered = RegisteredToolchains::new(
        vec![toolchain("linux_toolchain", "linux")],
        vec![
            platform("mac_exec", "macos"),
            platform("linux_exec", "linux"),
        ],
    );
    let request =
        ToolchainResolutionRequest::new(toolchain_type(), ConstraintSet::default(), registered);

    let resolution = request.resolve().unwrap();
    assert_eq!(
        resolution.selected_execution_platform().to_string(),
        "@@//platforms:linux_exec"
    );
    assert_eq!(
        resolution.selected_toolchain().to_string(),
        "@@//toolchains:linux_toolchain"
    );
    assert!(resolution.events()[0].contains("mac_exec"));
    assert!(
        resolution
            .events()
            .iter()
            .any(|event| event.contains("selected toolchain"))
    );
}

#[test]
fn mandatory_toolchain_missing_reports_events() {
    let registered = RegisteredToolchains::new(
        vec![toolchain("mac_toolchain", "macos")],
        vec![platform("linux_exec", "linux")],
    );
    let request =
        ToolchainResolutionRequest::new(toolchain_type(), ConstraintSet::default(), registered);

    let err = request.resolve().unwrap_err();
    assert!(matches!(
        err,
        ToolchainResolutionError::MandatoryToolchainMissing { .. }
    ));
    assert!(err.to_string().contains("mandatory toolchain type"));
    assert!(
        err.events()
            .iter()
            .any(|event| event.contains("reject toolchain"))
    );
}

#[test]
fn exec_groups_carry_toolchain_types_exec_properties_and_contexts() {
    let mut props = BTreeMap::new();
    props.insert("container-image".to_owned(), "toolchain:v1".to_owned());
    let group =
        ExecGroup::new("compile", vec![toolchain_type()]).with_exec_properties(props.clone());
    let mut groups = ExecGroupCollection::new(vec![group]);

    let registered = RegisteredToolchains::new(
        vec![toolchain("linux_toolchain", "linux")],
        vec![platform("linux_exec", "linux")],
    );
    let resolution =
        ToolchainResolutionRequest::new(toolchain_type(), ConstraintSet::default(), registered)
            .resolve()
            .unwrap();
    let mut context = ResolvedToolchainContext::new();
    context.insert(toolchain_type(), resolution);
    groups.set_resolved_context("compile", context);

    assert_eq!(groups.group("compile").unwrap().exec_properties(), &props);
    assert_eq!(
        groups
            .resolved_context("compile")
            .unwrap()
            .selected_execution_platform()
            .unwrap()
            .to_string(),
        "@@//platforms:linux_exec"
    );
}

#[test]
fn registered_toolchain_key_changes_with_bzlmod_or_flag_inputs() {
    let first = RegisteredToolchainsKey::new("bzlmod1", "flags1").unwrap();
    let changed_bzlmod = RegisteredToolchainsKey::new("bzlmod2", "flags1").unwrap();
    let changed_flags = RegisteredToolchainsKey::new("bzlmod1", "flags2").unwrap();

    assert_ne!(first, changed_bzlmod);
    assert_ne!(first, changed_flags);
    assert_eq!(first.stable_serialize(), "bzlmod=bzlmod1;flags=flags1");
}
