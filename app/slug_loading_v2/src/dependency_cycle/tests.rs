use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use dice::DynKey;
use dice::UserCycleDetectorGuard;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostBzlModuleObservationKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteKey;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteKey;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionKey;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/dependency-cycle-test").unwrap()
}

pub(super) fn load(repo: &str) -> DynKey {
    DynKey::from_key(HostCanonicalRepositoryLoadRouteKey::new(
        workspace(),
        CanonicalRepoName::new(repo).unwrap(),
    ))
}

#[tokio::test]
async fn selected_preparation_owner_and_observed_mapping_demand_keep_full_node_identity() {
    use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
    use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;

    use crate::SelectedPreparationOwnerKey;
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(1 << 20);
    let apparent = slug_identity_v2::ApparentRepoName::new("tools").unwrap();
    let owner = DynKey::from_key(
        SelectedPreparationOwnerKey::new(workspace(), &apparent, &budget).unwrap(),
    );
    let mapping = DynKey::from_key(HostRootRepositoryMappingObservationKey::new(workspace()));
    let demand = DynKey::from_key(HostSelectedExtensionDemandObservationKey::new(
        workspace(),
        CanonicalRepoName::new("canonical_tools").unwrap(),
    ));
    assert!(matches!(
        Node::from_dyn(&owner),
        Some(Node::SelectedPreparationOwner(_))
    ));
    assert!(matches!(
        Node::from_dyn(&mapping),
        Some(Node::ObservedRootMapping(_))
    ));
    assert!(matches!(
        Node::from_dyn(&demand),
        Some(Node::ObservedSelectedDemand(_))
    ));
    let other = DynKey::from_key(HostSelectedExtensionDemandObservationKey::new(
        workspace(),
        CanonicalRepoName::new("other_tools").unwrap(),
    ));
    assert_ne!(Node::from_dyn(&demand), Node::from_dyn(&other));
    let graph = Arc::new(DependencyCycleGraph::default());
    let a = graph.start(&owner).unwrap();
    a.add_edge(&mapping);
    let b = graph.start(&mapping).unwrap();
    b.add_edge(&demand);
    let c = graph.start(&demand).unwrap();
    c.add_edge(&owner);
    let (one, two, three) = tokio::join!(detected(&a), detected(&b), detected(&c));
    assert_eq!(one, two);
    assert_eq!(one, three);
    assert_eq!(one.nodes.len(), 4);
}

#[test]
fn bounded_formatter_counts_unicode_and_stops_before_formatting_tail() {
    struct WithTail(std::cell::Cell<bool>);
    impl std::fmt::Display for WithTail {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("é猫🙂x")?;
            self.0.set(true);
            f.write_str("unvisited tail")
        }
    }
    let value = WithTail(std::cell::Cell::new(false));
    let mut output = String::new();
    write_bounded(&mut output, &value, 3).unwrap();
    assert_eq!(output, "é猫🙂...");
    assert!(!value.0.get());
    output.clear();
    write_bounded(&mut output, &"é猫🙂", 3).unwrap();
    assert_eq!(output, "é猫🙂");
}

#[test]
fn cycle_display_preserves_short_identity_and_bounds_long_nodes_and_count() {
    const PREFIX: &str = "cycle detected in generated repository dependencies: ";
    let short = Arc::new(Node::from_dyn(&load("visible_identity")).unwrap());
    let cycle = DependencyCycle {
        nodes: Arc::from([short.clone()]),
    };
    assert_eq!(cycle.to_string(), format!("{PREFIX}{short}"));

    let long = Arc::new(
        Node::from_dyn(&DynKey::from_key(HostCanonicalRepositoryLoadRouteKey::new(
            NormalizedAbsolutePath::new(format!("/{}", "long_workspace".repeat(1024))).unwrap(),
            CanonicalRepoName::new("long_identity").unwrap(),
        )))
        .unwrap(),
    );
    let mut nodes = vec![long];
    nodes.extend(
        (1..13).map(|index| Arc::new(Node::from_dyn(&load(&format!("node{index}"))).unwrap())),
    );
    let cycle = DependencyCycle {
        nodes: nodes.into(),
    };
    let output = cycle.to_string();
    let rendered = output
        .strip_prefix(PREFIX)
        .unwrap()
        .split(" -> ")
        .collect::<Vec<_>>();
    assert_eq!(rendered.len(), 13); // Twelve nodes plus the omitted-node marker.
    assert_eq!(rendered[0].chars().count(), 259);
    assert!(rendered[0].ends_with("..."));
    assert_eq!(rendered[12], "...");
    assert!(
        rendered[..12]
            .iter()
            .all(|node| node.chars().count() <= 259)
    );
    assert!(!output.contains("node12"));
}

