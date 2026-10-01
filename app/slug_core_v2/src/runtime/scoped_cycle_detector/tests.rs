/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use allocative::Allocative;
use async_trait::async_trait;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;
use tokio::task::AbortHandle;

use super::*;

struct Gates {
    producer_entered: Semaphore,
    root_joined: Semaphore,
    release: Arc<Semaphore>,
    producer_starts: AtomicUsize,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            producer_entered: Semaphore::new(0),
            root_joined: Semaphore::new(0),
            release: Arc::new(Semaphore::new(0)),
            producer_starts: AtomicUsize::new(0),
        }
    }
}

struct RequestData {
    marker: u64,
    gates: Arc<Gates>,
    dropped: Option<oneshot::Sender<()>>,
}

impl Drop for RequestData {
    fn drop(&mut self) {
        if let Some(sender) = self.dropped.take() {
            let _ignored = sender.send(());
        }
    }
}

fn userdata(
    registry: &ScopedCycleDetectorRegistry,
    state: Arc<RequestData>,
) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(state);
    registry.install(&mut data);
    data
}

fn detector(transaction: &DiceTransaction) -> &Arc<dyn UserCycleDetector> {
    transaction
        .per_transaction_data()
        .data
        .get::<PendingCycleBinding>()
        .unwrap()
        .0
        .detector()
}

fn state(ctx: &DiceComputations) -> Arc<RequestData> {
    ctx.per_transaction_data()
        .data
        .get::<Arc<RequestData>>()
        .unwrap()
        .clone()
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct MarkerKey(u64);

impl fmt::Display for MarkerKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "request marker {}", self.0)
    }
}

#[async_trait]
impl Key for MarkerKey {
    type Value = u64;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> u64 {
        state(ctx).marker
    }

    fn equality(left: &u64, right: &u64) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct SharedProducer;

impl fmt::Display for SharedProducer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shared producer")
    }
}

#[async_trait]
impl Key for SharedProducer {
    type Value = u64;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> u64 {
        let state = state(ctx);
        state.gates.producer_starts.fetch_add(1, Ordering::SeqCst);
        cancellations
            .critical_section(|| async {
                state.gates.producer_entered.add_permits(1);
                state.gates.release.acquire().await.unwrap().forget();
            })
            .await;
        state.marker
    }

    fn equality(left: &u64, right: &u64) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct RequestKey(u64);

impl fmt::Display for RequestKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "shared-producer request {}", self.0)
    }
}

#[async_trait]
impl Key for RequestKey {
    type Value = (u64, u64);

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let state = state(ctx);
        let child = ctx.compute(&SharedProducer);
        state.gates.root_joined.add_permits(1);
        (state.marker, child.await.unwrap())
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct RevisionKey;

impl fmt::Display for RevisionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scoped detector test revision")
    }
}

impl InjectedKey for RevisionKey {
    type Value = u64;

    fn equality(left: &u64, right: &u64) -> bool {
        left == right
    }
}

struct Cleanup {
    first: AbortHandle,
    second: Option<AbortHandle>,
    release: Arc<Semaphore>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.first.abort();
        if let Some(second) = &self.second {
            second.abort();
        }
        self.release.add_permits(1);
    }
}

#[tokio::test]
async fn shared_context_deduplicates_producer_and_preserves_request_userdata() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let registry = ScopedCycleDetectorRegistry::new(Handle::current());
        let gates = Arc::new(Gates::default());
        let mut first = dice
            .updater_with_data(userdata(
                &registry,
                Arc::new(RequestData {
                    marker: 11,
                    gates: gates.clone(),
                    dropped: None,
                }),
            ))
            .commit()
            .await;
        registry.bind(&first);
        let mut second = dice
            .updater_with_data(userdata(
                &registry,
                Arc::new(RequestData {
                    marker: 22,
                    gates: gates.clone(),
                    dropped: None,
                }),
            ))
            .commit()
            .await;
        registry.bind(&second);
        assert_eq!(first.live_context_id(), second.live_context_id());
        assert!(Arc::ptr_eq(detector(&first), detector(&second)));
        assert!(!Arc::ptr_eq(
            first
                .per_transaction_data()
                .cycle_detector
                .as_ref()
                .unwrap(),
            second
                .per_transaction_data()
                .cycle_detector
                .as_ref()
                .unwrap(),
        ));

        let first = tokio::spawn(async move { first.compute(&RequestKey(1)).await });
        let mut cleanup = Cleanup {
            first: first.abort_handle(),
            second: None,
            release: gates.release.clone(),
        };
        gates.producer_entered.acquire().await.unwrap().forget();
        gates.root_joined.acquire().await.unwrap().forget();
        let second = tokio::spawn(async move { second.compute(&RequestKey(2)).await });
        cleanup.second = Some(second.abort_handle());
        gates.root_joined.acquire().await.unwrap().forget();
        gates.release.add_permits(1);
        assert_eq!(first.await.unwrap().unwrap(), (11, 11));
        assert_eq!(second.await.unwrap().unwrap(), (22, 11));
        assert_eq!(gates.producer_starts.load(Ordering::SeqCst), 1);
        dice.wait_for_idle().await;
    })
    .await
    .expect("shared producer fixture must retire within two seconds");
}

