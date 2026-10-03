use std::hash::Hash;
use std::hash::Hasher;
use std::time::Duration;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::InputImageAliasPolicy;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::SelectedToolRole;

fn workspace(b: &ImageDescriptorBudget) -> ImageChargedPath {
    ImageChargedPath::pin_existing(NormalizedAbsolutePath::new("/workspace").unwrap(), b).unwrap()
}
fn input<'a>(root: &'a str, guest: &'a str) -> RuntimeSourceDeclarationInput<'a> {
    RuntimeSourceDeclarationInput {
        source: RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor: root },
        guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix(guest),
    }
}
fn policy(
    w: &ImageChargedPath,
    b: &ImageDescriptorBudget,
    root: &str,
) -> RuntimeSourceDeclarationPolicy {
    RuntimeSourceDeclarationPolicy::new(
        w,
        &[input(root, "/runtime")],
        RuntimeSourceLimits::default(),
        b,
    )
    .unwrap()
}
fn hash(value: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

#[test]
fn ordered_policy_preserves_selector_mapping_limits_and_finite_fact_tier() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let rows = [
        input("/host/tools", "/runtime"),
        RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative: "" },
            guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix("/sources"),
        },
        RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: SelectedToolRole::Cargo,
                children: "lib/runtime",
            },
            guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix("/label"),
        },
    ];
    let p =
        RuntimeSourceDeclarationPolicy::new(&w, &rows, RuntimeSourceLimits::default(), &b).unwrap();
    assert_eq!(p.platform(), RuntimeSourcePlatform::LinuxX86_64);
    assert_eq!(p.fact_tier(), RuntimeSourceFactTier::TreeOnly);
    assert!(p.same_budget(&b));
    let declarations: Vec<_> = p.declarations().collect();
    assert!(matches!(
        declarations[0].guest_mapping(),
        RuntimeSourceGuestMappingRef::ExplicitPrefix(p) if p.as_path() == std::path::Path::new("/runtime")
    ));
    assert_eq!(
        declarations[0].image_policy().aliases(),
        InputImageAliasPolicy::Deny
    );
    assert!(
        matches!(declarations[0].selector(), RuntimeSourceSelectorRef::ExplicitHostDirectory { anchor } if anchor.as_path() == std::path::Path::new("/host/tools"))
    );
    assert!(
        matches!(declarations[1].selector(), RuntimeSourceSelectorRef::WorkspaceRelativeDirectory { components } if components.is_empty())
    );
    assert!(
        matches!(declarations[2].selector(), RuntimeSourceSelectorRef::SelectedToolOriginLabelParent { role: SelectedToolRole::Cargo, components } if components.iter().map(|c| c.as_str()).eq(["lib", "runtime"]))
    );
    let mut reversed = rows;
    reversed.reverse();
    let q = RuntimeSourceDeclarationPolicy::new(&w, &reversed, RuntimeSourceLimits::default(), &b)
        .unwrap();
    assert_ne!(
        p, q,
        "declaration order is semantic even with equal selected literals"
    );
    let q = RuntimeSourceDeclarationPolicy::new(
        &w,
        &rows,
        RuntimeSourceLimits {
            max_entries: 10,
            ..RuntimeSourceLimits::default()
        },
        &b,
    )
    .unwrap();
    assert_ne!(p, q);
    assert_eq!(
        p,
        RuntimeSourceDeclarationPolicy::new(&w, &rows, RuntimeSourceLimits::default(), &b).unwrap()
    );
    assert_eq!(hash(&p), hash(&p.dupe()));
}

