/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::size_of;
use std::sync::Arc;

use dupe::Dupe;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisTargetIdentity;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueError;
use slug_build_api_v2::AnalysisValueType;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetError;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_identity_v2::CanonicalLabel;

fn label(value: &str) -> CanonicalLabel {
    CanonicalLabel::parse(value).unwrap()
}

fn hash(value: &AnalysisValue) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn toolchain(fields: impl IntoIterator<Item = (&'static str, AnalysisValue)>) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("ToolchainInfo"),
        fields,
    ))
}

fn user(fields: impl IntoIterator<Item = (&'static str, AnalysisValue)>) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::user(ProviderId::new("//rules:defs.bzl", "Info").unwrap()),
        fields,
    ))
}

fn target(field: &str) -> AnalysisValue {
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::new("//rules:defs.bzl", "Info").unwrap()),
            [("value", AnalysisValue::string(field))],
        )),
    ])
    .unwrap();
    AnalysisValue::configured_target(ConfiguredTargetValue::new(
        AnalysisConfiguredTargetKey::new(label("@@//pkg:dep"), b"full-config".as_slice()),
        providers,
    ))
}

fn source_target(field: &str) -> AnalysisValue {
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::new("//rules:defs.bzl", "Info").unwrap()),
            [("value", AnalysisValue::string(field))],
        )),
    ])
    .unwrap();
    AnalysisValue::configured_target(ConfiguredTargetValue::new(
        AnalysisTargetIdentity::null(label("@@//pkg:dep")),
        providers,
    ))
}

#[test]
fn null_and_configured_target_identities_are_compact_and_collision_free() {
    assert!(size_of::<AnalysisTargetIdentity>() <= 2 * size_of::<usize>());
    assert!(size_of::<ConfiguredTargetValue>() <= 3 * size_of::<usize>());

    let null = AnalysisTargetIdentity::null(label("@@//pkg:dep"));
    let configured = AnalysisTargetIdentity::Configured(AnalysisConfiguredTargetKey::new(
        label("@@//pkg:dep"),
        Arc::<[u8]>::from([]),
    ));
    assert_eq!(null.label(), configured.label());
    assert!(null.configured().is_none());
    assert!(configured.configured().is_some());
    assert_ne!(null, configured);
    let cloned = null.dupe();
    let (AnalysisTargetIdentity::Null(left), AnalysisTargetIdentity::Null(right)) =
        (&null, &cloned)
    else {
        unreachable!("both identities are Null")
    };
    assert!(Arc::ptr_eq(left, right));

    let null_value = source_target("same");
    let configured_value = target("same");
    assert_ne!(null_value, configured_value);
    assert_ne!(hash(&null_value), hash(&configured_value));
    assert!(!null_value.publication_eq(&configured_value));
    assert_eq!(null_value, null_value.clone());
}

#[test]
fn all_admitted_value_kinds_are_heap_independent_and_distinct() {
    let target = target("one");
    let values = [
        AnalysisValue::none(),
        AnalysisValue::boolean(true),
        AnalysisValue::integer(7),
        AnalysisValue::float(1.5),
        AnalysisValue::string("x"),
        AnalysisValue::label(label("@@//pkg:x")),
        target.clone(),
        AnalysisValue::artifact(AnalysisArtifact::Source(label("@@//pkg:file"))),
        AnalysisValue::artifact(AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                label("@@//pkg:owner"),
                b"full-config".as_slice(),
            ),
            output: ActionOutput::new("pkg/out", ActionOutputKind::File),
        }),
        AnalysisValue::list(vec![AnalysisValue::integer(1)]),
        AnalysisValue::tuple(vec![AnalysisValue::integer(1)]),
        AnalysisValue::dictionary([(AnalysisValue::string("k"), AnalysisValue::integer(1))])
            .unwrap(),
        AnalysisValue::strukt([("field", AnalysisValue::integer(1))]),
        user([("field", AnalysisValue::integer(1))]),
        AnalysisValue::depset(
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::integer(1)],
                vec![],
            )
            .unwrap(),
        ),
    ];

    assert_eq!(values[9].value_type(), AnalysisValueType::List);
    assert_eq!(values[10].value_type(), AnalysisValueType::Tuple);
    assert_ne!(values[9], values[10]);
    assert!(values.iter().all(|value| value.clone() == *value));
    assert!(target.shares_storage_with(&values[6]));
}

