/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[allow(unused_imports)]
use gazebo::variants::VariantName;

use crate::impls::core::graph::storage::ValueReusable;
use crate::impls::core::internals::CoreState;
use crate::impls::core::state::CoreStateHandle;
use crate::impls::core::state::StateRequest;
use crate::impls::ctx::SharedLiveTransactionCtx;

static NEXT_DICE_ENGINE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct StateProcessor {
    engine: u64,
    state: CoreState,
    rx: tokio::sync::mpsc::UnboundedReceiver<StateRequest>,
}

impl StateProcessor {
    pub(super) fn spawn() -> CoreStateHandle {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = NEXT_DICE_ENGINE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("DICE engine identity space exhausted");
        let state = CoreState::new();

        std::thread::Builder::new()
            .name("slug-dice".to_owned())
            .spawn(move || StateProcessor { engine, state, rx }.event_loop())
            .unwrap();

        CoreStateHandle::new(tx, engine)
    }

    fn event_loop(mut self) {
        loop {
            // Skip tokio scheduling.
            while let Ok(message) = self.rx.try_recv() {
                self.iteration(message);
            }
            if let Some(message) = self.rx.blocking_recv() {
                self.iteration(message);
            } else {
                break;
            }
        }
        debug!("Processor terminated");
    }

    #[cfg_attr(debug_assertions, instrument(skip_all, fields(kind = %message.variant_name())))]
    fn iteration(&mut self, message: StateRequest) {
        match message {
            StateRequest::UpdateState { changes, resp } => {
                // ignore error if the requester dropped it.
                let _ = resp.send(self.state.update_state(changes));
            }
            StateRequest::CtxAtVersion {
                version,
                guard,
                resp,
            } => {
                let (version_epoch, cache) = self.state.ctx_at_version(version);

                let ctx = SharedLiveTransactionCtx::new(version, version_epoch, cache);
                let _ignored = resp.send((ctx, guard));
            }
            StateRequest::DropCtxAtVersion { version } => self.state.drop_ctx_at_version(version),
            StateRequest::CurrentVersion { resp } => {
                // ignore error if the requester dropped it.
                let _ = resp.send(self.state.current_version());
            }
            StateRequest::LookupKey {
                key,
                include_activation_dependencies,
                resp,
            } => drop(resp.send(self.state.lookup_key(key, include_activation_dependencies))),
            StateRequest::ActivationDependencies { version, key, resp } => {
                drop(resp.send(self.state.activation_dependencies(version, key)));
            }
            StateRequest::ActivationClosure {
                version,
                roots,
                resp,
            } => {
                drop(resp.send(self.state.activation_closure(self.engine, version, roots)));
            }
            #[cfg(test)]
            StateRequest::ActivationHistoryLen { key, resp } => {
                let _ignored = resp.send(self.state.activation_history_len(key));
            }
            #[cfg(test)]
            StateRequest::RemoveActivationHistory { key, resp } => {
                self.state.remove_activation_history(key);
                let _ignored = resp.send(());
            }
            StateRequest::UpdateComputed {
                key,
                epoch,
                storage,
                value,
                deps,
                invalidation_paths,
                resp,
            } => {
                // ignore error if the requester dropped it.
                drop(resp.send(self.state.update_computed(
                    key,
                    epoch,
                    storage,
                    value,
                    ValueReusable::EqualityBased,
                    deps,
                    invalidation_paths,
                )));
            }
            StateRequest::UpdateMismatchAsUnchanged {
                key,
                epoch,
                storage,
                previous,
                invalidation_paths,
                resp,
            } => {
                // ignore error if the requester dropped it.
                drop(resp.send(self.state.update_computed(
                    key,
                    epoch,
                    storage,
                    previous.entry,
                    ValueReusable::VersionBased(previous.prev_verified_version),
                    previous.deps_to_validate,
                    invalidation_paths,
                )));
            }
            StateRequest::GetTasksPendingCancellation { resp } => {
                let _ignored = resp.send(self.state.get_tasks_pending_cancellation());
            }
            StateRequest::UnstableDropEverything => self.state.unstable_drop_everything(),
            StateRequest::Metrics { resp } => {
                let _ignored = resp.send(self.state.metrics());
            }
            StateRequest::Introspection { resp } => {
                let _ignored = resp.send(self.state.introspection());
            }
            StateRequest::MakeAvailableForAllocative { resp } => {
                use std::sync::Arc;

                let (complete_tx, complete_rx) = tokio::sync::oneshot::channel();
                let state = std::mem::replace(&mut self.state, CoreState::new());
                let arc_state = Arc::new(state);
                drop(resp.send((Arc::clone(&arc_state), complete_tx)));
                drop(complete_rx.blocking_recv());
                // Correctness: Contract on `MakeAvailableForAllocative`
                let state =
                    Arc::into_inner(arc_state).expect("Other references to have been dropped");
                self.state = state;
            }
        }
    }
}
