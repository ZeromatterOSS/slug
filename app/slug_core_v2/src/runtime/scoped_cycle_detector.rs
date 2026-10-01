/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Nonsemantic cycle plumbing shared by exactly one live DICE cache.
//! Userdata stays request-local; only its deferred detector joins the coordinator.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

use dice::DiceLiveContextId;
use dice::DiceTransaction;
use dice::DynKey;
use dice::UserComputationData;
use dice::UserCycleDetector;
use dice::UserCycleDetectorGuard;
use slug_analysis_v2::analysis_cycle_detector;
use starlark_map::small_map::SmallMap;
use tokio::runtime::Handle;

pub(super) struct ScopedCycleDetectorRegistry {
    runtime: Handle,
    entries: Mutex<SmallMap<DiceLiveContextId, Weak<dyn UserCycleDetector>>>,
}

impl ScopedCycleDetectorRegistry {
    pub(super) fn new(runtime: Handle) -> Self {
        Self {
            runtime,
            entries: Mutex::new(SmallMap::new()),
        }
    }

    pub(super) fn install(&self, data: &mut UserComputationData) {
        let proxy = Arc::new(DeferredCycleDetector {
            bound: OnceLock::new(),
        });
        data.cycle_detector = Some(proxy.clone());
        data.data.set(PendingCycleBinding(proxy));
    }

    pub(super) fn bind(&self, transaction: &DiceTransaction) {
        // Standalone callers may explicitly supply their own detector. Every
        // userdata installed here has a pending handle and must bind before compute.
        let Ok(pending) = transaction
            .per_transaction_data()
            .data
            .get::<PendingCycleBinding>()
        else {
            return;
        };
        self.entries
            .lock()
            .expect("cycle registry mutex poisoned")
            .retain(|_, entry| entry.strong_count() != 0);
        let id = transaction.live_context_id();
        let bound = pending.0.bound.get_or_init(|| BoundCycleDetector {
            id,
            detector: self.detector_for(id),
        });
        assert_eq!(
            bound.id, id,
            "a deferred cycle proxy cannot migrate to another live DICE cache"
        );
    }

    fn detector_for(&self, id: DiceLiveContextId) -> Arc<dyn UserCycleDetector> {
        let mut entries = self.entries.lock().expect("cycle registry mutex poisoned");
        entries.retain(|_, entry| entry.strong_count() != 0);
        if let Some(detector) = entries.get(&id).and_then(Weak::upgrade) {
            return detector;
        }
        // Legacy detector actors belong to the retained workspace runtime,
        // even when a transaction's caller runs on a different Tokio runtime.
        // Construction is synchronous and never computes a DICE key.
        let _runtime = self.runtime.enter();
        let detector = analysis_cycle_detector();
        entries.insert(id, Arc::downgrade(&detector));
        detector
    }
}

struct PendingCycleBinding(Arc<DeferredCycleDetector>);

struct BoundCycleDetector {
    id: DiceLiveContextId,
    detector: Arc<dyn UserCycleDetector>,
}

struct DeferredCycleDetector {
    bound: OnceLock<BoundCycleDetector>,
}

impl DeferredCycleDetector {
    fn detector(&self) -> &Arc<dyn UserCycleDetector> {
        &self
            .bound
            .get()
            .expect("a deferred cycle proxy must bind before DICE computation")
            .detector
    }
}

impl UserCycleDetector for DeferredCycleDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        self.detector().start_computing_key(key)
    }

    fn finished_computing_key(&self, key: &DynKey) {
        self.detector().finished_computing_key(key);
    }
}

#[cfg(test)]
mod tests;
