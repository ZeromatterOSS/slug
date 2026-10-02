use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use derivative::Derivative;
use derive_more::Display;
use dice_futures::cancellation::CancellationContext;

use crate::Dice;
use crate::DiceComputations;
use crate::Key;
use crate::api::cycles::DetectCycles;
use crate::api::storage_type::StorageType;

#[derive(Debug, Clone, Display, Derivative, Allocative)]
#[derivative(Eq, PartialEq, Hash)]
#[display("guarded-injected-input")]
#[allocative(skip)]
struct GuardedInput {
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    entered: Arc<AtomicUsize>,
}

#[async_trait]
impl Key for GuardedInput {
    type Value = Option<i32>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        self.entered.fetch_add(1, Ordering::SeqCst);
        // Even returning a value cannot make an injected input computable.
        // The worker must reject lookup absence before entering this callback.
        None
    }

    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }

    fn storage_type() -> StorageType {
        StorageType::Injected
    }
}

#[derive(Debug, Clone, Display, Eq, PartialEq, Hash, Allocative)]
#[display("ordinary-unset-key")]
struct Ordinary;

#[async_trait]
impl Key for Ordinary {
    type Value = i32;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        42
    }

    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}

#[tokio::test]
async fn unset_injected_input_rejects_without_compute_and_later_injection_recovers() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let entered = Arc::new(AtomicUsize::new(0));
    let key = GuardedInput {
        entered: entered.clone(),
    };
    let mut before = dice.updater().commit().await;
    assert!(before.compute(&key).await.is_err());
    assert_eq!(entered.load(Ordering::SeqCst), 0);
    assert_eq!(before.compute(&Ordinary).await.unwrap(), 42);

    let mut updater = dice.updater();
    updater.changed_to([(key.clone(), Some(7))]).unwrap();
    let mut injected = updater.commit().await;
    assert_eq!(injected.compute(&key).await.unwrap(), Some(7));
    // A historical transaction predating this input remains unavailable.
    assert!(before.compute(&key).await.is_err());
    assert_eq!(entered.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn historical_unread_injected_input_survives_replacement_and_revocation() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let entered = Arc::new(AtomicUsize::new(0));
    let key = GuardedInput {
        entered: entered.clone(),
    };
    let mut updater = dice.updater();
    updater.changed_to([(key.clone(), Some(7))]).unwrap();
    let mut old_unread = updater.commit().await;
    let mut updater = dice.updater();
    updater.changed_to([(key.clone(), Some(11))]).unwrap();
    let mut changed = updater.commit().await;
    assert_eq!(changed.compute(&key).await.unwrap(), Some(11));
    assert_eq!(old_unread.compute(&key).await.unwrap(), Some(7));

    let mut updater = dice.updater();
    updater.changed_to([(key.clone(), None)]).unwrap();
    let mut revoked = updater.commit().await;
    assert_eq!(revoked.compute(&key).await.unwrap(), None);
    assert_eq!(changed.compute(&key).await.unwrap(), Some(11));
    assert_eq!(old_unread.compute(&key).await.unwrap(), Some(7));
    assert_eq!(entered.load(Ordering::SeqCst), 0);
}
