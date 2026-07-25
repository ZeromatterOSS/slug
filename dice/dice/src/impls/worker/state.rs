/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The main worker thread for the dice task

use std::sync::Arc;

use dice_error::result::CancellableResult;
use dice_error::result::CancellationReason;
use dice_futures::cancellation::CriticalSectionGuard;
use dice_futures::cancellation::DisableCancellationGuard;
use dupe::Dupe;
use itertools::Either;

use crate::ActivationData;
use crate::ActivationKind;
use crate::ActivationTracker;
use crate::DiceNodeId;
use crate::DynKey;
use crate::RichActivation;
use crate::VersionNumber;
use crate::impls::core::state::CoreStateHandle;
use crate::impls::evaluator::AsyncEvaluator;
use crate::impls::evaluator::KeyEvaluationResult;
use crate::impls::key::DiceKey;
use crate::impls::key::DiceKeyErased;
use crate::impls::key_index::DiceKeyIndex;
use crate::impls::task::PreviouslyCancelledTask;
use crate::impls::task::handle::DiceTaskHandle;
use crate::impls::user_cycle::KeyComputingUserCycleDetectorData;
use crate::impls::user_cycle::UserCycleDetectorData;
use crate::impls::value::DiceComputedValue;

/// Represents when we are in a spawned dice task worker and are currently waiting for the previous
/// cancelled instance of this task to finish cancelling.
pub(crate) struct DiceWorkerStateAwaitingPrevious<'a> {
    k: DiceKey,
    cycles: UserCycleDetectorData,
    prevent_cancellation: CriticalSectionGuard<'a>,
}

impl<'a> DiceWorkerStateAwaitingPrevious<'a> {
    pub(crate) fn new(
        k: DiceKey,
        cycles: UserCycleDetectorData,
        prevent_cancellation: CriticalSectionGuard<'a>,
    ) -> Self {
        debug!(msg = "Task started. Waiting for previously cancelled task if any");
        Self {
            k,
            cycles,
            prevent_cancellation,
        }
    }

    pub(crate) fn previously_finished(
        self,
        value: DiceComputedValue,
    ) -> CancellableResult<DiceWorkerStateFinishedAndCached> {
        debug!(msg = "previously cancelled task actually finished");

        let guard = self.prevent_cancellation.try_disable_cancellation();
        finish_with_cached_value(value, guard)
    }

    pub(crate) async fn previously_cancelled(
        self,
        internals: &mut DiceTaskHandle<'_>,
    ) -> DiceWorkerStateLookupNode {
        debug!(msg = "previously cancelled task was cancelled");

        self.prevent_cancellation.exit_critical_section().await;

        internals.report_initial_lookup();

        DiceWorkerStateLookupNode {
            k: self.k,
            cycles: self.cycles,
        }
    }

    pub(crate) async fn no_previous_task(
        self,
        internals: &mut DiceTaskHandle<'_>,
    ) -> DiceWorkerStateLookupNode {
        debug!(msg = "no previous task to wait for");

        self.prevent_cancellation.exit_critical_section().await;

        internals.report_initial_lookup();

        DiceWorkerStateLookupNode {
            k: self.k,
            cycles: self.cycles,
        }
    }

    pub(crate) async fn await_previous(
        self,
        internals: &mut DiceTaskHandle<'_>,
        previous: PreviouslyCancelledTask,
    ) -> Either<CancellableResult<DiceWorkerStateFinishedAndCached>, DiceWorkerStateLookupNode>
    {
        previous.previous.await_termination().await;

        // old task actually finished, so just use that result if it wasn't
        // cancelled

        match previous
            .previous
            .get_finished_value()
            .expect("Terminated task must have finished value")
        {
            Ok(res) => {
                return Either::Left(self.previously_finished(res));
            }
            Err(_cancelled) => {
                // actually was cancelled, so just continue re-evaluating
            }
        }

        Either::Right(self.previously_cancelled(internals).await)
    }
}

fn finish_with_cached_value(
    value: DiceComputedValue,
    disable_cancellation: Option<DisableCancellationGuard>,
) -> CancellableResult<DiceWorkerStateFinishedAndCached> {
    match disable_cancellation {
        None => Err(CancellationReason::Cached),
        Some(g) => Ok(DiceWorkerStateFinishedAndCached {
            value,
            _prevent_cancellation: g,
        }),
    }
}

