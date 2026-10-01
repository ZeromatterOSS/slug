use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;
use dice::UserCycleDetector;
use tokio::sync::Semaphore;
use tokio::task::AbortHandle;

use super::tests::load;
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Started(usize),
    ParentEdge,
    Finished(usize),
    ReturnEdge,
}

struct Gates {
    events: Mutex<Vec<Event>>,
    starts: AtomicUsize,
    finishes: AtomicUsize,
    computes: AtomicUsize,
    entered: Semaphore,
    parent_edge: Semaphore,
    release: Arc<Semaphore>,
}

struct Detector {
    graph: Arc<DependencyCycleGraph>,
    gates: Arc<Gates>,
}

// Each fixture key maps one-to-one to a complete concrete graph key. Only
// the test adapter substitutes identities; all graph lifecycle code is real.
fn mapped(key: &DynKey) -> Option<DynKey> {
    if key.downcast_ref::<Target>().is_some() {
        Some(load("handoff_target"))
    } else if key.downcast_ref::<Parent>().is_some() {
        Some(load("handoff_parent"))
    } else {
        None
    }
}

impl UserCycleDetector for Detector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        let mapped = mapped(key)?;
        if key.downcast_ref::<Target>().is_some() {
            let attempt = self.gates.starts.fetch_add(1, Ordering::SeqCst) + 1;
            self.gates
                .events
                .lock()
                .unwrap()
                .push(Event::Started(attempt));
        }
        Some(Arc::new(Guard {
            dependency: self.graph.start(&mapped).unwrap(),
            gates: self.gates.clone(),
            parent: key.downcast_ref::<Parent>().is_some(),
        }))
    }

    fn finished_computing_key(&self, key: &DynKey) {
        if let Some(mapped) = mapped(key) {
            self.graph.finish(&mapped);
        }
        if key.downcast_ref::<Target>().is_some() {
            let attempt = self.gates.finishes.fetch_add(1, Ordering::SeqCst) + 1;
            self.gates
                .events
                .lock()
                .unwrap()
                .push(Event::Finished(attempt));
        }
    }
}

struct Guard {
    dependency: DependencyCycleGuard,
    gates: Arc<Gates>,
    parent: bool,
}

impl UserCycleDetectorGuard for Guard {
    fn add_edge(&self, key: &DynKey) {
        if let Some(mapped) = mapped(key) {
            self.dependency.add_edge(&mapped);
            if self.parent && key.downcast_ref::<Target>().is_some() {
                self.gates.events.lock().unwrap().push(Event::ParentEdge);
                self.gates.parent_edge.add_permits(1);
            } else if !self.parent && key.downcast_ref::<Parent>().is_some() {
                self.gates.events.lock().unwrap().push(Event::ReturnEdge);
            }
        }
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct Target;

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled graph handoff target")
    }
}

async fn guarded<K: Key<Value = Result<usize, Arc<DependencyCycle>>>>(
    ctx: &mut DiceComputations<'_>,
    key: &K,
) -> Result<usize, Arc<DependencyCycle>> {
    let guard = ctx.cycle_guard::<Guard>().unwrap().unwrap();
    match guard.dependency.guard_this(ctx.compute(key)).await {
        Ok(value) => value.unwrap(),
        Err(cycle) => Err(Arc::new(cycle)),
    }
}

#[async_trait]
impl Key for Target {
    type Value = Result<usize, Arc<DependencyCycle>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        let gates = ctx
            .per_transaction_data()
            .data
            .get::<Arc<Gates>>()
            .unwrap()
            .clone();
        let attempt = gates.computes.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt == 1 {
            cancellations
                .critical_section(|| async {
                    gates.entered.add_permits(1);
                    gates.release.acquire().await.unwrap().forget();
                })
                .await;
            Ok(attempt)
        } else {
            guarded(ctx, &Parent).await
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct Parent;

impl fmt::Display for Parent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("replacement graph target parent")
    }
}

#[async_trait]
impl Key for Parent {
    type Value = Result<usize, Arc<DependencyCycle>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        guarded(ctx, &Target).await
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

struct Cleanup {
    old: AbortHandle,
    replacement: Option<AbortHandle>,
    release: Arc<Semaphore>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.old.abort();
        if let Some(replacement) = &self.replacement {
            replacement.abort();
        }
        self.release.add_permits(1);
    }
}

#[tokio::test]
async fn public_dice_cancelled_child_replacement_preserves_pending_edge_and_detects_cycle() {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let graph = Arc::new(DependencyCycleGraph::default());
        let gates = Arc::new(Gates {
            events: Mutex::new(Vec::new()),
            starts: AtomicUsize::new(0),
            finishes: AtomicUsize::new(0),
            computes: AtomicUsize::new(0),
            entered: Semaphore::new(0),
            parent_edge: Semaphore::new(0),
            release: Arc::new(Semaphore::new(0)),
        });
        let mut data = UserComputationData {
            cycle_detector: Some(Arc::new(Detector {
                graph: graph.clone(),
                gates: gates.clone(),
            })),
            ..Default::default()
        };
        data.data.set(gates.clone());
        let anchor = dice.updater_with_data(data).commit().await;
        let mut old = anchor.clone();
        let old_request = tokio::spawn(async move { old.compute(&Target).await });
        let mut cleanup = Cleanup {
            old: old_request.abort_handle(),
            replacement: None,
            release: gates.release.clone(),
        };
        gates.entered.acquire().await.unwrap().forget();
        old_request.abort();
        assert!(old_request.await.unwrap_err().is_cancelled());
        let mut replacement = anchor.clone();
        assert_eq!(replacement.live_context_id(), anchor.live_context_id());
        let replacement = tokio::spawn(async move { replacement.compute(&Parent).await });
        cleanup.replacement = Some(replacement.abort_handle());
        gates.parent_edge.acquire().await.unwrap().forget();
        gates.release.add_permits(1);
        let cycle = replacement.await.unwrap().unwrap().unwrap_err();
        assert!(cycle.to_string().contains("handoff_parent"), "{cycle}");
        assert!(cycle.to_string().contains("handoff_target"), "{cycle}");
        drop(anchor);
        dice.wait_for_idle().await;
        assert_eq!(
            *gates.events.lock().unwrap(),
            [
                Event::Started(1),
                Event::ParentEdge,
                Event::Finished(1),
                Event::Started(2),
                Event::ReturnEdge,
                Event::Finished(2)
            ]
        );
        assert!(graph.state.lock().unwrap().active.is_empty());
    })
    .await
    .expect("public graph handoff cycle must terminate within two seconds");
}
