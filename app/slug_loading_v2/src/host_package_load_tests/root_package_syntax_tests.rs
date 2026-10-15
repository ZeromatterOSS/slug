//! Normal BUILD parents own source freshness; their pure syntax child owns sharing.
use std::sync::Weak;

use slug_bzlmod_v2::RootPackageSourceObservationKey;

use super::super::ExternalBzlModuleError;
use super::super::HostRootBzlLabel;
use super::super::RootPackageBzlTarget;
use super::super::RootPackageLoadDiagnostic;
use super::super::RootPackageLoadError;
use super::super::RootPackageLoadErrorInner;
use super::super::root_package_load_origin;
use super::super::root_package_syntax::RootPackageSyntax;
use super::super::root_package_syntax::RootPackageSyntaxKey;
use super::super::root_package_syntax::RootPackageSyntaxOutcome;
use super::*;

const BUILD_A: &str = "load(':one.bzl', 'NAME')\nprint('settled BUILD')\nexports_files([NAME])\n";
const BUILD_B: &str =
    "load(':one.bzl', 'NAME')\nprint('settled BUILD')\nexports_files([NAME, 'b.txt'])\n";

#[test]
fn root_package_origin_reuses_only_within_driver_and_preserves_error_lifetime() {
    for (package, path, expected) in [
        ("", b"BUILD".as_slice(), "BUILD"),
        ("", b"BUILD.bazel".as_slice(), "BUILD.bazel"),
        ("pkg", b"BUILD".as_slice(), "pkg/BUILD"),
        ("pkg", b"BUILD.bazel".as_slice(), "pkg/BUILD.bazel"),
        ("", b"B\x80\xff".as_slice(), "B\u{80}\u{ff}"),
        ("pkg", b"B\x80\xff".as_slice(), "pkg/B\u{80}\u{ff}"),
    ] {
        let package = PackagePath::parse(package).unwrap();
        let mut cache = None;
        assert!(cache.is_none());
        let first = root_package_load_origin(&mut cache, &package, path);
        let again = root_package_load_origin(&mut cache, &package, path);
        assert_eq!(first.as_ref(), expected);
        assert!(Arc::ptr_eq(&first, &again));
        let mut other_cache = None;
        let other = root_package_load_origin(&mut other_cache, &package, path);
        assert_eq!(first, other);
        assert!(!Arc::ptr_eq(&first, &other));
        let weak = Arc::downgrade(&first);
        drop((first, again));
        assert!(weak.upgrade().is_some());
        drop(cache);
        assert!(weak.upgrade().is_none());
        assert_eq!(other.as_ref(), expected);
    }
    let package = PackagePath::parse("pkg").unwrap();
    let root_label = HostRootBzlLabel::new(
        package.clone(),
        RootPackageBzlTarget::parse("root.bzl").unwrap(),
    );
    let external_label = CanonicalLabel::parse("@@repo+//pkg:ext.bzl").unwrap();
    let root_cause = Arc::new(HostBzlModuleError::Parse {
        label: root_label.clone(),
        message: "ROOT_CAUSE".into(),
    });
    let external_cause = Arc::new(ExternalBzlModuleError::Evaluation {
        label: external_label.clone(),
        message: "EXT_CAUSE".into(),
    });
    let root_weak = Arc::downgrade(&root_cause);
    let external_weak = Arc::downgrade(&external_cause);
    let mut cache = None;
    let root_origin = root_package_load_origin(&mut cache, &package, b"BUILD");
    let origin_weak = Arc::downgrade(&root_origin);
    let root = RootPackageLoadError::new(RootPackageLoadErrorInner::Bzl {
        origin: root_origin,
        load: ":root.bzl".into(),
        label: root_label,
        error: root_cause,
    });
    let external = RootPackageLoadError::new(RootPackageLoadErrorInner::ExternalBzl {
        origin: root_package_load_origin(&mut cache, &package, b"BUILD"),
        load: "@repo//pkg:ext.bzl".into(),
        label: external_label,
        error: external_cause,
    });
    drop(cache);
    let RootPackageLoadDiagnostic::Bzl {
        origin,
        load,
        error,
    } = root.diagnostic_view()
    else {
        panic!("root carrier")
    };
    assert_eq!((origin, load), ("pkg/BUILD", ":root.bzl"));
    assert!(std::ptr::eq(error, root_weak.as_ptr()));
    let root_origin_pointer = origin.as_ptr();
    let RootPackageLoadDiagnostic::ExternalBzl {
        origin,
        load,
        error,
        ..
    } = external.diagnostic_view()
    else {
        panic!("external carrier")
    };
    assert_eq!((origin, load), ("pkg/BUILD", "@repo//pkg:ext.bzl"));
    assert_eq!(origin.as_ptr(), root_origin_pointer);
    assert!(std::ptr::eq(error, external_weak.as_ptr()));
    assert_eq!(
        root.diagnostic().to_string(),
        "bzl: pkg/BUILD load :root.bzl: RootBzl: Parse: ROOT_CAUSE"
    );
    assert_eq!(
        external.diagnostic().to_string(),
        "external_bzl: pkg/BUILD load @repo//pkg:ext.bzl @@repo+//pkg:ext.bzl: ExternalBzl: Evaluation: EXT_CAUSE"
    );
    drop(root);
    assert!(root_weak.upgrade().is_none());
    assert!(origin_weak.upgrade().is_some() && external_weak.upgrade().is_some());
    drop(external);
    assert!(origin_weak.upgrade().is_none() && external_weak.upgrade().is_none());
}