#[tokio::test]
async fn changed_foreign_and_revived_contexts_are_isolated_and_dead_entries_prune() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let registry = ScopedCycleDetectorRegistry::new(Handle::current());
        let gates = Arc::new(Gates::default());
        let make_data = |marker| {
            userdata(
                &registry,
                Arc::new(RequestData {
                    marker,
                    gates: gates.clone(),
                    dropped: None,
                }),
            )
        };
        let mut first = dice.updater_with_data(make_data(11)).commit().await;
        registry.bind(&first);
        let old_id = first.live_context_id();
        let old_version = first.version();
        let old_detector = Arc::downgrade(detector(&first));
        assert_eq!(first.compute(&MarkerKey(1)).await.unwrap(), 11);

        let mut updater = dice.updater_with_data(make_data(22));
        updater.changed_to([(RevisionKey, 1)]).unwrap();
        let mut changed = updater.commit().await;
        registry.bind(&changed);
        let changed_id = changed.live_context_id();
        let changed_version = changed.version();
        assert_ne!(changed.live_context_id(), old_id);
        assert!(!Arc::ptr_eq(detector(&first), detector(&changed)));
        assert_eq!(changed.compute(&MarkerKey(2)).await.unwrap(), 22);
        drop(first);
        drop(changed);
        dice.wait_for_idle().await;
        assert!(old_detector.upgrade().is_none());

        let mut reopened = dice.updater_with_data(make_data(33)).commit().await;
        registry.bind(&reopened);
        assert_ne!(reopened.live_context_id(), old_id);
        assert_ne!(reopened.live_context_id(), changed_id);
        assert_eq!(reopened.version(), changed_version);
        assert_ne!(reopened.version(), old_version);
        assert_eq!(reopened.compute(&MarkerKey(3)).await.unwrap(), 33);
        assert_eq!(registry.entries.lock().unwrap().len(), 1);

        let foreign = Dice::builder().build(DetectCycles::Enabled);
        let mut other = foreign.updater_with_data(make_data(44)).commit().await;
        registry.bind(&other);
        assert_ne!(other.live_context_id(), reopened.live_context_id());
        assert!(!Arc::ptr_eq(detector(&other), detector(&reopened)));
        assert_eq!(other.compute(&MarkerKey(4)).await.unwrap(), 44);
        drop(other);
        drop(reopened);
        dice.wait_for_idle().await;
        foreign.wait_for_idle().await;
    })
    .await
    .expect("context isolation fixture must retire within two seconds");
}

#[tokio::test]
#[should_panic(expected = "a deferred cycle proxy cannot migrate to another live DICE cache")]
async fn bound_proxy_recommit_cannot_migrate_to_changed_context() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let registry = ScopedCycleDetectorRegistry::new(Handle::current());
        let mut data = UserComputationData::default();
        registry.install(&mut data);
        let transaction = dice.updater_with_data(data).commit().await;
        registry.bind(&transaction);
        let mut updater = transaction.into_updater();
        updater.changed_to([(RevisionKey, 1)]).unwrap();
        let changed = updater.commit().await;
        registry.bind(&changed);
    })
    .await
    .expect("bound proxy migration fixture must finish within two seconds");
}

#[tokio::test]
async fn revival_does_not_share_coordinator_with_retained_cancellation_userdata() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let registry = ScopedCycleDetectorRegistry::new(Handle::current());
        let gates = Arc::new(Gates::default());
        let (dropped, retired) = oneshot::channel();
        let state = Arc::new(RequestData {
            marker: 11,
            gates: gates.clone(),
            dropped: Some(dropped),
        });
        let weak_state = Arc::downgrade(&state);
        let mut old = dice
            .updater_with_data(userdata(&registry, state))
            .commit()
            .await;
        registry.bind(&old);
        let old_version = old.version();
        let old_id = old.live_context_id();
        let old_detector = Arc::downgrade(detector(&old));
        let request = tokio::spawn(async move { old.compute(&SharedProducer).await });
        let _cleanup = Cleanup {
            first: request.abort_handle(),
            second: None,
            release: gates.release.clone(),
        };
        gates.producer_entered.acquire().await.unwrap().forget();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());

        let mut reopened = dice
            .updater_with_data(userdata(
                &registry,
                Arc::new(RequestData {
                    marker: 22,
                    gates: gates.clone(),
                    dropped: None,
                }),
            ))
            .commit()
            .await;
        registry.bind(&reopened);
        assert_eq!(reopened.version(), old_version);
        assert_ne!(reopened.live_context_id(), old_id);
        assert!(weak_state.upgrade().is_some());
        assert!(!Arc::ptr_eq(
            &old_detector.upgrade().unwrap(),
            detector(&reopened)
        ));
        assert_eq!(reopened.compute(&MarkerKey(1)).await.unwrap(), 22);
        gates.release.add_permits(1);
        dice.wait_for_idle().await;
        retired.await.unwrap();
        assert!(weak_state.upgrade().is_none());
        assert!(old_detector.upgrade().is_none());
        registry.bind(&reopened);
        // A repeat bind must also prune dead coordinator entries.
        assert_eq!(registry.entries.lock().unwrap().len(), 1);
        drop(reopened);
        dice.wait_for_idle().await;
    })
    .await
    .expect("revival cancellation fixture must retire within two seconds");
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HandoffEvent {
    TargetStarted(usize),
    ParentAddedTargetEdge,
    TargetFinished(usize),
}

