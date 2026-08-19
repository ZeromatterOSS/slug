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
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use allocative::Allocative;
use dice::ActivationClosure;
use dice::ActivationClosureError;
use dice::ActivationKind;
use dice::DiceNodeId;
use dice::DiceTransaction;
use dice::RichActivation;
use dice::RootActivation;
use dice::VersionNumber;
use dupe::Dupe;
use slug_events_v2::EvaluationDiagnosticLevel;
use slug_events_v2::EvaluationEvent;
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
    version: Option<VersionNumber>,
    roots: Arc<[DiceNodeId]>,
    allow_unavailable_roots: bool,
    armed: bool,
}

#[derive(Debug)]
pub(super) struct SelectedTerminalToken {
    owner: Arc<CommandEffectOwner>,
    id: CommandAttemptId,
    armed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelectedEventTransition {
    NoTransition,
    Known(Option<EventBatch>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EventReconciliationPolicy {
    Strict,
    SourceCertifiedCurrentClosure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedEventState {
    roots: Arc<[DiceNodeId]>,
    nodes: Arc<[(DiceNodeId, SelectedEventTransition)]>,
    #[cfg(test)]
    batches: Arc<[EventBatch]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub(super) struct AcceptedEventEpoch {
    roots: Arc<[DiceNodeId]>,
    pub(super) entries: Arc<[(DiceNodeId, EventBatch)]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub(super) struct ProvisionalEventEpoch {
    roots: Arc<[DiceNodeId]>,
    entries: Arc<[(DiceNodeId, Option<EventBatch>)]>,
}

/// Ordered event batches selected for publication by the current command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedEventBatches {
    batches: Arc<[EventBatch]>,
}

/// Infallible command-owned logical output, exposed only after acceptance.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CommandOutputBuffer {
    batches: Arc<[EventBatch]>,
}

/// An accepted semantic terminal inseparable from its selected command events.
#[must_use = "an accepted command must remain intact until output publication"]
pub struct AcceptedCommand<T> {
    terminal: T,
    events: CommandOutputBuffer,
}

/// Primitive terminal streams projected without consuming the accepted value.
#[derive(Debug, Eq, PartialEq)]
pub struct TerminalOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// A projected command that still owns its accepted value and selected events.
#[must_use = "projected command output must remain intact until publication"]
pub struct CommandOutput<T> {
    terminal: T,
    events: CommandOutputBuffer,
    output: TerminalOutput,
}

/// A fully rendered command whose primitive parts may be consumed.
#[must_use = "published command output must be consumed"]
pub struct PublishedCommand<T> {
    terminal: T,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
pub(super) struct SelectedCommandSidecars {
    events: SelectedEventState,
    demands: SelectedWorkspaceDemands,
    terminal: SelectedTerminalToken,
}

impl SelectedCommandSidecars {
    pub(super) fn events(&self) -> &SelectedEventState {
        &self.events
    }

    pub(super) fn demands(&self) -> &SelectedWorkspaceDemands {
        &self.demands
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        SelectedEventState,
        SelectedWorkspaceDemands,
        SelectedTerminalToken,
    ) {
        (self.events, self.demands, self.terminal)
    }

    #[cfg(test)]
    pub(super) fn for_test(demands: SelectedWorkspaceDemands) -> Self {
        Self {
            events: SelectedEventState {
                roots: Arc::from([]),
                nodes: Arc::from([]),
                batches: Arc::from([]),
            },
            demands,
            terminal: SelectedTerminalToken::detached_for_test(),
        }
    }
}

impl SelectedTerminalToken {
    fn new(owner: Arc<CommandEffectOwner>, id: CommandAttemptId) -> Self {
        Self {
            owner,
            id,
            armed: true,
        }
    }

    #[cfg(test)]
    fn detached_for_test() -> Self {
        Self {
            owner: CommandEffectOwner::new(),
            id: CommandAttemptId(0),
            armed: false,
        }
    }

    pub(super) fn reset_to_idle(mut self) -> Result<(), CommandEffectError> {
        self.owner.reset_terminal(self.id)?;
        self.armed = false;
        Ok(())
    }

    pub(super) fn disarm(mut self) -> Result<(), CommandEffectError> {
        if self.armed {
            self.owner.validate_terminal(self.id)?;
            self.armed = false;
        }
        Ok(())
    }
}

impl Drop for SelectedTerminalToken {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.owner.reset_terminal(self.id);
            self.armed = false;
        }
    }
}

impl AcceptedEventEpoch {
    pub(super) fn empty() -> Self {
        Self {
            roots: Arc::from([]),
            entries: Arc::from([]),
        }
    }
}

impl SelectedEventState {
    #[cfg(test)]
    pub(super) fn batches(&self) -> &[EventBatch] {
        &self.batches
    }

    pub(super) fn reconcile(
        self,
        prior: &AcceptedEventEpoch,
    ) -> (SelectedEventBatches, AcceptedEventEpoch) {
        let (batches, accepted, _) =
            self.reconcile_revision(prior, None, EventReconciliationPolicy::Strict);
        (batches, accepted)
    }

    pub(super) fn reconcile_revision(
        self,
        prior: &AcceptedEventEpoch,
        carried: Option<&ProvisionalEventEpoch>,
        policy: EventReconciliationPolicy,
    ) -> (
        SelectedEventBatches,
        AcceptedEventEpoch,
        ProvisionalEventEpoch,
    ) {
        let carried = carried.filter(|carried| carried.roots == self.roots);
        let source_association = policy == EventReconciliationPolicy::SourceCertifiedCurrentClosure
            && !self.roots.is_empty()
            && self.roots == prior.roots;
        use SelectedEventTransition::Known;
        use SelectedEventTransition::NoTransition;
        let mut prior_by_node = SmallMap::new();
        for (node, batch) in prior.entries.iter() {
            prior_by_node.insert(*node, batch.dupe());
        }
        let mut carried_by_node = SmallMap::new();
        let mut order = Vec::new();
        let mut ordered = SmallMap::new();
        if let Some(carried) = carried {
            for (node, batch) in carried.entries.iter() {
                carried_by_node.insert(*node, batch.as_ref().map(Dupe::dupe));
                if !source_association {
                    ordered.insert(*node, ());
                    order.push(*node);
                }
            }
        }
        let mut final_by_node = SmallMap::new();
        for (node, transition) in self.nodes.iter() {
            final_by_node.insert(*node, transition.clone());
            if ordered.insert(*node, ()).is_none() {
                order.push(*node);
            }
        }
        if source_association {
            let domain = carried.map(|carried| &carried.entries[..]);
            if let Some(domain) = domain {
                for (node, _) in domain {
                    if ordered.insert(*node, ()).is_none() {
                        order.push(*node);
                    }
                }
            } else {
                for (node, _) in prior.entries.iter() {
                    if ordered.insert(*node, ()).is_none() {
                        order.push(*node);
                    }
                }
            }
        }
        let mut batches = Vec::new();
        let mut accepted = Vec::new();
        let mut provisional = Vec::new();
        for node in order {
            let fallback = || match carried {
                Some(_) => carried_by_node.get(&node).cloned(),
                None => prior_by_node.get(&node).map(|batch| Some(batch.dupe())),
            };
            let effective = if source_association {
                match final_by_node.get(&node) {
                    Some(Known(Some(batch))) => Some(Some(batch.dupe())),
                    Some(Known(None) | NoTransition) => fallback(),
                    None => Some(None),
                }
            } else {
                match final_by_node.get(&node) {
                    Some(Known(Some(batch))) => Some(Some(batch.dupe())),
                    Some(Known(None)) => (carried_by_node.contains_key(&node)
                        || prior_by_node.contains_key(&node))
                    .then_some(None),
                    Some(NoTransition) => carried_by_node
                        .get(&node)
                        .cloned()
                        .or_else(|| prior_by_node.get(&node).map(|batch| Some(batch.dupe()))),
                    None => carried_by_node.get(&node).cloned(),
                }
            };
            let Some(effective) = effective else {
                continue;
            };
            if let Some(batch) = &effective {
                if prior_by_node.get(&node) != Some(batch) && !batch.events().is_empty() {
                    batches.push(batch.dupe());
                }
                accepted.push((node, batch.dupe()));
            }
            provisional.push((node, effective));
        }
        (
            SelectedEventBatches {
                batches: batches.into(),
            },
            AcceptedEventEpoch {
                roots: self.roots.dupe(),
                entries: accepted.into(),
            },
            ProvisionalEventEpoch {
                roots: self.roots,
                entries: provisional.into(),
            },
        )
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

    fn render_stderr(self) -> String {
        #[cfg(windows)]
        const LINE_SEPARATOR: &str = "\r\n";
        #[cfg(not(windows))]
        const LINE_SEPARATOR: &str = "\n";

        let mut stderr = String::new();
        for batch in self.batches.iter() {
            for event in batch.events() {
                let text = match event {
                    EvaluationEvent::StarlarkPrint { location, text } => {
                        write!(&mut stderr, "DEBUG: {location}: ")
                            .expect("formatting into a String is infallible");
                        text
                    }
                    EvaluationEvent::Diagnostic { level, text } => {
                        stderr.push_str(match level {
                            EvaluationDiagnosticLevel::Warning => "WARNING: ",
                            EvaluationDiagnosticLevel::Error => "ERROR: ",
                        });
                        text
                    }
                };
                stderr.push_str(text);
                if !text.ends_with('\n') {
                    stderr.push_str(LINE_SEPARATOR);
                }
            }
        }
        stderr
    }
}

impl<T> AcceptedCommand<T> {
    pub(super) fn new(terminal: T, events: CommandOutputBuffer) -> Self {
        Self { terminal, events }
    }

    pub(super) fn map_terminal<U>(self, map: impl FnOnce(T) -> U) -> AcceptedCommand<U> {
        AcceptedCommand {
            terminal: map(self.terminal),
            events: self.events,
        }
    }

    pub fn project(self, projection: impl FnOnce(&T) -> TerminalOutput) -> CommandOutput<T> {
        let output = projection(&self.terminal);
        CommandOutput {
            terminal: self.terminal,
            events: self.events,
            output,
        }
    }

    pub(super) fn terminal(&self) -> &T {
        &self.terminal
    }

    #[cfg(test)]
    pub(super) fn terminal_for_test(&self) -> &T {
        &self.terminal
    }

    #[cfg(test)]
    pub(super) fn batches_for_test(&self) -> &[EventBatch] {
        self.events.batches()
    }
}

impl<T> fmt::Debug for AcceptedCommand<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptedCommand(..)")
    }
}

impl TerminalOutput {
    pub fn new(exit_code: i32, stdout: String, stderr: String) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
        }
    }
}

