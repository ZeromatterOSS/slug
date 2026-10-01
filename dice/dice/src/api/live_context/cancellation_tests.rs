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
use std::future;
use std::sync::Arc;
use std::time::Duration;

use allocative::Allocative;
use async_trait::async_trait;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;
use tokio::task::AbortHandle;

use crate::CancellationContext;
use crate::DetectCycles;
use crate::Dice;
use crate::DiceComputations;
use crate::Key;
use crate::UserComputationData;

struct ProbeState {
    entered: Arc<Semaphore>,
    release: Arc<Semaphore>,
    dropped: Option<oneshot::Sender<()>>,
}

impl Drop for ProbeState {
    fn drop(&mut self) {
        let _ignored = self.dropped.take().unwrap().send(());
    }
}

// Neither the key nor its value retains the request's userdata.
#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct HeldKey;

impl fmt::Display for HeldKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("held live-context computation")
    }
}

#[async_trait]
impl Key for HeldKey {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        let state = ctx
            .per_transaction_data()
            .data
            .get::<Arc<ProbeState>>()
            .unwrap()
            .clone();
        cancellations
            .critical_section(|| async {
                state.entered.add_permits(1);
                state.release.acquire().await.unwrap().forget();
            })
            .await;
        // Cancellation must retire this computation, never publish a value.
        future::pending::<()>().await
    }

    fn equality(_left: &Self::Value, _right: &Self::Value) -> bool {
        true
    }
}

struct Cleanup {
    request: AbortHandle,
    release: Arc<Semaphore>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.request.abort();
        self.release.add_permits(1);
    }
}

#[tokio::test]
async fn reopened_context_is_distinct_while_cancelled_worker_is_retained() {
    tokio::time::timeout(Duration::from_secs(2), async {
        let dice = Dice::builder().build(DetectCycles::Disabled);
        let entered = Arc::new(Semaphore::new(0));
        let release = Arc::new(Semaphore::new(0));
        let (dropped_sender, dropped_receiver) = oneshot::channel();
        let state = Arc::new(ProbeState {
            entered: entered.clone(),
            release: release.clone(),
            dropped: Some(dropped_sender),
        });
        let weak_state = Arc::downgrade(&state);
        let mut data = UserComputationData::default();
        data.data.set(state);
        let mut old = dice.updater_with_data(data).commit().await;
        let old_version = old.version();
        let old_id = old.live_context_id();

        let simultaneous = dice.updater().commit().await;
        assert_eq!(simultaneous.version(), old_version);
        assert_eq!(simultaneous.live_context_id(), old_id);
        drop(simultaneous);

        // The requesting task now owns the sole remaining transaction.
        let request = tokio::spawn(async move { old.compute(&HeldKey).await });
        let _cleanup = Cleanup {
            request: request.abort_handle(),
            release: release.clone(),
        };
        entered.acquire().await.unwrap().forget();
        request.abort();
        let cancelled = request.await.unwrap_err().is_cancelled();

        let reopened = dice.updater().commit().await;
        let reopened_version = reopened.version();
        let reopened_id = reopened.live_context_id();
        let old_userdata_retained = weak_state.upgrade().is_some();
        drop(reopened);

        release.add_permits(1);
        dice.wait_for_idle().await;
        // Observe actual destruction rather than assuming its scheduling
        // relative to the worker's termination notification.
        dropped_receiver.await.unwrap();

        assert!(cancelled);
        assert!(old_userdata_retained);
        assert_eq!(reopened_version, old_version);
        assert_ne!(reopened_id, old_id);
        assert!(weak_state.upgrade().is_none());
    })
    .await
    .expect("live-context cancellation fixture must retire within two seconds");
}
