//! Provider and pending-action lowering share one evaluator-end publication memo.
use slug_build_api_v2::AnalysisDepsetOccurrence;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderId;

use super::*;

fn depset(value: &AnalysisValue) -> &AnalysisDepset {
    let AnalysisValueKind::Depset(set) = value.kind() else {
        panic!("expected retained depset");
    };
    set
}

#[test]
fn sequential_provider_action_and_new_parent_lowering_preserves_depset_aliases() {
    let (provider, direct, parent) = {
        let heap = FrozenHeap::new();
        // As in the dense-lowering regressions, retained=None denotes an
        // evaluator-local graph, not a previously published external depset.
        let child = alloc_starlark_depset_parts(
            &heap,
            DepsetOrder::Default,
            Some("string".into()),
            AnalysisDepsetOccurrence::new(),
            None,
            2,
            vec![
                StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("child-a").to_frozen_value()),
                StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("child-b").to_frozen_value()),
            ],
        );
        let provider = alloc_starlark_user_provider(
            &heap,
            ProviderId::new("@@//:compile_context.bzl", "Info").unwrap(),
            true,
            [("headers".into(), child)],
        );
        let mut lowerer = AnalysisValueLowerer::default();
        let provider = lowerer.lower(provider.to_value(), "$provider").unwrap();
        let direct = lowerer.lower(child.to_value(), "$action.headers").unwrap();
        // A later root must refer to the already-published child as an external
        // graph node, not rebuild it in another dense store and replace the memo.
        let parent = alloc_starlark_depset_parts(
            &heap,
            DepsetOrder::Default,
            Some("string".into()),
            AnalysisDepsetOccurrence::new(),
            None,
            3,
            vec![
                StarlarkDepsetSuccessorGen::Transitive(child),
                StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("parent").to_frozen_value()),
            ],
        );
        let parent = lowerer.lower(parent.to_value(), "$later.parent").unwrap();
        (provider, direct, parent)
    }; // The original heap is gone; all values below are graph-owned.

    let AnalysisValueKind::Provider(fields) = provider.kind() else {
        panic!("expected retained provider");
    };
    let provider_child = depset(fields.fields().get("headers").unwrap());
    let direct_child = depset(&direct);
    let parent_child = depset(&parent)
        .successors()
        .find_map(|successor| match successor {
            AnalysisDepsetSuccessor::Transitive(child) => Some(child),
            AnalysisDepsetSuccessor::Direct(_) => None,
        })
        .unwrap();
    for child in [direct_child, &parent_child] {
        assert!(provider_child.shares_occurrence_with(child));
        assert!(provider_child.shares_successors_with(child));
        assert!(provider_child.shares_store_with(child));
        assert_eq!(
            child.to_list(),
            [
                AnalysisValue::string("child-a"),
                AnalysisValue::string("child-b")
            ]
        );
    }
    assert_eq!(
        depset(&parent).to_list(),
        [
            AnalysisValue::string("child-a"),
            AnalysisValue::string("child-b"),
            AnalysisValue::string("parent")
        ]
    );

    // Content-equivalent but independently retained depsets must still fail the
    // publication alias bijection when only the action's reference is relinked.
    let split_child = AnalysisValue::depset(
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![
                AnalysisValue::string("child-a"),
                AnalysisValue::string("child-b"),
            ],
            vec![],
        )
        .unwrap(),
    );
    assert!(direct.publication_eq(&split_child));
    let shared = AnalysisValue::tuple(vec![provider.clone(), direct, parent.clone()]);
    let split = AnalysisValue::tuple(vec![provider, split_child, parent]);
    assert!(shared.publication_eq(&shared.clone()));
    assert!(!shared.publication_eq(&split));
    assert!(!split.publication_eq(&shared));
}