impl<T> fmt::Debug for CommandOutput<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommandOutput(..)")
    }
}

impl<T> CommandOutput<T> {
    pub fn publish(self) -> PublishedCommand<T> {
        let Self {
            terminal,
            events,
            output:
                TerminalOutput {
                    exit_code,
                    stdout,
                    stderr: terminal_stderr,
                },
        } = self;
        let mut stderr = events.render_stderr();
        stderr.push_str(&terminal_stderr);
        PublishedCommand {
            terminal,
            exit_code,
            stdout,
            stderr,
        }
    }
}

impl<T> PublishedCommand<T> {
    pub fn into_parts(self) -> (T, i32, String, String) {
        (self.terminal, self.exit_code, self.stdout, self.stderr)
    }
}

impl<T> fmt::Debug for PublishedCommand<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublishedCommand(..)")
    }
}

#[cfg(test)]
impl<T> CommandOutput<T> {
    pub(super) fn terminal_for_test(&self) -> &T {
        &self.terminal
    }

    pub(super) fn batches_for_test(&self) -> &[EventBatch] {
        self.events.batches()
    }

    pub(super) fn output_for_test(&self) -> &TerminalOutput {
        &self.output
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
        allow_empty_roots: bool,
        allow_unavailable_roots: bool,
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
        if roots.is_empty() && !allow_empty_roots {
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
        let version = roots.first().map(|root| root.version);
        if let Some(version) = version
            && roots.iter().any(|root| root.version != version)
        {
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
            allow_unavailable_roots,
            armed: true,
        })
    }

