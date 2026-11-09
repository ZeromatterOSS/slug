use super::*;

#[derive(Default)]
struct ResolutionCompletionTracker {
    completed: Mutex<Vec<(dice::RootActivation, bool)>>,
    packages: Mutex<Vec<String>>,
}

impl ActivationTracker for ResolutionCompletionTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        if let Some(key) = key.downcast_ref::<HostPackageInventoryKey>() {
            self.packages.lock().unwrap().push(key.to_string());
        }
        if let Some(key) = key.downcast_ref::<HostPackageInventoryObservationKey>() {
            self.packages.lock().unwrap().push(key.to_string());
        }
    }

    fn root_completed(&self, key: &DynKey, activation: dice::RootActivation, transient: bool) {
        if key
            .downcast_ref::<ConfiguredToolchainResolutionObservationKey>()
            .is_some()
        {
            self.completed.lock().unwrap().push((activation, transient));
        }
    }
}

#[cfg(feature = "native-probe-observer")]
fn assert_resolution_stage_advanced(
    before: slug_analysis_v2::ToolchainResolutionProbeSnapshot,
    stage: usize,
) {
    let after = slug_analysis_v2::toolchain_resolution_probe_snapshot();
    // The collector is process-wide and is never reset. Other tests may add
    // activity; assert only positive movement of the selected real query cells.
    for index in [0, stage] {
        assert!(
            after.counts[index]
                .checked_sub(before.counts[index])
                .is_some_and(|delta| delta >= 1),
            "resolution stage {index} did not advance"
        );
        assert_eq!(
            (before.overflow_mask | after.overflow_mask) & (1 << index),
            0
        );
    }
}

