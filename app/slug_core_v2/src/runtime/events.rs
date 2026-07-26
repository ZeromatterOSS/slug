#![allow(dead_code)] // Activated by the later shared command driver.

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
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use dice::ActivationClosure;
use dice::ActivationClosureError;
use dice::ActivationKind;
use dice::DiceNodeId;
use dice::DiceTransaction;
use dice::RichActivation;
use dice::RootActivation;
use dice::VersionNumber;
use dupe::Dupe;
use slug_events_v2::EventBatch;
use starlark_map::small_map::SmallMap;

use super::demands::DemandProvenanceError;
use super::demands::SelectedWorkspaceDemands;
use super::demands::WorkspaceDemandOwner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandAttemptId(u64);

#[derive(Debug)]
struct ActiveCommandAttempt {
    id: CommandAttemptId,
    installed: bool,
    demands: Option<Weak<WorkspaceDemandOwner>>,
    roots: Vec<AttemptRoot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AttemptRoot {
    ordinal: u64,
    node: DiceNodeId,
    version: VersionNumber,
}

#[derive(Debug)]
struct EventBatchTransition {
    version: VersionNumber,
    batch: Option<EventBatch>,
}

#[derive(Debug)]
enum CommandEffectPhase {
    Idle,
    Open(ActiveCommandAttempt),
    Terminal(CommandAttemptId),
}

#[derive(Debug)]
struct CommandEffectState {
    next_attempt: u64,
    phase: CommandEffectPhase,
    lineage: SmallMap<DiceNodeId, Vec<EventBatchTransition>>,
}

/// One command-local owner for event lineage across serial DICE attempts.
#[derive(Debug)]
pub(super) struct CommandEffectOwner {
    state: Mutex<CommandEffectState>,
}

/// One attempt-scoped rich DICE activation tracker.
#[derive(Debug)]
pub(super) struct AttemptEffectTracker {
    owner: Arc<CommandEffectOwner>,
    id: CommandAttemptId,
}

/// A terminal attempt whose exact-version activation closure may be selected.
#[derive(Debug)]
pub(super) struct SealedCommandAttempt {
    owner: Arc<CommandEffectOwner>,
    demands: Weak<WorkspaceDemandOwner>,
    id: CommandAttemptId,
    version: VersionNumber,
    roots: Arc<[DiceNodeId]>,
}

/// Ordered command-local batches selected from one exact terminal closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedEventBatches {
    batches: Arc<[EventBatch]>,
}

/// Infallible command-owned logical output, exposed only after acceptance.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CommandOutputBuffer {
    batches: Arc<[EventBatch]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedCommandSidecars {
    events: SelectedEventBatches,
    demands: SelectedWorkspaceDemands,
}

impl SelectedCommandSidecars {
    pub(super) fn events(&self) -> &SelectedEventBatches {
        &self.events
    }

    pub(super) fn demands(&self) -> &SelectedWorkspaceDemands {
        &self.demands
    }

    pub(super) fn into_parts(self) -> (SelectedEventBatches, SelectedWorkspaceDemands) {
        (self.events, self.demands)
    }

    #[cfg(test)]
    pub(super) fn for_test(demands: SelectedWorkspaceDemands) -> Self {
        Self {
            events: SelectedEventBatches {
                batches: Arc::from([]),
            },
            demands,
        }
    }
}

impl SelectedEventBatches {
    pub(super) fn batches(&self) -> &[EventBatch] {
        &self.batches
    }

    pub(super) fn into_output_buffer(self) -> CommandOutputBuffer {
        CommandOutputBuffer {
            batches: self.batches,
        }
    }
}