    fn validate_terminal(&self, id: CommandAttemptId) -> Result<(), CommandEffectError> {
        let state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        if matches!(state.phase, CommandEffectPhase::Terminal(active) if active == id) {
            Ok(())
        } else {
            Err(CommandEffectError::StaleAttempt)
        }
    }

    fn reset_terminal(&self, id: CommandAttemptId) -> Result<(), CommandEffectError> {
        let mut state = self
            .state
            .lock()
            .expect("command effect owner mutex poisoned");
        if matches!(state.phase, CommandEffectPhase::Terminal(active) if active == id) {
            state.phase = CommandEffectPhase::Idle;
            Ok(())
        } else {
            Err(CommandEffectError::StaleAttempt)
        }
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
    ) -> Result<SelectedEventState, CommandEffectError> {
        if let Some(version) = sealed.version
            && closure.version() != version
        {
            return Err(CommandEffectError::ClosureVersion {
                expected: version,
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
        let mut nodes = Vec::with_capacity(closure.nodes().len());
        #[cfg(test)]
        let mut batches = Vec::new();
        for node in closure.nodes() {
            let transition = state
                .lineage
                .get(&node.node())
                .and_then(|transitions| {
                    transitions
                        .iter()
                        .rev()
                        .find(|entry| entry.version <= closure.version())
                })
                .map_or(SelectedEventTransition::NoTransition, |entry| {
                    SelectedEventTransition::Known(entry.batch.as_ref().map(Dupe::dupe))
                });
            #[cfg(test)]
            if let SelectedEventTransition::Known(Some(batch)) = &transition
                && !batch.events().is_empty()
            {
                batches.push(batch.dupe());
            }
            nodes.push((node.node(), transition));
        }
        Ok(SelectedEventState {
            roots: closure.roots().into(),
            nodes: nodes.into(),
            #[cfg(test)]
            batches: batches.into(),
        })
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
        self.owner.seal_terminal(self.id, false, false)
    }

    pub(super) fn seal_terminal_allowing_empty_roots(
        &self,
    ) -> Result<SealedCommandAttempt, CommandEffectError> {
        self.owner.seal_terminal(self.id, true, false)
    }

    pub(super) fn seal_terminal_allowing_unavailable_roots(
        &self,
    ) -> Result<SealedCommandAttempt, CommandEffectError> {
        self.owner.seal_terminal(self.id, true, true)
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
        mut self,
        transaction: &DiceTransaction,
    ) -> Result<SelectedCommandSidecars, CommandEffectError> {
        let demands = self
            .demands
            .upgrade()
            .ok_or(CommandEffectError::DemandOwnerExpired)?;
        let closure = loop {
            match transaction
                .activation_closure(self.roots.iter().copied())
                .await
            {
                Ok(closure) => break closure,
                Err(ActivationClosureError::UnavailableRoot { root })
                    if self.allow_unavailable_roots =>
                {
                    let retained = self
                        .roots
                        .iter()
                        .copied()
                        .filter(|candidate| candidate != &root)
                        .collect::<Vec<_>>();
                    if retained.len() == self.roots.len() {
                        return Err(CommandEffectError::Closure(
                            ActivationClosureError::UnavailableRoot { root },
                        ));
                    }
                    self.roots = retained.into();
                }
                Err(error) => return Err(CommandEffectError::Closure(error)),
            }
        };
        let events = self.owner.select(&self, &closure)?;
        let selected_demands = demands
            .select(&closure)
            .map_err(CommandEffectError::Demand)?;
        let terminal = SelectedTerminalToken::new(self.owner.clone(), self.id);
        self.armed = false;
        Ok(SelectedCommandSidecars {
            events,
            terminal,
            demands: selected_demands,
        })
    }
}

impl Drop for SealedCommandAttempt {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.owner.reset_terminal(self.id);
            self.armed = false;
        }
    }
}
#[cfg(test)]
mod tests {
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;
    use std::sync::Weak;
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