#[test]
fn strict_literals_and_whole_borrowed_admission_precede_owned_allocation() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    // Hold all remaining credit: a bad second declaration must still report the
    // deterministic syntax refusal, not allocation pressure from the first.
    let held = b.try_reserve(b.limit_bytes() - b.used_bytes()).unwrap();
    let used = b.used_bytes();
    for bad in [
        "/", "relative", "/a/../b", "/a/./b", "/a//b", "/a/", "/a\\b", "/a\0b",
    ] {
        let result = RuntimeSourceDeclarationPolicy::new(
            &w,
            &[input("/valid", "/good"), input(bad, "/bad")],
            RuntimeSourceLimits::default(),
            &b,
        );
        assert_eq!(
            result.unwrap_err(),
            RuntimeSourceDeclarationError::InvalidDeclaration,
            "{bad:?}"
        );
        assert_eq!(b.used_bytes(), used);
    }
    for bad in [
        "/absolute",
        ".",
        "..",
        "a/../b",
        "a//b",
        "a/",
        "a\\b",
        "C:foo",
    ] {
        let result = RuntimeSourceDeclarationPolicy::new(
            &w,
            &[
                input("/valid", "/good"),
                RuntimeSourceDeclarationInput {
                    source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
                        relative: bad,
                    },
                    guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix("/bad"),
                },
            ],
            RuntimeSourceLimits::default(),
            &b,
        );
        assert_eq!(
            result.unwrap_err(),
            RuntimeSourceDeclarationError::InvalidDeclaration
        );
        assert_eq!(b.used_bytes(), used);
    }
    drop(held);
    let valid = policy(&w, &b, "/valid");
    drop(valid);
    drop(w);
    assert_eq!(b.used_bytes(), 0);
}

#[test]
fn component_overlap_rejects_workspace_host_and_reserved_guest_boundaries() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    for root in ["/workspace", "/workspace/inside"] {
        assert_eq!(
            RuntimeSourceDeclarationPolicy::new(
                &w,
                &[input(root, "/runtime")],
                RuntimeSourceLimits::default(),
                &b
            )
            .unwrap_err(),
            RuntimeSourceDeclarationError::Overlap
        );
    }
    let deeper =
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new("/host/workspace").unwrap(), &b)
            .unwrap();
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(
            &deeper,
            &[input("/host", "/runtime")],
            RuntimeSourceLimits::default(),
            &b
        )
        .unwrap_err(),
        RuntimeSourceDeclarationError::Overlap
    );
    for guest in ["/__slug_module", "/__slug_module/child"] {
        assert_eq!(
            RuntimeSourceDeclarationPolicy::new(
                &w,
                &[input("/outside", guest)],
                RuntimeSourceLimits::default(),
                &b
            )
            .unwrap_err(),
            RuntimeSourceDeclarationError::Overlap
        );
    }
    for rows in [
        [input("/a", "/guest"), input("/b", "/guest/child")],
        [input("/a", "/guest"), input("/a/child", "/other")],
        [input("/a", "/guest"), input("/a", "/other")],
    ] {
        assert_eq!(
            RuntimeSourceDeclarationPolicy::new(&w, &rows, RuntimeSourceLimits::default(), &b)
                .unwrap_err(),
            RuntimeSourceDeclarationError::Overlap
        );
    }
    // Textual prefixes are not path component containment.
    let p = RuntimeSourceDeclarationPolicy::new(
        &w,
        &[
            input("/workspace2", "/__slug_module2"),
            input("/workspace20", "/__slug_module20"),
        ],
        RuntimeSourceLimits::default(),
        &b,
    )
    .unwrap();
    assert_eq!(p.declarations().len(), 2);
}