#[derive(Default)]
struct SyntaxTracker {
    events: EventTracker,
    syntax: Mutex<
        Vec<(
            RootPackageSyntaxKey,
            ActivationKind,
            Weak<RootPackageSyntax>,
        )>,
    >,
}
impl ActivationTracker for SyntaxTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        dependencies: &mut dyn Iterator<Item = &DynKey>,
        data: ActivationData,
    ) {
        self.events.key_activated(key, dependencies, data);
    }
    fn tracks_rich_activations(&self) -> bool {
        true
    }
    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        if let Some(key) = key.downcast_ref::<RootPackageSyntaxKey>() {
            let weak = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<Weak<RootPackageSyntax>>())
                .cloned()
                .unwrap_or_default();
            self.syntax
                .lock()
                .unwrap()
                .push((key.clone(), activation.kind(), weak));
        }
        self.events.key_activated_rich(key, activation);
    }
}
impl SyntaxTracker {
    fn evaluations(&self) -> usize {
        self.syntax
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, kind, _)| *kind == ActivationKind::Evaluated)
            .count()
    }
    fn value(&self) -> Arc<RootPackageSyntax> {
        self.syntax
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(_, _, weak)| weak.upgrade())
            .expect("normal parent owns live syntax")
    }
}
async fn tracked(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<SyntaxTracker>,
) -> DiceTransaction {
    let mut data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    transaction(dice, epoch, true, None)
        .await
        .into_updater()
        .commit_with_data(data)
        .await
}
fn child_bytes() -> PathObservationDemand {
    EpochBuilder::demand(
        "/workspace/pkg/one.bzl",
        PathObservationOperation::FileBytes,
    )
}
fn fixture(build: &str, child: bool, unrelated: i64) -> PathObservationEpoch {
    let mut epoch =
        EpochBuilder::workspace_sources("", build, &[("one.bzl", "NAME = 'a.txt'\n")], 981);
    if !child {
        assert!(epoch.entries.shift_remove(&child_bytes()).is_some());
    }
    epoch.file(
        "/workspace/unrelated.bzl",
        format!("X = {unrelated}\n"),
        unrelated,
    );
    epoch.build()
}
// Preserve the actual original Arc owner for unchanged fixture observations.
fn revision(
    base: &PathObservationEpoch,
    build: &str,
    child: bool,
    unrelated: i64,
) -> PathObservationEpoch {
    let fresh = fixture(build, child, unrelated);
    PathObservationEpoch::from_shared(fresh.observations().iter().map(|(demand, result)| {
        let original = base.get(demand).filter(|original| *original == result);
        (demand.dupe(), original.unwrap_or(result).dupe())
    }))
    .unwrap()
}
fn names(value: &ObservedHostPackageOutcome) -> Vec<String> {
    observed_package(value)
        .result()
        .as_ref()
        .as_ref()
        .unwrap()
        .targets
        .iter()
        .map(|target| target.name.to_string())
        .collect()
}