    use super::AcceptedCommand;
    use super::AcceptedEventEpoch;
    use super::CommandAttemptId;
    use super::CommandEffectError;
    use super::CommandEffectOwner;
    use super::CommandEffectPhase;
    use super::CommandOutputBuffer;
    use super::EventReconciliationPolicy;
    use super::SealedCommandAttempt;
    use super::SelectedEventBatches;
    use super::SelectedEventState;
    use super::SelectedEventTransition;
    use super::SelectedTerminalToken;
    use super::TerminalOutput;
    use crate::runtime::demands::WorkspaceDemandOwner;

    fn terminal_owner(id: u64) -> (Arc<CommandEffectOwner>, CommandAttemptId) {
        let owner = CommandEffectOwner::new();
        let id = CommandAttemptId(id);
        owner.state.lock().unwrap().phase = CommandEffectPhase::Terminal(id);
        (owner, id)
    }

    #[test]
    fn accepted_terminal_mapping_moves_value_and_preserves_events() {
        let events = CommandOutputBuffer {
            batches: Arc::from([EventBatch::empty()]),
        };
        let accepted = AcceptedCommand::new(String::from("terminal"), events);
        let mapped = accepted.map_terminal(|terminal| terminal.len());
        assert_eq!(mapped.terminal_for_test(), &8);
        assert_eq!(mapped.batches_for_test().len(), 1);
    }

