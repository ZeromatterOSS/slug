/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice_error::DiceErrorImpl;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use tokio::sync::Notify;

use crate::ActivationClosureError;
use crate::ActivationData;
use crate::ActivationKind;
use crate::ActivationTracker;
use crate::Dice;
use crate::DiceDataBuilder;
use crate::DiceNodeId;
use crate::DynKey;
use crate::InjectedKey;
use crate::RichActivation;
use crate::RootActivation;
use crate::VersionNumber;
use crate::api::computations::DiceComputations;
use crate::api::cycles::DetectCycles;
use crate::api::key::Key;
use crate::api::projection::DiceProjectionComputations;
use crate::api::projection::ProjectionKey;
use crate::api::user_data::UserComputationData;

#[derive(Default, Allocative)]
struct Tracker {
    /// Key, deps, data, reused
    state: Mutex<Vec<(Kind, Vec<Kind>, Option<Data>, bool)>>,
    rich: Mutex<Vec<RichEvent>>,
    roots: Mutex<Vec<RootEvent>>,
    completed: Mutex<Vec<(RootEvent, bool)>>,
}

impl Tracker {
    fn new() -> Self {
        Self {
            state: Mutex::new(Vec::new()),
            rich: Mutex::new(Vec::new()),
            roots: Mutex::new(Vec::new()),
            completed: Mutex::new(Vec::new()),
        }
    }
}

impl ActivationTracker for Tracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        activation_data: ActivationData,
    ) {
        let (data, reused) = match activation_data {
            ActivationData::Evaluated(d) => (d.map(|d| *d.downcast::<Data>().unwrap()), false),
            ActivationData::Reused => (None, true),
        };

        self.state.lock().unwrap().push((
            Kind::from_dyn_key(key),
            deps.into_iter().map(Kind::from_dyn_key).collect(),
            data,
            reused,
        ));
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.rich.lock().unwrap().push(RichEvent {
            key: Kind::from_dyn_key(key),
            node: activation.node(),
            version: activation.version(),
            kind: activation.kind(),
            has_data: activation.evaluation_data().is_some(),
            dependencies: activation.dependencies().to_vec(),
        });
    }

    fn root_activated(&self, key: &DynKey, activation: RootActivation) {
        self.roots.lock().unwrap().push(RootEvent {
            key: Kind::from_dyn_key(key),
            node: activation.node(),
            version: activation.version(),
            ordinal: activation.ordinal(),
            activation,
        });
    }

    fn root_completed(&self, key: &DynKey, activation: RootActivation, is_transient: bool) {
        let event = RootEvent {
            key: Kind::from_dyn_key(key),
            node: activation.node(),
            version: activation.version(),
            ordinal: activation.ordinal(),
            activation,
        };
        assert!(self.roots.lock().unwrap().contains(&event));
        self.completed.lock().unwrap().push((event, is_transient));
    }
}

#[derive(PartialEq, Eq, Debug, Dupe, Clone, Allocative)]
enum Kind {
    Injected,
    Stage0,
    Stage1,
    Selector,
    Graph(GraphKey),
    Transient,
    Value,
    Projection,
    Chain(u32),
    MaybeTransient,
    InheritedTransient,
    TransientProjection,
    RetainedTransient,
}

impl Kind {
    fn from_dyn_key(key: &DynKey) -> Self {
        if key.downcast_ref::<Injected>().is_some() {
            return Self::Injected;
        }

        if key.downcast_ref::<Stage0>().is_some() {
            return Self::Stage0;
        }

        if key.downcast_ref::<Stage1>().is_some() {
            return Self::Stage1;
        }

        if key.downcast_ref::<Selector>().is_some() {
            return Self::Selector;
        }

        if let Some(key) = key.downcast_ref::<GraphKey>() {
            return Self::Graph(key.dupe());
        }

        if key.downcast_ref::<TransientKey>().is_some() {
            return Self::Transient;
        }

        if key.downcast_ref::<ValueKey>().is_some() {
            return Self::Value;
        }

        if key.key_type_name() == ValueProjection::key_type_name() {
            return Self::Projection;
        }

        if let Some(key) = key.downcast_ref::<ChainKey>() {
            return Self::Chain(key.0);
        }
        if key.downcast_ref::<MaybeTransientKey>().is_some() {
            return Self::MaybeTransient;
        }
        if key.downcast_ref::<InheritedTransientKey>().is_some() {
            return Self::InheritedTransient;
        }
        if key.key_type_name() == TransientProjection::key_type_name() {
            return Self::TransientProjection;
        }
        if key.downcast_ref::<RetainedTransientKey>().is_some() {
            return Self::RetainedTransient;
        }

        panic!("Unexpected key: {key}")
    }
}

#[derive(Allocative, Clone, Debug, Eq, PartialEq)]
struct RichEvent {
    key: Kind,
    node: DiceNodeId,
    version: VersionNumber,
    kind: ActivationKind,
    has_data: bool,
    dependencies: Vec<DiceNodeId>,
}

