use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;

fn policy() -> GuestNamespacePolicy {
    GuestNamespacePolicy::new(
        1000,
        100,
        1001,
        101,
        7,
        12,
        34,
        0o755,
        GuestNamespaceLimits::default(),
    )
    .unwrap()
}
fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/guest-policy-test").unwrap()
}

#[test]
fn explicit_policy_identity_owns_all_credentials_metadata_and_limits() {
    let p = policy();
    assert_eq!(p.scheme(), GuestNamespaceMetadataScheme::UniformLinuxV1);
    assert_eq!(
        (p.owner_uid(), p.owner_gid(), p.caller_uid(), p.caller_gid()),
        (1000, 100, 1001, 101)
    );
    assert_eq!(
        (
            p.device(),
            p.epoch_seconds(),
            p.epoch_nanoseconds(),
            p.scaffold_mode()
        ),
        (7, 12, 34, 0o755)
    );
    // Each independently observable field participates structurally; no device,
    // credential or time is silently inferred from native staging.
    for changed in [
        GuestNamespacePolicy {
            owner_uid: 1002,
            ..p
        },
        GuestNamespacePolicy {
            owner_gid: 102,
            ..p
        },
        GuestNamespacePolicy {
            caller_uid: 1002,
            ..p
        },
        GuestNamespacePolicy {
            caller_gid: 102,
            ..p
        },
        GuestNamespacePolicy { device: 8, ..p },
        GuestNamespacePolicy {
            epoch_seconds: 13,
            ..p
        },
        GuestNamespacePolicy {
            epoch_nanoseconds: 35,
            ..p
        },
        GuestNamespacePolicy {
            scaffold_mode: 0o700,
            ..p
        },
        GuestNamespacePolicy {
            limits: GuestNamespaceLimits {
                max_entries: 1,
                ..p.limits()
            },
            ..p
        },
    ] {
        assert!(!RawGuestNamespacePolicyKey::equality(
            &Some(p),
            &Some(changed)
        ));
    }
    assert!(RawGuestNamespacePolicyKey::equality(&Some(p), &Some(p)));
    assert!(!RawGuestNamespacePolicyKey::equality(&None, &Some(p)));
}

#[test]
fn uniform_linux_policy_rejects_privileged_and_unrepresentable_fields() {
    let make = |uid, caller, dev, seconds, nanos, mode| {
        GuestNamespacePolicy::new(
            uid,
            0,
            caller,
            0,
            dev,
            seconds,
            nanos,
            mode,
            GuestNamespaceLimits::default(),
        )
    };
    assert!(make(1, 1, 1, 0, 0, 0).is_ok());
    assert!(make(u32::MAX, u32::MAX, u64::MAX, i64::MAX, 999_999_999, 0o777).is_ok());
    for result in [
        make(0, 1, 1, 0, 0, 0),
        make(1, 0, 1, 0, 0, 0),
        make(1, 1, 0, 0, 0, 0),
        make(1, 1, 1, -1, 0, 0),
        make(1, 1, 1, 0, 1_000_000_000, 0),
        make(1, 1, 1, 0, 0, 0o1000),
        make(1, 1, 1, 0, 0, 0o4000),
    ] {
        assert_eq!(
            result.unwrap_err(),
            GuestNamespacePolicyError::InvalidPolicy
        );
    }
    assert!(!GuestNamespacePolicyError::InvalidPolicy.is_transient());
    assert!(GuestNamespacePolicyError::UnauthorizedCurrentPolicy.is_transient());
}

#[test]
fn exact_limit_ceilings_and_small_negative_policies_are_explicit() {
    let c = GuestNamespaceLimits::default();
    let make = |limits| GuestNamespacePolicy::new(1, 1, 1, 1, 1, 0, 0, 0o755, limits);
    assert!(make(c).is_ok());
    assert!(
        make(GuestNamespaceLimits {
            max_entries: 1,
            max_depth: 0,
            max_path_bytes: 1,
            max_descriptor_bytes: 0,
            max_total_file_bytes: 0
        })
        .is_ok()
    );
    for invalid in [
        GuestNamespaceLimits {
            max_entries: 0,
            ..c
        },
        GuestNamespaceLimits {
            max_entries: c.max_entries + 1,
            ..c
        },
        GuestNamespaceLimits {
            max_depth: c.max_depth + 1,
            ..c
        },
        GuestNamespaceLimits {
            max_path_bytes: 0,
            ..c
        },
        GuestNamespaceLimits {
            max_path_bytes: c.max_path_bytes + 1,
            ..c
        },
        GuestNamespaceLimits {
            max_descriptor_bytes: c.max_descriptor_bytes + 1,
            ..c
        },
        GuestNamespaceLimits {
            max_total_file_bytes: c.max_total_file_bytes + 1,
            ..c
        },
    ] {
        assert_eq!(
            make(invalid).unwrap_err(),
            GuestNamespacePolicyError::InvalidPolicy
        );
    }
}