#[test]
fn numeric_equality_and_hash_follow_starlark_cross_kind_rules() {
    for (left, right) in [
        (AnalysisValue::integer(1), AnalysisValue::float(1.0)),
        (AnalysisValue::float(0.0), AnalysisValue::float(-0.0)),
        (
            AnalysisValue::float(f64::from_bits(0x7ff8_0000_0000_0001)),
            AnalysisValue::float(f64::from_bits(0x7ff8_0000_0000_0002)),
        ),
    ] {
        assert_eq!(left, right);
        assert_eq!(hash(&left), hash(&right));
        assert_eq!(
            left.starlark_hash().unwrap(),
            right.starlark_hash().unwrap()
        );
    }
    assert_ne!(
        AnalysisValue::integer(9_007_199_254_740_993_i64),
        AnalysisValue::float(9_007_199_254_740_993_f64)
    );
}

#[test]
fn numeric_publication_equality_preserves_rematerialization_payload() {
    for (left, right) in [
        (AnalysisValue::integer(1), AnalysisValue::float(1.0)),
        (AnalysisValue::float(0.0), AnalysisValue::float(-0.0)),
        (
            AnalysisValue::float(f64::from_bits(0x7ff8_0000_0000_0001)),
            AnalysisValue::float(f64::from_bits(0x7ff8_0000_0000_0002)),
        ),
    ] {
        assert_eq!(left, right);
        assert!(!left.publication_eq(&right));
    }
    assert!(AnalysisValue::integer(1).publication_eq(&AnalysisValue::integer(1)));
    assert!(AnalysisValue::float(-0.0).publication_eq(&AnalysisValue::float(-0.0)));
}

#[test]
fn provider_pair_publication_equality_preserves_cross_occurrence_aliases() {
    let leaf = || {
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::string("leaf")],
            vec![],
        )
        .unwrap()
    };
    let shared = leaf();
    let left = [
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("value", AnalysisValue::depset(shared.clone()))],
        ),
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("value", AnalysisValue::depset(shared))],
        ),
    ];
    let right = [
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("value", AnalysisValue::depset(leaf()))],
        ),
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("value", AnalysisValue::depset(leaf()))],
        ),
    ];

    assert!(ProviderOccurrence::publication_eq_pairs([(
        &left[0], &right[0]
    )]));
    assert!(ProviderOccurrence::publication_eq_pairs([(
        &left[1], &right[1]
    )]));
    assert!(!ProviderOccurrence::publication_eq_pairs(
        left.iter().zip(&right)
    ));
}

#[test]
fn dictionary_visible_equality_ignores_order_but_publication_does_not() {
    let left = AnalysisValue::dictionary([
        (AnalysisValue::string("a"), AnalysisValue::integer(1)),
        (AnalysisValue::string("b"), AnalysisValue::integer(2)),
    ])
    .unwrap();
    let right = AnalysisValue::dictionary([
        (AnalysisValue::string("b"), AnalysisValue::integer(2)),
        (AnalysisValue::string("a"), AnalysisValue::integer(1)),
    ])
    .unwrap();

    assert_eq!(left, right);
    assert_eq!(hash(&left), hash(&right));
    assert!(!left.publication_eq(&right));
    assert!(!left.is_starlark_hashable());
}

