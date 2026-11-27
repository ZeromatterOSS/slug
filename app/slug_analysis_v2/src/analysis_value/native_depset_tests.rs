use std::hash::Hash;

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::RunfilesSymlink;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use starlark::values::Value;
use starlark::values::ValueLike;

use super::*;
use crate::analysis_value::AnalysisValueMaterializer;
use crate::analysis_value::StarlarkRunfilesSymlink;

fn diamond<T: Clone + Eq + Hash>(leaf: impl Fn(&str) -> T) -> (Depset<T>, Depset<T>) {
    let order = DepsetOrder::Preorder;
    // A singleton transitive child is normalized into a direct item.
    let shared = Depset::from_direct(order, vec![leaf("shared"), leaf("second")]).unwrap();
    let left = Depset::new(order, vec![leaf("left")], vec![shared.dupe()]).unwrap();
    let right = Depset::new(order, vec![leaf("right")], vec![shared.dupe()]).unwrap();
    let root = Depset::new(order, vec![leaf("root")], vec![left, right]).unwrap();
    (root, shared)
}

fn assert_diamond<'v>(
    root: Value<'v>,
    shared: Value<'v>,
    element_type: &str,
    expected_depth: usize,
    leaf_text: impl Fn(Value<'v>) -> &'v str,
) {
    use StarlarkDepsetSuccessorGen::Direct;
    use StarlarkDepsetSuccessorGen::Transitive;

    let (order, typ, _, retained, depth, row) = StarlarkDepset::parts_from_value(root).unwrap();
    assert_eq!(order, DepsetOrder::Preorder);
    assert_eq!(typ.as_deref(), Some(element_type));
    assert!(retained.is_none());
    assert_eq!(depth, expected_depth);
    let [Direct(root_leaf), Transitive(left), Transitive(right)] = row.as_slice() else {
        panic!("unexpected root successors");
    };
    assert_eq!(leaf_text(*root_leaf), "root");
    for (branch, expected) in [(*left, "left"), (*right, "right")] {
        let (_, _, _, _, _, row) = StarlarkDepset::parts_from_value(branch).unwrap();
        let [Direct(leaf), Transitive(child)] = row.as_slice() else {
            panic!("unexpected branch successors");
        };
        assert_eq!(leaf_text(*leaf), expected);
        assert!(child.ptr_eq(shared));
    }
    let (_, _, _, _, _, row) = StarlarkDepset::parts_from_value(shared).unwrap();
    let [Direct(first), Direct(second)] = row.as_slice() else {
        panic!("unexpected shared successors");
    };
    assert_eq!(leaf_text(*first), "shared");
    assert_eq!(leaf_text(*second), "second");
}

#[test]
fn native_depset_projection_preserves_ordered_diamonds_and_empty_owners() {
    let heap = FrozenHeap::new();
    let mut materializer = AnalysisValueMaterializer::new(&heap);
    let (files, shared_file) = diamond(str::to_owned);
    let file_root = materializer.file_depset(&files);
    let shared = materializer.file_depset(&shared_file);
    assert_diamond(
        file_root.to_value(),
        shared.to_value(),
        "string",
        files.depth(),
        |value| value.unpack_str().unwrap(),
    );
    let independent = Depset::from_direct(
        DepsetOrder::Preorder,
        vec!["shared".to_owned(), "second".to_owned()],
    )
    .unwrap();
    assert_eq!(independent, shared_file);
    let independent = materializer.file_depset(&independent);
    let (_, _, original, _, _, _) = StarlarkDepset::parts_from_value(shared.to_value()).unwrap();
    let (_, _, separate, _, _, _) =
        StarlarkDepset::parts_from_value(independent.to_value()).unwrap();
    assert!(!original.shares_with(&separate));

    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:fixture").unwrap());
    let (symlinks, shared_symlink) = diamond(|path| RunfilesSymlink::new(path, artifact.clone()));
    let shared = materializer.symlink_depset(&shared_symlink);
    let symlink_root = materializer.symlink_depset(&symlinks);
    assert_diamond(
        symlink_root.to_value(),
        shared.to_value(),
        "SymlinkEntry",
        symlinks.depth(),
        |value| {
            let entry = value.downcast_ref::<StarlarkRunfilesSymlink>().unwrap();
            assert_eq!(entry.retained.artifact, artifact);
            assert_eq!(
                entry.path.to_value().unpack_str(),
                Some(entry.retained.path.as_str())
            );
            let target_file =
                AnalysisArtifactValue::from_starlark(entry.target_file.to_value()).unwrap();
            assert_eq!(target_file.artifact(), &artifact);
            entry.retained.path.as_str()
        },
    );

    let files = Depset::from_direct(DepsetOrder::Preorder, Vec::new()).unwrap();
    let symlinks = Depset::from_direct(DepsetOrder::Preorder, Vec::new()).unwrap();
    for (root, expects_retained) in [
        (materializer.file_depset(&files), true),
        (materializer.symlink_depset(&symlinks), false),
    ] {
        let (order, typ, occurrence, retained, depth, row) =
            StarlarkDepset::parts_from_value(root.to_value()).unwrap();
        assert_eq!(order, DepsetOrder::Preorder);
        assert!(typ.is_none());
        assert_eq!(depth, 0);
        assert!(row.is_empty());
        assert_eq!(retained.is_some(), expects_retained);
        if let Some(retained) = retained {
            assert!(retained.occurrence().shares_with(&occurrence));
        }
    }
}

#[test]
fn native_file_depset_projection_walks_a_deep_chain_on_a_small_stack() {
    use StarlarkDepsetSuccessorGen::Direct;
    use StarlarkDepsetSuccessorGen::Transitive;

    const DEPTH: usize = 2048;
    let order = DepsetOrder::Preorder;
    let mut sources =
        vec![Depset::from_direct(order, vec!["0".to_owned(), "base".to_owned()]).unwrap()];
    for index in 1..DEPTH {
        sources.push(
            Depset::new(
                order,
                vec![index.to_string()],
                vec![sources.last().unwrap().dupe()],
            )
            .unwrap(),
        );
    }
    // Source owners stay live outside the worker. This checks projection stack
    // use independently from destruction of external retained stores.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn_scoped(scope, || {
                let heap = FrozenHeap::new();
                let mut materializer = AnalysisValueMaterializer::new(&heap);
                let mut current = materializer.file_depset(sources.last().unwrap()).to_value();
                for index in (0..DEPTH).rev() {
                    let (_, _, _, _, depth, row) =
                        StarlarkDepset::parts_from_value(current).unwrap();
                    assert_eq!(depth, sources[index].depth());
                    match row.as_slice() {
                        [Direct(leaf), Transitive(child)] if index > 0 => {
                            assert_eq!(leaf.unpack_str().unwrap().parse::<usize>().unwrap(), index);
                            current = *child;
                        }
                        [Direct(first), Direct(second)] if index == 0 => {
                            assert_eq!(first.unpack_str(), Some("0"));
                            assert_eq!(second.unpack_str(), Some("base"));
                        }
                        _ => panic!("unexpected chain successors"),
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    });
    while sources.pop().is_some() {}
}
