use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;

use super::*;

const SHARED_WRITE_DEFS: &str = r#"def _write(ctx):
    out = ctx.actions.declare_file(ctx.attr.out)
    ctx.actions.write(out, ctx.attr.content, is_executable = ctx.attr.executable)
    return [DefaultInfo(files = depset([out]))]
write = rule(implementation = _write, attrs = {
    "content": attr.string(),
    "executable": attr.bool(default = False),
    "out": attr.string(default = "shared.txt"),
})
"#;

fn shared_write_epoch(revision: i64, left: &str, right: &str) -> PathObservationEpoch {
    shared_write_epoch_with_executable(revision, left, right, false, false)
}

fn shared_write_epoch_with_executable(
    revision: i64,
    left: &str,
    right: &str,
    left_executable: bool,
    right_executable: bool,
) -> PathObservationEpoch {
    shared_write_epoch_with_paths(
        revision,
        left,
        right,
        left_executable,
        right_executable,
        "shared.txt",
        "shared.txt",
    )
}

fn shared_write_epoch_with_paths(
    revision: i64,
    left: &str,
    right: &str,
    left_executable: bool,
    right_executable: bool,
    left_path: &str,
    right_path: &str,
) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name = \"root\")\nregister_execution_platforms(\"//:platform\")\n",
        revision,
    );
    epoch.file("/workspace/defs.bzl", SHARED_WRITE_DEFS, revision);
    epoch.package(
        "",
        &format!(
            "load(\":defs.bzl\", \"write\")\nplatform(name = \"platform\")\nwrite(name = \"left\", content = \"{left}\", executable = {}, out = \"{left_path}\")\nwrite(name = \"right\", content = \"{right}\", executable = {}, out = \"{right_path}\")\n",
            if left_executable { "True" } else { "False" },
            if right_executable { "True" } else { "False" },
        ),
        revision,
    );
    epoch.build()
}

fn shared_write_key(targets: &[&str]) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &targets
            .iter()
            .map(|target| TargetPattern::parse(target).unwrap())
            .collect::<Vec<_>>(),
        build_test_configuration("target"),
    )
    .unwrap()
}

#[tokio::test]
async fn configured_action_conflicts_equal_filewrites_keep_owners_but_share_execution() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = shared_write_key(&["//:left", "//:right"]);
    let mut transaction =
        build_root_transaction(&dice, shared_write_epoch(1, "same", "same")).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let evaluation = complete_build_evaluation(&outcome);
    let semantic = evaluation
        .resolved_file_write_semantic_views_in_closure()
        .unwrap();
    let execution = evaluation
        .resolved_file_write_execution_views_in_closure()
        .unwrap();
    assert_eq!(semantic.len(), 2);
    assert_eq!(execution.len(), 1);
    assert_ne!(semantic[0].action().owner(), semantic[1].action().owner());
    assert_eq!(semantic[0].action().output().path(), "shared.txt");
}

#[tokio::test]
async fn configured_action_conflicts_content_is_rootset_scoped_and_repeats_warm() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let separate = shared_write_key(&["//:left"]);
    let combined = shared_write_key(&["//:left", "//:right"]);
    let epoch = shared_write_epoch(2, "left", "right");
    let mut separate_transaction = build_root_transaction(&dice, epoch.clone()).await;
    assert!(
        complete_build_evaluation(&separate_transaction.compute(&separate).await.unwrap())
            .resolved_file_write_semantic_views()
            .is_ok()
    );
    for _ in 0..2 {
        let mut transaction = build_root_transaction(&dice, epoch.clone()).await;
        let outcome = transaction.compute(&combined).await.unwrap();
        let PreparationOutcome::Complete(value) = outcome else {
            panic!("conflicting root set retained Needs")
        };
        assert!(matches!(
            value.as_ref().as_ref().unwrap_err().kind,
            BuildCommandErrorKind::ActionClosure(
                ConfiguredActionClosureError::OutputConflict { .. }
            )
        ));
    }
}