#[derive(Allocative, Clone, Debug, Eq, PartialEq)]
struct RootEvent {
    key: Kind,
    node: DiceNodeId,
    version: VersionNumber,
    ordinal: u64,
    activation: RootActivation,
}

#[derive(PartialEq, Eq, Debug, Dupe, Clone, Allocative)]
struct Data;

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct Injected;

#[async_trait]
impl InjectedKey for Injected {
    type Value = i32;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Dupe, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("{:?}", self)]
struct Stage0;

#[async_trait]
impl Key for Stage0 {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.store_evaluation_data(Data).unwrap();
        ctx.compute(&Injected).await.unwrap();
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }
}

#[derive(Clone, Dupe, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("{:?}", self)]
struct Stage1;

#[async_trait]
impl Key for Stage1 {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.store_evaluation_data(Data).unwrap();
        ctx.compute(&Stage0).await.unwrap()
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct Selector;

#[async_trait]
impl InjectedKey for Selector {
    type Value = bool;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
enum GraphKey {
    OldLeaf,
    NewLeaf,
    Shared,
    Left,
    Right,
    Branch,
    DelayedBranch,
}

#[async_trait]
impl Key for GraphKey {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self {
            Self::Left | Self::Right => {
                ctx.compute(&Self::Shared).await.unwrap();
            }
            Self::Branch | Self::DelayedBranch => {
                if ctx.compute(&Selector).await.unwrap() {
                    if matches!(self, Self::DelayedBranch) {
                        wait_for_test_gate(ctx).await;
                    }
                    ctx.compute(&Self::NewLeaf).await.unwrap();
                } else {
                    if matches!(self, Self::DelayedBranch) {
                        wait_for_test_gate(ctx).await;
                    }
                    ctx.compute(&Self::OldLeaf).await.unwrap();
                }
            }
            Self::OldLeaf | Self::NewLeaf | Self::Shared => {}
        }
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }
}

struct TestGate {
    started: Arc<Notify>,
    release: Arc<Notify>,
}

async fn wait_for_test_gate(ctx: &DiceComputations<'_>) {
    if let Ok(gate) = ctx.per_transaction_data().data.get::<TestGate>() {
        gate.started.notify_one();
        gate.release.notified().await;
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct TransientKey;

#[async_trait]
impl Key for TransientKey {
    type Value = ();

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct ValueKey;

#[async_trait]
impl Key for ValueKey {
    type Value = i32;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        1
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct ValueProjection;

impl ProjectionKey for ValueProjection {
    type DeriveFromKey = ValueKey;
    type Value = i32;

    fn compute(&self, derive_from: &i32, _ctx: &DiceProjectionComputations) -> Self::Value {
        *derive_from
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct MaybeTransientKey;

#[async_trait]
impl Key for MaybeTransientKey {
    type Value = i32;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> i32 {
        ctx.compute(&Injected).await.unwrap()
    }

    fn equality(x: &i32, y: &i32) -> bool {
        x == y
    }

    fn validity(value: &i32) -> bool {
        *value >= 0
    }

    fn retain_transient_activation(value: &i32) -> bool {
        *value == -2
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct InheritedTransientKey {
    independently_transient: bool,
}

#[async_trait]
impl Key for InheritedTransientKey {
    type Value = i32;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> i32 {
        let value = ctx.compute(&MaybeTransientKey).await.unwrap();
        if self.independently_transient {
            i32::MIN
        } else {
            value
        }
    }

    fn equality(x: &i32, y: &i32) -> bool {
        x == y
    }

    fn validity(value: &i32) -> bool {
        *value != i32::MIN
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct TransientProjection(bool);

impl ProjectionKey for TransientProjection {
    type DeriveFromKey = MaybeTransientKey;
    type Value = i32;

    fn compute(&self, _: &i32, _: &DiceProjectionComputations) -> i32 {
        if self.0 { -1 } else { 1 }
    }

    fn equality(x: &i32, y: &i32) -> bool {
        x == y
    }

    fn validity(value: &i32) -> bool {
        *value >= 0
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct RetainedTransientKey {
    include_unmarked: bool,
}

#[async_trait]
impl Key for RetainedTransientKey {
    type Value = i32;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> i32 {
        let (projected, stable) = ctx
            .compute2(
                |ctx| {
                    async move {
                        let base = ctx.compute_opaque(&MaybeTransientKey).await.unwrap();
                        ctx.projection(&base, &TransientProjection(false)).unwrap()
                    }
                    .boxed()
                },
                |ctx| ctx.compute(&GraphKey::Left).boxed(),
            )
            .await;
        assert_eq!(projected, 1);
        stable.unwrap();
        if self.include_unmarked {
            ctx.compute(&TransientKey).await.unwrap();
        }
        ctx.compute(&Injected).await.unwrap()
    }

    fn equality(x: &i32, y: &i32) -> bool {
        x == y
    }
}

#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("chain({})", _0)]
struct ChainKey(u32);

#[async_trait]
impl Key for ChainKey {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if self.0 != 0 {
            ctx.compute(&Self(self.0 - 1)).await.unwrap();
        }
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }
}

async fn test_events_impl(builder: DiceDataBuilder) -> anyhow::Result<()> {
    let dice = builder.build(DetectCycles::Enabled);

    {
        let activation_tracker = Arc::new(Tracker::new());

        let data = UserComputationData {
            activation_tracker: Some(activation_tracker.dupe()),
            ..Default::default()
        };

        let mut updater = dice.updater_with_data(data);
        updater.changed_to(vec![(Injected, 123)])?;

        let mut transaction = updater.commit().await;
        transaction.compute(&Stage1).await?;

        assert_eq!(
            &*activation_tracker.state.lock().unwrap(),
            &[
                (Kind::Stage0, vec![Kind::Injected], Some(Data), false),
                (Kind::Stage1, vec![Kind::Stage0], Some(Data), false),
            ]
        );
    }

    {
        let activation_tracker = Arc::new(Tracker::default());

        let data = UserComputationData {
            activation_tracker: Some(activation_tracker.dupe()),
            ..Default::default()
        };

        // Change the value.
        let mut updater = dice.updater_with_data(data);
        updater.changed_to(vec![(Injected, 456)])?;

        let mut transaction = updater.commit().await;
        transaction.compute(&Stage1).await?;

        assert_eq!(
            &*activation_tracker.state.lock().unwrap(),
            &[
                (Kind::Stage0, vec![Kind::Injected], Some(Data), false),
                (Kind::Stage1, vec![Kind::Stage0], None, true),
            ]
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_events_modern() -> anyhow::Result<()> {
    test_events_impl(Dice::builder()).await
}

fn user_data(tracker: &Arc<Tracker>) -> UserComputationData {
    UserComputationData {
        activation_tracker: Some(tracker.dupe()),
        ..Default::default()
    }
}

fn node_for(tracker: &Tracker, kind: Kind) -> DiceNodeId {
    tracker
        .rich
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|event| event.key == kind)
        .unwrap_or_else(|| panic!("no rich activation for {kind:?}"))
        .node
}

#[tokio::test]
async fn rich_callbacks_preserve_legacy_frequency_and_exact_metadata() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Injected, 1)])?;
    let mut transaction = updater.commit().await;

    transaction.compute(&Stage1).await?;
    let version = tracker.roots.lock().unwrap()[0].version;
    let rich = tracker.rich.lock().unwrap().clone();
    assert_eq!(rich.len(), 3);
    assert_eq!(rich[0].key, Kind::Injected);
    assert_eq!(rich[0].kind, ActivationKind::Reused);
    assert_eq!(rich[1].key, Kind::Stage0);
    assert_eq!(rich[1].kind, ActivationKind::Evaluated);
    assert!(rich[1].has_data);
    assert_eq!(rich[1].version, version);
    assert_eq!(rich[1].dependencies, vec![rich[0].node]);
    assert_eq!(rich[2].key, Kind::Stage1);
    assert_eq!(rich[2].dependencies, vec![rich[1].node]);
    assert_eq!(tracker.state.lock().unwrap().len(), 2);

    transaction.compute(&Stage1).await?;
    assert_eq!(tracker.state.lock().unwrap().len(), 2);
    let rich = tracker.rich.lock().unwrap();
    assert_eq!(rich.len(), 4);
    assert_eq!(rich[3].key, Kind::Stage1);
    assert_eq!(rich[3].kind, ActivationKind::Reused);
    assert_eq!(rich[3].node, rich[2].node);
    assert_eq!(rich[3].dependencies, vec![rich[1].node]);
    drop(rich);

    drop(transaction);
    let mut graph_match = dice.updater_with_data(user_data(&tracker)).commit().await;
    graph_match.compute(&Stage1).await?;
    assert_eq!(tracker.state.lock().unwrap().len(), 2);
    let rich = tracker.rich.lock().unwrap();
    assert_eq!(rich.len(), 5);
    assert_eq!(rich[4].kind, ActivationKind::Reused);
    assert_eq!(rich[4].dependencies, vec![rich[1].node]);
    drop(rich);

    drop(graph_match);
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Injected, 2)])?;
    let mut dependency_check = updater.commit().await;
    dependency_check.compute(&Stage1).await?;
    let legacy = tracker.state.lock().unwrap();
    assert_eq!(legacy.len(), 4);
    assert_eq!(legacy[2].0, Kind::Stage0);
    assert!(!legacy[2].3);
    assert_eq!(legacy[3].0, Kind::Stage1);
    assert!(legacy[3].3);
    Ok(())
}

#[tokio::test]
async fn parentless_root_ordinals_are_clone_shared_and_include_cached_projections()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut transaction = dice.updater_with_data(user_data(&tracker)).commit().await;
    let mut clone = transaction.dupe();

    transaction.compute(&ValueKey).await?;
    clone.compute(&ValueKey).await?;
    let opaque = transaction.compute_opaque(&ValueKey).await?;
    assert_eq!(transaction.projection(&opaque, &ValueProjection)?, 1);
    assert_eq!(transaction.projection(&opaque, &ValueProjection)?, 1);

    let roots = tracker.roots.lock().unwrap();
    assert_eq!(
        roots.iter().map(|root| root.ordinal).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        roots.iter().map(|root| root.key.dupe()).collect::<Vec<_>>(),
        vec![
            Kind::Value,
            Kind::Value,
            Kind::Value,
            Kind::Projection,
            Kind::Projection,
        ]
    );
    assert!(roots.iter().all(|root| root.version == roots[0].version));
    assert_eq!(roots[0].node, roots[1].node);
    assert_eq!(roots[0].node, roots[2].node);
    assert_eq!(roots[3].node, roots[4].node);
    assert_eq!(
        tracker.completed.lock().unwrap().as_slice(),
        roots
            .iter()
            .cloned()
            .map(|root| (root, false))
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn root_completions_track_actual_validity_through_repair_and_cached_projections()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut prior_identity = None;
    for value in [1, -1, -2, 2] {
        let tracker = Arc::new(Tracker::new());
        let mut updater = dice.updater_with_data(user_data(&tracker));
        updater.changed_to(vec![(Injected, value)])?;
        let mut transaction = updater.commit().await;
        // The parent declares these values valid: its actual transience must
        // still include its dependency, including on the second cached request.
        let inherited = InheritedTransientKey {
            independently_transient: false,
        };
        assert_eq!(transaction.compute(&inherited).await?, value);
        assert_eq!(transaction.compute(&inherited).await?, value);
        let opaque = transaction.compute_opaque(&MaybeTransientKey).await?;
        for transient in [false, true] {
            for _ in 0..2 {
                assert_eq!(
                    transaction.projection(&opaque, &TransientProjection(transient))?,
                    if transient { -1 } else { 1 }
                );
            }
        }
        let roots = tracker.roots.lock().unwrap();
        let completed = tracker.completed.lock().unwrap();
        assert_eq!(roots.len(), 7);
        assert_eq!(completed.len(), roots.len());
        for (ordinal, (root, (completion, transient))) in
            roots.iter().zip(completed.iter()).enumerate()
        {
            assert_eq!(root.ordinal, ordinal as u64);
            assert_eq!(root.version, transaction.version());
            assert_eq!(root, completion);
            assert_eq!(*transient, value < 0 || ordinal >= 5);
        }
        let identity = roots.iter().map(|root| root.node).collect::<Vec<_>>();
        if let Some(prior) = prior_identity {
            assert_eq!(identity, prior);
        }
        prior_identity = Some(identity);
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_parentless_request_has_start_without_completion() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut data = user_data(&tracker);
    data.data.set(TestGate {
        started: started.dupe(),
        release: release.dupe(),
    });
    let mut updater = dice.updater_with_data(data);
    updater.changed_to(vec![(Selector, false)])?;
    let mut transaction = updater.commit().await;
    let closure_reader = transaction.dupe();
    let activation;
    {
        let request = transaction.compute(&GraphKey::DelayedBranch);
        tokio::pin!(request);
        tokio::select! {
            _ = started.notified() => {},
            result = &mut request => panic!("gated request completed early: {result:?}"),
        }
        assert_eq!(tracker.roots.lock().unwrap().len(), 1);
        assert!(tracker.completed.lock().unwrap().is_empty());
        activation = tracker.roots.lock().unwrap()[0].activation;
        assert_eq!(
            closure_reader
                .activation_closure_with_transient_root(
                    activation,
                    closure_reader.live_context_id()
                )
                .await
                .unwrap_err(),
            ActivationClosureError::NotVerified {
                node: activation.node(),
                version: closure_reader.version(),
            }
        );
    }
    assert_eq!(
        transaction
            .activation_closure_with_transient_root(activation, transaction.live_context_id())
            .await
            .unwrap_err(),
        ActivationClosureError::NotVerified {
            node: activation.node(),
            version: transaction.version(),
        }
    );
    release.notify_one();
    transaction.compute(&ValueKey).await?;
    let roots = tracker.roots.lock().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(
        tracker.completed.lock().unwrap().as_slice(),
        &[(roots[1].clone(), false)]
    );
    Ok(())
}

#[tokio::test]
async fn activation_closure_preserves_roots_dependency_order_and_shared_dedup() -> anyhow::Result<()>
{
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut transaction = dice.updater_with_data(user_data(&tracker)).commit().await;
    transaction.compute(&GraphKey::Left).await?;
    transaction.compute(&GraphKey::Right).await?;

    let shared = node_for(&tracker, Kind::Graph(GraphKey::Shared));
    let left = node_for(&tracker, Kind::Graph(GraphKey::Left));
    let right = node_for(&tracker, Kind::Graph(GraphKey::Right));
    let closure = transaction.activation_closure([left, right, left]).await?;
    assert_eq!(closure.roots(), &[left, right, left]);
    assert_eq!(
        closure
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![shared, left, right]
    );
    assert!(closure.nodes()[0].dependencies().is_empty());
    assert_eq!(closure.nodes()[1].dependencies(), &[shared]);
    assert_eq!(closure.nodes()[2].dependencies(), &[shared]);
    assert_eq!(closure.version(), tracker.roots.lock().unwrap()[0].version);

    let empty = transaction.activation_closure([]).await?;
    assert!(empty.roots().is_empty());
    assert!(empty.nodes().is_empty());
    Ok(())
}

#[tokio::test]
async fn transient_activation_closure_inherits_only_through_locally_valid_serial_parent()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Injected, -2)])?;
    let mut inherited = updater.commit().await;
    let key = InheritedTransientKey {
        independently_transient: false,
    };
    assert_eq!(inherited.compute(&key).await?, -2);
    let (completion, transient) = tracker.completed.lock().unwrap()[0].clone();
    assert!(transient);
    let closure = inherited
        .activation_closure_with_transient_root(completion.activation, inherited.live_context_id())
        .await?;
    let injected = node_for(&tracker, Kind::Injected);
    let marked = node_for(&tracker, Kind::MaybeTransient);
    assert_eq!(
        closure
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![injected, marked, completion.node],
    );
    assert_eq!(closure.nodes()[1].dependencies(), &[injected]);
    assert_eq!(closure.nodes()[2].dependencies(), &[marked]);

    let tracker = Arc::new(Tracker::new());
    let mut independent = dice.updater_with_data(user_data(&tracker)).commit().await;
    assert_eq!(independent.live_context_id(), inherited.live_context_id());
    let key = InheritedTransientKey {
        independently_transient: true,
    };
    assert_eq!(independent.compute(&key).await?, i32::MIN);
    let (completion, transient) = tracker.completed.lock().unwrap()[0].clone();
    assert!(transient);
    {
        let rich = tracker.rich.lock().unwrap();
        let root = rich
            .iter()
            .find(|event| event.node == completion.node)
            .unwrap();
        assert_eq!(root.dependencies, vec![marked]);
    }
    assert_eq!(
        independent
            .activation_closure_with_transient_root(
                completion.activation,
                independent.live_context_id()
            )
            .await
            .unwrap_err(),
        ActivationClosureError::NotVerified {
            node: completion.node,
            version: independent.version(),
        }
    );
    Ok(())
}

#[tokio::test]
async fn transient_activation_closure_includes_projection_and_exact_stable_dependencies()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = RetainedTransientKey {
        include_unmarked: false,
    };
    let mut expected_nodes: Option<Vec<crate::ActivationClosureNode>> = None;
    for (index, value) in [-2, 1, -2].into_iter().enumerate() {
        let tracker = Arc::new(Tracker::new());
        let mut updater = dice.updater_with_data(user_data(&tracker));
        updater.changed_to(vec![(Injected, value)])?;
        let mut transaction = updater.commit().await;
        assert_eq!(transaction.compute(&key).await?, value);
        let (activation, is_transient) = {
            let completed = tracker.completed.lock().unwrap();
            assert_eq!(completed.len(), 1);
            (completed[0].0.activation, completed[0].1)
        };
        let root = activation.node();
        let version = transaction.version();
        assert_eq!(is_transient, value < 0);
        if value < 0 {
            let expected = if index == 0 {
                ActivationClosureError::UnavailableRoot { root }
            } else {
                ActivationClosureError::Dirty {
                    node: root,
                    version,
                }
            };
            assert_eq!(
                transaction.activation_closure([root]).await.unwrap_err(),
                expected
            );
            if index == 0 {
                assert_eq!(transaction.activation_history_len(root).await, 0);
            }
        }
        let closure = if value < 0 {
            transaction
                .activation_closure_with_transient_root(activation, transaction.live_context_id())
                .await?
        } else {
            assert_eq!(
                transaction
                    .activation_closure_with_transient_root(
                        activation,
                        transaction.live_context_id()
                    )
                    .await
                    .unwrap_err(),
                ActivationClosureError::NotVerified {
                    node: root,
                    version,
                }
            );
            transaction.activation_closure([root]).await?
        };
        assert_eq!(closure.roots(), &[root]);
        assert_eq!(closure.version(), version);
        let nodes = closure.nodes();
        assert_eq!(nodes.len(), 6);
        let [injected, base, projection, shared, left, terminal] = nodes else {
            unreachable!();
        };
        assert_eq!(injected.node(), node_for(&tracker, Kind::Injected));
        assert_eq!(base.node(), node_for(&tracker, Kind::MaybeTransient));
        assert_eq!(left.node(), node_for(&tracker, Kind::Graph(GraphKey::Left)));
        assert_eq!(terminal.node(), root);
        assert!(injected.dependencies().is_empty());
        assert_eq!(base.dependencies(), &[injected.node()]);
        assert_eq!(projection.dependencies(), &[base.node()]);
        assert!(shared.dependencies().is_empty());
        assert_eq!(left.dependencies(), &[shared.node()]);
        assert_eq!(
            terminal.dependencies(),
            &[projection.node(), left.node(), injected.node()]
        );
        if let Some(expected) = &expected_nodes {
            assert_eq!(nodes, expected.as_slice());
        } else {
            assert_eq!(
                shared.node(),
                node_for(&tracker, Kind::Graph(GraphKey::Shared))
            );
            expected_nodes = Some(nodes.to_vec());
        }
    }
    Ok(())
}

#[tokio::test]
async fn opaque_consumption_checks_origin_before_using_cached_projection() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater.changed_to([(Injected, -2)])?;
    let first = updater.commit().await;
    let old_base = first.compute_opaque(&MaybeTransientKey).await?;
    let mut peer = dice.updater().commit().await;
    let projection = TransientProjection(false);
    assert_eq!(peer.projection(&old_base, &projection)?, 1);
    assert_eq!(
        peer.opaque_into_value(first.compute_opaque(&MaybeTransientKey).await?)?,
        -2
    );

    let mut updater = dice.updater();
    updater.changed_to([(Injected, -3)])?;
    let mut changed = updater.commit().await;
    let current_base = changed.compute_opaque(&MaybeTransientKey).await?;
    assert_eq!(changed.projection(&current_base, &projection)?, 1);
    assert!(matches!(
        &*changed.projection(&old_base, &projection).unwrap_err().0,
        DiceErrorImpl::ForeignOpaqueValue
    ));
    assert!(matches!(
        &*changed.opaque_into_value(old_base).unwrap_err().0,
        DiceErrorImpl::ForeignOpaqueValue
    ));
    assert_eq!(changed.opaque_into_value(current_base)?, -3);
    Ok(())
}

#[tokio::test]
async fn transient_activation_closure_fences_root_receipts_and_unmarked_dependencies()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = RetainedTransientKey {
        include_unmarked: false,
    };
    let tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Injected, -2)])?;
    let mut old = updater.commit().await;
    assert_eq!(old.compute(&key).await?, -2);
    let root = tracker.completed.lock().unwrap()[0].0.activation;
    let old_context = old.live_context_id();
    let closure = old
        .activation_closure_with_transient_root(root, old_context)
        .await?;

    let reused_tracker = Arc::new(Tracker::new());
    let mut reused = dice
        .updater_with_data(user_data(&reused_tracker))
        .commit()
        .await;
    assert_eq!(reused.live_context_id(), old_context);
    assert_eq!(reused.compute(&key).await?, -2);
    let reused_root = reused_tracker.completed.lock().unwrap()[0].0.activation;
    assert_eq!(
        reused
            .activation_closure_with_transient_root(reused_root, reused.live_context_id())
            .await?,
        closure
    );
    assert_eq!(reused.compute(&key).await?, -2);
    let second_root = reused_tracker.completed.lock().unwrap()[1].0.activation;
    let not_verified = ActivationClosureError::NotVerified {
        node: root.node(),
        version: old.version(),
    };
    assert_eq!(
        reused
            .activation_closure_with_transient_root(second_root, old_context)
            .await
            .unwrap_err(),
        not_verified
    );
    drop(reused);
    drop(old);

    let tracker = Arc::new(Tracker::new());
    let mut reopened = dice.updater_with_data(user_data(&tracker)).commit().await;
    assert_eq!(reopened.version(), root.version());
    assert_ne!(reopened.live_context_id(), old_context);
    assert_eq!(reopened.compute(&key).await?, -2);
    let reopened_root = tracker.completed.lock().unwrap()[0].0.activation;
    assert_eq!(
        reopened
            .activation_closure_with_transient_root(reopened_root, reopened.live_context_id())
            .await?,
        closure
    );
    assert_eq!(
        reopened
            .activation_closure_with_transient_root(root, reopened.live_context_id())
            .await
            .unwrap_err(),
        not_verified
    );

    let other = Dice::builder().build(DetectCycles::Enabled);
    let other_transaction = other.updater().commit().await;
    assert_eq!(
        other_transaction
            .activation_closure_with_transient_root(root, other_transaction.live_context_id())
            .await
            .unwrap_err(),
        ActivationClosureError::ForeignEngine { node: root.node() }
    );

    let tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Selector, true)])?;
    let mut newer = updater.commit().await;
    assert!(newer.version() > reopened.version());
    assert_eq!(
        newer
            .activation_closure_with_transient_root(reopened_root, newer.live_context_id())
            .await
            .unwrap_err(),
        ActivationClosureError::NotVerified {
            node: root.node(),
            version: newer.version(),
        }
    );
    assert_eq!(
        newer
            .compute(&RetainedTransientKey {
                include_unmarked: true
            })
            .await?,
        -2
    );
    let includes_unmarked = tracker.completed.lock().unwrap()[0].0.activation;
    let unmarked = node_for(&tracker, Kind::Transient);
    assert_eq!(
        newer
            .activation_closure_with_transient_root(includes_unmarked, newer.live_context_id())
            .await
            .unwrap_err(),
        ActivationClosureError::NotVerified {
            node: unmarked,
            version: newer.version(),
        }
    );
    Ok(())
}

