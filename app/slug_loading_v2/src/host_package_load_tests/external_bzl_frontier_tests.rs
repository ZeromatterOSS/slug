//! Observed siblings settle earlier source Needs before a later semantic error.
//! Legacy precedence, outer errors and AST label prevalidation are unchanged.
use std::sync::Weak;

use dice::ActivationClosureError;
use dice::DiceNodeId;
use dice::RootActivation;
use slug_workspace_v2::PathObservationKey;

use super::super::BzlModuleContext;
use super::super::ExternalBzlChildLoad;
use super::super::ExternalBzlModuleMode;
use super::super::external_bzl_syntax::ExternalBzlSyntax;
use super::super::external_bzl_syntax::ExternalBzlSyntaxKey;
use super::super::prepare_external_bzl_child_loads;
use super::*;
use crate::HostCanonicalRepositoryLoadRouteKey;

const ROOT: &[u8] = b"load(':left.bzl', 'LEFT')\nload(':right.bzl', 'RIGHT')\nload(':unused.bzl', 'UNUSED')\nRESULT = LEFT + RIGHT\n";
const REVERSED: &[u8] = b"load(':right.bzl', 'RIGHT')\nload(':left.bzl', 'LEFT')\nload(':unused.bzl', 'UNUSED')\nRESULT = LEFT + RIGHT\n";

#[derive(Default)]
struct FrontierTracker {
    completed: Mutex<Vec<(String, DiceNodeId, bool)>>,
    paths: Mutex<Vec<PathObservationDemand>>,
    modules: Mutex<Vec<String>>,
    syntax: Mutex<
        Vec<(
            ExternalBzlSyntaxKey,
            ActivationKind,
            Weak<ExternalBzlSyntax>,
        )>,
    >,
    events: Mutex<Vec<(String, EventBatch)>>,
}

impl ActivationTracker for FrontierTracker {
    fn key_activated(&self, _: &DynKey, _: &mut dyn Iterator<Item = &DynKey>, _: ActivationData) {}

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        if let Some(syntax) = key.downcast_ref::<ExternalBzlSyntaxKey>() {
            let weak = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<Weak<ExternalBzlSyntax>>())
                .cloned()
                .unwrap_or_default();
            self.syntax
                .lock()
                .unwrap()
                .push((syntax.clone(), activation.kind(), weak));
        }
        if key
            .downcast_ref::<ExternalBzlModuleObservationKey>()
            .is_some()
        {
            if let Some(batch) = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
            {
                self.events
                    .lock()
                    .unwrap()
                    .push((key.to_string(), batch.dupe()));
            }
        }
        if let Some(key) = key.downcast_ref::<PathObservationKey>() {
            self.paths.lock().unwrap().push(key.demand().dupe());
        }
        if key
            .downcast_ref::<ExternalBzlModuleObservationKey>()
            .is_some()
        {
            self.modules.lock().unwrap().push(key.to_string());
        }
    }

    fn root_completed(&self, key: &DynKey, root: RootActivation, transient: bool) {
        self.completed
            .lock()
            .unwrap()
            .push((key.to_string(), root.node(), transient));
    }
}

impl FrontierTracker {
    fn completion(&self, key: &ExternalBzlModuleObservationKey) -> (DiceNodeId, bool) {
        self.completed
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(name, node, transient)| {
                (name == &key.to_string()).then_some((*node, *transient))
            })
            .expect("the exact parentless module request completed")
    }

    fn assert_not_activated(&self, target: &str) {
        assert!(!self.modules.lock().unwrap().iter().any(|module| {
            module == &format!("observed-external-bzl-module:@@dep+//:{target}")
        }));
    }
}

fn missing_bytes() -> PathObservationDemand {
    EpochBuilder::demand(
        "/workspace/dep/left.bzl",
        PathObservationOperation::FileBytes,
    )
}

fn epoch(root: &[u8], left: &[u8], include_left_bytes: bool) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::external_sources(
        &[
            ("root.bzl", root),
            ("left.bzl", left),
            ("right.bzl", b"RIGHT =\n"),
            ("unused.bzl", b"UNUSED = 3\n"),
        ],
        877,
    );
    if !include_left_bytes {
        // Retain the genuine regular-file Lstat; only its exact byte demand
        // is unobserved. This is Need, never an observed missing source file.
        assert!(epoch.entries.shift_remove(&missing_bytes()).is_some());
    }
    epoch.build()
}