    #[test]
    fn selected_terminal_reset_suppresses_and_permits_fresh_attempt() {
        let (owner, id) = terminal_owner(7);
        SelectedTerminalToken::new(owner.clone(), id)
            .reset_to_idle()
            .unwrap();
        let fresh = owner.begin_attempt().unwrap();
        assert_ne!(fresh.id, id);
        fresh.seal_retry().unwrap();
    }

    #[test]
    fn selected_terminal_drop_performs_cancellation_cleanup() {
        let (owner, id) = terminal_owner(11);
        drop(SelectedTerminalToken::new(owner.clone(), id));
        let fresh = owner.begin_attempt().unwrap();
        assert_ne!(fresh.id, id);
        fresh.seal_retry().unwrap();
    }
    #[test]
    fn sealed_terminal_drop_cleans_up_cancelled_selection() {
        let (owner, id) = terminal_owner(12);
        drop(SealedCommandAttempt {
            owner: owner.clone(),
            demands: Weak::new(),
            id,
            version: None,
            roots: Arc::from([]),
            allow_unavailable_roots: true,
            armed: true,
        });
        let fresh = owner.begin_attempt().unwrap();
        assert_ne!(fresh.id, id);
        fresh.seal_retry().unwrap();
    }

    #[test]
    fn selected_terminal_accept_disarms_without_reopening_command() {
        let (owner, id) = terminal_owner(13);
        SelectedTerminalToken::new(owner.clone(), id)
            .disarm()
            .unwrap();
        assert_eq!(
            owner.begin_attempt().unwrap_err(),
            CommandEffectError::CommandFinished
        );
    }

    #[test]
    fn selected_terminal_reset_rejects_nonmatching_attempt() {
        let (owner, id) = terminal_owner(17);
        let error = SelectedTerminalToken::new(owner.clone(), CommandAttemptId(18))
            .reset_to_idle()
            .unwrap_err();
        assert_eq!(error, CommandEffectError::StaleAttempt);
        assert_eq!(
            owner.begin_attempt().unwrap_err(),
            CommandEffectError::CommandFinished
        );
        owner.reset_terminal(id).unwrap();
    }

    #[test]
    fn opaque_projection_borrows_once_and_retains_exact_terminal_identity() {
        let terminal: Arc<str> = Arc::from("terminal");
        let identity = terminal.clone();
        let accepted = AcceptedCommand::new(
            terminal,
            CommandOutputBuffer {
                batches: Arc::from([EventBatch::from_events([EvaluationEvent::StarlarkPrint {
                    location: slug_events_v2::StarlarkSourceLocation::new(
                        Arc::from("selected.bzl"),
                        1,
                        6,
                    ),
                    text: CompactString::new("selected"),
                }])]),
            },
        );
        assert_eq!(format!("{accepted:?}"), "AcceptedCommand(..)");

        let mut calls = 0;
        let projected = accepted.project(|terminal| {
            calls += 1;
            assert!(Arc::ptr_eq(terminal, &identity));
            TerminalOutput::new(2, "stdout".into(), "stderr".into())
        });

        assert_eq!(calls, 1);
        assert!(Arc::ptr_eq(&projected.terminal, &identity));
        assert_eq!(projected.events.batches().len(), 1);
        assert_eq!(
            projected.output,
            TerminalOutput::new(2, "stdout".into(), "stderr".into())
        );
        assert_eq!(format!("{projected:?}"), "CommandOutput(..)");
    }

