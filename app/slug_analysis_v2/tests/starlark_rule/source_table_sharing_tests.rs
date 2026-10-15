//! Original configured functions retain one certified defining source table.
use slug_build_api_v2::published_value::FunctionEvaluationOwner;
use slug_build_api_v2::published_value::FunctionOwnerDomain;
use slug_build_api_v2::published_value::PublishedNode;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;

fn callback_owner(result: &ConfiguredNodeResult) -> FunctionEvaluationOwner {
    callback_owner_for(result, &ProviderId::new("//:defs.bzl", "Payload").unwrap())
}

fn callback_owner_for(
    result: &ConfiguredNodeResult,
    provider: &ProviderId,
) -> FunctionEvaluationOwner {
    let callback = result
        .providers()
        .user(provider)
        .unwrap()
        .field("callback")
        .unwrap();
    let PublishedNode::Function(function) = callback.original_origin().unwrap().node() else {
        panic!("original configured callback")
    };
    let owner = function.symbol.owner();
    assert_eq!(owner.domain(), FunctionOwnerDomain::ConfiguredRule);
    assert_eq!(owner.input_validity().shared_inputs().len(), 1);
    // The full mapping child edge is part of every repeated-path depth.
    let table = &owner.input_validity().shared_inputs()[0];
    assert!(owner.retained_shape().depth >= 1 + table.retained_shape().depth);
    owner.clone()
}