#[test]
fn current_transaction_requires_full_policy_workspace_and_actual_service() {
    let b = ImageDescriptorBudget::new(1 << 16);
    let p = policy();
    let tx = GuestNamespaceTransaction::new(workspace(), Some(p), b.dupe()).unwrap();
    let changed = GuestNamespacePolicy {
        caller_uid: 2000,
        ..p
    };
    assert_eq!(tx.workspace(), &workspace());
    assert_eq!(tx.current(), Some(p));
    assert!(tx.authorizes(&p));
    assert!(!tx.authorizes(&changed));
    let absent = GuestNamespaceTransaction::new(workspace(), None, b.dupe()).unwrap();
    assert!(!absent.authorizes(&p));
    let foreign = ImageDescriptorBudget::new(1 << 16);
    let foreign_tx = GuestNamespaceTransaction::new(workspace(), Some(p), foreign.dupe()).unwrap();
    assert_eq!(tx, foreign_tx, "credit is nonsemantic");
    assert!(!tx.same_budget(&foreign));
    assert!(!foreign_tx.same_budget(&b));
    let wrong_workspace = GuestNamespaceTransaction::new(
        NormalizedAbsolutePath::new("/foreign").unwrap(),
        Some(p),
        b.dupe(),
    )
    .unwrap();
    assert_ne!(wrong_workspace.workspace(), tx.workspace());
    // Copy policy itself carries no workspace: consumers must check workspace
    // AND actual service in addition to full policy authorization on warm hits.
    assert!(wrong_workspace.authorizes(&p));
    drop((tx, absent, foreign_tx, wrong_workspace));
    assert_eq!(b.used_bytes(), 0);
    assert_eq!(foreign.used_bytes(), 0);
}

#[test]
fn transaction_precharge_repairs_and_clones_retain_until_actual_last_drop() {
    let b = ImageDescriptorBudget::new(1 << 16);
    let w = workspace();
    let p = policy();
    let held = b.try_reserve(b.limit_bytes()).unwrap();
    let key = RawGuestNamespacePolicyKey::new(w.dupe());
    let key_clone = key.dupe();
    assert_eq!(key.workspace(), key_clone.workspace());
    assert_eq!(
        b.used_bytes(),
        b.limit_bytes(),
        "raw key does not reserve service bytes"
    );
    let error = GuestNamespaceTransaction::new(w.dupe(), Some(p), b.dupe()).unwrap_err();
    assert_eq!(
        error,
        GuestNamespacePolicyError::Resource(ImageDescriptorBudgetError::ResourceLimit)
    );
    assert!(error.is_transient());
    assert_eq!(
        b.used_bytes(),
        b.limit_bytes(),
        "failed block leaves no partial credit"
    );
    drop(held);
    let tx = GuestNamespaceTransaction::new(w, Some(p), b.dupe()).unwrap();
    let credit = b.used_bytes();
    assert!(credit > 0);
    let retained = tx.dupe();
    drop(tx);
    assert_eq!(b.used_bytes(), credit);
    drop(retained);
    assert_eq!(b.used_bytes(), 0);
    assert!(RawGuestNamespacePolicyKey::validity(&None));
    assert!(RawGuestNamespacePolicyKey::validity(&Some(p)));
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct PolicyConsumer(RawGuestNamespacePolicyKey);
impl std::fmt::Display for PolicyConsumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("guest-policy-test-consumer")
    }
}
#[async_trait]
impl Key for PolicyConsumer {
    type Value = Option<GuestNamespacePolicy>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&self.0).await.unwrap()
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}

#[tokio::test]
async fn normal_raw_absence_a_b_a_restore_and_history_keep_policy_inline() {
    let b = ImageDescriptorBudget::new(1 << 16);
    let held = b.try_reserve(b.limit_bytes()).unwrap();
    let key = RawGuestNamespacePolicyKey::new(workspace());
    let consumer = PolicyConsumer(key.dupe());
    let p = policy();
    let changed = GuestNamespacePolicy {
        epoch_nanoseconds: 35,
        ..p
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let data = || {
        let mut data = UserComputationData::default();
        data.data.set(b.dupe());
        data
    };
    let mut tx = dice.updater_with_data(data()).commit().await;
    assert_eq!(tx.compute(&consumer).await.unwrap(), None);
    drop(tx);
    let leaf_one = p
        .with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: 1 })
        .unwrap();
    let leaf_max = p
        .with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: 64 })
        .unwrap();
    for expected in [
        Some(p),
        Some(changed),
        Some(p),
        Some(leaf_one),
        Some(leaf_max),
        Some(leaf_one),
        Some(p),
        None,
    ] {
        let mut updater = dice.updater_with_data(data());
        updater.changed_to(vec![(key.dupe(), expected)]).unwrap();
        let mut tx = updater.commit().await;
        assert_eq!(tx.compute(&consumer).await.unwrap(), expected);
        assert!(RawGuestNamespacePolicyKey::validity(&expected));
        assert_eq!(b.used_bytes(), b.limit_bytes());
        drop(tx);
    }
    dice.wait_for_idle().await;
    drop(held);
    assert_eq!(
        b.used_bytes(),
        0,
        "raw policy values/history own no new heap credit"
    );
    drop(dice);
    assert_eq!(b.used_bytes(), 0);
}

#[test]
fn issued_label_data_opt_in_limits_and_current_identity() {
    let old = policy();
    assert_eq!(old.label_data(), GuestLabelDataPolicy::Disabled);
    for n in [0, 65, 255] {
        assert_eq!(
            old.with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: n }),
            Err(GuestNamespacePolicyError::InvalidPolicy)
        );
    }
    for n in [1, 64] {
        let enabled = old
            .with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: n })
            .unwrap();
        assert_ne!(enabled, old);
        assert_eq!(
            enabled
                .with_label_data(GuestLabelDataPolicy::Disabled)
                .unwrap(),
            old
        );
        let budget = ImageDescriptorBudget::new(4096);
        let current =
            GuestNamespaceTransaction::new(workspace(), Some(enabled), budget.dupe()).unwrap();
        assert!(current.authorizes(&enabled));
        assert!(!current.authorizes(&old));
    }
}