async fn detected(guard: &DependencyCycleGuard) -> DependencyCycle {
    tokio::time::timeout(Duration::from_secs(1), guard.guard_this(pending::<()>()))
        .await
        .expect("an immediate cycle notification must not wait for graph idleness")
        .unwrap_err()
}

#[tokio::test]
async fn two_and_three_node_bridge_cycles_notify_every_waiter() {
    for third in [false, true] {
        let graph = Arc::new(DependencyCycleGraph::default());
        let a = load("a");
        let b = DynKey::from_key(HostCanonicalRepositoryRouteKey::new(
            workspace(),
            CanonicalRepoName::new("b").unwrap(),
        ));
        let c = DynKey::from_key(HostGeneratedRepositoryDefinitionKey::new(
            workspace(),
            CanonicalRepoName::new("c").unwrap(),
        ));
        let first = graph.start(&a).unwrap();
        // Immediate edges precede a cold child start.
        first.add_edge(&b);
        let second = graph.start(&b).unwrap();
        let third_guard = third.then(|| {
            second.add_edge(&c);
            graph.start(&c).unwrap()
        });
        third_guard.as_ref().unwrap_or(&second).add_edge(&a);
        let (one, two, duplicate_waiter) =
            tokio::join!(detected(&first), detected(&second), detected(&first));
        assert_eq!(one, two);
        assert_eq!(one, duplicate_waiter);
        assert_eq!(one.nodes.len(), if third { 4 } else { 3 });
        if let Some(guard) = third_guard.as_ref() {
            assert_eq!(one, detected(guard).await);
        }
        drop((first, second, third_guard));
        assert!(graph.state.lock().unwrap().active.is_empty());
    }
}

