/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetError;
use slug_build_api_v2::DepsetOrder;

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn leaf(order: DepsetOrder, item: &str) -> Depset<String> {
    Depset::from_direct(order, s(&[item])).unwrap()
}

#[test]
fn depset_order_strings_match_bazel_surface() {
    assert_eq!(
        "default".parse::<DepsetOrder>().unwrap(),
        DepsetOrder::Default
    );
    assert_eq!(
        "postorder".parse::<DepsetOrder>().unwrap(),
        DepsetOrder::Postorder
    );
    assert_eq!(
        "preorder".parse::<DepsetOrder>().unwrap(),
        DepsetOrder::Preorder
    );
    assert_eq!(
        "topological".parse::<DepsetOrder>().unwrap(),
        DepsetOrder::Topological
    );
    assert_eq!(
        "stable".parse::<DepsetOrder>().unwrap_err().to_string(),
        "Invalid order: stable"
    );
}

#[test]
fn depset_to_list_orders_match_bazel_9_probe_cases() {
    let pre_c = Depset::new(
        DepsetOrder::Preorder,
        s(&["c"]),
        vec![
            leaf(DepsetOrder::Preorder, "a"),
            leaf(DepsetOrder::Preorder, "b"),
        ],
    )
    .unwrap();
    assert_eq!(pre_c.to_list(), s(&["c", "a", "b"]));

    let post_c = Depset::new(
        DepsetOrder::Postorder,
        s(&["c"]),
        vec![
            leaf(DepsetOrder::Postorder, "a"),
            leaf(DepsetOrder::Postorder, "b"),
        ],
    )
    .unwrap();
    assert_eq!(post_c.to_list(), s(&["a", "b", "c"]));

    let default_c = Depset::new(
        DepsetOrder::Default,
        s(&["c"]),
        vec![leaf(DepsetOrder::Preorder, "a")],
    )
    .unwrap();
    assert_eq!(default_c.to_list(), s(&["a", "c"]));

    let mixed_default = Depset::new(
        DepsetOrder::Default,
        s(&["c"]),
        vec![
            leaf(DepsetOrder::Preorder, "a"),
            leaf(DepsetOrder::Postorder, "b"),
        ],
    )
    .unwrap();
    assert_eq!(mixed_default.to_list(), s(&["a", "b", "c"]));
}

#[test]
fn topological_order_delays_shared_dependencies() {
    let top_a = leaf(DepsetOrder::Topological, "a");
    let top_b = Depset::new(DepsetOrder::Topological, s(&["b"]), vec![top_a.clone()]).unwrap();
    let top_c = Depset::new(DepsetOrder::Topological, s(&["c"]), vec![top_a.clone()]).unwrap();
    let top_d = Depset::new(DepsetOrder::Topological, s(&["d"]), vec![top_b, top_c]).unwrap();
    assert_eq!(top_d.to_list(), s(&["d", "b", "c", "a"]));
}

#[test]
fn composition_retains_shared_child_nodes_without_recursive_cloning() {
    let shared = leaf(DepsetOrder::Default, "shared");
    let left = Depset::new(DepsetOrder::Default, s(&["left"]), vec![shared.clone()]).unwrap();
    let right = Depset::new(DepsetOrder::Default, s(&["right"]), vec![shared.clone()]).unwrap();

    assert!(left.transitive()[0].shares_node_with(&shared));
    assert!(right.transitive()[0].shares_node_with(&shared));
    assert!(left.transitive()[0].shares_node_with(&right.transitive()[0]));
}

#[test]
fn depset_to_list_deduplicates_preserving_flatten_order() {
    let child = Depset::from_direct(DepsetOrder::Default, s(&["a", "b"])).unwrap();
    let parent = Depset::new(DepsetOrder::Default, s(&["a", "a"]), vec![child]).unwrap();
    assert_eq!(parent.to_list(), s(&["a", "b"]));

    let tuple_like = Depset::from_direct(
        DepsetOrder::Default,
        vec![
            ("x".to_owned(), 1),
            ("y".to_owned(), 2),
            ("x".to_owned(), 1),
        ],
    )
    .unwrap();
    assert_eq!(
        tuple_like.to_list(),
        vec![("x".to_owned(), 1), ("y".to_owned(), 2)]
    );
}

#[test]
fn incompatible_non_default_orders_are_rejected() {
    let err = Depset::new(
        DepsetOrder::Preorder,
        s(&["x"]),
        vec![leaf(DepsetOrder::Postorder, "y")],
    )
    .unwrap_err();

    assert_eq!(
        err,
        DepsetError::IncompatibleOrder {
            parent: DepsetOrder::Preorder,
            child: DepsetOrder::Postorder
        }
    );
    assert_eq!(
        err.to_string(),
        "Order 'preorder' is incompatible with order 'postorder'"
    );
}

#[test]
fn parents_without_direct_items_do_not_increase_depth() {
    let mut depset = Depset::from_direct(DepsetOrder::Default, s(&["0"])).unwrap();
    for index in 0..3499 {
        let item = index.to_string();
        depset = Depset::new(DepsetOrder::Default, s(&[&item]), vec![depset]).unwrap();
    }
    assert_eq!(depset.depth(), 3500);

    for _ in 0..3501 {
        depset = Depset::new(DepsetOrder::Default, Vec::new(), vec![depset]).unwrap();
    }
    assert_eq!(depset.depth(), 3500);

    let err = Depset::new(DepsetOrder::Default, s(&["overflow"]), vec![depset]).unwrap_err();
    assert_eq!(err.to_string(), "depset depth 3501 exceeds limit (3500)");
}
