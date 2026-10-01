use std::fmt;
use std::time::Duration;

use allocative::Allocative;
use async_trait::async_trait;
use dice::ActivationData;
use dice::ActivationTracker;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceEvent;
use dice::DiceEventListener;
use dice::DiceTransaction;
use dice::InjectedKey;
use dice::UserComputationData;
use dice::UserCycleDetector;
use tokio::task::AbortHandle;

use super::tests::load;
use super::*;

#[derive(Allocative, Debug)]
enum Trace {
    Event(DiceEvent),
    OwnerCycle,
}

#[derive(Allocative, Default)]
struct Tracker {
    trace: Mutex<Vec<Trace>>,
    dependencies: Mutex<Vec<(String, Vec<String>)>>,
}

impl DiceEventListener for Tracker {
    fn event(&self, event: DiceEvent) {
        self.trace.lock().unwrap().push(Trace::Event(event));
    }
}

impl ActivationTracker for Tracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        _: ActivationData,
    ) {
        self.dependencies
            .lock()
            .unwrap()
            .push((key.to_string(), deps.map(ToString::to_string).collect()));
    }
}

struct Detector(Arc<DependencyCycleGraph>);

fn mapped(key: &DynKey) -> Option<DynKey> {
    if key.downcast_ref::<WarmBridge>().is_some() {
        Some(load("warm_bridge"))
    } else if key.downcast_ref::<FreshOwner>().is_some() {
        Some(load("fresh_owner"))
    } else {
        None
    }
}

impl UserCycleDetector for Detector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        Some(Arc::new(Guard(self.0.start(&mapped(key)?).unwrap())))
    }

    fn finished_computing_key(&self, key: &DynKey) {
        if let Some(key) = mapped(key) {
            self.0.finish(&key);
        }
    }
}

struct Guard(DependencyCycleGuard);

impl UserCycleDetectorGuard for Guard {
    fn add_edge(&self, key: &DynKey) {
        if let Some(key) = mapped(key) {
            self.0.add_edge(&key);
        }
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

type Value = Result<u64, Arc<DependencyCycle>>;

async fn guarded<K: Key<Value = Value>>(ctx: &mut DiceComputations<'_>, key: &K) -> Value {
    let guard = ctx.cycle_guard::<Guard>().unwrap().unwrap();
    match compute_with_guard(ctx, key, Some(&guard.0)).await {
        Ok(value) => value,
        Err(DependencyComputeError::Cycle(cycle)) => Err(Arc::new(cycle)),
        Err(error) => panic!("unexpected DICE failure: {error}"),
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct Revision;

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("warm fixture revision")
    }
}

impl InjectedKey for Revision {
    type Value = u64;

    fn equality(left: &u64, right: &u64) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct WarmBridge;

impl fmt::Display for WarmBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("warm bridge")
    }
}

#[async_trait]
impl Key for WarmBridge {
    type Value = Value;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Value {
        guarded(ctx, &FreshOwner).await
    }

    fn equality(left: &Value, right: &Value) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct FreshOwner;

impl fmt::Display for FreshOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fresh owner")
    }
}

#[async_trait]
impl Key for FreshOwner {
    type Value = Value;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Value {
        if ctx.compute(&Revision).await.unwrap() == 0 {
            Ok(0)
        } else {
            let value = guarded(ctx, &WarmBridge).await;
            if value.is_err() {
                ctx.per_transaction_data()
                    .data
                    .get::<Arc<Tracker>>()
                    .unwrap()
                    .trace
                    .lock()
                    .unwrap()
                    .push(Trace::OwnerCycle);
            }
            value
        }
    }

    fn equality(left: &Value, right: &Value) -> bool {
        left == right
    }
}

fn userdata(graph: Arc<DependencyCycleGraph>, tracker: Arc<Tracker>) -> UserComputationData {
    let mut data = UserComputationData {
        cycle_detector: Some(Arc::new(Detector(graph))),
        tracker: tracker.clone(),
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    };
    data.data.set(tracker);
    data
}