async fn tracked_transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<FrontierTracker>,
) -> DiceTransaction {
    transaction(dice, epoch, false, None)
        .await
        .into_updater()
        .commit_with_data(UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        })
        .await
}

fn assert_child_parse_error(value: &ObservedExternalBzlOutcome, target: &str) {
    let ExternalBzlModuleError::Child {
        raw_load,
        canonical_label,
        error,
    } = observed_external_error(value)
    else {
        panic!("expected the decisive child error");
    };
    assert_eq!(raw_load.as_ref(), format!(":{target}"));
    assert_eq!(canonical_label.to_string(), format!("@@dep+//:{target}"));
    assert!(matches!(
        error.as_ref(),
        ExternalBzlModuleError::Parse { .. }
    ));
    assert!(ExternalBzlModuleObservationKey::validity(value));
}

#[tokio::test]
async fn observed_sibling_need_precedes_later_semantic_error_and_repairs_strict_closure() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(FrontierTracker::default());
    let mut cold =
        tracked_transaction(&dice, epoch(ROOT, b"LEFT = 1\n", false), tracker.dupe()).await;
    let route = external_route(&mut cold).await;
    let key = observed_external_bzl_key(route, "", "root.bzl");
    let value = cold.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(need) = &value else {
        panic!("earlier source Need must precede the later semantic error: {value:?}");
    };
    assert_eq!(
        need.path_observations().unwrap().demands(),
        &[missing_bytes()]
    );
    assert!(!ExternalBzlModuleObservationKey::validity(&value));
    assert!(tracker.paths.lock().unwrap().contains(&missing_bytes()));
    tracker.assert_not_activated("unused.bzl");
    let (node, transient) = tracker.completion(&key);
    assert!(transient);
    assert_eq!(
        cold.activation_closure([node]).await.unwrap_err(),
        ActivationClosureError::UnavailableRoot { root: node }
    );

    let repaired_tracker = Arc::new(FrontierTracker::default());
    let mut repaired = tracked_transaction(
        &dice,
        epoch(ROOT, b"LEFT = 1\n", true),
        repaired_tracker.dupe(),
    )
    .await;
    let repaired_value = repaired.compute(&key).await.unwrap();
    assert_child_parse_error(&repaired_value, "right.bzl");
    repaired_tracker.assert_not_activated("unused.bzl");
    let (repaired_node, transient) = repaired_tracker.completion(&key);
    assert_eq!(repaired_node, node);
    assert!(!transient);
    assert_eq!(
        repaired.activation_closure([node]).await.unwrap().roots(),
        &[node]
    );

    let earlier_tracker = Arc::new(FrontierTracker::default());
    let mut earlier = tracked_transaction(
        &dice,
        epoch(ROOT, b"LEFT =\n", true),
        earlier_tracker.dupe(),
    )
    .await;
    assert_child_parse_error(&earlier.compute(&key).await.unwrap(), "left.bzl");
    earlier_tracker.assert_not_activated("right.bzl");
    earlier_tracker.assert_not_activated("unused.bzl");
    let (node, transient) = earlier_tracker.completion(&key);
    assert!(!transient);
    assert_eq!(
        earlier.activation_closure([node]).await.unwrap().roots(),
        &[node]
    );
}

#[tokio::test]
async fn earlier_semantic_error_does_not_probe_missing_later_sibling() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(FrontierTracker::default());
    let mut transaction =
        tracked_transaction(&dice, epoch(REVERSED, b"LEFT = 1\n", false), tracker.dupe()).await;
    let route = external_route(&mut transaction).await;
    let key = observed_external_bzl_key(route, "", "root.bzl");
    let value = transaction.compute(&key).await.unwrap();
    assert_child_parse_error(&value, "right.bzl");
    assert!(!tracker.paths.lock().unwrap().contains(&missing_bytes()));
    tracker.assert_not_activated("left.bzl");
    tracker.assert_not_activated("unused.bzl");
    let (node, transient) = tracker.completion(&key);
    assert!(!transient);
    assert_eq!(
        transaction
            .activation_closure([node])
            .await
            .unwrap()
            .roots(),
        &[node]
    );
}