#[tokio::test]
async fn activation_closure_uses_nonrecursive_deep_traversal() -> anyhow::Result<()> {
    const DEPTH: u32 = 4096;
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut transaction = dice.updater_with_data(user_data(&tracker)).commit().await;
    transaction.compute(&ChainKey(DEPTH)).await?;
    let root = node_for(&tracker, Kind::Chain(DEPTH));
    let leaf = node_for(&tracker, Kind::Chain(0));
    let closure = transaction.activation_closure([root]).await?;
    assert_eq!(closure.nodes().len(), DEPTH as usize + 1);
    assert_eq!(closure.nodes().first().unwrap().node(), leaf);
    assert_eq!(closure.nodes().last().unwrap().node(), root);
    Ok(())
}

#[tokio::test]
async fn activation_closure_is_exact_version_and_excludes_abandoned_branches() -> anyhow::Result<()>
{
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let old_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&old_tracker));
    updater.changed_to(vec![(Selector, false)])?;
    let mut old = updater.commit().await;
    old.compute(&GraphKey::Branch).await?;
    let branch = node_for(&old_tracker, Kind::Graph(GraphKey::Branch));
    let selector = node_for(&old_tracker, Kind::Selector);
    let old_leaf = node_for(&old_tracker, Kind::Graph(GraphKey::OldLeaf));

    let new_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&new_tracker));
    updater.changed_to(vec![(Selector, true)])?;
    let mut new = updater.commit().await;
    new.compute(&GraphKey::Branch).await?;
    let new_leaf = node_for(&new_tracker, Kind::Graph(GraphKey::NewLeaf));

    let old_rich_len = old_tracker.rich.lock().unwrap().len();
    old.compute(&GraphKey::Branch).await?;
    let old_rich = old_tracker.rich.lock().unwrap();
    assert_eq!(old_rich.len(), old_rich_len + 1);
    assert_eq!(old_rich.last().unwrap().kind, ActivationKind::Reused);
    assert_eq!(
        old_rich.last().unwrap().dependencies,
        vec![selector, old_leaf]
    );
    drop(old_rich);

    let old_closure = old.activation_closure([branch]).await?;
    assert_eq!(
        old_closure
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![selector, old_leaf, branch]
    );
    let new_closure = new.activation_closure([branch]).await?;
    assert_eq!(
        new_closure
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![selector, new_leaf, branch]
    );
    assert!(
        !new_closure
            .nodes()
            .iter()
            .any(|node| node.node() == old_leaf)
    );
    Ok(())
}