#[tokio::test]
async fn configured_action_conflicts_executable_bit_is_not_shareable() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = shared_write_key(&["//:left", "//:right"]);
    let mut transaction = build_root_transaction(
        &dice,
        shared_write_epoch_with_executable(3, "same", "same", false, true),
    )
    .await;
    let outcome = transaction.compute(&key).await.unwrap();
    let PreparationOutcome::Complete(value) = outcome else {
        panic!("executable conflict retained Needs")
    };
    assert!(matches!(
        value.as_ref().as_ref().unwrap_err().kind,
        BuildCommandErrorKind::ActionClosure(ConfiguredActionClosureError::OutputConflict { .. })
    ));
}

#[tokio::test]
async fn configured_action_conflicts_distinct_paths_do_not_share_or_reject() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = shared_write_key(&["//:left", "//:right"]);
    let mut transaction = build_root_transaction(
        &dice,
        shared_write_epoch_with_paths(4, "same", "same", false, false, "left.txt", "right.txt"),
    )
    .await;
    let outcome = transaction.compute(&key).await.unwrap();
    let evaluation = complete_build_evaluation(&outcome);
    assert_eq!(
        evaluation
            .resolved_file_write_semantic_views_in_closure()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        evaluation
            .resolved_file_write_execution_views_in_closure()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn configured_action_conflicts_cross_group_spawns_reject_before_publication() {
    let mut epoch = BuildRootEpoch::base(5);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name = 'root')\nregister_execution_platforms('//:platform')\n",
        5,
    );
    epoch.file(
        "/workspace/defs.bzl",
        r#"def _default(ctx):
    out = ctx.actions.declare_file('shared.txt')
    ctx.actions.run(outputs = [out], executable = 'tool')
    return [DefaultInfo(files = depset([out]))]
def _named(ctx):
    out = ctx.actions.declare_file('shared.txt')
    ctx.actions.run(outputs = [out], executable = 'tool', exec_group = 'named')
    return [DefaultInfo(files = depset([out]))]
default_rule = rule(implementation = _default)
named_rule = rule(implementation = _named, exec_groups = {'named': exec_group()})
"#,
        5,
    );
    epoch.package(
        "",
        "load(':defs.bzl', 'default_rule', 'named_rule')\nplatform(name = 'platform')\ndefault_rule(name = 'left')\nnamed_rule(name = 'right')\n",
        5,
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(
        slug_configuration_v2::native::host::ActionEnvironmentHost::without_environment(
            slug_configuration_v2::native::host::ActionEnvironmentHostOs::Linux,
        ),
    );
    let configuration = ConfigurationKey::from_slug(
        SlugConfiguration::default_target(&host)
            .unwrap()
            .with_host_platform_label(&CanonicalLabel::parse("@@//.slug_test_host:host").unwrap()),
    );
    let key = BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[
            TargetPattern::parse("//:left").unwrap(),
            TargetPattern::parse("//:right").unwrap(),
        ],
        configuration,
    )
    .unwrap();
    let mut transaction = build_root_transaction(&dice, epoch.build()).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let PreparationOutcome::Complete(value) = outcome else {
        panic!("cross-group conflict retained Needs")
    };
    let error = value.as_ref().as_ref().unwrap_err();
    assert!(
        matches!(
            error.kind,
            BuildCommandErrorKind::ActionClosure(
                ConfiguredActionClosureError::UnsupportedEquivalence { .. }
            )
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn configured_action_conflicts_concurrent_rootsets_restore_without_poisoning() {
    // OutputArtifactConflictTest new/overlapping roots and invalidation themes.
    // All concurrent requests use the same observed revision; old results are
    // held as immutable values, not reconstructed historical Host snapshots.
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let left = shared_write_key(&["//:left"]);
    let right = shared_write_key(&["//:right"]);
    let both = shared_write_key(&["//:left", "//:right"]);
    let mut initial = build_root_transaction(&dice, shared_write_epoch(10, "same", "same")).await;
    let a = initial.compute(&both).await.unwrap();
    let held = complete_build_evaluation(&a);
    assert_eq!(
        held.resolved_file_write_execution_views_in_closure()
            .unwrap()
            .len(),
        1
    );

    let mut combined =
        build_root_transaction(&dice, shared_write_epoch(11, "same", "changed")).await;
    let mut left_request = combined.dupe();
    let mut right_request = combined.dupe();
    let (l, r, conflict) = tokio::join!(
        left_request.compute(&left),
        right_request.compute(&right),
        combined.compute(&both),
    );
    for result in [l.unwrap(), r.unwrap()] {
        assert_eq!(
            complete_build_evaluation(&result)
                .resolved_file_write_execution_views_in_closure()
                .unwrap()
                .len(),
            1
        );
    }
    let conflict = conflict.unwrap();
    let PreparationOutcome::Complete(error) = &conflict else {
        panic!("conflict retained Needs")
    };
    assert!(matches!(
        &error.as_ref().as_ref().unwrap_err().kind,
        BuildCommandErrorKind::ActionClosure(ConfiguredActionClosureError::OutputConflict { .. })
    ));
    let warm_conflict = combined.compute(&both).await.unwrap();
    assert!(BuildCommandRootKey::equality(&conflict, &warm_conflict));
    if let PreparationOutcome::Complete(warm) = &warm_conflict {
        assert!(Arc::ptr_eq(error, warm));
    }

    let mut restored = build_root_transaction(&dice, shared_write_epoch(12, "same", "same")).await;
    let restored_value = restored.compute(&both).await.unwrap();
    let restored_views = complete_build_evaluation(&restored_value)
        .resolved_file_write_execution_views_in_closure()
        .unwrap();
    let held_views = held
        .resolved_file_write_execution_views_in_closure()
        .unwrap();
    assert_eq!(restored_views.len(), 1);
    assert_eq!(held_views.len(), 1);
    assert_eq!(
        crate::runtime::FileWriteSemanticIdentity::from_resolved(&restored_views[0]).unwrap(),
        crate::runtime::FileWriteSemanticIdentity::from_resolved(&held_views[0]).unwrap(),
    );
    // Held conflict remains an error after restoration; an independent request
    // never installs a global collision registry that could poison restored A.
    assert!(error.is_err());
    assert_eq!(
        held.resolved_file_write_semantic_views_in_closure()
            .unwrap()
            .len(),
        2
    );
}

fn shared_write_platform_epoch(
    revision: i64,
    raw: &str,
    message: Option<&str>,
    toolchain: bool,
) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    let registrations = if toolchain {
        "register_toolchains('//:registration')\n"
    } else {
        ""
    };
    epoch.file(
        "/workspace/MODULE.bazel",
        &format!(
            "module(name='root')\nregister_execution_platforms('//:platform')\n{registrations}"
        ),
        revision,
    );
    let defs = if toolchain {
        SHARED_WRITE_DEFS.replace("})", "}, toolchains=['//:kind'])")
    } else {
        SHARED_WRITE_DEFS.to_owned()
    };
    epoch.file("/workspace/defs.bzl", &format!("{defs}\ndef _tc(ctx): return [platform_common.ToolchainInfo()]\nimplementation=rule(implementation=_tc)\n"), revision);
    let declarations = if toolchain {
        "toolchain_type(name='kind')\nimplementation(name='impl')\ntoolchain(name='registration',toolchain_type=':kind',toolchain=':impl')\n"
    } else {
        ""
    };
    let message = message
        .map(|text| format!(", missing_toolchain_error={text:?}"))
        .unwrap_or_default();
    epoch.package("", &format!(
        "load(':defs.bzl', 'write', 'implementation')\nplatform(name='platform', exec_properties={{'cpu':{raw:?}}}{message})\n{declarations}write(name='left',content='same',exec_properties={{'cpu':'masked'}})\nwrite(name='right',content='same',exec_properties={{'cpu':'masked'}})\n"
    ), revision);
    epoch.build()
}

#[tokio::test]
async fn configured_action_conflicts_loaded_raw_facts_and_message_cutoff_are_exact() {
    // PlatformInfo.addTo and ActionKeyComputer: raw platform properties and
    // normalized missing_toolchain_error remain inputs even when owner property
    // overrides hide a platform edit. Exercise loaded facts, not hand-built ones.
    const DEFAULT_MESSAGE: &str = "For more information on platforms or toolchains see https://bazel.build/concepts/platforms-intro.";
    for toolchain in [false, true] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = shared_write_key(&["//:left", "//:right"]);
        let cases = [
            ("raw-a", None, Some(DEFAULT_MESSAGE)),
            ("raw-b", None, Some(DEFAULT_MESSAGE)),
            ("raw-a", None, Some(DEFAULT_MESSAGE)),
            ("raw-a", Some(""), None),
            ("raw-a", Some("custom message"), Some("custom message")),
            ("raw-a", Some(""), None),
            ("raw-a", Some(DEFAULT_MESSAGE), Some(DEFAULT_MESSAGE)),
            ("raw-a", None, Some(DEFAULT_MESSAGE)),
        ];
        let mut values = Vec::new();
        let mut identities = Vec::new();
        for (index, (raw, message, expected)) in cases.iter().enumerate() {
            let epoch = shared_write_platform_epoch(20 + index as i64, raw, *message, toolchain);
            let mut transaction = build_root_transaction(&dice, epoch).await;
            let value = transaction.compute(&key).await.unwrap();
            let evaluation = complete_build_evaluation(&value);
            let views = evaluation
                .resolved_file_write_execution_views_in_closure()
                .unwrap();
            assert_eq!(views.len(), 1);
            let view = &views[0];
            assert_eq!(view.action().toolchain().is_some(), toolchain);
            assert_eq!(view.platform_fact().exec_properties[0].1.as_str(), "masked");
            assert_eq!(view.raw_platform_fact().exec_properties[0].1.as_str(), *raw);
            assert_eq!(
                view.raw_platform_fact().missing_toolchain_error.as_deref(),
                *expected
            );
            assert_eq!(
                view.platform_fact().missing_toolchain_error.as_deref(),
                *expected
            );
            identities
                .push(crate::runtime::FileWriteSemanticIdentity::from_resolved(view).unwrap());
            let warm = transaction.compute(&key).await.unwrap();
            assert!(BuildCommandRootKey::equality(&value, &warm));
            if let (PreparationOutcome::Complete(a), PreparationOutcome::Complete(b)) =
                (&value, &warm)
            {
                assert!(Arc::ptr_eq(a, b));
            }
            values.push(value);
        }
        for (a, b) in [(0, 1), (0, 3), (3, 4), (4, 6)] {
            assert_ne!(identities[a], identities[b]);
            assert!(!BuildCommandRootKey::equality(&values[a], &values[b]));
        }
        for (a, b) in [(0, 2), (3, 5), (0, 6)] {
            assert_eq!(identities[a], identities[b]);
        }
        // Same default value written explicitly must cut off at the retained
        // configured action context even though its loaded BUILD syntax differs.
        let context = |index| {
            complete_build_evaluation(&values[index])
                .resolved_file_write_execution_views_in_closure()
                .unwrap()[0]
                .action()
                .context()
                .clone()
        };
        assert_eq!(context(0), context(6));
        assert_eq!(identities[6], identities[7]);
        assert!(
            Arc::ptr_eq(&context(6), &context(7)),
            "unchanged loaded platform fact must cut off"
        );
        assert_ne!(context(0), context(1));
        assert_ne!(context(3), context(4));
    }
}