#[tokio::test]
async fn observed_root_package_syntax_reuses_across_child_need_and_source_revisions() {
    let dice = host_fixture_dice();
    let tracker = Arc::new(SyntaxTracker::default());
    let key = observed_package_key();
    let base = fixture(BUILD_A, false, 1);
    let mut first = tracked(&dice, base.dupe(), tracker.dupe()).await;
    let need = first.compute(&key).await.unwrap();
    assert!(
        matches!(&need, LoadingPreparationOutcome::Need(n) if n.path_observations().unwrap().demands() == &[child_bytes()])
    );
    assert!(!RootPackageLoadObservationKey::validity(&need));
    let a = tracker.value();
    let loads = a.result.as_ref().unwrap();
    assert_eq!(loads.as_ref(), [":one.bzl"]);
    assert_eq!(tracker.evaluations(), 1);
    let source = first
        .compute(&RootPackageSourceObservationKey::for_build(
            workspace(),
            PackagePath::parse("pkg").unwrap(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
        panic!("completed BUILD source");
    };
    assert!(Arc::ptr_eq(
        source.result().as_ref().as_ref().unwrap().bytes(),
        &a.bytes
    ));
    let batches = tracker.events.take();
    assert!(
        batches.iter().any(|batch| batch.key == key.to_string()),
        "normal Need parent activation is observed"
    );
    assert!(
        batches
            .iter()
            .filter(|batch| batch.key == key.to_string())
            .all(|batch| batch.batch.is_none()),
        "Need publishes no parent event batch"
    );
    drop(source);
    drop(first);
    dice.wait_for_idle().await;
    let mut again = tracked(&dice, revision(&base, BUILD_A, false, 2), tracker.dupe()).await;
    assert!(matches!(
        again.compute(&key).await.unwrap(),
        LoadingPreparationOutcome::Need(_)
    ));
    assert_eq!(
        tracker.evaluations(),
        1,
        "child-only/unrelated revisions do not reparse BUILD"
    );
    let reused = tracker.value();
    assert!(Arc::ptr_eq(&a, &reused));
    assert!(Arc::ptr_eq(loads, reused.result.as_ref().unwrap()));
    assert!(
        tracker
            .syntax
            .lock()
            .unwrap()
            .iter()
            .any(|(_, kind, _)| *kind == ActivationKind::Reused)
    );
    let batches = tracker.events.take();
    assert!(
        batches.iter().any(|batch| batch.key == key.to_string()),
        "repeated Need parent activation is observed"
    );
    assert!(
        batches
            .iter()
            .filter(|batch| batch.key == key.to_string())
            .all(|batch| batch.batch.is_none()),
        "repeated Need publishes no parent event batch"
    );
    drop(again);
    dice.wait_for_idle().await;
    let mut repaired = tracked(&dice, revision(&base, BUILD_A, true, 2), tracker.dupe()).await;
    let terminal = repaired.compute(&key).await.unwrap();
    assert_eq!(names(&terminal), ["a.txt"]);
    assert_eq!(tracker.evaluations(), 1);
    let epoch = repaired.compute(&PathObservationEpochKey).await.unwrap();
    for (demand, result) in observed_package(&terminal).observations().observations() {
        assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
    }
    assert!(
        observed_package(&terminal)
            .observations()
            .get(&child_bytes())
            .is_some()
    );
    let batches = tracker.events.take();
    assert!(
        batches.iter().any(|entry| entry.key == key.to_string()),
        "settled parent activation is observed after draining Need rows"
    );
    for events in batches.iter().filter(|entry| entry.key == key.to_string()) {
        assert_eq!(
            event_texts(
                events
                    .batch
                    .as_ref()
                    .expect("settled parent publishes its Complete event batch")
            ),
            ["settled BUILD"]
        );
    }
    drop(terminal);
    drop(epoch);
    drop(repaired);
    let mut changed = tracked(&dice, revision(&base, BUILD_B, true, 2), tracker.dupe()).await;
    assert_eq!(
        names(&changed.compute(&key).await.unwrap()),
        ["a.txt", "b.txt"]
    );
    let b = tracker.value();
    assert_eq!(a.result, b.result, "load-only identity is insufficient");
    assert_ne!(a, b);
    assert_eq!(tracker.evaluations(), 2);
    drop(changed);
    let mut restored = tracked(&dice, revision(&base, BUILD_A, true, 2), tracker.dupe()).await;
    assert_eq!(names(&restored.compute(&key).await.unwrap()), ["a.txt"]);
    assert_eq!(tracker.value(), a);
}

#[tokio::test]
async fn root_package_syntax_preserves_exact_source_identity_and_retirement() {
    let dice = host_fixture_dice();
    let tracker = Arc::new(SyntaxTracker::default());
    let key = observed_package_key();
    let mut tx = tracked(&dice, fixture(BUILD_A, false, 1), tracker.dupe()).await;
    let need = tx.compute(&key).await.unwrap();
    assert!(matches!(&need, LoadingPreparationOutcome::Need(_)));
    let syntax = tracker.value();
    let original_key = RootPackageSyntaxKey {
        package: package_key(),
        mode: HostPackageLoadMode::Observed,
    };
    let mut changed_key = original_key.clone();
    changed_key.mode = HostPackageLoadMode::Legacy;
    assert_ne!(original_key, changed_key);
    changed_key = original_key.clone();
    changed_key.package.package = PackagePath::parse("other").unwrap();
    assert_ne!(original_key, changed_key);
    changed_key = original_key.clone();
    changed_key.package.workspace = NormalizedAbsolutePath::new("/other").unwrap();
    assert_ne!(original_key, changed_key);
    let defensive: RootPackageSyntaxOutcome = Err(need.map(|_| unreachable!()));
    assert!(!RootPackageSyntaxKey::validity(&defensive));
    assert!(!RootPackageSyntaxKey::equality(&defensive, &defensive));
    drop(defensive);
    drop(tx);

    // Independent source-key graph supplies equal bytes with distinct actual storage.
    let other_dice = host_fixture_dice();
    let mut other = transaction(&other_dice, fixture(BUILD_A, false, 1), false, None).await;
    let source_key =
        RootPackageSourceObservationKey::for_build(workspace(), PackagePath::parse("pkg").unwrap());
    let LoadingPreparationOutcome::Complete(Ok(source)) = other.compute(&source_key).await.unwrap()
    else {
        panic!("independent original BUILD source");
    };
    let equal = RootPackageSyntax::from_source(
        source.result().as_ref().as_ref().unwrap(),
        &PackagePath::parse("pkg").unwrap(),
    );
    assert!(!Arc::ptr_eq(&syntax.bytes, &equal.bytes));
    assert_eq!(*syntax, equal);
    drop(equal);
    drop(source);
    drop(other);
    other_dice.wait_for_idle().await;
    drop(other_dice);

    // BUILD selection changes exact filename even when bytes and ordered loads are equal.
    let mut renamed_epoch =
        EpochBuilder::workspace_sources("", BUILD_A, &[("one.bzl", "NAME = 'a.txt'\n")], 981);
    renamed_epoch.missing("/workspace/pkg/BUILD.bazel");
    renamed_epoch.file("/workspace/pkg/BUILD", BUILD_A, 981);
    let mut renamed = tracked(&dice, renamed_epoch.build(), tracker.dupe()).await;
    assert_eq!(names(&renamed.compute(&key).await.unwrap()), ["a.txt"]);
    let renamed_syntax = tracker.value();
    assert_eq!(renamed_syntax.bytes, syntax.bytes);
    assert_eq!(renamed_syntax.result, syntax.result);
    assert_ne!(renamed_syntax.logical_path, syntax.logical_path);
    assert_ne!(renamed_syntax.relative_build, syntax.relative_build);
    assert_ne!(renamed_syntax, syntax);
    drop(renamed_syntax);
    drop(renamed);

    // Pure errors precede child Need and are source-bound cache-valid metadata.
    for (body, parse) in [
        (b"\xff".as_slice(), false),
        (
            b"load(':one.bzl', 'NAME')\nexports_files(\n".as_slice(),
            true,
        ),
    ] {
        let mut epoch =
            EpochBuilder::workspace_sources("", BUILD_A, &[("one.bzl", "NAME = 'a.txt'\n")], 981);
        epoch.file("/workspace/pkg/BUILD.bazel", body, 981);
        assert!(epoch.entries.shift_remove(&child_bytes()).is_some());
        let mut failed = tracked(&dice, epoch.build(), tracker.dupe()).await;
        let result = failed.compute(&key).await.unwrap();
        let error = observed_package(&result)
            .result()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert!(if parse {
            matches!(error.inner, RootPackageLoadErrorInner::Parse { .. })
        } else {
            matches!(error.inner, RootPackageLoadErrorInner::Input(_))
        });
        assert!(
            !observed_package(&result)
                .observations()
                .get(&child_bytes())
                .is_some()
        );
        let pure = tracker.value();
        assert!(RootPackageSyntaxKey::validity(&Ok(pure.dupe())));
        let before = tracker.evaluations();
        assert!(RootPackageLoadObservationKey::equality(
            &result,
            &failed.compute(&key).await.unwrap()
        ));
        assert_eq!(tracker.evaluations(), before);
    }
    // A previously usable syntax can never supply absent current BUILD authority.
    let mut absent = EpochBuilder::workspace_sources("", BUILD_A, &[], 981);
    assert!(
        absent
            .entries
            .shift_remove(&EpochBuilder::demand(
                "/workspace/pkg/BUILD.bazel",
                PathObservationOperation::FileBytes
            ))
            .is_some()
    );
    let mut absent = tracked(&dice, absent.build(), tracker.dupe()).await;
    let result = absent.compute(&key).await.unwrap();
    assert!(matches!(&result, LoadingPreparationOutcome::Need(_)));
    assert!(!RootPackageLoadObservationKey::validity(&result));
    drop(result);
    drop(absent);
    let mut deleted = EpochBuilder::workspace_sources("", BUILD_A, &[], 981);
    deleted.missing("/workspace/pkg/BUILD.bazel");
    deleted.missing("/workspace/pkg/BUILD");
    let mut deleted = tracked(&dice, deleted.build(), tracker.dupe()).await;
    let result = deleted.compute(&key).await.unwrap();
    assert!(matches!(
        observed_package(&result)
            .result()
            .as_ref()
            .as_ref()
            .unwrap_err()
            .inner,
        RootPackageLoadErrorInner::Source(_)
    ));
    drop(result);
    drop(deleted);

    // Sample the actual current graph owner, without assuming historical A retention.
    let mut final_tx = tracked(&dice, fixture(BUILD_A, true, 3), tracker.dupe()).await;
    assert_eq!(names(&final_tx.compute(&key).await.unwrap()), ["a.txt"]);
    let retiring = tracker.value();
    let held_body = retiring.bytes.dupe();
    let body_weak = Arc::downgrade(&held_body);
    let syntax_weak = Arc::downgrade(&retiring);
    let loads_weak = Arc::downgrade(retiring.result.as_ref().unwrap());
    drop(final_tx);
    println!(
        "WP1098_LAYOUT syntax={}:{} key={}:{} source={}:{} shared_body={}:{} shared_loads={}:{} body_bytes={} loads={}",
        std::mem::size_of::<RootPackageSyntax>(),
        std::mem::align_of::<RootPackageSyntax>(),
        std::mem::size_of::<RootPackageSyntaxKey>(),
        std::mem::align_of::<RootPackageSyntaxKey>(),
        std::mem::size_of::<slug_bzlmod_v2::RootPackageSource>(),
        std::mem::align_of::<slug_bzlmod_v2::RootPackageSource>(),
        std::mem::size_of::<Arc<[u8]>>(),
        std::mem::align_of::<Arc<[u8]>>(),
        std::mem::size_of::<Arc<[String]>>(),
        std::mem::align_of::<Arc<[String]>>(),
        syntax.bytes.len(),
        syntax.result.as_ref().unwrap().len()
    );
    assert!(!format!("{syntax:?}").contains("exports_files"));
    drop(syntax);
    drop(retiring);
    assert!(
        syntax_weak.upgrade().is_some(),
        "DICE graph still owns original syntax after local handles drop"
    );
    dice.wait_for_idle().await;
    drop(dice);
    tokio::time::timeout(Duration::from_secs(1), async {
        while syntax_weak.strong_count() != 0
            || loads_weak.strong_count() != 0
            || body_weak.strong_count() != 1
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual actor retirement releases syntax, loads and original body graph owners");
    assert!(syntax_weak.upgrade().is_none());
    assert!(loads_weak.upgrade().is_none());
    assert_eq!(body_weak.strong_count(), 1);
    drop(held_body);
    assert!(body_weak.upgrade().is_none());
}