#[tokio::test]
async fn required_toolchain_declaration_need_repairs_with_strict_closure_without_speculative_implementation()
 {
    let workspace = scratch();
    let bad = workspace.join("speculative/bad");
    fs::create_dir_all(&bad).unwrap();
    let needed = workspace.join("speculative/needed");
    fs::create_dir_all(&needed).unwrap();
    for (relative, source) in [
        (
            "MODULE.bazel",
            "module(name='root')\nbazel_dep(name='speculative',version='1.0')\nlocal_path_override(module_name='speculative',path='speculative')\nregister_toolchains('//:chosen','//:unused')\n",
        ),
        (
            "defs.bzl",
            "def _impl(ctx): return [platform_common.ToolchainInfo(marker='chosen')]\nimpl = rule(implementation=_impl)\n",
        ),
        (
            "BUILD.bazel",
            "load(':defs.bzl','impl')\ntoolchain_type(name='other')\nimpl(name='impl')\ntoolchain(name='chosen',toolchain_type='@speculative//needed:wanted',toolchain=':impl')\ntoolchain(name='unused',toolchain_type=':other',toolchain='@speculative//bad:impl')\n",
        ),
        (
            "speculative/MODULE.bazel",
            "module(name='speculative',version='1.0')\n",
        ),
        ("speculative/REPO.bazel", ""),
        ("speculative/.bazelignore", ""),
        ("speculative/BUILD.bazel", ""),
        (
            "speculative/needed/BUILD.bazel",
            "load(':a.bzl','A')\ntoolchain_type(name='wanted')\n",
        ),
        ("speculative/needed/a.bzl", "A = 1\n"),
        (
            "speculative/bad/BUILD.bazel",
            "load(':outer.bzl','impl')\nimpl(name='impl')\n",
        ),
        (
            "speculative/bad/outer.bzl",
            "load(':a.bzl','A')\nload(':b.bzl','B')\ndef _impl(ctx): return [platform_common.ToolchainInfo()]\nimpl = rule(implementation=_impl)\n",
        ),
        ("speculative/bad/a.bzl", "A = 1\n"),
        ("speculative/bad/b.bzl", "B = (\n"),
    ] {
        fs::write(workspace.join(relative), source).unwrap();
    }
    let key = ConfiguredToolchainResolutionObservationKey::new(
        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
        test_configuration(),
        Arc::from([ToolchainTypeRequirement::new(
            CanonicalLabel::parse("@@speculative+//needed:wanted").unwrap(),
            true,
        )]),
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(ResolutionCompletionTracker::default());
    let a_bytes = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(needed.join("a.bzl")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let full_epoch = root_epoch(&workspace);
    assert!(full_epoch.get(&a_bytes).is_some());
    // Withhold a genuine required declaration input. The malformed unselected
    // implementation is independently present and must never join this frontier.
    let cold_epoch = PathObservationEpoch::from_shared(
        full_epoch
            .observations()
            .iter()
            .filter(|(demand, _)| *demand != &a_bytes)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let request_data = || {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            cycle_detector: Some(analysis_cycle_detector()),
            ..Default::default()
        };
        let host = builtin_fixture::RepositoryHostInputs::prepare(&workspace, &mut data);
        (data, host)
    };
    let (data, host) = request_data();
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    let entries = inject_root_target_inputs_with_entries(
        &mut updater,
        &workspace,
        cold_epoch,
        &[("speculative+", "speculative")],
    );
    let cold = updater.commit().await;
    let mut cold = builtin_fixture::prepare_generated_registrations(cold, &workspace, entries)
        .await
        .unwrap();
    #[cfg(feature = "native-probe-observer")]
    let stages = slug_analysis_v2::toolchain_resolution_probe_snapshot();
    let outcome = cold.compute(&key).await.unwrap();
    #[cfg(feature = "native-probe-observer")]
    assert_resolution_stage_advanced(stages, 7);
    assert!(!ConfiguredToolchainResolutionObservationKey::validity(
        &outcome
    ));
    let AnalysisPreparationOutcome::Need(need) = outcome else {
        panic!("required declaration bytes did not produce Need: {outcome:?}")
    };
    assert!(
        need.path_observations()
            .unwrap()
            .demands()
            .contains(&a_bytes)
    );
    let completed = tracker.completed.lock().unwrap().clone();
    assert_eq!(completed.len(), 1);
    assert!(completed[0].1);
    assert_eq!(completed[0].0.version(), cold.version());
    drop(cold);

    let (data, host) = request_data();
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    let entries = inject_root_target_inputs_with_entries(
        &mut updater,
        &workspace,
        root_epoch(&workspace),
        &[("speculative+", "speculative")],
    );
    let repaired = updater.commit().await;
    let mut repaired =
        builtin_fixture::prepare_generated_registrations(repaired, &workspace, entries)
            .await
            .unwrap();
    let outcome = repaired.compute(&key).await.unwrap();
    assert!(ConfiguredToolchainResolutionObservationKey::validity(
        &outcome
    ));
    let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
        panic!("repaired declaration loading did not complete successfully: {outcome:?}")
    };
    let resolution = result.as_ref().as_ref().unwrap();
    assert_eq!(resolution.rows().len(), 1);
    assert_eq!(
        resolution.rows()[0].declaration(),
        Some(&CanonicalLabel::parse("@@//:chosen").unwrap())
    );
    assert_eq!(
        resolution.rows()[0].implementation(),
        Some(&CanonicalLabel::parse("@@//:impl").unwrap())
    );
    // The implementation parse failure is irrelevant to this declaration graph.
    assert_eq!(fs::read_to_string(bad.join("b.bzl")).unwrap(), "B = (\n");
    let packages = tracker.packages.lock().unwrap().clone();
    assert!(
        packages.iter().all(|package| !package.contains("//bad")),
        "unselected implementation package was activated: {packages:?}"
    );
    let completed = tracker.completed.lock().unwrap().clone();
    assert_eq!(completed.len(), 2);
    let (root, transient) = completed[1];
    assert!(!transient);
    assert_eq!(root.version(), repaired.version());
    let closure = repaired.activation_closure([root.node()]).await.unwrap();
    assert_eq!(closure.version(), repaired.version());
    assert_eq!(closure.roots(), &[root.node()]);
    assert!(
        closure
            .nodes()
            .iter()
            .any(|node| node.node() == root.node())
    );
    drop(repaired);
    fs::remove_dir_all(workspace).unwrap();
}

#[cfg(feature = "native-probe-observer")]
#[tokio::test]
async fn resolution_stage_probe_observes_real_target_platform_need_and_repair() {
    let workspace = scratch();
    fs::create_dir(workspace.join("target")).unwrap();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(workspace.join("BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("target/BUILD.bazel"),
        "platform(name='platform')\n",
    )
    .unwrap();
    let target = CanonicalLabel::parse("@@//target:platform").unwrap();
    let configuration = ConfigurationKey::from_slug(
        test_configuration()
            .slug_configuration()
            .unwrap()
            .with_transition_target_platform(Some(&target))
            .unwrap(),
    );
    let key = ConfiguredToolchainResolutionObservationKey::new(
        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
        configuration,
        Arc::from([]),
    )
    .unwrap();
    let tracker = Arc::new(ResolutionCompletionTracker::default());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let request_data = || {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            cycle_detector: Some(analysis_cycle_detector()),
            ..Default::default()
        };
        let host = builtin_fixture::RepositoryHostInputs::prepare(&workspace, &mut data);
        (data, host)
    };
    let (data, host) = request_data();
    let omitted = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("target")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let full = root_epoch(&workspace);
    assert!(full.get(&omitted).is_some());
    let cold = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| *d != &omitted)
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    let entries = inject_root_target_inputs_with_entries(&mut updater, &workspace, cold, &[]);
    let tx = updater.commit().await;
    let mut cold = builtin_fixture::prepare_generated_registrations(tx, &workspace, entries)
        .await
        .unwrap();
    let before = slug_analysis_v2::toolchain_resolution_probe_snapshot();
    let outcome = cold.compute(&key).await.unwrap();
    assert_resolution_stage_advanced(before, 1);
    let AnalysisPreparationOutcome::Need(needs) = &outcome else {
        panic!("missing target platform observation must suspend: {outcome:?}")
    };
    assert!(
        needs
            .path_observations()
            .unwrap()
            .demands()
            .contains(&omitted)
    );
    assert!(!ConfiguredToolchainResolutionObservationKey::validity(
        &outcome
    ));
    drop(cold);

    let (data, host) = request_data();
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    let entries = inject_root_target_inputs_with_entries(
        &mut updater,
        &workspace,
        root_epoch(&workspace),
        &[],
    );
    let tx = updater.commit().await;
    let mut repaired = builtin_fixture::prepare_generated_registrations(tx, &workspace, entries)
        .await
        .unwrap();
    let outcome = repaired.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
        panic!("target platform repair must complete")
    };
    let resolution = result.as_ref().as_ref().unwrap();
    assert_eq!(resolution.target_platform().actual().label(), &target);
    assert!(resolution.rows().is_empty());
    let (root, transient) = tracker.completed.lock().unwrap().last().copied().unwrap();
    assert!(!transient);
    let closure = repaired.activation_closure([root.node()]).await.unwrap();
    assert_eq!(closure.version(), repaired.version());
    assert_eq!(closure.roots(), &[root.node()]);
    drop(repaired);
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(workspace).unwrap();
}