impl CommandOutputBuffer {
    pub(super) fn batches(&self) -> &[EventBatch] {
        &self.batches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CommandEffectError {
    AttemptBusy,
    AttemptIdExhausted,
    CommandFinished,
    StaleAttempt,
    ActivationTrackerAlreadyInstalled,
    ForeignDemandOwner,
    AttemptTrackerAlreadyInstalled,
    DemandOwnerNotInstalled,
    DemandOwnerExpired,
    NoTerminalRoots,
    RootOrdinal {
        expected: u64,
        actual: u64,
    },
    MixedRootVersions,
    ClosureVersion {
        expected: VersionNumber,
        actual: VersionNumber,
    },
    ClosureRoots,
    Closure(ActivationClosureError),
    Demand(DemandProvenanceError),
}

impl fmt::Display for CommandEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttemptBusy => f.write_str("a command effect attempt is already open"),
            Self::AttemptIdExhausted => f.write_str("command effect attempt IDs are exhausted"),
            Self::CommandFinished => f.write_str("the command effect owner is terminal"),
            Self::StaleAttempt => f.write_str("the command effect attempt is stale"),
            Self::ActivationTrackerAlreadyInstalled => {
                f.write_str("a DICE activation tracker is already installed")
            }
            Self::ForeignDemandOwner => {
                f.write_str("the demand owner belongs to a different DICE engine")
            }
            Self::AttemptTrackerAlreadyInstalled => {
                f.write_str("the command effect attempt tracker is already installed")
            }
            Self::DemandOwnerNotInstalled => {
                f.write_str("the command effect attempt has no installed demand owner")
            }
            Self::DemandOwnerExpired => {
                f.write_str("the installed workspace demand owner has expired")
            }
            Self::NoTerminalRoots => f.write_str("the terminal attempt has no DICE roots"),
            Self::RootOrdinal { expected, actual } => {
                write!(
                    f,
                    "root activation ordinal {actual} did not match {expected}"
                )
            }
            Self::MixedRootVersions => {
                f.write_str("terminal root activations span multiple DICE versions")
            }
            Self::ClosureVersion { expected, actual } => {
                write!(
                    f,
                    "activation closure version {actual} did not match {expected}"
                )
            }
            Self::ClosureRoots => {
                f.write_str("activation closure roots did not match the terminal attempt")
            }
            Self::Closure(error) => write!(f, "reading the activation closure failed: {error}"),
            Self::Demand(error) => write!(f, "selecting workspace demands failed: {error}"),
        }
    }
}

impl std::error::Error for CommandEffectError {}