struct AbortOnDrop(AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn request(mut tx: DiceTransaction) -> (DiceTransaction, Value) {
    let request = tokio::spawn(async move {
        let value = tx.compute(&WarmBridge).await.unwrap();
        (tx, value)
    });
    let _cleanup = AbortOnDrop(request.abort_handle());
    request.await.unwrap()
}

#[test]
fn warm_adapter_rejects_unknown_keys_without_changing_structural_conversion() {
    let graph = Arc::new(DependencyCycleGraph::default());
    let unknown = DynKey::from_key(Revision);
    assert!(mapped(&unknown).is_none());
    assert!(Node::from_dyn(&unknown).is_none());
    assert!(
        Detector(graph.clone())
            .start_computing_key(&unknown)
            .is_none()
    );
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[tokio::test]
async fn historical_checking_deps_notifies_fresh_owner_and_poison_allows_recovery() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let first_graph = Arc::new(DependencyCycleGraph::default());
        let mut updater =
            dice.updater_with_data(userdata(first_graph.clone(), Arc::new(Tracker::default())));
        updater.changed_to([(Revision, 0)]).unwrap();
        let (tx, value) = request(updater.commit().await).await;
        assert_eq!(value, Ok(0));
        dice.wait_for_idle().await;
        assert!(first_graph.state.lock().unwrap().active.is_empty());

        let graph = Arc::new(DependencyCycleGraph::default());
        let tracker = Arc::new(Tracker::default());
        let mut updater = tx.into_updater();
        updater.changed_to([(Revision, 1)]).unwrap();
        updater.changed([FreshOwner]).unwrap(); // WarmBridge keeps its historical deps.
        let (tx, value) = request(
            updater
                .commit_with_data(userdata(graph.clone(), tracker.clone()))
                .await,
        )
        .await;
        let cycle = value.unwrap_err();
        assert!(cycle.to_string().contains("warm_bridge"), "{cycle}");
        assert!(cycle.to_string().contains("fresh_owner"), "{cycle}");
        dice.wait_for_idle().await;
        assert!(graph.state.lock().unwrap().active.is_empty());
        let trace = tracker.trace.lock().unwrap();
        let observed = trace
            .iter()
            .position(|event| matches!(event, Trace::OwnerCycle))
            .expect("fresh owner must consume its typed cycle");
        assert!(
            trace[..observed].iter().any(|event| matches!(
                event,
                Trace::Event(DiceEvent::CheckDepsStarted {
                    key_type: "WarmBridge"
                })
            )),
            "{trace:?}"
        );
        assert!(
            !trace[..observed].iter().any(|event| matches!(
                event,
                Trace::Event(DiceEvent::ComputeStarted {
                    key_type: "WarmBridge"
                })
            )),
            "{trace:?}"
        );
        drop(trace);
        let dependencies = tracker.dependencies.lock().unwrap();
        assert!(
            dependencies.iter().any(|(key, deps)| key == "fresh owner"
                && deps.iter().any(|key| key == "bzl-load-cycle-poison")),
            "{dependencies:?}"
        );
        drop(dependencies);

        let recovery_graph = Arc::new(DependencyCycleGraph::default());
        let mut updater = tx.into_updater();
        updater.changed_to([(Revision, 0)]).unwrap();
        updater.changed([FreshOwner]).unwrap();
        let (tx, value) = request(
            updater
                .commit_with_data(userdata(
                    recovery_graph.clone(),
                    Arc::new(Tracker::default()),
                ))
                .await,
        )
        .await;
        assert_eq!(value, Ok(0));
        drop(tx);
        dice.wait_for_idle().await;
        assert!(recovery_graph.state.lock().unwrap().active.is_empty());
    })
    .await
    .expect("historical CheckingDeps cycle must retire and recover within two seconds");
}