    #[test]
    fn publication_consumes_events_in_order_before_terminal_stderr() {
        let terminal: Arc<str> = Arc::from("terminal-value");
        let identity = terminal.clone();
        let events = CommandOutputBuffer {
            batches: Arc::from([
                EventBatch::from_events([
                    EvaluationEvent::StarlarkPrint {
                        location: slug_events_v2::StarlarkSourceLocation::new(
                            Arc::from("/workspace/defs.bzl"),
                            3,
                            14,
                        ),
                        text: CompactString::new("first\nsecond"),
                    },
                    EvaluationEvent::Diagnostic {
                        level: slug_events_v2::EvaluationDiagnosticLevel::Warning,
                        text: CompactString::new("/workspace/REPO.bazel: warning\n"),
                    },
                ]),
                EventBatch::from_events([
                    EvaluationEvent::StarlarkPrint {
                        location: slug_events_v2::StarlarkSourceLocation::new(
                            Arc::from("/workspace/BUILD.bazel"),
                            8,
                            6,
                        ),
                        text: CompactString::new("already terminated\n"),
                    },
                    EvaluationEvent::Diagnostic {
                        level: slug_events_v2::EvaluationDiagnosticLevel::Error,
                        text: CompactString::new("/workspace/REPO.bazel: error"),
                    },
                ]),
            ]),
        };
        let projected = AcceptedCommand::new(terminal, events)
            .project(|_| TerminalOutput::new(7, "stdout\n".into(), "terminal stderr\n".into()));
        let published = projected.publish();
        assert_eq!(format!("{published:?}"), "PublishedCommand(..)");
        let (terminal, exit_code, stdout, stderr) = published.into_parts();

        #[cfg(windows)]
        let separator = "\r\n";
        #[cfg(not(windows))]
        let separator = "\n";
        assert!(Arc::ptr_eq(&terminal, &identity));
        assert_eq!(exit_code, 7);
        assert_eq!(stdout, "stdout\n");
        assert_eq!(
            stderr,
            format!(
                "DEBUG: /workspace/defs.bzl:3:14: first\nsecond{separator}\
WARNING: /workspace/REPO.bazel: warning\n\
DEBUG: /workspace/BUILD.bazel:8:6: already terminated\n\
ERROR: /workspace/REPO.bazel: error{separator}terminal stderr\n"
            )
        );
    }

    #[test]
    fn publication_preserves_empty_events_and_typed_error_terminal() {
        let error: Arc<str> = Arc::from("typed error");
        let identity = error.clone();
        let projected = AcceptedCommand::new(
            Result::<(), Arc<str>>::Err(error),
            CommandOutputBuffer {
                batches: Arc::from([]),
            },
        )
        .project(|_| TerminalOutput::new(2, String::new(), "exact".into()));
        let (terminal, exit_code, stdout, stderr) = projected.publish().into_parts();
        let terminal_error = terminal.unwrap_err();
        assert!(Arc::ptr_eq(&terminal_error, &identity));
        assert_eq!(exit_code, 2);
        assert!(stdout.is_empty());
        assert_eq!(stderr, "exact");
    }

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
                    match mode {
                        0 | 2 => ctx.store_evaluation_data(batch("leaf")).unwrap(),
                        1 => ctx.store_evaluation_data(EventBatch::empty()).unwrap(),
                        3 => {}
                        4 => ctx.store_evaluation_data(batch("changed-leaf")).unwrap(),
                        _ => unreachable!("event mode is bounded by its tests"),
                    }
                }
                Self::Parent => {
                    ctx.compute(&Self::Leaf).await.unwrap();
                    let text = if ctx.compute(&EventMode).await.unwrap() == 4 {
                        "changed-parent"
                    } else {
                        "parent"
                    };
                    ctx.store_evaluation_data(batch(text)).unwrap();
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
            location: slug_events_v2::StarlarkSourceLocation::new(Arc::from("synthetic.bzl"), 1, 6),
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
        let state = sealed.select(transaction).await?.events().clone();
        Ok(state.reconcile(&AcceptedEventEpoch::empty()).0)
    }