impl CommandEffectOwner {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(CommandEffectState {
                next_attempt: 1,
                phase: CommandEffectPhase::Idle,
                lineage: SmallMap::new(),
            }),
        })
    }

    pub(super) fn begin_attempt(
        self: &Arc<Self>,
    ) -> Result<Arc<AttemptEffectTracker>, CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        match state.phase {
            CommandEffectPhase::Idle => {}
            CommandEffectPhase::Open(_) => return Err(CommandEffectError::AttemptBusy),
            CommandEffectPhase::Terminal(_) => return Err(CommandEffectError::CommandFinished),
        }
        let current = state.next_attempt;
        if current == 0 {
            return Err(CommandEffectError::AttemptIdExhausted);
        }
        state.next_attempt = current
            .checked_add(1)
            .ok_or(CommandEffectError::AttemptIdExhausted)?;
        let id = CommandAttemptId(current);
        state.phase = CommandEffectPhase::Open(ActiveCommandAttempt {
            id,
            installed: false,
            demands: None,
            roots: Vec::new(),
        });
        Ok(Arc::new(AttemptEffectTracker {
            owner: self.clone(),
            id,
        }))
    }

    fn install(
        &self,
        id: CommandAttemptId,
        demands: &Arc<WorkspaceDemandOwner>,
    ) -> Result<(), CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        let CommandEffectPhase::Open(active) = &mut state.phase else {
            return Err(CommandEffectError::StaleAttempt);
        };
        if active.id != id {
            return Err(CommandEffectError::StaleAttempt);
        }
        if active.installed {
            return Err(CommandEffectError::AttemptTrackerAlreadyInstalled);
        }
        active.installed = true;
        active.demands = Some(Arc::downgrade(demands));
        Ok(())
    }

    fn record_activation(&self, id: CommandAttemptId, activation: RichActivation<'_>) {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        if !matches!(&state.phase, CommandEffectPhase::Open(active) if active.id == id) {
            return;
        }
        if activation.kind() != ActivationKind::Evaluated {
            return;
        }
        let batch = activation
            .evaluation_data()
            .and_then(|data| data.downcast_ref::<EventBatch>())
            .map(Dupe::dupe);
        if batch.is_none() && !state.lineage.contains_key(&activation.node()) {
            return;
        }
        if !state.lineage.contains_key(&activation.node()) {
            state.lineage.insert(activation.node(), Vec::new());
        }
        let transitions = state
            .lineage
            .get_mut(&activation.node())
            .expect("event lineage was inserted");
        match transitions.binary_search_by_key(&activation.version(), |entry| entry.version) {
            Ok(index) => transitions[index].batch = batch,
            Err(index) => transitions.insert(
                index,
                EventBatchTransition {
                    version: activation.version(),
                    batch,
                },
            ),
        }
    }

    fn record_root(&self, id: CommandAttemptId, activation: RootActivation) {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        let CommandEffectPhase::Open(active) = &mut state.phase else {
            return;
        };
        if active.id != id {
            return;
        }
        active.roots.push(AttemptRoot {
            ordinal: activation.ordinal(),
            node: activation.node(),
            version: activation.version(),
        });
    }

    fn seal_retry(&self, id: CommandAttemptId) -> Result<(), CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        match &state.phase {
            CommandEffectPhase::Open(active) if active.id == id => {
                state.phase = CommandEffectPhase::Idle;
                Ok(())
            }
            _ => Err(CommandEffectError::StaleAttempt),
        }
    }

    fn seal_terminal(
        self: &Arc<Self>,
        id: CommandAttemptId,
    ) -> Result<SealedCommandAttempt, CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        let CommandEffectPhase::Open(active) = &state.phase else {
            return Err(CommandEffectError::StaleAttempt);
        };
        if active.id != id {
            return Err(CommandEffectError::StaleAttempt);
        }
        let mut roots = active.roots.clone();
        roots.sort_by_key(|root| root.ordinal);
        if roots.is_empty() {
            return Err(CommandEffectError::NoTerminalRoots);
        }
        for (expected, root) in roots.iter().enumerate() {
            let expected =
                u64::try_from(expected).map_err(|_| CommandEffectError::AttemptIdExhausted)?;
            if root.ordinal != expected {
                return Err(CommandEffectError::RootOrdinal {
                    expected,
                    actual: root.ordinal,
                });
            }
        }
        let version = roots[0].version;
        if roots.iter().any(|root| root.version != version) {
            return Err(CommandEffectError::MixedRootVersions);
        }
        let nodes = roots.into_iter().map(|root| root.node).collect();
        let demands = active
            .demands
            .clone()
            .ok_or(CommandEffectError::DemandOwnerNotInstalled)?;
        if demands.upgrade().is_none() {
            return Err(CommandEffectError::DemandOwnerExpired);
        }
        state.phase = CommandEffectPhase::Terminal(id);
        Ok(SealedCommandAttempt {
            owner: self.clone(),
            demands,
            id,
            version,
            roots: nodes,
        })
    }

    fn finish_suppressed(&self, id: CommandAttemptId) -> Result<(), CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        match &state.phase {
            CommandEffectPhase::Open(active) if active.id == id => {
                state.phase = CommandEffectPhase::Terminal(id);
                Ok(())
            }
            _ => Err(CommandEffectError::StaleAttempt),
        }
    }

    fn select(
        &self,
        sealed: &SealedCommandAttempt,
        closure: &ActivationClosure,
    ) -> Result<SelectedEventBatches, CommandEffectError> {
        if closure.version() != sealed.version {
            return Err(CommandEffectError::ClosureVersion {
                expected: sealed.version,
                actual: closure.version(),
            });
        }
        if closure.roots() != sealed.roots.as_ref() {
            return Err(CommandEffectError::ClosureRoots);
        }
        let state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        if !matches!(state.phase, CommandEffectPhase::Terminal(id) if id == sealed.id) {
            return Err(CommandEffectError::StaleAttempt);
        }
        let batches = closure
            .nodes()
            .iter()
            .filter_map(|node| {
                state.lineage.get(&node.node()).and_then(|transitions| {
                    transitions
                        .iter()
                        .rev()
                        .find(|entry| entry.version <= closure.version())
                        .and_then(|entry| entry.batch.as_ref())
                        .filter(|batch| !batch.events().is_empty())
                        .map(Dupe::dupe)
                })
            })
            .collect::<Vec<_>>()
            .into();
        Ok(SelectedEventBatches { batches })
    }
}