struct HandoffState {
    events: Mutex<Vec<HandoffEvent>>,
    target_starts: AtomicUsize,
    target_finishes: AtomicUsize,
    target_computes: AtomicUsize,
    entered: Semaphore,
    parent_edge: Semaphore,
    release: Arc<Semaphore>,
}

impl HandoffState {
    fn record(&self, event: HandoffEvent) {
        self.events.lock().unwrap().push(event);
    }
}

struct HandoffDetector(Arc<HandoffState>);

impl UserCycleDetector for HandoffDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        let parent = if key.downcast_ref::<HandoffTargetKey>().is_some() {
            let attempt = self.0.target_starts.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.record(HandoffEvent::TargetStarted(attempt));
            false
        } else if key.downcast_ref::<HandoffParentKey>().is_some() {
            true
        } else {
            return None;
        };
        Some(Arc::new(HandoffGuard {
            state: self.0.clone(),
            parent,
        }))
    }

    fn finished_computing_key(&self, key: &DynKey) {
        if key.downcast_ref::<HandoffTargetKey>().is_some() {
            let attempt = self.0.target_finishes.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.record(HandoffEvent::TargetFinished(attempt));
        }
    }
}

struct HandoffGuard {
    state: Arc<HandoffState>,
    parent: bool,
}

impl UserCycleDetectorGuard for HandoffGuard {
    fn add_edge(&self, key: &DynKey) {
        if self.parent && key.downcast_ref::<HandoffTargetKey>().is_some() {
            self.state.record(HandoffEvent::ParentAddedTargetEdge);
            self.state.parent_edge.add_permits(1);
        }
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct HandoffTargetKey;

impl fmt::Display for HandoffTargetKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cancelled predecessor handoff target")
    }
}

#[async_trait]
impl Key for HandoffTargetKey {
    type Value = usize;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> usize {
        let state = ctx
            .per_transaction_data()
            .data
            .get::<Arc<HandoffState>>()
            .unwrap()
            .clone();
        let attempt = state.target_computes.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt == 1 {
            cancellations
                .critical_section(|| async {
                    state.entered.add_permits(1);
                    state.release.acquire().await.unwrap().forget();
                })
                .await;
        }
        attempt
    }

    fn equality(left: &usize, right: &usize) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct HandoffParentKey;

impl fmt::Display for HandoffParentKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("replacement target's waiting parent")
    }
}

#[async_trait]
impl Key for HandoffParentKey {
    type Value = usize;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> usize {
        ctx.compute(&HandoffTargetKey).await.unwrap()
    }

    fn equality(left: &usize, right: &usize) -> bool {
        left == right
    }
}

#[tokio::test]
async fn replacement_parent_edge_precedes_cancelled_target_finish_and_restart() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let state = Arc::new(HandoffState {
            events: Mutex::new(Vec::new()),
            target_starts: AtomicUsize::new(0),
            target_finishes: AtomicUsize::new(0),
            target_computes: AtomicUsize::new(0),
            entered: Semaphore::new(0),
            parent_edge: Semaphore::new(0),
            release: Arc::new(Semaphore::new(0)),
        });
        let mut data = UserComputationData {
            cycle_detector: Some(Arc::new(HandoffDetector(state.clone()))),
            ..Default::default()
        };
        data.data.set(state.clone());
        // Keep this live cache open while the sole old target waiter cancels.
        let anchor = dice.updater_with_data(data).commit().await;
        let mut old = anchor.clone();
        let old_request = tokio::spawn(async move { old.compute(&HandoffTargetKey).await });
        let mut cleanup = Cleanup {
            first: old_request.abort_handle(),
            second: None,
            release: state.release.clone(),
        };
        state.entered.acquire().await.unwrap().forget();
        old_request.abort();
        assert!(old_request.await.unwrap_err().is_cancelled());

        let mut replacement = anchor.clone();
        assert_eq!(replacement.live_context_id(), anchor.live_context_id());
        let replacement = tokio::spawn(async move { replacement.compute(&HandoffParentKey).await });
        cleanup.second = Some(replacement.abort_handle());
        state.parent_edge.acquire().await.unwrap().forget();
        state.release.add_permits(1);
        assert_eq!(replacement.await.unwrap().unwrap(), 2);
        drop(anchor);
        dice.wait_for_idle().await;

        assert_eq!(
            *state.events.lock().unwrap(),
            [
                HandoffEvent::TargetStarted(1),
                HandoffEvent::ParentAddedTargetEdge,
                HandoffEvent::TargetFinished(1),
                HandoffEvent::TargetStarted(2),
                HandoffEvent::TargetFinished(2),
            ]
        );
    })
    .await
    .expect("cancelled target handoff fixture must retire within two seconds");
}