#[test]
fn exact_key_and_semantic_immutability_barriers_are_distinct() {
    let direct_toolchain = toolchain([]);
    let frozen_list_barrier = AnalysisValue::list(vec![direct_toolchain.clone()]);
    let frozen_dict_barrier =
        AnalysisValue::dictionary([(AnalysisValue::string("toolchain"), direct_toolchain.clone())])
            .unwrap();
    let accepted = [
        AnalysisValue::none(),
        AnalysisValue::boolean(false),
        AnalysisValue::integer(1),
        AnalysisValue::float(1.25),
        AnalysisValue::string("x"),
        AnalysisValue::label(label("@@//pkg:x")),
        target("value"),
        AnalysisValue::artifact(AnalysisArtifact::Source(label("@@//pkg:file"))),
        AnalysisValue::tuple(vec![AnalysisValue::string("x")]),
        AnalysisValue::strukt([("items", frozen_list_barrier.clone())]),
        user([("items", frozen_list_barrier.clone())]),
        user([("items", frozen_dict_barrier.clone())]),
        AnalysisValue::depset(
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::string("x")],
                vec![],
            )
            .unwrap(),
        ),
    ];
    assert!(accepted.iter().all(AnalysisValue::is_starlark_hashable));
    assert!(accepted.iter().all(|value| value.starlark_hash().is_ok()));

    let rejected = [
        frozen_list_barrier,
        frozen_dict_barrier,
        direct_toolchain.clone(),
        AnalysisValue::tuple(vec![direct_toolchain.clone()]),
        AnalysisValue::strukt([("toolchain", direct_toolchain.clone())]),
        user([("toolchain", direct_toolchain)]),
    ];
    assert!(rejected.iter().all(|value| !value.is_starlark_hashable()));
    assert!(rejected.iter().all(|value| value.starlark_hash().is_err()));
}

#[test]
fn provider_identity_fields_and_target_publication_are_exact() {
    let ordered_left = user([
        ("z", AnalysisValue::integer(2)),
        ("a", AnalysisValue::integer(1)),
    ]);
    let ordered_right = user([
        ("a", AnalysisValue::integer(1)),
        ("z", AnalysisValue::integer(2)),
    ]);
    assert_eq!(ordered_left, ordered_right);
    assert_eq!(hash(&ordered_left), hash(&ordered_right));
    assert_ne!(
        ordered_left,
        toolchain([
            ("a", AnalysisValue::integer(1)),
            ("z", AnalysisValue::integer(2))
        ])
    );

    let first = target("one");
    let second = target("two");
    assert_eq!(first, second);
    assert!(!first.publication_eq(&second));
}

#[test]
fn depsets_preserve_occurrence_type_order_and_composition_rules() {
    let empty = AnalysisDepset::empty(DepsetOrder::Postorder);
    assert!(empty.shares_occurrence_with(&AnalysisDepset::empty(DepsetOrder::Postorder)));
    assert!(!empty.shares_occurrence_with(&AnalysisDepset::empty(DepsetOrder::Preorder)));

    let child = AnalysisDepset::new(
        DepsetOrder::Postorder,
        vec![AnalysisValue::string("child")],
        vec![],
    )
    .unwrap();
    let reused = AnalysisDepset::new(DepsetOrder::Postorder, vec![], vec![child.clone()]).unwrap();
    assert!(child.shares_occurrence_with(&reused));
    assert_eq!(reused.element_type(), AnalysisValueType::String);

    let compatible_different_order =
        AnalysisDepset::new(DepsetOrder::Default, vec![], vec![child.clone()]).unwrap();
    assert!(!child.shares_occurrence_with(&compatible_different_order));
    assert_eq!(compatible_different_order.to_list(), child.to_list());

    let canonical_child = AnalysisDepset::new(
        DepsetOrder::Postorder,
        vec![AnalysisValue::string("a"), AnalysisValue::string("b")],
        vec![],
    )
    .unwrap();
    let canonical_parent =
        AnalysisDepset::new(DepsetOrder::Default, vec![], vec![canonical_child.clone()]).unwrap();
    let canonical_direct = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("a"), AnalysisValue::string("b")],
        vec![],
    )
    .unwrap();
    assert!(canonical_parent.shares_successors_with(&canonical_child));
    assert!(
        AnalysisValue::depset(canonical_parent)
            .publication_eq(&AnalysisValue::depset(canonical_direct))
    );

    let distinct = AnalysisDepset::new(
        DepsetOrder::Postorder,
        vec![AnalysisValue::string("child")],
        vec![],
    )
    .unwrap();
    assert_ne!(
        AnalysisValue::depset(child.clone()),
        AnalysisValue::depset(distinct)
    );
    assert!(matches!(
        AnalysisDepset::new(
            DepsetOrder::Postorder,
            vec![AnalysisValue::integer(1)],
            vec![child.clone()]
        ),
        Err(AnalysisValueError::HeterogeneousDepset { .. })
    ));
    assert!(matches!(
        AnalysisDepset::new(
            DepsetOrder::Preorder,
            vec![AnalysisValue::string("x")],
            vec![child]
        ),
        Err(AnalysisValueError::Depset(
            DepsetError::IncompatibleOrder { .. }
        ))
    ));
}