impl AttemptEffectTracker {
    pub(super) fn reserve_install(
        self: &Arc<Self>,
        demands: &Arc<WorkspaceDemandOwner>,
    ) -> Result<(), CommandEffectError> {
        self.owner.install(self.id, demands)
    }

    pub(super) fn record_activation(&self, activation: RichActivation<'_>) {
        self.owner.record_activation(self.id, activation);
    }

    pub(super) fn record_root(&self, activation: RootActivation) {
        self.owner.record_root(self.id, activation);
    }

    pub(super) fn seal_retry(&self) -> Result<(), CommandEffectError> {
        self.owner.seal_retry(self.id)
    }

    pub(super) fn seal_terminal(&self) -> Result<SealedCommandAttempt, CommandEffectError> {
        self.owner.seal_terminal(self.id)
    }

    pub(super) fn finish_suppressed(&self) -> Result<(), CommandEffectError> {
        self.owner.finish_suppressed(self.id)
    }
}

impl SealedCommandAttempt {
    pub(super) fn root_count(&self) -> usize {
        self.roots.len()
    }

    pub(super) async fn select(
        self,
        transaction: &DiceTransaction,
    ) -> Result<SelectedCommandSidecars, CommandEffectError> {
        let demands = self
            .demands
            .upgrade()
            .ok_or(CommandEffectError::DemandOwnerExpired)?;
        let closure = transaction
            .activation_closure(self.roots.iter().copied())
            .await
            .map_err(CommandEffectError::Closure)?;
        let events = self.owner.select(&self, &closure)?;
        let selected_demands = demands
            .select(&closure)
            .map_err(CommandEffectError::Demand)?;
        Ok(SelectedCommandSidecars {
            events,
            demands: selected_demands,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    use allocative::Allocative;
    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::CancellationContext;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::InjectedKey;
    use dice::Key;
    use dice::UserComputationData;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use tokio::sync::Notify;

    use super::CommandEffectError;
    use super::CommandEffectOwner;
    use super::SelectedEventBatches;
    use crate::runtime::demands::WorkspaceDemandOwner;

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative)]
    struct EventMode;

    impl fmt::Display for EventMode {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("event-mode")
        }
    }

    #[async_trait]
    impl InjectedKey for EventMode {
        type Value = u8;

        fn equality(left: &Self::Value, right: &Self::Value) -> bool {
            left == right
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative)]
    struct BranchMode;