const SYNOPSIS_A: &[u8] = b"load(':left.bzl', 'LEFT')\nprint('settled root')\nRESULT = LEFT + 1\n";
const SYNOPSIS_B: &[u8] = b"load(':left.bzl', 'LEFT')\nprint('settled root')\nRESULT = LEFT + 2\n";
fn synopsis_epoch(root: &[u8], include: bool, unrelated: &[u8]) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::external_sources(
        &[
            ("root.bzl", root),
            ("left.bzl", b"LEFT = 40\n"),
            ("spare.bzl", unrelated),
        ],
        878,
    );
    if !include {
        assert!(epoch.entries.shift_remove(&missing_bytes()).is_some());
    }
    epoch.build()
}
async fn synopsis_transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<FrontierTracker>,
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
impl FrontierTracker {
    fn syntax_evaluations(&self, key: &ExternalBzlModuleObservationKey) -> usize {
        self.syntax
            .lock()
            .unwrap()
            .iter()
            .filter(|(syntax, kind, _)| {
                syntax.module == key.0 && *kind == ActivationKind::Evaluated
            })
            .count()
    }
    fn syntax_value(&self, key: &ExternalBzlModuleObservationKey) -> Arc<ExternalBzlSyntax> {
        self.syntax
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(syntax, _, value)| {
                (syntax.module == key.0).then(|| value.upgrade()).flatten()
            })
            .expect("normal parent evaluated a live graph-owned syntax child")
    }
}
fn synopsis_result(value: &ObservedExternalBzlOutcome) -> i32 {
    observed_external(value)
        .result()
        .as_ref()
        .as_ref()
        .unwrap()
        .module
        .get("RESULT")
        .unwrap()
        .value()
        .unpack_i32()
        .unwrap()
}

#[tokio::test]
async fn external_bzl_synopsis_reuses_across_child_need_and_current_epoch_changes() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(FrontierTracker::default());
    let mut first = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_A, false, b"X = 1\n"),
        tracker.dupe(),
    )
    .await;
    let route = external_route(&mut first).await;
    let key = observed_external_bzl_key(route, "", "root.bzl");
    let need = first.compute(&key).await.unwrap();
    assert!(
        matches!(&need, LoadingPreparationOutcome::Need(n) if n.path_observations().unwrap().demands() == &[missing_bytes()])
    );
    assert!(!ExternalBzlModuleObservationKey::validity(&need));
    let syntax = tracker.syntax_value(&key);
    let loads = syntax.result.as_ref().unwrap();
    assert_eq!(loads.as_ref(), [":left.bzl"]);
    let root_demand = EpochBuilder::demand(
        "/workspace/dep/root.bzl",
        PathObservationOperation::FileBytes,
    );
    let retained_epoch = first.compute(&PathObservationEpochKey).await.unwrap();
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
        retained_epoch.get(&root_demand).unwrap().as_ref()
    else {
        panic!("fixture source bytes");
    };
    assert!(Arc::ptr_eq(bytes, &syntax.bytes));
    let body_weak = Arc::downgrade(bytes);
    let held_body = bytes.dupe();
    let syntax_weak = Arc::downgrade(&syntax);
    let loads_weak = Arc::downgrade(loads);
    assert_eq!(tracker.syntax_evaluations(&key), 1);
    assert!(
        tracker
            .events
            .lock()
            .unwrap()
            .iter()
            .all(|(name, _)| name != &key.to_string())
    );
    drop(retained_epoch);
    drop(first);
    dice.wait_for_idle().await;
    let mut again = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_A, false, b"X = 2\n"),
        tracker.dupe(),
    )
    .await;
    let second_need = again.compute(&key).await.unwrap();
    assert!(matches!(&second_need, LoadingPreparationOutcome::Need(_)));
    assert!(!ExternalBzlModuleObservationKey::validity(&second_need));
    assert_eq!(tracker.syntax_evaluations(&key), 1);
    let reused = tracker.syntax_value(&key);
    assert!(Arc::ptr_eq(&syntax, &reused));
    assert!(Arc::ptr_eq(loads, reused.result.as_ref().unwrap()));
    drop(again);
    dice.wait_for_idle().await;
    let mut repaired = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_A, true, b"X = 2\n"),
        tracker.dupe(),
    )
    .await;
    let terminal = repaired.compute(&key).await.unwrap();
    assert_eq!(synopsis_result(&terminal), 41);
    let certificate = observed_external(&terminal).observations();
    assert!(certificate.get(&missing_bytes()).is_some());
    assert_eq!(tracker.syntax_evaluations(&key), 1);
    let events = tracker.events.lock().unwrap();
    let root_events = &events
        .iter()
        .rev()
        .find(|(name, _)| name == &key.to_string())
        .unwrap()
        .1;
    assert_eq!(event_texts(root_events), ["settled root"]);
    drop(events);
    // Borrowed child names point at the shared synopsis, including duplicate order.
    let prepared = prepare_external_bzl_child_loads(
        &key.0,
        &key.0.route,
        loads,
        key.0.canonical_label(),
        certificate,
    )
    .unwrap();
    let name = match &prepared[0] {
        ExternalBzlChildLoad::Canonical(name) | ExternalBzlChildLoad::Resolved(name, _) => *name,
    };
    assert_eq!(name.as_ptr(), loads[0].as_ptr());
    drop(prepared);
    println!(
        "WP1092_LAYOUT syntax={}:{} key={}:{} module_key={}:{} shared_body={} shared_loads={}",
        std::mem::size_of::<ExternalBzlSyntax>(),
        std::mem::align_of::<ExternalBzlSyntax>(),
        std::mem::size_of::<ExternalBzlSyntaxKey>(),
        std::mem::align_of::<ExternalBzlSyntaxKey>(),
        std::mem::size_of::<ExternalBzlModuleEvalKey>(),
        std::mem::align_of::<ExternalBzlModuleEvalKey>(),
        syntax.bytes.len(),
        loads.len()
    );
    drop(terminal);
    drop(repaired);
    drop(reused);
    drop(syntax);
    assert!(
        syntax_weak.upgrade().is_some(),
        "completed DICE value owns the synopsis beyond local handles"
    );
    dice.wait_for_idle().await;
    drop(dice);
    // The dedicated DICE state thread owns the graph until its closed channel
    // is drained; idle only joins cancellation tasks. Wait for actual owners.
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while syntax_weak.strong_count() != 0
            || loads_weak.strong_count() != 0
            || body_weak.strong_count() != 1
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("retired graph releases synopsis, loads and all but held body ref");
    assert!(syntax_weak.upgrade().is_none());
    assert!(loads_weak.upgrade().is_none());
    assert!(
        body_weak.upgrade().is_some(),
        "independent original byte reference owns the allocation"
    );
    drop(held_body);
    assert!(body_weak.upgrade().is_none());
}

