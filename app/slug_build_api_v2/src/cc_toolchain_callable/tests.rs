use super::*;
use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::DepsetOrder;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::artifact_root::test_support as root_support;

const KINDS: [CcToolchainCallableKind; 4] = [
    CcToolchainCallableKind::SupportsPic,
    CcToolchainCallableKind::AlwaysPic,
    CcToolchainCallableKind::StaticRuntime,
    CcToolchainCallableKind::DynamicRuntime,
];

fn model(
    kind: CcToolchainCallableKind,
    capture: AnalysisValue,
    identity: Arc<()>,
) -> CcToolchainCallableModel {
    CcToolchainCallableModel {
        kind,
        source: CcToolchainCallableSource {
            label: CanonicalLabel::parse(
                "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl",
            )
            .unwrap(),
            digest: [19; 32],
        },
        identity,
        name_hash: 47,
        capture,
    }
}
fn value(model: CcToolchainCallableModel) -> AnalysisValue {
    AnalysisValue::cc_toolchain_callable(CcToolchainCallable::new(model).unwrap())
}
fn fresh(kind: CcToolchainCallableKind, capture: AnalysisValue) -> AnalysisValue {
    value(model(kind, capture, Arc::new(())))
}
fn capture(owner: &str, configuration: u8, order: DepsetOrder) -> AnalysisValue {
    AnalysisValue::depset(
        AnalysisDepset::new(
            order,
            vec![AnalysisValue::artifact(AnalysisArtifact::Derived {
                owner: AnalysisConfiguredTargetKey::new(
                    CanonicalLabel::parse(owner).unwrap(),
                    vec![configuration],
                ),
                output: ActionOutput::new(root_support::root(), "same.a", ActionOutputKind::File),
            })],
            vec![],
        )
        .unwrap(),
    )
}

#[test]
fn publication_checks_each_recipe_source_and_cached_name_hash() {
    for kind in KINDS {
        let identity = Arc::new(());
        let baseline = value(model(kind, AnalysisValue::none(), identity.clone()));
        assert!(baseline.publication_eq(&value(model(
            kind,
            AnalysisValue::none(),
            identity.clone()
        ))));
        for other in KINDS {
            if other != kind {
                assert!(!baseline.publication_eq(&value(model(
                    other,
                    AnalysisValue::none(),
                    identity.clone()
                ))));
            }
        }
        for component in 0..3 {
            let mut changed = model(kind, AnalysisValue::none(), identity.clone());
            match component {
                0 => {
                    changed.source.label = CanonicalLabel::parse(
                        "@@other+//cc/private/rules_impl:cc_toolchain_info.bzl",
                    )
                    .unwrap()
                }
                1 => changed.source.digest[31] ^= 1,
                2 => changed.name_hash += 1,
                _ => unreachable!(),
            }
            assert!(
                !baseline.publication_eq(&value(changed)),
                "{kind:?}, component {component}"
            );
        }
        assert!(baseline.is_starlark_hashable());
        assert_eq!(
            baseline.starlark_hash().unwrap(),
            baseline.clone().starlark_hash().unwrap()
        );
    }
}

#[test]
fn fresh_occurrences_alpha_match_but_module_global_requires_original_token() {
    for kind in KINDS {
        let a = fresh(kind, AnalysisValue::none());
        let b = fresh(kind, AnalysisValue::none());
        assert_ne!(a, b);
        assert_eq!(a, a.clone());
        assert_eq!(
            a.publication_eq(&b),
            kind != CcToolchainCallableKind::SupportsPic
        );
        assert!(a.publication_eq(&a.clone()));
        if kind == CcToolchainCallableKind::SupportsPic {
            continue;
        }
        let same_aliases = AnalysisValue::tuple(vec![a.clone(), a.clone()]);
        let rebuilt_aliases = AnalysisValue::tuple(vec![b.clone(), b.clone()]);
        let split_aliases = AnalysisValue::tuple(vec![b, fresh(kind, AnalysisValue::none())]);
        assert!(same_aliases.publication_eq(&rebuilt_aliases));
        assert!(!same_aliases.publication_eq(&split_aliases));
        assert!(!split_aliases.publication_eq(&same_aliases));
    }
}

#[test]
fn runtime_capture_discriminates_none_order_file_owner_and_configuration() {
    for kind in [
        CcToolchainCallableKind::StaticRuntime,
        CcToolchainCallableKind::DynamicRuntime,
    ] {
        let baseline = fresh(kind, capture("@@//:a", 1, DepsetOrder::Default));
        assert!(baseline.publication_eq(&fresh(kind, capture("@@//:a", 1, DepsetOrder::Default))));
        for changed in [
            AnalysisValue::none(),
            AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
            capture("@@//:b", 1, DepsetOrder::Default),
            capture("@@//:a", 2, DepsetOrder::Default),
            capture("@@//:a", 1, DepsetOrder::Preorder),
        ] {
            assert!(!baseline.publication_eq(&fresh(kind, changed)));
        }
    }
}

#[test]
fn runtime_capture_aliases_share_global_publication_state_with_provider_fields() {
    fn provider(shared: bool, reversed: bool) -> AnalysisValue {
        let captured = capture("@@//:a", 1, DepsetOrder::Default);
        let outside = if shared {
            captured.clone()
        } else {
            capture("@@//:a", 1, DepsetOrder::Default)
        };
        let callable = fresh(CcToolchainCallableKind::StaticRuntime, captured);
        // Different field ordering makes capture-vs-field traversal occur in
        // both directions; alias bijection cannot be local to the callable.
        let (first, second) = if reversed {
            (outside, callable)
        } else {
            (callable, outside)
        };
        AnalysisValue::provider(ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("first", first), ("second", second)],
        ))
    }
    for reversed in [false, true] {
        let shared = provider(true, reversed);
        let rebuilt = provider(true, reversed);
        let split = provider(false, reversed);
        assert!(shared.publication_eq(&rebuilt));
        assert!(!shared.publication_eq(&split));
        assert!(!split.publication_eq(&shared));
    }
}

#[test]
fn constructor_rejects_non_admitted_capture_shapes() {
    for kind in KINDS {
        for invalid in [
            AnalysisValue::boolean(false),
            AnalysisValue::string("not a depset"),
            AnalysisValue::list(vec![]),
            AnalysisValue::tuple(vec![]),
        ] {
            assert!(CcToolchainCallable::new(model(kind, invalid, Arc::new(()))).is_err());
        }
        let set = capture("@@//:a", 1, DepsetOrder::Default);
        assert_eq!(
            CcToolchainCallable::new(model(kind, set, Arc::new(()))).is_ok(),
            matches!(
                kind,
                CcToolchainCallableKind::StaticRuntime | CcToolchainCallableKind::DynamicRuntime
            )
        );
    }
}