    impl fmt::Display for BranchMode {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("branch-mode")
        }
    }

    #[async_trait]
    impl InjectedKey for BranchMode {
        type Value = bool;

        fn equality(left: &Self::Value, right: &Self::Value) -> bool {
            left == right
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative)]
    enum EventGraphKey {
        Leaf,
        Parent,
        OldLeaf,
        NewLeaf,
        Branch,
        Shared,
        Left,
        Right,
    }

    impl fmt::Display for EventGraphKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{self:?}")
        }
    }

    #[async_trait]
    impl Key for EventGraphKey {
        type Value = ();

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            match self {
                Self::Leaf => {
                    let mode = ctx.compute(&EventMode).await.unwrap();
                    ctx.store_evaluation_data(if mode == 0 {
                        batch("leaf")
                    } else {
                        EventBatch::empty()
                    })
                    .unwrap();
                }
                Self::Parent => {
                    ctx.compute(&Self::Leaf).await.unwrap();
                    ctx.store_evaluation_data(batch("parent")).unwrap();
                }
                Self::OldLeaf => {
                    ctx.store_evaluation_data(batch("old")).unwrap();
                }
                Self::NewLeaf => {
                    ctx.store_evaluation_data(batch("new")).unwrap();
                }
                Self::Branch => {
                    let branch = if ctx.compute(&BranchMode).await.unwrap() {
                        Self::NewLeaf
                    } else {
                        Self::OldLeaf
                    };
                    ctx.compute(&branch).await.unwrap();
                    ctx.store_evaluation_data(batch("branch")).unwrap();
                }
                Self::Shared => {
                    ctx.store_evaluation_data(batch("shared")).unwrap();
                }
                Self::Left => {
                    ctx.compute(&Self::Shared).await.unwrap();
                    ctx.store_evaluation_data(batch("left")).unwrap();
                }
                Self::Right => {
                    ctx.compute(&Self::Shared).await.unwrap();
                    ctx.store_evaluation_data(batch("right")).unwrap();
                }
            }
        }

        fn equality(_left: &Self::Value, _right: &Self::Value) -> bool {
            true
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative)]
    struct TransientEventKey;

    impl fmt::Display for TransientEventKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("transient-event")
        }
    }

    #[derive(Clone, Debug, Allocative)]
    struct GatedEventKey {
        id: u64,
        #[allocative(skip)]
        entered: Arc<Notify>,
        #[allocative(skip)]
        release: Arc<Notify>,
    }

    impl GatedEventKey {
        fn new(entered: Arc<Notify>, release: Arc<Notify>) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
            Self {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                entered,
                release,
            }
        }
    }

    impl PartialEq for GatedEventKey {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl Eq for GatedEventKey {}

    impl Hash for GatedEventKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }

    impl fmt::Display for GatedEventKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "gated-event-{}", self.id)
        }
    }

    #[async_trait]
    impl Key for GatedEventKey {
        type Value = ();

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            self.entered.notify_one();
            self.release.notified().await;
            ctx.compute(&EventGraphKey::Leaf).await.unwrap();
            ctx.store_evaluation_data(batch("late")).unwrap();
        }

        fn equality(_left: &Self::Value, _right: &Self::Value) -> bool {
            true
        }
    }

    #[async_trait]
    impl Key for TransientEventKey {
        type Value = ();

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            ctx.store_evaluation_data(batch("transient")).unwrap();
        }

        fn equality(_left: &Self::Value, _right: &Self::Value) -> bool {
            true
        }

        fn validity(_value: &Self::Value) -> bool {
            false
        }
    }

    fn batch(text: &str) -> EventBatch {
        EventBatch::from_events([EvaluationEvent::StarlarkPrint {
            text: CompactString::new(text),
        }])
    }

    fn user_data(
        dice: &Arc<Dice>,
        tracker: &Arc<super::AttemptEffectTracker>,
    ) -> Result<UserComputationData, CommandEffectError> {
        let mut data = UserComputationData::default();
        assert!(data.data.get::<CaptureEvaluationEvents>().is_err());
        WorkspaceDemandOwner::new(dice, NormalizedAbsolutePath::new("/workspace").unwrap())
            .install(dice, &mut data, Some(tracker.clone()))?;
        assert!(data.data.get::<CaptureEvaluationEvents>().is_ok());
        Ok(data)
    }

    async fn select_events(
        sealed: super::SealedCommandAttempt,
        transaction: &dice::DiceTransaction,
    ) -> Result<SelectedEventBatches, CommandEffectError> {
        Ok(sealed.select(transaction).await?.events().clone())
    }

    fn selected_text(selected: &SelectedEventBatches) -> Vec<&str> {
        selected
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text } => text.as_str(),
            })
            .collect()
    }

    #[tokio::test]
    async fn serial_attempts_retain_reachable_child_and_fresh_owner_does_not_replay()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();

        let first = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &first)?);
        updater.changed_to(vec![(EventMode, 0)])?;
        let mut first_transaction = updater.commit().await;
        first_transaction.compute(&EventGraphKey::Leaf).await?;
        first.seal_retry()?;

        let terminal = owner.begin_attempt()?;
        let mut terminal_transaction = dice
            .updater_with_data(user_data(&dice, &terminal)?)
            .commit()
            .await;
        terminal_transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = terminal.seal_terminal()?;
        let selected = select_events(sealed, &terminal_transaction).await?;
        assert_eq!(selected_text(&selected), ["leaf", "parent"]);
        assert_eq!(
            owner.begin_attempt().unwrap_err(),
            CommandEffectError::CommandFinished
        );

        let fresh_owner = CommandEffectOwner::new();
        let fresh = fresh_owner.begin_attempt()?;
        let mut fresh_transaction = dice
            .updater_with_data(user_data(&dice, &fresh)?)
            .commit()
            .await;
        fresh_transaction.compute(&EventGraphKey::Parent).await?;
        let fresh_sealed = fresh.seal_terminal()?;
        assert!(
            select_events(fresh_sealed, &fresh_transaction)
                .await?
                .batches()
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn evaluated_empty_clears_prior_batch_and_abandoned_branch_is_excluded()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        let first = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &first)?);
        updater.changed_to(vec![(EventMode, 0)])?;
        let mut first_transaction = updater.commit().await;
        first_transaction.compute(&EventGraphKey::Parent).await?;
        first.seal_retry()?;

        let terminal = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &terminal)?);
        updater.changed_to(vec![(EventMode, 1)])?;
        let mut terminal_transaction = updater.commit().await;
        terminal_transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = terminal.seal_terminal()?;
        assert_eq!(
            selected_text(&select_events(sealed, &terminal_transaction).await?),
            ["parent"]
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        let first = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &first)?);
        updater.changed_to(vec![(BranchMode, false)])?;
        let mut first_transaction = updater.commit().await;
        first_transaction.compute(&EventGraphKey::Branch).await?;
        first.seal_retry()?;

        let terminal = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &terminal)?);
        updater.changed_to(vec![(BranchMode, true)])?;
        let mut terminal_transaction = updater.commit().await;
        terminal_transaction.compute(&EventGraphKey::Branch).await?;
        let sealed = terminal.seal_terminal()?;
        assert_eq!(
            selected_text(&select_events(sealed, &terminal_transaction).await?),
            ["new", "branch"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn closure_order_roots_shared_dedup_and_in_flight_post_seal_quarantine_are_exact()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        let mut transaction = dice
            .updater_with_data(user_data(&dice, &attempt)?)
            .commit()
            .await;
        transaction.compute(&EventGraphKey::Left).await?;
        transaction.compute(&EventGraphKey::Right).await?;
        transaction.compute(&EventGraphKey::Left).await?;
        let sealed = attempt.seal_terminal()?;
        assert_eq!(
            selected_text(&select_events(sealed, &transaction).await?),
            ["shared", "left", "right"]
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        let stale = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(&dice, &stale)?);
        updater.changed_to(vec![(EventMode, 0)])?;
        let mut stale_transaction = updater.commit().await;
        stale_transaction.compute(&EventGraphKey::Shared).await?;
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let gated = GatedEventKey::new(entered.clone(), release.clone());
        let pending = stale_transaction.compute(&gated);
        tokio::pin!(pending);
        tokio::select! {
            result = &mut pending => panic!("gated computation completed before seal: {result:?}"),
            () = entered.notified() => {}
        }
        stale.seal_retry()?;
        release.notify_one();
        pending.await?;

        let terminal = owner.begin_attempt()?;
        let mut terminal_transaction = dice
            .updater_with_data(user_data(&dice, &terminal)?)
            .commit()
            .await;
        terminal_transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = terminal.seal_terminal()?;
        assert_eq!(
            selected_text(&select_events(sealed, &terminal_transaction).await?),
            ["parent"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn lifecycle_and_closure_failures_are_typed_and_select_nothing() -> anyhow::Result<()> {
        let exhausted = CommandEffectOwner::new();
        exhausted
            .state
            .lock()
            .expect("command effect owner mutex poisoned")
            .next_attempt = u64::MAX;
        assert_eq!(
            exhausted.begin_attempt().unwrap_err(),
            CommandEffectError::AttemptIdExhausted
        );

        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        assert_eq!(
            owner.begin_attempt().unwrap_err(),
            CommandEffectError::AttemptBusy
        );
        let install_dice = Dice::builder().build(DetectCycles::Enabled);
        let install_demands = WorkspaceDemandOwner::new(
            &install_dice,
            NormalizedAbsolutePath::new("/workspace").unwrap(),
        );
        let mut occupied = UserComputationData::default();
        install_demands.install(&install_dice, &mut occupied, None)?;
        assert_eq!(
            install_demands.install(&install_dice, &mut occupied, Some(attempt.clone())),
            Err(CommandEffectError::ActivationTrackerAlreadyInstalled)
        );
        assert!(occupied.data.get::<CaptureEvaluationEvents>().is_err());
        let _installed = user_data(&install_dice, &attempt)?;
        let duplicate = UserComputationData::default();
        assert_eq!(
            attempt.reserve_install(&install_demands),
            Err(CommandEffectError::AttemptTrackerAlreadyInstalled)
        );
        assert!(duplicate.activation_tracker.is_none());
        assert!(duplicate.data.get::<CaptureEvaluationEvents>().is_err());
        assert_eq!(
            attempt.seal_terminal().unwrap_err(),
            CommandEffectError::NoTerminalRoots
        );
        attempt.finish_suppressed()?;
        let terminal = UserComputationData::default();
        assert_eq!(
            attempt.reserve_install(&install_demands),
            Err(CommandEffectError::StaleAttempt)
        );
        assert!(terminal.activation_tracker.is_none());
        assert!(terminal.data.get::<CaptureEvaluationEvents>().is_err());
        assert_eq!(
            attempt.finish_suppressed(),
            Err(CommandEffectError::StaleAttempt)
        );
        let owner = CommandEffectOwner::new();
        let stale = owner.begin_attempt()?;
        stale.seal_retry()?;
        let stale_data = UserComputationData::default();
        assert_eq!(
            stale.reserve_install(&install_demands),
            Err(CommandEffectError::StaleAttempt)
        );
        assert!(stale_data.activation_tracker.is_none());
        assert!(stale_data.data.get::<CaptureEvaluationEvents>().is_err());

        let state_only_owner = CommandEffectOwner::new();
        let state_only = state_only_owner.begin_attempt()?;
        let state_only_data = UserComputationData::default();
        state_only.reserve_install(&install_demands)?;
        assert!(state_only_data.activation_tracker.is_none());
        assert!(
            state_only_data
                .data
                .get::<CaptureEvaluationEvents>()
                .is_err()
        );
        state_only.finish_suppressed()?;

        let first_dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        let mut updater = first_dice.updater_with_data(user_data(&first_dice, &attempt)?);
        updater.changed_to(vec![(EventMode, 0)])?;
        let mut transaction = updater.commit().await;
        transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = attempt.seal_terminal()?;

        let second_dice = Dice::builder().build(DetectCycles::Enabled);
        let foreign_transaction = second_dice.updater().commit().await;
        assert!(matches!(
            sealed.select(&foreign_transaction).await,
            Err(CommandEffectError::Closure(
                dice::ActivationClosureError::ForeignEngine { .. }
            ))
        ));

        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        let mut updater = first_dice.updater_with_data(user_data(&first_dice, &attempt)?);
        updater.changed_to(vec![(EventMode, 0)])?;
        let mut transaction = updater.commit().await;
        transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = attempt.seal_terminal()?;
        let empty_closure = transaction.activation_closure([]).await?;
        assert_eq!(
            sealed.owner.select(&sealed, &empty_closure),
            Err(CommandEffectError::ClosureRoots)
        );

        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        let mut transaction = first_dice
            .updater_with_data(user_data(&first_dice, &attempt)?)
            .commit()
            .await;
        transaction.compute(&EventGraphKey::Parent).await?;
        let sealed = attempt.seal_terminal()?;
        let mut dirty_updater = first_dice.updater();
        dirty_updater.changed_to(vec![(EventMode, 1)])?;
        let dirty_transaction = dirty_updater.commit().await;
        assert!(matches!(
            sealed.select(&dirty_transaction).await,
            Err(CommandEffectError::Closure(
                dice::ActivationClosureError::Dirty { .. }
            ))
        ));

        let transient_dice = Dice::builder().build(DetectCycles::Enabled);
        let transient_owner = CommandEffectOwner::new();
        let transient_attempt = transient_owner.begin_attempt()?;
        let mut transient_transaction = transient_dice
            .updater_with_data(user_data(&transient_dice, &transient_attempt)?)
            .commit()
            .await;
        transient_transaction.compute(&TransientEventKey).await?;
        let transient_sealed = transient_attempt.seal_terminal()?;
        assert!(matches!(
            transient_sealed.select(&transient_transaction).await,
            Err(CommandEffectError::Closure(
                dice::ActivationClosureError::UnavailableRoot { .. }
            ))
        ));
        Ok(())
    }
}