/// Represents when we are currently looking up the current requested key from the core state, and
/// are waiting for it to respond.
pub(crate) struct DiceWorkerStateLookupNode {
    k: DiceKey,
    cycles: UserCycleDetectorData,
}

impl DiceWorkerStateLookupNode {
    pub(crate) fn checking_deps(
        self,
        internals: &mut DiceTaskHandle,
        eval: &AsyncEvaluator,
    ) -> (
        DiceWorkerStateCheckingDeps,
        KeyComputingUserCycleDetectorData,
    ) {
        debug!(msg = "found existing entry with mismatching version. checking if deps changed.");

        internals.checking_deps();

        let cycles = self.cycles.start_computing_key(
            self.k,
            &eval.dice.key_index,
            eval.user_data.cycle_detector.as_ref(),
        );

        (DiceWorkerStateCheckingDeps {}, cycles)
    }

    pub(crate) fn lookup_dirtied(
        self,
        internals: &mut DiceTaskHandle,
        eval: &AsyncEvaluator,
    ) -> (DiceWorkerStateEvaluating, KeyComputingUserCycleDetectorData) {
        debug!(msg = "lookup requires recompute.");

        internals.computing();

        let cycles = self.cycles.start_computing_key(
            self.k,
            &eval.dice.key_index,
            eval.user_data.cycle_detector.as_ref(),
        );

        (DiceWorkerStateEvaluating {}, cycles)
    }

    pub(crate) fn lookup_matches(
        self,
        internals: &mut DiceTaskHandle,
        value: DiceComputedValue,
    ) -> CancellableResult<DiceWorkerStateFinishedAndCached> {
        debug!(msg = "found existing entry with matching version in cache. reusing result.");

        let guard = internals.cancellation_ctx().try_disable_cancellation();
        finish_with_cached_value(value, guard)
    }
}

/// When the spawned dice task worker is checking if the dependencies have changed since the last
/// time this node was verified, and are waiting for the results of the dependency re-computation.
pub(crate) struct DiceWorkerStateCheckingDeps {}

impl DiceWorkerStateCheckingDeps {
    pub(crate) fn deps_not_match(
        self,
        internals: &mut DiceTaskHandle,
    ) -> DiceWorkerStateEvaluating {
        debug!(msg = "deps changed");
        internals.computing();

        DiceWorkerStateEvaluating {}
    }

    pub(crate) fn deps_match(
        self,
        internals: &mut DiceTaskHandle,
    ) -> CancellableResult<DiceWorkerStateFinished> {
        debug!(msg = "reusing previous value because deps didn't change. Updating caches");

        let guard = match internals.cancellation_ctx().try_disable_cancellation() {
            Some(g) => g,
            None => {
                debug!("evaluation cancelled, skipping cache updates");
                return Err(CancellationReason::DepsMatch);
            }
        };

        Ok(DiceWorkerStateFinished {
            _prevent_cancellation: guard,
        })
    }
}

/// When the spawned dice worker is currently actively evaluating the `Key::compute` function
pub(crate) struct DiceWorkerStateEvaluating {}

impl DiceWorkerStateEvaluating {
    pub(crate) fn finished(
        self,
        internals: &mut DiceTaskHandle,
        cycles: KeyComputingUserCycleDetectorData,
        result: KeyEvaluationResult,
        activation_data: ActivationData,
    ) -> CancellableResult<DiceWorkerStateFinishedEvaluating> {
        debug!(msg = "evaluation finished. updating caches");

        let guard = match internals.cancellation_ctx().try_disable_cancellation() {
            Some(g) => g,
            None => {
                debug!("evaluation cancelled, skipping cache updates");
                return Err(CancellationReason::WorkerFinished);
            }
        };

        drop(cycles);

        Ok(DiceWorkerStateFinishedEvaluating {
            state: DiceWorkerStateFinished {
                _prevent_cancellation: guard,
            },
            activation_data,
            result,
        })
    }
}

/// When the spawned dice worker has just finished evaluating the `Key::compute` function
pub(crate) struct DiceWorkerStateFinishedEvaluating {
    pub(crate) state: DiceWorkerStateFinished,
    pub(crate) activation_data: ActivationData,
    pub(crate) result: KeyEvaluationResult,
}

