//! Observed siblings settle earlier source Needs before a later semantic error.
//! Legacy precedence, outer errors and AST label prevalidation are unchanged.
use dice::ActivationClosureError;
use dice::DiceNodeId;
use dice::RootActivation;
use slug_workspace_v2::PathObservationKey;

use super::*;

const ROOT: &[u8] = b"load(':left.bzl', 'LEFT')\nload(':right.bzl', 'RIGHT')\nload(':unused.bzl', 'UNUSED')\nRESULT = LEFT + RIGHT\n";
const REVERSED: &[u8] = b"load(':right.bzl', 'RIGHT')\nload(':left.bzl', 'LEFT')\nload(':unused.bzl', 'UNUSED')\nRESULT = LEFT + RIGHT\n";

#[derive(Default)]
struct FrontierTracker {
    completed: Mutex<Vec<(String, DiceNodeId, bool)>>,
    paths: Mutex<Vec<PathObservationDemand>>,
    modules: Mutex<Vec<String>>,
}

impl ActivationTracker for FrontierTracker {
    fn key_activated(&self, _: &DynKey, _: &mut dyn Iterator<Item = &DynKey>, _: ActivationData) {}

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, _: RichActivation<'_>) {
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
