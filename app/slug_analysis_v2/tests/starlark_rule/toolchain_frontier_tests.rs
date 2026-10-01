use super::*;

#[derive(Default)]
struct ResolutionCompletionTracker {
    completed: Mutex<Vec<(dice::RootActivation, bool)>>,
}

impl ActivationTracker for ResolutionCompletionTracker {
    fn key_activated(
        &self,
        _key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
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

#[tokio::test]
async fn speculative_toolchain_package_need_repairs_to_success_with_strict_closure() {
    let workspace = scratch();
    let bad = workspace.join("speculative/bad");
    fs::create_dir_all(&bad).unwrap();
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
            "load(':defs.bzl','impl')\ntoolchain_type(name='wanted')\ntoolchain_type(name='other')\nimpl(name='impl')\ntoolchain(name='chosen',toolchain_type=':wanted',toolchain=':impl')\ntoolchain(name='unused',toolchain_type=':other',toolchain='@speculative//bad:impl')\n",
        ),
        (
            "speculative/MODULE.bazel",
            "module(name='speculative',version='1.0')\n",
        ),
        ("speculative/REPO.bazel", ""),
        ("speculative/.bazelignore", ""),
        ("speculative/BUILD.bazel", ""),
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
            CanonicalLabel::parse("@@//:wanted").unwrap(),
            true,
        )]),
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(ResolutionCompletionTracker::default());
    let a_bytes = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(bad.join("a.bzl")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let full_epoch = root_epoch(&workspace);
    assert!(full_epoch.get(&a_bytes).is_some());
    // Only A's bytes are withheld: its file metadata and B's real malformed
    // source remain observed. The imports retain their genuine AST order.
    let cold_epoch = PathObservationEpoch::from_shared(
        full_epoch
            .observations()
            .iter()
            .filter(|(demand, _)| *demand != &a_bytes)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let data = || UserComputationData {
        activation_tracker: Some(tracker.dupe()),
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    };
    let mut updater = dice.updater_with_data(data());
    inject_root_target_inputs(
        &mut updater,
        &workspace,
        cold_epoch,
        &[("speculative+", "speculative")],
    );
    let mut cold = updater.commit().await;
    let outcome = cold.compute(&key).await.unwrap();
    assert!(!ConfiguredToolchainResolutionObservationKey::validity(
        &outcome
    ));
    let AnalysisPreparationOutcome::Need(need) = outcome else {
        panic!("earlier missing bytes were obscured by later parse failure: {outcome:?}")
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

    let mut updater = dice.updater_with_data(data());
    inject_root_target_inputs(
        &mut updater,
        &workspace,
        root_epoch(&workspace),
        &[("speculative+", "speculative")],
    );
    let mut repaired = updater.commit().await;
    let outcome = repaired.compute(&key).await.unwrap();
    assert!(ConfiguredToolchainResolutionObservationKey::validity(
        &outcome
    ));
    let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
        panic!("repaired speculative loading did not complete successfully: {outcome:?}")
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
    // B's parse failure remains in the unselected implementation inventory;
    // repaired A is enough for the normal successful resolution closure.
    assert_eq!(fs::read_to_string(bad.join("b.bzl")).unwrap(), "B = (\n");
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