/// When the spawned dice worker is finished checking dependencies or finished computing the key.
/// At this point, the value of the node is known. We are just waiting for core state to finish
/// updating the caches and return the correct instance of the value.
pub(crate) struct DiceWorkerStateFinished {
    _prevent_cancellation: DisableCancellationGuard,
}

impl DiceWorkerStateFinished {
    pub(crate) fn cached(
        self,
        value: DiceComputedValue,
        activation_info: Option<ActivationInfo>,
    ) -> DiceWorkerStateFinishedAndCached {
        debug!(msg = "Update caches complete");

        if let Some(activation_info) = activation_info {
            activation_info.notify_with_legacy();
        }

        DiceWorkerStateFinishedAndCached {
            value,
            _prevent_cancellation: self._prevent_cancellation,
        }
    }
}

pub(crate) struct ActivationInfo {
    activation_tracker: Arc<dyn ActivationTracker>,
    node: DiceNodeId,
    version: VersionNumber,
    key: DiceKeyErased,
    deps: Vec<DiceKeyErased>,
    dependency_ids: Vec<DiceNodeId>,
    activation_data: ActivationData,
}

impl ActivationInfo {
    pub(crate) fn new<'a>(
        key_index: &DiceKeyIndex,
        state: &CoreStateHandle,
        activation_tracker: &Option<Arc<dyn ActivationTracker>>,
        version: VersionNumber,
        key: DiceKey,
        deps: impl Iterator<Item = DiceKey> + 'a,
        activation_data: ActivationData,
    ) -> Option<ActivationInfo> {
        if let Some(activation_tracker) = activation_tracker {
            let node = state.node_id(key);
            let key = key_index.get(key).dupe();
            let tracks_rich_activations = activation_tracker.tracks_rich_activations();
            let mut dependency_ids = Vec::new();
            let deps = deps
                .map(|dep| {
                    if tracks_rich_activations {
                        dependency_ids.push(state.node_id(dep));
                    }
                    key_index.get(dep).dupe()
                })
                .collect();

            Some(ActivationInfo {
                activation_tracker: activation_tracker.dupe(),
                node,
                version,
                key,
                deps,
                dependency_ids,
                activation_data,
            })
        } else {
            None
        }
    }

    pub(crate) fn new_rich<'a>(
        key_index: &DiceKeyIndex,
        state: &CoreStateHandle,
        activation_tracker: &Option<Arc<dyn ActivationTracker>>,
        version: VersionNumber,
        key: DiceKey,
        deps: impl Iterator<Item = DiceKey> + 'a,
        kind: ActivationKind,
    ) -> Option<Self> {
        let activation_tracker = activation_tracker
            .as_ref()
            .filter(|tracker| tracker.tracks_rich_activations())?;
        let key_erased = key_index.get(key).dupe();
        let dependency_ids = deps.map(|dep| state.node_id(dep)).collect();
        Some(Self {
            activation_tracker: activation_tracker.dupe(),
            node: state.node_id(key),
            version,
            key: key_erased,
            deps: Vec::new(),
            dependency_ids,
            activation_data: match kind {
                ActivationKind::Evaluated => ActivationData::Evaluated(None),
                ActivationKind::Reused => ActivationData::Reused,
            },
        })
    }

    pub(crate) fn notify_rich_only(self) {
        self.notify_rich();
    }

    fn notify_with_legacy(self) {
        self.notify_rich();
        self.activation_tracker.key_activated(
            DynKey::ref_cast(&self.key),
            &mut self.deps.iter().map(DynKey::ref_cast),
            self.activation_data,
        );
    }

    fn notify_rich(&self) {
        if self.activation_tracker.tracks_rich_activations() {
            self.activation_tracker.key_activated_rich(
                DynKey::ref_cast(&self.key),
                RichActivation::new(
                    self.node,
                    self.version,
                    self.activation_data.kind(),
                    self.activation_data.evaluation_data(),
                    &self.dependency_ids,
                ),
            );
        }
    }
}

/// When the spawned dice worker is done computing and saving the value to core state cache.
/// The final value is known.
pub(crate) struct DiceWorkerStateFinishedAndCached {
    pub(crate) value: DiceComputedValue,
    pub(crate) _prevent_cancellation: DisableCancellationGuard,
}