#[tokio::test]
async fn external_bzl_synopsis_preserves_source_identity_encoding_and_error_order() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(FrontierTracker::default());
    let mut a = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_A, true, b"X = 1\n"),
        tracker.dupe(),
    )
    .await;
    let route = external_route(&mut a).await;
    let key = observed_external_bzl_key(route, "", "root.bzl");
    assert_eq!(synopsis_result(&a.compute(&key).await.unwrap()), 41);
    let a_syntax = tracker.syntax_value(&key);
    drop(a);
    let mut b = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_B, true, b"X = 1\n"),
        tracker.dupe(),
    )
    .await;
    assert_eq!(synopsis_result(&b.compute(&key).await.unwrap()), 42);
    let b_syntax = tracker.syntax_value(&key);
    assert_eq!(
        a_syntax.result, b_syntax.result,
        "same loads do not imply same executable source"
    );
    assert_ne!(a_syntax, b_syntax);
    assert_eq!(tracker.syntax_evaluations(&key), 2);
    drop(b);
    let mut restored = synopsis_transaction(
        &dice,
        synopsis_epoch(SYNOPSIS_A, true, b"X = 1\n"),
        tracker.dupe(),
    )
    .await;
    assert_eq!(synopsis_result(&restored.compute(&key).await.unwrap()), 41);
    let current = tracker.syntax_value(&key);
    assert_eq!(current, a_syntax);
    assert_ne!(current, b_syntax);
    let original = ExternalBzlSyntaxKey {
        module: key.0.clone(),
        mode: ExternalBzlModuleMode::Observed,
    };
    let mut changed = original.clone();
    changed.mode = ExternalBzlModuleMode::Legacy;
    assert_ne!(original, changed);
    changed = original.clone();
    changed.module.context = BzlModuleContext::Bzlmod;
    assert_ne!(original, changed);
    changed = original.clone();
    changed.module.label =
        resolve_external_load_label(&PackagePath::parse("").unwrap(), ":other.bzl").unwrap();
    assert_ne!(original, changed);
    let LoadingPreparationOutcome::Complete(builtin) = restored
        .compute(
            &RootRepositoryRouteKey::for_root_build(
                workspace(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("original builtin route");
    };
    changed = original.clone();
    changed.module.route =
        HostRepositorySourceRoute::root(builtin.as_ref().as_ref().unwrap().clone());
    assert_ne!(original, changed, "full source route remains structural");
    // Both variants use one qualified in-memory builtin graph and workspace.
    use crate::canonical_repository_route_tests::tests as builtin_fixture;
    let builtin_dice = builtin_fixture::builtin_graph_dice();
    let builtin_module = builtin_fixture::builtin_graph_module();
    let mut builtin_tx = builtin_fixture::transaction(
        &builtin_dice,
        &builtin_module,
        builtin_fixture::EXTENSION_A,
        true,
        None,
    )
    .await;
    let builtin_workspace = NormalizedAbsolutePath::new(builtin_fixture::WORKSPACE).unwrap();
    let LoadingPreparationOutcome::Complete(root) = builtin_tx
        .compute(
            &RootRepositoryRouteKey::for_root_build(
                builtin_workspace.clone(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap()
    else {
        panic!("qualified builtin root route");
    };
    let LoadingPreparationOutcome::Complete(canonical) = builtin_tx
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            builtin_workspace,
            CanonicalRepoName::new("bazel_tools").unwrap(),
        ))
        .await
        .unwrap()
    else {
        panic!("qualified builtin canonical route");
    };
    let mut root_key = original.clone();
    root_key.module.route =
        HostRepositorySourceRoute::root(root.as_ref().as_ref().unwrap().clone());
    let mut canonical_key = root_key.clone();
    canonical_key.module.route =
        HostRepositorySourceRoute::canonical(canonical.as_ref().as_ref().unwrap().input().clone());
    assert_eq!(
        root_key.module.canonical_label(),
        canonical_key.module.canonical_label()
    );
    assert_ne!(
        root_key, canonical_key,
        "Root/Canonical owners remain distinct for the same workspace and builtin"
    );
    drop(builtin_tx);
    builtin_dice.wait_for_idle().await;
    drop(restored);
    // Parent source/syntax failure wins before any missing child or synopsis fallback.
    for (body, parse) in [
        (b"\xff".as_slice(), false),
        (b"load(':left.bzl', 'LEFT')\nRESULT =\n".as_slice(), true),
    ] {
        let before = tracker.syntax_evaluations(&key);
        let mut failed = synopsis_transaction(
            &dice,
            synopsis_epoch(body, false, b"X = 1\n"),
            tracker.dupe(),
        )
        .await;
        let value = failed.compute(&key).await.unwrap();
        assert!(ExternalBzlModuleObservationKey::validity(&value));
        assert!(if parse {
            matches!(
                observed_external_error(&value),
                ExternalBzlModuleError::Parse { .. }
            )
        } else {
            matches!(
                observed_external_error(&value),
                ExternalBzlModuleError::Encoding { .. }
            )
        });
        assert_eq!(tracker.syntax_evaluations(&key), before + 1);
        assert!(
            !observed_external(&value)
                .observations()
                .get(&missing_bytes())
                .is_some()
        );
    }
    let before = tracker.syntax_evaluations(&key);
    let mut absent = EpochBuilder::external_sources(&[("left.bzl", b"LEFT = 40\n")], 878);
    absent.entries.insert(
        EpochBuilder::demand("/workspace/dep/root.bzl", PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    );
    let mut absent = synopsis_transaction(&dice, absent.build(), tracker.dupe()).await;
    assert!(matches!(
        observed_external_error(&absent.compute(&key).await.unwrap()),
        ExternalBzlModuleError::Absent { .. }
    ));
    assert_eq!(
        tracker.syntax_evaluations(&key),
        before,
        "failed original source never looks up syntax"
    );
    drop(absent);
    let mut missing = EpochBuilder::external_sources(
        &[("root.bzl", SYNOPSIS_A), ("left.bzl", b"LEFT = 40\n")],
        878,
    );
    assert!(
        missing
            .entries
            .shift_remove(&EpochBuilder::demand(
                "/workspace/dep/root.bzl",
                PathObservationOperation::FileBytes
            ))
            .is_some()
    );
    let mut missing = synopsis_transaction(&dice, missing.build(), tracker.dupe()).await;
    let need = missing.compute(&key).await.unwrap();
    assert!(matches!(&need, LoadingPreparationOutcome::Need(_)));
    assert!(!ExternalBzlModuleObservationKey::validity(&need));
    assert_eq!(
        tracker.syntax_evaluations(&key),
        before,
        "unsettled original source never looks up syntax"
    );
    let defensive: super::super::external_bzl_syntax::ExternalBzlSyntaxOutcome =
        Err(need.map(|_| unreachable!()));
    assert!(!ExternalBzlSyntaxKey::validity(&defensive));
    assert!(!ExternalBzlSyntaxKey::equality(&defensive, &defensive));
}