#[tokio::test]
async fn activation_closure_includes_untouched_cached_intermediates() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let old_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&old_tracker));
    updater.changed_to(vec![(Injected, 1)])?;
    let mut old = updater.commit().await;
    old.compute(&Stage1).await?;
    let injected = node_for(&old_tracker, Kind::Injected);
    let stage0 = node_for(&old_tracker, Kind::Stage0);
    let stage1 = node_for(&old_tracker, Kind::Stage1);
    drop(old);

    let new_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&new_tracker));
    updater.changed_to(vec![(Selector, false)])?;
    let mut new = updater.commit().await;
    new.compute(&Stage1).await?;
    let closure = new.activation_closure([stage1]).await?;
    assert_eq!(
        closure
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![injected, stage0, stage1]
    );
    Ok(())
}

#[tokio::test]
async fn out_of_order_old_completion_preserves_each_terminal_version() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let old_tracker = Arc::new(Tracker::new());
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut old_data = user_data(&old_tracker);
    old_data.data.set(TestGate {
        started: started.dupe(),
        release: release.dupe(),
    });
    let mut updater = dice.updater_with_data(old_data);
    updater.changed_to(vec![(Selector, false)])?;
    let old = updater.commit().await;
    let mut old_compute = old.dupe();
    let old_task = tokio::spawn(async move {
        old_compute.compute(&GraphKey::DelayedBranch).await.unwrap();
    });
    started.notified().await;
    assert_eq!(old_tracker.roots.lock().unwrap().len(), 1);
    assert!(old_tracker.completed.lock().unwrap().is_empty());

    let new_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&new_tracker));
    updater.changed_to(vec![(Selector, true)])?;
    let mut new = updater.commit().await;
    new.compute(&GraphKey::DelayedBranch).await?;
    assert_eq!(new_tracker.completed.lock().unwrap().len(), 1);
    assert!(old_tracker.completed.lock().unwrap().is_empty());
    release.notify_one();
    old_task.await.unwrap();
    for tracker in [&old_tracker, &new_tracker] {
        assert_eq!(
            tracker.completed.lock().unwrap().as_slice(),
            &[(tracker.roots.lock().unwrap()[0].clone(), false)]
        );
    }

    let branch = node_for(&new_tracker, Kind::Graph(GraphKey::DelayedBranch));
    let selector = node_for(&new_tracker, Kind::Selector);
    let new_leaf = node_for(&new_tracker, Kind::Graph(GraphKey::NewLeaf));
    let old_leaf = node_for(&old_tracker, Kind::Graph(GraphKey::OldLeaf));
    assert_eq!(
        old.activation_closure([branch])
            .await?
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![selector, old_leaf, branch]
    );
    assert_eq!(
        new.activation_closure([branch])
            .await?
            .nodes()
            .iter()
            .map(|node| node.node())
            .collect::<Vec<_>>(),
        vec![selector, new_leaf, branch]
    );
    Ok(())
}