#[test]
fn depset_leaf_matrix_observes_toolchain_and_frozen_container_barriers() {
    let toolchain = toolchain([]);
    let list_barrier = AnalysisValue::list(vec![toolchain.clone()]);
    let dict_barrier =
        AnalysisValue::dictionary([(AnalysisValue::string("k"), toolchain.clone())]).unwrap();
    for accepted in [
        AnalysisValue::tuple(vec![list_barrier.clone()]),
        AnalysisValue::strukt([("items", list_barrier.clone())]),
        user([("items", list_barrier)]),
        AnalysisValue::tuple(vec![dict_barrier.clone()]),
        AnalysisValue::strukt([("items", dict_barrier.clone())]),
        user([("items", dict_barrier)]),
    ] {
        AnalysisDepset::new(DepsetOrder::Default, vec![accepted], vec![]).unwrap();
    }
    for rejected in [
        toolchain.clone(),
        AnalysisValue::tuple(vec![toolchain.clone()]),
        AnalysisValue::strukt([("toolchain", toolchain.clone())]),
        user([("toolchain", toolchain)]),
        AnalysisValue::list(vec![AnalysisValue::integer(1)]),
        AnalysisValue::dictionary([(AnalysisValue::string("k"), AnalysisValue::integer(1))])
            .unwrap(),
    ] {
        assert!(matches!(
            AnalysisDepset::new(DepsetOrder::Default, vec![rejected], vec![]),
            Err(AnalysisValueError::InvalidDepsetLeaf { .. })
        ));
    }
}

#[test]
fn depset_publication_compares_alias_partition_and_deep_leaf_payload() {
    let shared = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![
            AnalysisValue::string("leaf-a"),
            AnalysisValue::string("leaf-b"),
        ],
        vec![],
    )
    .unwrap();
    let left = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("left")],
        vec![shared.clone()],
    )
    .unwrap();
    let right = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("right")],
        vec![shared],
    )
    .unwrap();
    let aliased = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("root")],
        vec![left, right],
    )
    .unwrap();
    let base = || {
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![
                AnalysisValue::string("leaf-a"),
                AnalysisValue::string("leaf-b"),
            ],
            vec![],
        )
        .unwrap()
    };
    let separated = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("root")],
        vec![
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::string("left")],
                vec![base()],
            )
            .unwrap(),
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::string("right")],
                vec![base()],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert!(!AnalysisValue::depset(aliased).publication_eq(&AnalysisValue::depset(separated)));

    let hoisted = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![],
        vec![
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::string("a")],
                vec![],
            )
            .unwrap(),
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::string("b")],
                vec![],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let direct = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("a"), AnalysisValue::string("b")],
        vec![],
    )
    .unwrap();
    assert!(AnalysisValue::depset(hoisted).publication_eq(&AnalysisValue::depset(direct)));

    let first = AnalysisDepset::new(DepsetOrder::Default, vec![target("one")], vec![]).unwrap();
    let second = AnalysisDepset::new(DepsetOrder::Default, vec![target("two")], vec![]).unwrap();
    assert!(!AnalysisValue::depset(first).publication_eq(&AnalysisValue::depset(second)));
}