    async fn select_parent_state(
        dice: &Arc<Dice>,
        mode: Option<u8>,
    ) -> Result<super::SelectedEventState, CommandEffectError> {
        select_graph_state(dice, EventGraphKey::Parent, mode).await
    }

    async fn select_graph_state(
        dice: &Arc<Dice>,
        root: EventGraphKey,
        mode: Option<u8>,
    ) -> Result<super::SelectedEventState, CommandEffectError> {
        let owner = CommandEffectOwner::new();
        let attempt = owner.begin_attempt()?;
        let mut updater = dice.updater_with_data(user_data(dice, &attempt)?);
        if let Some(mode) = mode {
            updater
                .changed_to(vec![(EventMode, mode)])
                .expect("event mode injection is valid");
        }
        let mut transaction = updater.commit().await;
        transaction
            .compute(&root)
            .await
            .expect("event graph compute");
        let sealed = attempt.seal_terminal()?;
        Ok(sealed.select(&transaction).await?.events().clone())
    }

    fn selected_text(selected: &SelectedEventBatches) -> Vec<&str> {
        selected
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => {
                    unreachable!("diagnostic events are not produced by this packet")
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn accepted_epoch_distinguishes_reuse_none_and_empty() -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let (first, accepted) = select_parent_state(&dice, Some(0))
            .await?
            .reconcile(&AcceptedEventEpoch::empty());
        assert_eq!(selected_text(&first), ["leaf", "parent"]);

        let (warm, carried) = select_parent_state(&dice, None).await?.reconcile(&accepted);
        assert!(warm.batches().is_empty());
        assert_eq!(carried, accepted);
        let (equal, equal_epoch) = select_parent_state(&dice, Some(2))
            .await?
            .reconcile(&carried);
        assert!(equal.batches().is_empty());
        assert_eq!(equal_epoch, accepted);

        let (removed, removed_epoch) = select_parent_state(&dice, Some(3))
            .await?
            .reconcile(&equal_epoch);
        assert!(removed.batches().is_empty());
        assert_eq!(removed_epoch.entries.len(), 1);
        let (still_removed, still_removed_epoch) = select_parent_state(&dice, None)
            .await?
            .reconcile(&removed_epoch);
        assert!(still_removed.batches().is_empty());
        assert_eq!(still_removed_epoch, removed_epoch);
        let (reappeared, reappeared_epoch) = select_parent_state(&dice, Some(2))
            .await?
            .reconcile(&still_removed_epoch);
        assert_eq!(selected_text(&reappeared), ["leaf"]);

        let (empty_output, empty_epoch) = select_parent_state(&dice, Some(1))
            .await?
            .reconcile(&reappeared_epoch);
        assert!(empty_output.batches().is_empty());
        assert_eq!(empty_epoch.entries.len(), 2);
        assert!(
            empty_epoch
                .entries
                .iter()
                .any(|(_, batch)| batch.events().is_empty())
        );
        let (changed, _) = select_parent_state(&dice, Some(4))
            .await?
            .reconcile(&empty_epoch);
        assert_eq!(selected_text(&changed), ["changed-leaf", "changed-parent"]);
        Ok(())
    }

    #[tokio::test]
    async fn source_certified_policy_carries_known_none_only_for_matching_roots()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let (_, accepted) = select_parent_state(&dice, Some(0))
            .await?
            .reconcile(&AcceptedEventEpoch::empty());

        let removed_state = select_parent_state(&dice, Some(3)).await?;
        let (strict, strict_epoch, _) = removed_state.clone().reconcile_revision(
            &accepted,
            None,
            EventReconciliationPolicy::Strict,
        );
        assert!(strict.batches().is_empty());
        assert_eq!(strict_epoch.entries.len(), 1);

        let (carried, carried_epoch, _) = removed_state.clone().reconcile_revision(
            &accepted,
            None,
            EventReconciliationPolicy::SourceCertifiedCurrentClosure,
        );
        assert!(carried.batches().is_empty());
        assert_eq!(carried_epoch, accepted);

        let mut mismatched = removed_state;
        mismatched.roots = Arc::from([]);
        let (_, mismatched_epoch, _) = mismatched.reconcile_revision(
            &accepted,
            None,
            EventReconciliationPolicy::SourceCertifiedCurrentClosure,
        );
        assert_eq!(mismatched_epoch.entries.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn source_certified_retries_fold_mixed_nodes_in_current_closure_order()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let base = select_parent_state(&dice, Some(0)).await?;
        let (_, mut prior) = base.clone().reconcile(&AcceptedEventEpoch::empty());
        let left = select_graph_state(&dice, EventGraphKey::Left, None).await?;
        let right = select_graph_state(&dice, EventGraphKey::Right, None).await?;
        let mut extras = left
            .nodes
            .iter()
            .chain(right.nodes.iter())
            .map(|(node, _)| *node)
            .filter(|node| prior.entries.iter().all(|(known, _)| known != node))
            .collect::<Vec<_>>();
        extras.sort();
        extras.dedup();
        let [removed, empty, new, ..] = extras.as_slice() else {
            panic!("synthetic graph did not provide three distinct nodes");
        };
        prior.entries = prior
            .entries
            .iter()
            .cloned()
            .chain([(*removed, batch("removed"))])
            .collect();
        let leaf = prior.entries[0].0;
        let parent = prior.entries[1].0;
        let retry = SelectedEventState {
            roots: prior.roots.clone(),
            nodes: Arc::from([
                (leaf, SelectedEventTransition::Known(None)),
                (parent, SelectedEventTransition::NoTransition),
            ]),
            batches: Arc::from([]),
        };
        let (_, _, carried) = retry.reconcile_revision(
            &prior,
            None,
            EventReconciliationPolicy::SourceCertifiedCurrentClosure,
        );
        let retry_again = SelectedEventState {
            roots: prior.roots.clone(),
            nodes: Arc::from([
                (leaf, SelectedEventTransition::NoTransition),
                (parent, SelectedEventTransition::NoTransition),
                (*removed, SelectedEventTransition::NoTransition),
            ]),
            batches: Arc::from([]),
        };
        let (_, _, carried) = retry_again.reconcile_revision(
            &prior,
            Some(&carried),
            EventReconciliationPolicy::SourceCertifiedCurrentClosure,
        );
        let final_state = SelectedEventState {
            roots: prior.roots.clone(),
            nodes: Arc::from([
                (leaf, SelectedEventTransition::NoTransition),
                (
                    parent,
                    SelectedEventTransition::Known(Some(batch("changed"))),
                ),
                (*removed, SelectedEventTransition::NoTransition),
                (
                    *empty,
                    SelectedEventTransition::Known(Some(EventBatch::empty())),
                ),
                (*new, SelectedEventTransition::Known(Some(batch("new")))),
            ]),
            batches: Arc::from([]),
        };
        let (output, accepted, _) = final_state.reconcile_revision(
            &prior,
            Some(&carried),
            EventReconciliationPolicy::SourceCertifiedCurrentClosure,
        );
        assert_eq!(selected_text(&output), ["changed", "new"]);
        assert_eq!(
            accepted
                .entries
                .iter()
                .map(|(node, _)| *node)
                .collect::<Vec<_>>(),
            [leaf, parent, *empty, *new]
        );
        assert!(accepted.entries[2].1.events().is_empty());
        Ok(())
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
        let reopened = owner.begin_attempt()?;
        reopened.seal_retry()?;

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

        let transient_owner = CommandEffectOwner::new();
        let transient_attempt = transient_owner.begin_attempt()?;
        let mut transient_transaction = transient_dice
            .updater_with_data(user_data(&transient_dice, &transient_attempt)?)
            .commit()
            .await;
        transient_transaction.compute(&TransientEventKey).await?;
        let transient_sealed = transient_attempt.seal_terminal_allowing_unavailable_roots()?;
        let selected = select_events(transient_sealed, &transient_transaction).await?;
        assert!(selected.batches().is_empty());
        Ok(())
    }
}