#[tokio::test]
async fn distinct_rule_definitions_and_targets_share_only_their_actual_source_table() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    let prepared = builtin_fixture::prepare(&workspace);
    let repositories = prepared
        .iter()
        .map(|(name, path)| (name.as_str(), path.as_str()))
        .collect::<Vec<_>>();
    fs::write(
        workspace.join("defs.bzl"),
        r#"
Payload = provider(fields = ['callback', 'value'])
def _first(ctx):
    def callback():
        return 'first'
    return [Payload(callback = callback)]
def _second(ctx):
    def callback():
        return 'second'
    return [Payload(callback = callback)]
def _consume(ctx):
    return [Payload(value = ctx.attr.left[Payload].callback() + ':' + ctx.attr.right[Payload].callback())]
first = rule(implementation = _first)
second = rule(implementation = _second)
consume = rule(implementation = _consume, attrs = {'left': attr.label(), 'right': attr.label()})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(':defs.bzl', 'first', 'second', 'consume')\nfirst(name='a')\nfirst(name='b')\nsecond(name='c')\nconsume(name='joined', left=':a', right=':c')\n",
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = test_configuration();
    let key = |name| {
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
            configuration.clone(),
        )
    };
    let mut owners = Vec::new();
    for name in ["a", "b", "c"] {
        let result = analyze_request_with_repositories(
            &dice,
            &workspace,
            &key(name),
            None,
            false,
            &repositories,
            false,
        )
        .await
        .unwrap();
        owners.push(callback_owner(&result));
    }
    assert_ne!(
        owners[0], owners[1],
        "different actual configured target keys"
    );
    assert_ne!(owners[0], owners[2], "different original rule definitions");
    let joined = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("joined"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        provider_value(&joined, &ProviderId::new("//:defs.bzl", "Payload").unwrap()),
        "first:second"
    );

    // A different real evaluator/DICE producer at the SAME source path builds
    // equal semantics and a separately allocated table, not a guard alias.
    let foreign_dice = Dice::builder().build(DetectCycles::Enabled);
    let foreign_result = analyze_request_with_repositories(
        &foreign_dice,
        &workspace,
        &key("a"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    let foreign_owner = callback_owner(&foreign_result);
    assert_eq!(
        owners[0], foreign_owner,
        "accounting guard is not semantic identity"
    );
    drop(foreign_result);
    drop(foreign_dice);
    drop(joined);
    drop(dice);

    // The actual original owner guards outlive both producers and results.
    // Independently visit declared LEAVES, then account the positive parent
    // intrinsic plus every declared child encounter. Expanded full remains
    // conservative; an actual alias pays only its two encounter operations.
    let table = &owners[0].input_validity().shared_inputs()[0];
    let full = table.retained_shape();
    assert!(full.visits > 0 && full.bytes > 0 && full.depth > 0);
    let mut raw = PublishedValueArenaBuilder::new();
    raw.audit_shared_input_at_depth(table, 1).unwrap();
    let unique_bodies = mapping_leaf_union(table);
    let body = table.intrinsic_shape().visits + unique_bodies;
    let first = body + 2 * (1 + table.shared_inputs().len());
    assert_eq!(raw.local_shape().visits, first);
    for (index, owner) in owners.iter().enumerate().skip(1) {
        raw.audit_shared_input_at_depth(&owner.input_validity().shared_inputs()[0], 1)
            .unwrap();
        assert_eq!(raw.local_shape().visits, first + 2 * index);
    }
    let old_full_bodies = owners.len() * full.visits;
    let new_full_bodies =
        raw.local_shape().visits - 2 * owners.len() - 2 * table.shared_inputs().len();
    assert_eq!(new_full_bodies, body);
    assert!(old_full_bodies > new_full_bodies);

    let foreign = &foreign_owner.input_validity().shared_inputs()[0];
    let foreign_full = foreign.retained_shape();
    assert_eq!(
        (
            foreign_full.nodes,
            foreign_full.edges,
            foreign_full.bytes,
            foreign_full.visits,
            foreign_full.depth
        ),
        (full.nodes, full.edges, full.bytes, full.visits, full.depth),
    );
    let before = raw.local_shape().visits;
    raw.audit_shared_input_at_depth(foreign, 1).unwrap();
    assert_eq!(raw.local_shape().visits - before, first);

    // Existing strict constructor/import logic consumes the real owners with
    // their declared table and original source/native/callable closure intact.
    let mut consumer = PublishedValueArenaBuilder::new();
    for owner in &owners {
        consumer.audit_owner_shape(owner).unwrap();
    }
    consumer.audit_owner_shape(&foreign_owner).unwrap();
}

// Discover actual guard aliases through the existing raw LEAF surface, without
// private pointer access or a second copy of the table recursion implementation.
// A leaf hit costs2; a fresh full leaf costs its independently cached intrinsic+2.
fn mapping_leaf_union(table: &slug_build_api_v2::published_value::SharedPublicationInput) -> usize {
    assert!(!table.shared_inputs().is_empty());
    let mut leaves = PublishedValueArenaBuilder::new();
    let mut bodies = 0;
    for leaf in table.shared_inputs() {
        assert!(leaf.shared_inputs().is_empty());
        let full = leaf.retained_shape();
        let intrinsic = leaf.intrinsic_shape();
        assert_eq!(
            (full.nodes, full.edges, full.bytes, full.visits, full.depth),
            (
                intrinsic.nodes,
                intrinsic.edges,
                intrinsic.bytes,
                intrinsic.visits,
                intrinsic.depth
            )
        );
        assert!(intrinsic.visits > 0 && intrinsic.bytes > 0);
        let before = leaves.charged_visits();
        leaves.audit_shared_input_at_depth(leaf, 2).unwrap();
        let delta = leaves.charged_visits() - before;
        assert!(delta == 2 || delta == intrinsic.visits + 2);
        bodies += delta - 2;
    }
    assert_eq!(
        leaves.charged_visits(),
        bodies + 2 * table.shared_inputs().len()
    );
    bodies
}

#[tokio::test]
async fn different_defining_tables_reuse_actual_child_mapping_guards_and_execute_original_callbacks()
 {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    let prepared = builtin_fixture::prepare(&workspace);
    let repositories = prepared
        .iter()
        .map(|(n, p)| (n.as_str(), p.as_str()))
        .collect::<Vec<_>>();
    fs::write(workspace.join("common.bzl"), r#"
Payload = provider(fields = ['callback', 'value'])
def message(value):
    print('source:' + value)
    return value
def _consume(ctx):
    return [Payload(value = ctx.attr.left[Payload].callback() + ':' + ctx.attr.right[Payload].callback())]
consume = rule(implementation = _consume, attrs = {'left': attr.label(), 'right': attr.label()})
"#).unwrap();
    for (file, tag) in [("left.bzl", "left"), ("right.bzl", "right")] {
        fs::write(workspace.join(file),format!("load(':common.bzl', 'Payload', 'message')\ndef _impl(ctx):\n    def callback():\n        return message('{tag}')\n    return [Payload(callback = callback)]\nmake = rule(implementation = _impl)\n")).unwrap();
    }
    fs::write(workspace.join("BUILD.bazel"), "load(':left.bzl', left='make')\nload(':right.bzl', right='make')\nload(':common.bzl', 'consume')\nleft(name='left')\nright(name='right')\nconsume(name='joined', left=':left', right=':right')\n").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let foreign_dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = test_configuration();
    let key = |name| {
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
            configuration.clone(),
        )
    };
    let provider = ProviderId::new("//:common.bzl", "Payload").unwrap();
    let mut owners = Vec::new();
    let mut arenas = Vec::new();
    for name in ["left", "right"] {
        let result = analyze_request_with_repositories(
            &dice,
            &workspace,
            &key(name),
            None,
            false,
            &repositories,
            false,
        )
        .await
        .unwrap();
        owners.push(callback_owner_for(&result, &provider));
        arenas.push(
            result
                .providers()
                .user(&provider)
                .unwrap()
                .field("callback")
                .unwrap()
                .original_origin()
                .unwrap()
                .arena()
                .clone(),
        );
    }
    assert_ne!(owners[0], owners[1]);
    let tracker = Arc::new(AnalysisEventTracker::default());
    let joined = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("joined"),
        Some(tracker.clone()),
        true,
        &repositories,
        false,
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&joined, &provider), "left:right");
    let activations = tracker.take();
    assert_eq!(
        event_texts(
            analysis_event(&activations, &workspace, &key("joined"))
                .batch
                .as_ref()
                .unwrap()
        ),
        vec!["source:left", "source:right"]
    );
    let foreign = analyze_request_with_repositories(
        &foreign_dice,
        &workspace,
        &key("left"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    let foreign_owner = callback_owner_for(&foreign, &provider);
    assert_eq!(
        owners[0], foreign_owner,
        "actual guard identity never enters semantic equality"
    );
    drop(foreign);
    drop(joined);
    drop(dice);
    drop(foreign_dice);
    drop(activations);
    drop(tracker);

    let a = &owners[0].input_validity().shared_inputs()[0];
    let b = &owners[1].input_validity().shared_inputs()[0];
    let mut leaf_oracle = PublishedValueArenaBuilder::new();
    for child in a.shared_inputs() {
        leaf_oracle.audit_shared_input_at_depth(child, 2).unwrap();
    }
    let mut fresh_b = 0;
    let mut reused = 0;
    for child in b.shared_inputs() {
        assert!(child.shared_inputs().is_empty());
        let before = leaf_oracle.charged_visits();
        leaf_oracle.audit_shared_input_at_depth(child, 2).unwrap();
        let delta = leaf_oracle.charged_visits() - before;
        assert!(delta == 2 || delta == child.intrinsic_shape().visits + 2);
        // Recheck against A alone: a duplicate earlier B occurrence must not
        // masquerade as genuine cross-table propagation.
        let mut a_only = PublishedValueArenaBuilder::new();
        for prior in a.shared_inputs() {
            a_only.audit_shared_input_at_depth(prior, 2).unwrap();
        }
        let prior = a_only.charged_visits();
        a_only.audit_shared_input_at_depth(child, 2).unwrap();
        let from_a = a_only.charged_visits() - prior;
        assert!(from_a == 2 || from_a == child.intrinsic_shape().visits + 2);
        if from_a == 2 {
            reused += 1;
        }
        fresh_b += delta - 2;
    }
    assert!(
        reused > 0,
        "actual child/environment mappings propagate across different defining tables"
    );
    let mut raw = PublishedValueArenaBuilder::new();
    raw.audit_shared_input_at_depth(a, 1).unwrap();
    assert_eq!(
        raw.charged_visits(),
        a.intrinsic_shape().visits + mapping_leaf_union(a) + 2 * (1 + a.shared_inputs().len())
    );
    let before = raw.charged_visits();
    raw.audit_shared_input_at_depth(b, 1).unwrap();
    assert_eq!(
        raw.charged_visits() - before,
        b.intrinsic_shape().visits + fresh_b + 2 * (1 + b.shared_inputs().len())
    );
    assert!(
        fresh_b < mapping_leaf_union(b),
        "cross-table reuse excludes a body that B alone really pays"
    );
    let foreign_table = &foreign_owner.input_validity().shared_inputs()[0];
    let before = raw.charged_visits();
    raw.audit_shared_input_at_depth(foreign_table, 1).unwrap();
    assert_eq!(
        raw.charged_visits() - before,
        foreign_table.intrinsic_shape().visits
            + mapping_leaf_union(foreign_table)
            + 2 * (1 + foreign_table.shared_inputs().len()),
        "foreign DICE guards pay their full unique body union"
    );
    let before = raw.charged_visits();
    raw.audit_shared_input_at_depth(b, 1).unwrap();
    assert_eq!(raw.charged_visits() - before, 2);
    // Actual callback arenas include original functions/sources, not just our
    // isolated descriptor admissions; retain/import them after producer disposal.
    let mut consumer = PublishedValueArenaBuilder::new();
    for owner in &owners {
        consumer.audit_owner_shape(owner).unwrap();
    }
    consumer.audit_owner_shape(&foreign_owner).unwrap();
    for arena in arenas {
        consumer.import(arena).unwrap();
    }
    assert_eq!(consumer.imports().len(), 2);
}
