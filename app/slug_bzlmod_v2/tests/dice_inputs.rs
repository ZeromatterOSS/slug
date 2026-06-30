/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bzlmod_v2::BzlmodDiceInputs;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ResolvedBzlmodGraphDiceKey;
use slug_bzlmod_v2::YankedVersionPolicy;

fn inputs(env_value: Option<&str>, lockfile_mode: LockfileMode) -> BzlmodDiceInputs {
    BzlmodDiceInputs::new(
        "rootabc",
        "includesabc",
        "registriesabc",
        "lockfileabc",
        lockfile_mode,
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(env_value).unwrap(),
    )
    .unwrap()
}

#[test]
fn environment_policy_serializes_yanked_allowlist_stably() {
    let policy =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("zzz@2.0.0,yyy@1.0.0"))
            .unwrap();

    assert_eq!(
        policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0]"
    );
    assert_eq!(
        policy.yanked_versions_policy(),
        &YankedVersionPolicy::AllowList(std::collections::BTreeSet::from([
            ModuleKey::new("yyy", "1.0.0"),
            ModuleKey::new("zzz", "2.0.0"),
        ]))
    );
}

#[test]
fn resolved_graph_key_changes_when_environment_policy_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let reject = ResolvedBzlmodGraphDiceKey::new(root.clone(), inputs(None, LockfileMode::Update));
    let allow =
        ResolvedBzlmodGraphDiceKey::new(root, inputs(Some("yyy@1.0.0"), LockfileMode::Update));

    assert_ne!(reject, allow);
    assert!(
        reject
            .stable_serialize()
            .contains("env=allow_yanked=reject")
    );
    assert!(
        allow
            .stable_serialize()
            .contains("env=allow_yanked=[yyy@1.0.0]")
    );
}

#[test]
fn resolved_graph_key_changes_when_lockfile_mode_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let update = ResolvedBzlmodGraphDiceKey::new(root.clone(), inputs(None, LockfileMode::Update));
    let error = ResolvedBzlmodGraphDiceKey::new(root, inputs(None, LockfileMode::Error));

    assert_ne!(update, error);
    assert!(update.stable_serialize().contains("mode=update"));
    assert!(error.stable_serialize().contains("mode=error"));
}

#[test]
fn dice_inputs_reject_empty_or_unstable_digests() {
    let env = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();

    let empty = BzlmodDiceInputs::new(
        "",
        "includesabc",
        "registriesabc",
        "lockfileabc",
        LockfileMode::Update,
        env.clone(),
    )
    .unwrap_err();
    assert!(empty.contains("root_module_digest must not be empty"));

    let bad = BzlmodDiceInputs::new(
        "root/abc",
        "includesabc",
        "registriesabc",
        "lockfileabc",
        LockfileMode::Update,
        env,
    )
    .unwrap_err();
    assert!(bad.contains("invalid root_module_digest"));
}