#[test]
fn declaration_limits_have_exact_boundary_and_one_over_refusals() {
    let b = ImageDescriptorBudget::new(2 << 20);
    let w = workspace(&b);
    let roots: Vec<_> = (0..65).map(|i| format!("/root{i}")).collect();
    let guests: Vec<_> = (0..65).map(|i| format!("/guest{i}")).collect();
    let rows: Vec<_> = roots
        .iter()
        .zip(&guests)
        .map(|(r, g)| input(r, g))
        .collect();
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &rows[..64], RuntimeSourceLimits::default(), &b)
            .unwrap()
            .declarations()
            .len(),
        64
    );
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &rows, RuntimeSourceLimits::default(), &b)
            .unwrap_err(),
        RuntimeSourceDeclarationError::LimitExceeded(RuntimeSourceLimit::Declarations)
    );
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &[], RuntimeSourceLimits::default(), &b)
            .unwrap()
            .declarations()
            .len(),
        0
    );
    let exact = "/é";
    let limits = RuntimeSourceLimits {
        max_path_bytes: exact.len(),
        ..RuntimeSourceLimits::default()
    };
    assert!(RuntimeSourceDeclarationPolicy::new(&w, &[input(exact, "/g")], limits, &b).is_ok());
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &[input("/éa", "/g")], limits, &b).unwrap_err(),
        RuntimeSourceDeclarationError::LimitExceeded(RuntimeSourceLimit::PathBytes)
    );
    let limits = RuntimeSourceLimits {
        max_components: 1,
        ..RuntimeSourceLimits::default()
    };
    let make = |s| RuntimeSourceDeclarationInput {
        source: RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
            role: SelectedToolRole::Generator,
            children: s,
        },
        guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix("/g"),
    };
    assert!(RuntimeSourceDeclarationPolicy::new(&w, &[make("one")], limits, &b).is_ok());
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &[make("one/two")], limits, &b).unwrap_err(),
        RuntimeSourceDeclarationError::LimitExceeded(RuntimeSourceLimit::Components)
    );
    // One row logical accounting: literal source+guest, fixed64 record, no children.
    let exact = RuntimeSourceLimits {
        max_declaration_bytes: 64 + "/r".len() + "/g".len(),
        ..RuntimeSourceLimits::default()
    };
    assert!(RuntimeSourceDeclarationPolicy::new(&w, &[input("/r", "/g")], exact, &b).is_ok());
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(&w, &[input("/rr", "/g")], exact, &b).unwrap_err(),
        RuntimeSourceDeclarationError::LimitExceeded(RuntimeSourceLimit::DeclarationBytes)
    );
    assert!(
        RuntimeSourceDeclarationPolicy::new(
            &w,
            &[input("/r", "/g")],
            RuntimeSourceLimits {
                max_entries: 32_769,
                ..RuntimeSourceLimits::default()
            },
            &b
        )
        .is_err()
    );
}

#[test]
fn physical_precharge_repair_and_cloned_policy_last_drop_keep_paths_charged() {
    let b = ImageDescriptorBudget::new(64 << 10);
    let w = workspace(&b);
    let baseline = b.used_bytes();
    let held = b.try_reserve(b.limit_bytes() - baseline).unwrap();
    let used = b.used_bytes();
    let error = RuntimeSourceDeclarationPolicy::new(
        &w,
        &[input("/external", "/runtime")],
        RuntimeSourceLimits::default(),
        &b,
    )
    .unwrap_err();
    assert_eq!(
        error,
        RuntimeSourceDeclarationError::Resource(ImageDescriptorBudgetError::ResourceLimit)
    );
    assert!(error.is_transient());
    assert_eq!(b.used_bytes(), used);
    drop(held);
    let long = format!("/{}", "x".repeat(3000));
    let p = policy(&w, &b, &long);
    assert!(b.used_bytes() >= baseline + long.len());
    let keep = p.dupe();
    let used = b.used_bytes();
    drop(p);
    drop(w);
    assert_eq!(
        b.used_bytes(),
        used,
        "policy clone retains full charged origin and rows"
    );
    drop(keep);
    assert_eq!(b.used_bytes(), 0);
}

#[test]
fn current_transaction_checks_full_policy_and_actual_foreign_budget() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let a = policy(&w, &b, "/a");
    let changed = policy(&w, &b, "/b");
    let tx = RuntimeSourceDeclarationTransaction::new(w.dupe(), Some(a.dupe()), b.dupe()).unwrap();
    assert!(tx.authorizes(&a));
    assert!(!tx.authorizes(&changed));
    let absent = RuntimeSourceDeclarationTransaction::new(w.dupe(), None, b.dupe()).unwrap();
    assert!(!absent.authorizes(&a));
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let fw = workspace(&foreign);
    let fa = policy(&fw, &foreign, "/a");
    assert_eq!(
        a, fa,
        "nonsemantic credits must not alter full raw equality"
    );
    assert_eq!(hash(&a), hash(&fa));
    assert!(!tx.authorizes(&fa));
    assert!(!a.same_budget(&foreign));
    assert_eq!(
        RuntimeSourceDeclarationTransaction::new(w.dupe(), Some(fa), b.dupe()).unwrap_err(),
        RuntimeSourceDeclarationError::Resource(ImageDescriptorBudgetError::ForeignBudget)
    );
    let wrong_workspace =
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new("/other").unwrap(), &b).unwrap();
    assert_eq!(
        RuntimeSourceDeclarationTransaction::new(wrong_workspace, Some(a), b).unwrap_err(),
        RuntimeSourceDeclarationError::UnauthorizedCurrentPolicy
    );
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct PolicyConsumer(RuntimeSourceDeclarationPolicyKey);
impl std::fmt::Display for PolicyConsumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("runtime-policy-test-consumer")
    }
}
#[async_trait]
impl Key for PolicyConsumer {
    type Value = Option<RuntimeSourceDeclarationPolicy>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&self.0).await.unwrap()
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}
fn data(
    b: &ImageDescriptorBudget,
    p: Option<&RuntimeSourceDeclarationPolicy>,
) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(b.dupe());
    if let Some(p) = p {
        data.data.set(
            RuntimeSourceDeclarationTransaction::new(
                p.workspace().dupe(),
                Some(p.dupe()),
                b.dupe(),
            )
            .unwrap(),
        );
    }
    data
}

