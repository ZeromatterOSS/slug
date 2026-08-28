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
use slug_build_api_v2::DepsetSuccessor;

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn leaf(order: DepsetOrder, item: &str) -> Depset<String> {
    Depset::from_direct(order, s(&[item])).unwrap()
}

fn transitive_child(value: &Depset<String>) -> &Depset<String> {
    match &value.successors()[0] {
        DepsetSuccessor::Transitive(child) => child,
        DepsetSuccessor::Direct(_) => panic!("expected retained non-singleton child"),
    }
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
fn mixed_order_topological_matches_bazel_link_order() {
    let child = Depset::from_direct(DepsetOrder::Default, s(&["2", "4", "6"])).unwrap();
    let parent = Depset::new(DepsetOrder::Topological, s(&["3", "4", "5"]), vec![child]).unwrap();

    assert_eq!(parent.to_list(), s(&["3", "5", "6", "4", "2"]));
}

#[test]
fn sole_different_order_child_shares_canonical_successors() {
    let child = Depset::from_direct(DepsetOrder::Default, s(&["a", "b"])).unwrap();
    let parent = Depset::new(DepsetOrder::Topological, Vec::new(), vec![child.clone()]).unwrap();

    assert_eq!(parent.order(), DepsetOrder::Topological);
    assert_eq!(parent.depth(), child.depth());
    assert!(!parent.shares_node_with(&child));
    assert!(parent.shares_successors_with(&child));
    assert_eq!(parent.to_list(), s(&["b", "a"]));
}

#[test]
fn topological_reverses_before_deduplicating_transitive_nodes() {
    let a = leaf(DepsetOrder::Topological, "a");
    let a_star = leaf(DepsetOrder::Topological, "a");
    let b = leaf(DepsetOrder::Topological, "b");
    let parent = Depset::new(DepsetOrder::Topological, Vec::new(), vec![a, b, a_star]).unwrap();

    assert_eq!(parent.to_list(), s(&["b", "a"]));
}

#[test]
fn topological_deep_diamond_delays_one_shared_node() {
    let shared = Depset::from_direct(DepsetOrder::Topological, s(&["s0", "s1"])).unwrap();
    let left = Depset::new(DepsetOrder::Topological, s(&["left"]), vec![shared.clone()]).unwrap();
    let right = Depset::new(DepsetOrder::Topological, s(&["right"]), vec![shared]).unwrap();
    let root = Depset::new(DepsetOrder::Topological, s(&["root"]), vec![left, right]).unwrap();

    assert_eq!(root.to_list(), s(&["root", "left", "right", "s0", "s1"]));
}

#[test]
fn composition_retains_shared_child_nodes_without_recursive_cloning() {
    let shared = Depset::from_direct(DepsetOrder::Default, s(&["shared-a", "shared-b"])).unwrap();
    let left = Depset::new(DepsetOrder::Default, s(&["left"]), vec![shared.clone()]).unwrap();
    let right = Depset::new(DepsetOrder::Default, s(&["right"]), vec![shared.clone()]).unwrap();

    assert!(transitive_child(&left).shares_node_with(&shared));
    assert!(transitive_child(&right).shares_node_with(&shared));
    assert!(transitive_child(&left).shares_node_with(transitive_child(&right)));
}

#[test]
fn singleton_hoisting_preserves_interleaved_successor_order() {
    let nested = Depset::from_direct(DepsetOrder::Preorder, s(&["a", "b"])).unwrap();
    let singleton = leaf(DepsetOrder::Preorder, "x");
    let preorder = Depset::new(
        DepsetOrder::Preorder,
        s(&["d"]),
        vec![nested.clone(), singleton],
    )
    .unwrap();
    assert_eq!(preorder.to_list(), s(&["d", "a", "b", "x"]));
    assert!(matches!(
        preorder.successors(),
        [
            DepsetSuccessor::Direct(d),
            DepsetSuccessor::Transitive(child),
            DepsetSuccessor::Direct(x),
        ] if d == "d" && child.shares_node_with(&nested) && x == "x"
    ));

    let nested = Depset::from_direct(DepsetOrder::Default, s(&["a", "b"])).unwrap();
    let postorder = Depset::new(
        DepsetOrder::Default,
        s(&["d"]),
        vec![
            leaf(DepsetOrder::Default, "x"),
            nested.clone(),
            leaf(DepsetOrder::Default, "y"),
        ],
    )
    .unwrap();
    assert_eq!(postorder.to_list(), s(&["x", "a", "b", "y", "d"]));
    assert!(matches!(
        postorder.successors(),
        [
            DepsetSuccessor::Direct(x),
            DepsetSuccessor::Transitive(child),
            DepsetSuccessor::Direct(y),
            DepsetSuccessor::Direct(d),
        ] if x == "x" && child.shares_node_with(&nested) && y == "y" && d == "d"
    ));
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
        let item = (index + 1).to_string();
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

#[test]
fn depth_matches_bazel_builder_hoisting_and_reuse() {
    let empty = Depset::<String>::new(DepsetOrder::Default, Vec::new(), Vec::new()).unwrap();
    let a = leaf(DepsetOrder::Default, "a");
    let b = leaf(DepsetOrder::Default, "b");

    assert_eq!(empty.depth(), 0);
    assert_eq!(a.depth(), 1);
    assert_eq!(b.depth(), 1);

    let only_empty = Depset::new(
        DepsetOrder::Default,
        Vec::new(),
        vec![empty.clone(), empty.clone()],
    )
    .unwrap();
    assert_eq!(only_empty.depth(), 0);

    let empty_and_a =
        Depset::new(DepsetOrder::Default, Vec::new(), vec![empty, a.clone()]).unwrap();
    assert_eq!(empty_and_a.depth(), 1);
    assert!(empty_and_a.shares_node_with(&a));

    let repeated_a =
        Depset::new(DepsetOrder::Default, Vec::new(), vec![a.clone(), a.clone()]).unwrap();
    assert_eq!(repeated_a.depth(), 1);
    assert!(repeated_a.shares_node_with(&a));

    let ab = Depset::new(DepsetOrder::Default, Vec::new(), vec![a.clone(), b.clone()]).unwrap();
    assert_eq!(ab.depth(), 2);

    let matching_direct = Depset::new(DepsetOrder::Default, s(&["a"]), vec![a.clone()]).unwrap();
    assert!(matching_direct.shares_node_with(&a));

    let direct_only = Depset::from_direct(DepsetOrder::Default, s(&["a", "b", "c"])).unwrap();
    assert_eq!(direct_only.depth(), 2);
}