#[test]
fn callback_replacement_then_old_guard_drop_preserves_new_attempt() {
    let graph = Arc::new(DependencyCycleGraph::default());
    let a = load("a");
    let b = load("b");
    let old = graph.start(&a).unwrap();
    let other = graph.start(&b).unwrap();
    old.add_edge(&b);
    other.add_edge(&a);
    graph.finish(&a);
    assert!(
        graph
            .state
            .lock()
            .unwrap()
            .active
            .get(&other.node)
            .unwrap()
            .edges
            .contains(&old.node)
    );
    let replacement = graph.start(&a).unwrap();
    replacement.add_edge(&b);
    let replacement_id = replacement.id;
    old.add_edge(&a); // Late work cannot attach an edge to the replacement.
    drop(old);
    let state = graph.state.lock().unwrap();
    assert_eq!(
        state.active.get(&replacement.node).unwrap().id,
        replacement_id
    );
    assert_eq!(state.active.get(&replacement.node).unwrap().edges.len(), 1);
    drop(state);
    drop((replacement, other));
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[test]
fn retirement_removes_outgoing_targets_and_continuous_overlap_is_bounded() {
    let graph = Arc::new(DependencyCycleGraph::default());
    let root = graph.start(&load("root")).unwrap();
    for index in 0..128 {
        let target = load(&format!("child{index}"));
        let child = graph.start(&target).unwrap();
        child.add_edge(&load("not_started"));
        let state = graph.state.lock().unwrap();
        assert_eq!(state.active.len(), 2);
        assert_eq!(
            state
                .active
                .values()
                .map(|attempt| attempt.edges.len())
                .sum::<usize>(),
            1
        );
        drop(state);
        graph.finish(&target);
        drop(child);
        let state = graph.state.lock().unwrap();
        assert_eq!(state.active.len(), 1);
        assert!(state.active.get(&root.node).unwrap().edges.is_empty());
    }
    drop(root);
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[tokio::test]
async fn pending_incoming_edge_survives_child_callback_and_closes_replacement_cycle() {
    let graph = Arc::new(DependencyCycleGraph::default());
    let parent_key = load("parent");
    let child_key = load("child");
    let parent = graph.start(&parent_key).unwrap();
    let old_child = graph.start(&child_key).unwrap();
    parent.add_edge(&child_key);
    old_child.add_edge(&load("retired_outgoing"));
    graph.finish(&child_key);
    assert_eq!(graph.state.lock().unwrap().active.len(), 1);
    assert!(parent.receiver.borrow().is_none());
    let replacement = graph.start(&child_key).unwrap();
    assert!(
        graph
            .state
            .lock()
            .unwrap()
            .active
            .get(&replacement.node)
            .unwrap()
            .edges
            .is_empty()
    );
    drop(old_child);
    replacement.add_edge(&parent_key);
    assert_eq!(detected(&parent).await, detected(&replacement).await);
    drop((parent, replacement));
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[test]
fn completed_inactive_child_and_retired_outgoing_edges_cannot_close_cycle() {
    let graph = Arc::new(DependencyCycleGraph::default());
    let parent_key = load("parent");
    let child_key = load("child");
    let parent = graph.start(&parent_key).unwrap();
    let child = graph.start(&child_key).unwrap();
    child.add_edge(&parent_key);
    graph.finish(&child_key);
    parent.add_edge(&child_key);
    assert!(parent.receiver.borrow().is_none());
    let replacement = graph.start(&child_key).unwrap();
    assert!(replacement.receiver.borrow().is_none());
    assert!(
        graph
            .state
            .lock()
            .unwrap()
            .active
            .get(&replacement.node)
            .unwrap()
            .edges
            .is_empty()
    );
    graph.finish(&parent_key);
    replacement.add_edge(&parent_key);
    assert!(replacement.receiver.borrow().is_none());
    drop((parent, child, replacement));
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[test]
fn structural_identity_includes_workspace_label_context_and_mode() {
    let label = HostRootBzlLabel::new(
        PackagePath::parse("pkg").unwrap(),
        RootPackageBzlTarget::parse("defs.bzl").unwrap(),
    );
    let variants = [
        DynKey::from_key(HostBzlModuleEvalKey::new(workspace(), label.clone())),
        DynKey::from_key(HostBzlModuleEvalKey::new_bzlmod(workspace(), label.clone())),
        DynKey::from_key(HostBzlModuleObservationKey::new_bzlmod(
            workspace(),
            label.clone(),
        )),
        DynKey::from_key(HostBzlModuleEvalKey::new_bzlmod(
            NormalizedAbsolutePath::new("/another-workspace").unwrap(),
            label,
        )),
        DynKey::from_key(HostBzlModuleEvalKey::new_bzlmod(
            workspace(),
            HostRootBzlLabel::new(
                PackagePath::parse("pkg").unwrap(),
                RootPackageBzlTarget::parse("other.bzl").unwrap(),
            ),
        )),
        load("same"),
        DynKey::from_key(HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace(),
            CanonicalRepoName::new("same").unwrap(),
        )),
    ];
    let graph = Arc::new(DependencyCycleGraph::default());
    let guards = variants
        .iter()
        .map(|key| graph.start(key).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(graph.state.lock().unwrap().active.len(), variants.len());
    assert_eq!(Node::from_dyn(&variants[1]), Node::from_dyn(&variants[1]));
    for (index, left) in variants.iter().enumerate() {
        for right in &variants[index + 1..] {
            assert_ne!(Node::from_dyn(left), Node::from_dyn(right));
        }
    }
    drop(guards);
    assert!(graph.state.lock().unwrap().active.is_empty());
}

#[tokio::test]
async fn concrete_bzl_guards_preserve_pure_projection_and_receive_mixed_cycles() {
    let detector = crate::bzl_load_cycle_detector();
    let first_key = DynKey::from_key(crate::keys::BzlModuleEvalKey {
        workspace: "/bzl-cycle-test".into(),
        path: "/bzl-cycle-test/a.bzl".into(),
    });
    let second_key = DynKey::from_key(crate::keys::BzlModuleEvalKey {
        workspace: "/bzl-cycle-test".into(),
        path: "/bzl-cycle-test/b.bzl".into(),
    });
    let first = detector
        .start_computing_key(&first_key)
        .unwrap()
        .as_any_arc()
        .downcast::<crate::cycle_detector::BzlLoadCycleGuard>()
        .unwrap_or_else(|_| panic!("old concrete guard type changed"));
    let second = detector.start_computing_key(&second_key).unwrap();
    first.add_edge(&second_key);
    second.add_edge(&first_key);
    let pure = tokio::time::timeout(Duration::from_secs(1), first.guard_this(pending::<()>()))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(pure.keys.len(), 2);
    assert!(first.dependency().receiver.borrow().is_none());
    detector.finished_computing_key(&first_key);
    detector.finished_computing_key(&second_key);
    drop((first, second));

    let first = detector
        .start_computing_key(&first_key)
        .unwrap()
        .as_any_arc()
        .downcast::<crate::cycle_detector::BzlLoadCycleGuard>()
        .unwrap_or_else(|_| panic!("old concrete guard type changed"));
    let bridge_key = load("generated");
    first.add_edge(&bridge_key);
    let bridge = detector
        .start_computing_key(&bridge_key)
        .unwrap()
        .as_any_arc()
        .downcast::<DependencyCycleGuard>()
        .unwrap_or_else(|_| panic!("bridge guard missing"));
    bridge.add_edge(&first_key);
    let mixed = detected(first.dependency()).await;
    assert_eq!(mixed, detected(&bridge).await);
    assert!(
        mixed
            .to_string()
            .contains("generated repository dependencies")
    );
    detector.finished_computing_key(&first_key);
    detector.finished_computing_key(&bridge_key);
}

#[test]
fn selected_label_source_node_keeps_full_origin_and_ordered_recipe_identity() {
    use slug_bzlmod_v2::HostRepositoryLabelPathSource;
    use slug_bzlmod_v2::SelectedLabelSourceKey;
    use slug_bzlmod_v2::SelectedLabelSourcePolicy;
    use slug_bzlmod_v2::SelectedLabelSourceRecipe;
    use slug_identity_v2::CanonicalLabel;
    use slug_workspace_v2::ImageChargedPath;
    use slug_workspace_v2::ImageDescriptorBudget;
    let budget = ImageDescriptorBudget::new(64 * 1024 * 1024);
    let make = |target| {
        let label = CanonicalLabel::parse(target).unwrap();
        SelectedLabelSourceRecipe::new(
            ImageChargedPath::pin_existing(workspace(), &budget).unwrap(),
            &label,
            HostRepositoryLabelPathSource::Root(workspace()),
            SelectedLabelSourcePolicy::plain(),
            &budget,
        )
        .unwrap()
    };
    let a = make("@@//:marker");
    let same_target = a
        .parent(&budget)
        .unwrap()
        .child("bin/tool", &budget)
        .unwrap();
    let alternate_order = same_target
        .parent(&budget)
        .unwrap()
        .child("tool", &budget)
        .unwrap();
    let node =
        |recipe| Node::from_dyn(&DynKey::from_key(SelectedLabelSourceKey::new(recipe))).unwrap();
    let a_node = node(same_target.clone());
    assert_eq!(a_node, node(same_target));
    assert_ne!(a_node, node(alternate_order));
    assert_ne!(node(a), node(make("@@//:different-marker")));
    assert!(a_node.to_string().starts_with("SelectedLabelSource("));
}