#[tokio::test]
async fn normal_raw_none_policy_a_b_a_and_revocation_invalidate_dependencies() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let a = policy(&w, &b, "/a");
    let changed = policy(&w, &b, "/b");
    let empty =
        RuntimeSourceDeclarationPolicy::new(&w, &[], RuntimeSourceLimits::default(), &b).unwrap();
    let used = b.used_bytes();
    let held = b.try_reserve(b.limit_bytes() - used).unwrap();
    let key = RuntimeSourceDeclarationPolicyKey::new(w.normalized_ref().dupe());
    let consumer = PolicyConsumer(key.dupe());
    assert_eq!(
        b.used_bytes(),
        b.limit_bytes(),
        "raw key clone adds no service credit even under pressure"
    );
    drop(held);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    assert!(
        tx.compute(&consumer).await.unwrap().is_none(),
        "uninstalled raw key supplies absence only"
    );
    drop(tx);
    for value in [
        Some(a.dupe()),
        Some(empty.dupe()),
        None,
        Some(empty),
        Some(changed.dupe()),
        Some(a.dupe()),
        None,
    ] {
        let mut updater = dice.updater_with_data(data(&b, value.as_ref()));
        updater
            .changed_to(vec![(key.dupe(), value.dupe())])
            .unwrap();
        let mut tx = updater.commit().await;
        let actual = tx.compute(&consumer).await.unwrap();
        assert_eq!(actual, value);
        assert!(RuntimeSourceDeclarationPolicyKey::validity(&actual));
        if let Some(actual) = actual {
            let current = tx
                .per_transaction_data()
                .data
                .get::<RuntimeSourceDeclarationTransaction>()
                .unwrap();
            assert!(current.authorizes(&actual));
        }
    }
    // Keep the raw input A fixed but change only current userdata: a cached normal
    // value cannot establish current command authorization by itself.
    let mut updater = dice.updater_with_data(data(&b, Some(&a)));
    updater.changed_to(vec![(key, Some(a.dupe()))]).unwrap();
    let mut tx = updater.commit().await;
    assert_eq!(tx.compute(&consumer).await.unwrap(), Some(a.dupe()));
    drop(tx);
    let mut tx = dice
        .updater_with_data(data(&b, Some(&changed)))
        .commit()
        .await;
    let warm = tx.compute(&consumer).await.unwrap().unwrap();
    assert_eq!(warm, a);
    assert!(
        !tx.per_transaction_data()
            .data
            .get::<RuntimeSourceDeclarationTransaction>()
            .unwrap()
            .authorizes(&warm)
    );
}