#[tokio::test]
async fn activation_history_prunes_only_after_final_old_guard_drops() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let old_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&old_tracker));
    updater.changed_to(vec![(Selector, false)])?;
    let mut old = updater.commit().await;
    old.compute(&GraphKey::Branch).await?;
    let branch = node_for(&old_tracker, Kind::Graph(GraphKey::Branch));
    let old_guard = old.dupe();

    let new_tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&new_tracker));
    updater.changed_to(vec![(Selector, true)])?;
    let mut new = updater.commit().await;
    new.compute(&GraphKey::Branch).await?;
    assert_eq!(new.activation_history_len(branch).await, 2);

    drop(old);
    assert_eq!(new.activation_history_len(branch).await, 2);
    drop(old_guard);
    assert_eq!(new.activation_history_len(branch).await, 1);
    Ok(())
}

#[tokio::test]
async fn activation_closure_returns_typed_foreign_unavailable_dirty_and_unverified_errors()
-> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::new());
    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Selector, false)])?;
    let mut old = updater.commit().await;
    old.compute(&GraphKey::Branch).await?;
    old.compute(&TransientKey).await?;
    let branch = node_for(&tracker, Kind::Graph(GraphKey::Branch));
    let transient = node_for(&tracker, Kind::Transient);
    assert_eq!(
        old.activation_closure([transient]).await.unwrap_err(),
        ActivationClosureError::UnavailableRoot { root: transient }
    );

    let other_dice = Dice::builder().build(DetectCycles::Enabled);
    let other_tracker = Arc::new(Tracker::new());
    let mut other = other_dice
        .updater_with_data(user_data(&other_tracker))
        .commit()
        .await;
    other.compute(&ValueKey).await?;
    let foreign = node_for(&other_tracker, Kind::Value);
    assert_eq!(
        old.activation_closure([foreign]).await.unwrap_err(),
        ActivationClosureError::ForeignEngine { node: foreign }
    );

    old.remove_activation_history(branch).await;
    assert_eq!(
        old.activation_closure([branch]).await.unwrap_err(),
        ActivationClosureError::NotVerified {
            node: branch,
            version: old.version(),
        }
    );

    let mut updater = dice.updater_with_data(user_data(&tracker));
    updater.changed_to(vec![(Selector, true)])?;
    let dirty = updater.commit().await;
    assert_eq!(
        dirty.activation_closure([branch]).await.unwrap_err(),
        ActivationClosureError::Dirty {
            node: branch,
            version: dirty.version(),
        }
    );

    let cleared = old.dupe().into_updater().unstable_take().commit().await;
    assert_eq!(
        old.activation_closure([branch]).await.unwrap_err(),
        ActivationClosureError::NotVerified {
            node: branch,
            version: old.version(),
        }
    );
    let roots_before_failed_request = tracker.roots.lock().unwrap().len();
    let completions_before_failed_request = tracker.completed.lock().unwrap().len();
    assert!(old.compute(&ValueKey).await.is_err());
    assert_eq!(
        tracker.completed.lock().unwrap().len(),
        completions_before_failed_request
    );
    let roots = tracker.roots.lock().unwrap();
    assert_eq!(roots.len(), roots_before_failed_request + 1);
    assert_eq!(roots.last().unwrap().key, Kind::Value);
    assert_eq!(
        roots.last().unwrap().ordinal,
        roots[roots_before_failed_request - 1].ordinal + 1
    );
    drop(cleared);
    Ok(())
}