#[test]
fn empty_policy_is_present_current_and_has_no_declarations() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let make = |limits| RuntimeSourceDeclarationPolicy::new(&w, &[], limits, &b).unwrap();
    let p = make(RuntimeSourceLimits::default());
    assert_eq!(p.declarations().len(), 0);
    assert_ne!(Some(p.dupe()), None);
    let changed = make(RuntimeSourceLimits {
        max_entries: 1,
        ..RuntimeSourceLimits::default()
    });
    assert_ne!(p, changed, "empty configuration still owns every limit");
    let tx = RuntimeSourceDeclarationTransaction::new(w.dupe(), Some(p.dupe()), b.dupe()).unwrap();
    assert!(tx.authorizes(&p));
    assert!(!tx.authorizes(&changed));
    let absent = RuntimeSourceDeclarationTransaction::new(w.dupe(), None, b.dupe()).unwrap();
    assert!(!absent.authorizes(&p));
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let fp = RuntimeSourceDeclarationPolicy::new(
        &workspace(&foreign),
        &[],
        RuntimeSourceLimits::default(),
        &foreign,
    )
    .unwrap();
    assert_eq!(p, fp);
    assert_eq!(hash(&p), hash(&fp));
    assert!(
        !tx.authorizes(&fp),
        "equal empty rows do not waive actual service"
    );
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(
            &w,
            &[],
            RuntimeSourceLimits {
                max_declarations: 0,
                ..RuntimeSourceLimits::default()
            },
            &b
        )
        .unwrap_err(),
        RuntimeSourceDeclarationError::InvalidDeclaration,
    );
}

#[test]
fn original_mapping_and_equal_explicit_spelling_have_distinct_identity() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let input = RuntimeSourceDeclarationInput {
        source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
            anchor: "/host/tools",
        },
        guest_mapping: RuntimeSourceGuestMappingInput::OriginalSource,
    };
    let a = RuntimeSourceDeclarationPolicy::new(&w, &[input], RuntimeSourceLimits::default(), &b)
        .unwrap();
    let explicit = RuntimeSourceDeclarationInput {
        guest_mapping: RuntimeSourceGuestMappingInput::ExplicitPrefix("/host/tools"),
        ..input
    };
    let e =
        RuntimeSourceDeclarationPolicy::new(&w, &[explicit], RuntimeSourceLimits::default(), &b)
            .unwrap();
    assert_ne!(a, e);
    assert_ne!(hash(&a), hash(&e));
    assert!(matches!(
        a.declarations().next().unwrap().guest_mapping(),
        RuntimeSourceGuestMappingRef::OriginalSource
    ));
    let resolved = RuntimeSourceGuestPrefix::new("/host/tools", a.limits(), &b).unwrap();
    assert!(
        matches!(e.declarations().next().unwrap().guest_mapping(), RuntimeSourceGuestMappingRef::ExplicitPrefix(p) if p == &resolved)
    );
    assert!(resolved.same_budget(&b));
    assert!(!resolved.same_budget(&ImageDescriptorBudget::new(1 << 20)));
    for bad in ["/", "/__slug_module", "/__slug_module/lib", "/host/../lib"] {
        assert!(RuntimeSourceGuestPrefix::new(bad, a.limits(), &b).is_err());
    }
    let changed = RuntimeSourceDeclarationPolicy::new(
        &w,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                anchor: "/other/tools",
            },
            ..input
        }],
        a.limits(),
        &b,
    )
    .unwrap();
    assert_ne!(a, changed, "OriginalSource preserves its actual selector");
}

#[test]
fn original_mapping_whole_input_and_resolved_prefix_precharge_are_bounded() {
    let b = ImageDescriptorBudget::new(1 << 20);
    let w = workspace(&b);
    let original = RuntimeSourceDeclarationInput {
        source: RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor: "/host/a" },
        guest_mapping: RuntimeSourceGuestMappingInput::OriginalSource,
    };
    let used = b.used_bytes();
    assert!(
        RuntimeSourceDeclarationPolicy::new(
            &w,
            &[original, input("/host/b", "/bad/../guest")],
            RuntimeSourceLimits::default(),
            &b
        )
        .is_err()
    );
    assert_eq!(
        b.used_bytes(),
        used,
        "invalid later raw row allocates no earlier owned row"
    );
    assert_eq!(
        RuntimeSourceDeclarationPolicy::new(
            &w,
            &[
                original,
                RuntimeSourceDeclarationInput {
                    source: RuntimeSourceSelectorInput::ExplicitHostDirectory {
                        anchor: "/host/a/sub"
                    },
                    guest_mapping: RuntimeSourceGuestMappingInput::OriginalSource,
                }
            ],
            RuntimeSourceLimits::default(),
            &b
        )
        .unwrap_err(),
        RuntimeSourceDeclarationError::Overlap
    );
    let held = b.try_reserve(b.limit_bytes() - used).unwrap();
    assert_eq!(
        RuntimeSourceGuestPrefix::new("/resolved", RuntimeSourceLimits::default(), &b).unwrap_err(),
        RuntimeSourceDeclarationError::Resource(ImageDescriptorBudgetError::ResourceLimit)
    );
    assert_eq!(b.used_bytes(), b.limit_bytes());
    drop(held);
    let prefix =
        RuntimeSourceGuestPrefix::new("/resolved", RuntimeSourceLimits::default(), &b).unwrap();
    let held = prefix.dupe();
    let charged = b.used_bytes();
    drop(prefix);
    assert_eq!(b.used_bytes(), charged);
    drop(held);
    assert_eq!(b.used_bytes(), used);
}

#[tokio::test]
async fn empty_and_original_policy_history_retains_credit_until_engine_drop() {
    let b = ImageDescriptorBudget::new(128 << 10);
    let w = workspace(&b);
    let key = RuntimeSourceDeclarationPolicyKey::new(w.normalized_ref().dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let empty =
        RuntimeSourceDeclarationPolicy::new(&w, &[], RuntimeSourceLimits::default(), &b).unwrap();
    let original = RuntimeSourceDeclarationPolicy::new(
        &w,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative: "lib" },
            guest_mapping: RuntimeSourceGuestMappingInput::OriginalSource,
        }],
        RuntimeSourceLimits::default(),
        &b,
    )
    .unwrap();
    let mut old = None;
    for value in [Some(empty.dupe()), Some(original), Some(empty), None] {
        let mut updater = dice.updater_with_data(data(&b, value.as_ref()));
        updater
            .changed_to(vec![(key.dupe(), value.dupe())])
            .unwrap();
        let mut tx = updater.commit().await;
        assert_eq!(
            tx.compute(&PolicyConsumer(key.dupe())).await.unwrap(),
            value
        );
        if old.is_none() {
            old = Some(tx);
        }
    }
    drop(w);
    dice.wait_for_idle().await;
    assert!(b.used_bytes() > 0);
    assert!(
        old.as_mut()
            .unwrap()
            .compute(&PolicyConsumer(key.dupe()))
            .await
            .unwrap()
            .unwrap()
            .declarations()
            .len()
            == 0
    );
    drop(old);
    drop(dice);
    tokio::time::timeout(Duration::from_secs(2), async {
        while b.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("empty and original policy history must retire on actual engine drop");
    assert_eq!(b.used_bytes(), 0);
}

#[tokio::test]
async fn raw_policy_history_credit_retires_only_at_actual_engine_last_drop() {
    let b = ImageDescriptorBudget::new(128 << 10);
    let w = workspace(&b);
    let key = RuntimeSourceDeclarationPolicyKey::new(w.normalized_ref().dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut old_tx = None;
    for suffix in ["a", "b"] {
        let long = format!("/{}{}", "x".repeat(3000), suffix);
        let p = policy(&w, &b, &long);
        let mut updater = dice.updater_with_data(data(&b, Some(&p)));
        updater.changed_to(vec![(key.dupe(), Some(p))]).unwrap();
        let mut tx = updater.commit().await;
        assert!(
            tx.compute(&PolicyConsumer(key.dupe()))
                .await
                .unwrap()
                .is_some()
        );
        old_tx = Some(tx);
    }
    drop(old_tx);
    drop(w);
    dice.wait_for_idle().await;
    assert!(
        b.used_bytes() > 6000,
        "both long policy allocations remain charged through actual history"
    );
    // Service pressure cannot reuse old history credit merely because tx retired.
    let used = b.used_bytes();
    let held = b.try_reserve(b.limit_bytes() - used).unwrap();
    assert_eq!(
        b.try_reserve(1).unwrap_err(),
        ImageDescriptorBudgetError::ResourceLimit
    );
    drop(held);
    assert_eq!(b.used_bytes(), used);
    drop(dice);
    tokio::time::timeout(Duration::from_secs(2), async {
        while b.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("engine shutdown must actually retire historical policy allocations");
    assert_eq!(
        b.used_bytes(),
        0,
        "compact normalized key does not retain policy charge"
    );
    drop(key);
}
