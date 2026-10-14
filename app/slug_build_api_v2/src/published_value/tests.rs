use super::*;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::CallableParameterMode;
use crate::NativeCallableApiId;
#[derive(Debug)]
struct Originals {
    count: usize,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.count
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn seal(b: PublishedValueArenaBuilder, count: usize) -> PublishedValueArena {
    let imports = b.imports().to_vec();
    b.seal(Arc::new(Originals { count, imports })).unwrap()
}
fn atom(b: &mut PublishedValueArenaBuilder, n: i64) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    let a = b.native_atom(AnalysisValue::integer(n)).unwrap();
    b.set_node(id, PublishedNode::Native(a)).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
    id
}
fn cyclic(reverse: bool, split: bool, default: i64) -> PublishedValueRef {
    let mut b = PublishedValueArenaBuilder::new();
    let first = b.reserve().unwrap();
    let second = b.reserve().unwrap();
    let third = b.reserve().unwrap();
    let (root, cell) = if reverse {
        (second, first)
    } else {
        (first, second)
    };
    let d = atom(&mut b, default);
    b.set_node(
        cell,
        PublishedNode::Cell {
            contents: Some(PublishedEdge::Local(root)),
        },
    )
    .unwrap();
    b.set_node(
        third,
        PublishedNode::Cell {
            contents: Some(PublishedEdge::Local(root)),
        },
    )
    .unwrap();
    b.set_node(
        root,
        PublishedNode::Intrinsic(PublishedIntrinsic {
            api: NativeCallableApiId::new("test.identity", 1).unwrap(),
            parameters: Arc::from([PublishedParameter {
                name: "x".into(),
                mode: CallableParameterMode::PositionalOrNamed,
                requirement: PublishedParameterRequirement::Defaulted(PublishedEdge::Local(d)),
                annotation: Some(PublishedEdge::Local(if split { third } else { cell })),
            }]),
            receiver: Some(PublishedEdge::Local(cell)),
            return_annotation: None,
        }),
    )
    .unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(root.index()).unwrap())
        .unwrap();
    seal(b, 4).value(root).unwrap()
}
#[test]
fn descriptor_cycles_aliases_defaults_and_numbering_share_one_arena() {
    let a = cyclic(false, false, 1);
    let b = cyclic(true, false, 1);
    assert!(a.publication_eq(&b));
    assert!(!a.publication_eq(&cyclic(true, true, 1)));
    assert!(!a.publication_eq(&cyclic(true, false, 2)));
    assert!(a.identity() == a.clone().identity());
    assert!(!(a.identity() == b.identity()));
}
#[test]
fn checked_builder_rejects_projection_cycles_incomplete_slots_and_budgets() {
    let mut b = PublishedValueArenaBuilder::new();
    let n = b.reserve().unwrap();
    let edges = b.append_edges(&[PublishedEdge::Local(n)]).unwrap();
    b.set_node(n, PublishedNode::List(edges)).unwrap();
    b.set_original_slot(n, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    assert!(matches!(
        b.seal(Arc::new(Originals {
            count: 1,
            imports: vec![]
        })),
        Err(PublishedValueError::Unsupported("cyclic native projection"))
    ));
    let mut b = PublishedValueArenaBuilder::new();
    b.reserve().unwrap();
    assert!(matches!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        })),
        Err(PublishedValueError::Invalid("incomplete row"))
    ));
    let mut b = PublishedValueArenaBuilder::new();
    assert!(b.charge_bytes(BYTE_LIMIT + 1).is_err());
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
    let mut b = PublishedValueArenaBuilder::new();
    assert!(b.observe_depth(DEPTH_LIMIT + 1).is_err());
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
    assert!(PublishedNodeId::new(NODE_LIMIT).is_err());
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_owner_edges(EDGE_LIMIT).unwrap();
    let n = b.reserve().unwrap();
    assert!(b.append_edges(&[PublishedEdge::Local(n)]).is_err());
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
}
#[test]
fn original_projection_preserves_origin_and_sealed_import_lifetimes() {
    let mut b = PublishedValueArenaBuilder::new();
    let n = atom(&mut b, 7);
    let a = seal(b, 1);
    let origin = a.value(n).unwrap();
    let projected =
        AnalysisValue::with_original_origin(AnalysisValue::integer(7), origin.clone()).unwrap();
    assert!(matches!(projected.kind(), AnalysisValueKind::Number(_)));
    assert!(
        AnalysisValue::with_original_origin(AnalysisValue::integer(8), origin.clone()).is_err()
    );
    let mut b = PublishedValueArenaBuilder::new();
    let imported = b.import(a.clone()).unwrap();
    assert_eq!(b.import(a.clone()).unwrap(), imported);
    let root = b.reserve().unwrap();
    let e = b
        .append_edges(&[PublishedEdge::Imported {
            arena: imported,
            node: n,
        }])
        .unwrap();
    b.set_node(root, PublishedNode::List(e)).unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let b = seal(b, 1);
    drop(a);
    drop(origin);
    let v = AnalysisValue::with_original_origin(
        AnalysisValue::list(vec![projected]),
        b.value(root).unwrap(),
    )
    .unwrap();
    assert!(matches!(v.kind(), AnalysisValueKind::List(_)));
    let mut rejected = PublishedValueArenaBuilder::new();
    assert!(rejected.native_atom(v.clone()).is_err());
    let mut owner = PublishedValueArenaBuilder::new();
    owner.audit_owner_native(&v).unwrap();
    assert_eq!(owner.imports().len(), 1);
    assert!(owner.charged_bytes() > owner.charged_local_bytes());
    let occurrence = crate::ProviderOccurrence::new(
        crate::ProviderIdentity::builtin("Example"),
        [("child", v.clone())],
    );
    let mut owner = PublishedValueArenaBuilder::new();
    owner.audit_owner_provider_occurrence(&occurrence).unwrap();
    assert_eq!(owner.imports().len(), 1);
    let inputs = crate::RetainedArgsInputs::Sequence(Arc::from([v]));
    assert_eq!(inputs.values().unwrap().len(), 1);
}
#[test]
fn raw_constructor_occurrences_and_runfiles_language_identity_are_distinct() {
    let empty = crate::RetainedRunfiles::empty();
    let occurrence = RunfilesOccurrence::new();
    let a = AnalysisValue::runfiles(empty.clone(), occurrence.clone());
    let b = AnalysisValue::runfiles(empty, RunfilesOccurrence::new());
    assert_eq!(a, b);
    assert!(a.is_starlark_immutable());
    assert!(a.is_starlark_hashable());
    assert_eq!(a.starlark_hash().unwrap(), b.starlark_hash().unwrap());
    assert_ne!(
        RawProviderConstructorOccurrence::new(),
        RawProviderConstructorOccurrence::new()
    );
    assert_eq!(occurrence, occurrence.clone());
    let raw =
        AnalysisValue::raw_provider_constructor_symbol(PublishedRawProviderConstructorSymbol {
            provider: crate::ProviderId::unqualified("P").unwrap(),
            occurrence: RawProviderConstructorOccurrence::new(),
        });
    assert!(!raw.is_starlark_immutable());
    assert!(!raw.is_starlark_hashable());
    assert!(raw.starlark_hash().is_err());
}

#[derive(Debug, allocative::Allocative)]
struct ShapePart {
    id: u8,
    shape: PublicationShape,
}
impl StructuralFunctionOwnerPart for ShapePart {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|v| v.id == self.id)
    }
    fn hash_part(&self, state: &mut dyn std::hash::Hasher) {
        state.write_u8(self.id)
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
fn counted_owner() -> FunctionEvaluationOwner {
    let part = |id| {
        Arc::new(ShapePart {
            id,
            shape: PublicationShape {
                nodes: 1,
                edges: 2,
                bytes: 8,
                visits: 3,
                depth: 7,
            },
        })
    };
    FunctionEvaluationOwner::new(FunctionOwnerDomain::ConfiguredRule, part(1), part(2)).unwrap()
}
#[test]
fn opaque_owner_shape_shares_graph_budget_and_deduplicates_actual_owner() {
    let owner = counted_owner();
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&owner).unwrap();
    let once = b.local_shape();
    let once_intrinsic = b.intrinsic_shape();
    b.audit_owner_shape(&owner.clone()).unwrap();
    let twice = b.local_shape();
    assert_eq!(once.nodes, 2);
    assert_eq!(once.depth, 7);
    assert_eq!(once.nodes - once_intrinsic.nodes, 2);
    assert_eq!(once.visits - once_intrinsic.visits, 6);
    // A hit still charges the actual incoming reference/lookup, but never
    // repeats the opaque payload or allocates a second owning root.
    assert_eq!(twice.nodes, once.nodes);
    assert_eq!(twice.bytes, once.bytes);
    assert_eq!(twice.visits, once.visits + 2);
    assert_eq!(twice.edges, once.edges + 1);
    b.audit_owner_shape(&counted_owner()).unwrap();
    assert_eq!(b.local_shape().nodes - b.intrinsic_shape().nodes, 4);
    assert_eq!(b.local_shape().visits - b.intrinsic_shape().visits, 12);
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_owner_edges(EDGE_LIMIT - 3).unwrap();
    assert!(b.audit_owner_shape(&owner).is_err());
    assert!(
        b.seal(Arc::new(Originals {
            count: 0,
            imports: vec![]
        }))
        .is_err()
    );
    // The immutable body still contributes its full six facts, while only
    // the actual owner lookup and owning-root operation consume producer work.
    for (remaining, succeeds) in [(2, true), (1, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        for _ in 0..NODE_LIMIT - remaining {
            b.charge_visit().unwrap();
        }
        assert_eq!(b.audit_owner_shape(&owner).is_ok(), succeeds);
        assert_eq!(b.charged_visits(), NODE_LIMIT);
        assert_eq!(b.local_shape().visits - b.intrinsic_shape().visits, 6);
        if !succeeds {
            assert!(b.charge_bytes(0).is_err());
        }
    }
}

fn dense_native_rows<T>(value: T, depth: usize) -> Vec<crate::depset::DenseDepsetNodeInput<T, ()>> {
    use crate::depset::DenseDepsetInput;
    use crate::depset::DenseDepsetNodeInput;
    use crate::depset::DenseDepsetRowSource;
    let mut rows = vec![DenseDepsetNodeInput {
        order: crate::DepsetOrder::Default,
        row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Direct(value)]),
        depth: 1,
        metadata: (),
    }];
    for i in 1..depth {
        rows.push(DenseDepsetNodeInput {
            order: crate::DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Local((i - 1) as u32)]),
            depth: i + 1,
            metadata: (),
        });
    }
    rows
}
fn deep_native_depset<T>(value: T) -> crate::Depset<T> {
    let root = crate::Depset::from_dense_nodes(dense_native_rows(value, crate::depset::MAX_DEPTH))
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(root.depth(), crate::depset::MAX_DEPTH);
    root
}
fn deep_analysis_depset(value: AnalysisValue) -> crate::AnalysisDepset {
    use crate::analysis_value::AnalysisDepsetGraphInput;
    use crate::analysis_value::AnalysisDepsetGraphNode;
    use crate::analysis_value::AnalysisDepsetGraphRow;
    let mut rows = vec![AnalysisDepsetGraphNode::new(
        crate::AnalysisDepsetOccurrence::new(),
        crate::DepsetOrder::Default,
        1,
        AnalysisDepsetGraphRow::Successors(vec![AnalysisDepsetGraphInput::Direct(value)]),
    )];
    for i in 1..crate::depset::MAX_DEPTH {
        rows.push(AnalysisDepsetGraphNode::new(
            crate::AnalysisDepsetOccurrence::new(),
            crate::DepsetOrder::Default,
            i + 1,
            AnalysisDepsetGraphRow::Successors(vec![AnalysisDepsetGraphInput::Local(i - 1)]),
        ));
    }
    crate::AnalysisDepset::from_local_graph(rows)
        .unwrap()
        .pop()
        .unwrap()
}
#[test]
fn every_runfiles_and_package_channel_checks_full_retained_depth() {
    let artifact = crate::AnalysisArtifact::Source(
        slug_identity_v2::CanonicalLabel::parse("@@//:input").unwrap(),
    );
    let link = crate::RunfilesSymlink::new("link", artifact);
    for root in [false, true] {
        let mut r = crate::RetainedRunfiles::empty();
        if root {
            r.root_symlinks = deep_native_depset(link.clone());
        } else {
            r.symlinks = deep_native_depset(link.clone());
        }
        PublishedValueArenaBuilder::new()
            .audit_native(&AnalysisValue::runfiles(r, RunfilesOccurrence::new()))
            .unwrap();
    }
    let mut r = crate::RetainedRunfiles::empty();
    r.empty_filenames = deep_native_depset("empty".to_owned());
    r.files = deep_analysis_depset(AnalysisValue::string("leaf"));
    PublishedValueArenaBuilder::new()
        .audit_native(&AnalysisValue::runfiles(r, RunfilesOccurrence::new()))
        .unwrap();
    assert!(
        crate::Depset::new(
            crate::DepsetOrder::Default,
            vec!["parent".to_owned()],
            vec![deep_native_depset("leaf".to_owned())]
        )
        .is_err()
    );

    // Private dense input can represent malformed cached hints. Both selected
    // and unreachable physical3501 rows must fail the native typed audit.
    for shallow_selected in [false, true] {
        let mut hidden = dense_native_rows("hidden".to_owned(), crate::depset::MAX_DEPTH + 1);
        if shallow_selected {
            hidden.push(crate::depset::DenseDepsetNodeInput {
                order: crate::DepsetOrder::Default,
                row: crate::depset::DenseDepsetRowSource::Successors(vec![]),
                depth: 0,
                metadata: (),
            });
        }
        let mut r = crate::RetainedRunfiles::empty();
        r.empty_filenames = crate::Depset::from_dense_nodes(hidden)
            .unwrap()
            .pop()
            .unwrap();
        assert!(matches!(
            PublishedValueArenaBuilder::new()
                .audit_native(&AnalysisValue::runfiles(r, RunfilesOccurrence::new())),
            Err(PublishedValueError::Limit("depset depth"))
        ));
    }

    // Ordinary outer + Direct payload depth still composes through a full
    // logical3500 DAG. The transitive chain consumes no ordinary levels.
    for (nested, succeeds) in [(DEPTH_LIMIT - 3, true), (DEPTH_LIMIT - 2, false)] {
        let mut payload = AnalysisValue::string("leaf");
        for _ in 0..nested {
            payload = AnalysisValue::tuple(vec![payload]);
        }
        let value = AnalysisValue::list(vec![AnalysisValue::depset(deep_analysis_depset(payload))]);
        let result = PublishedValueArenaBuilder::new().audit_owner_native(&value);
        if succeeds {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(PublishedValueError::Limit("depth"))));
        }
    }
    let public = Arc::new(crate::PackageGroupContents::default());
    let private = Arc::new(crate::PackageGroupContents::new(
        crate::PackageGroupContentsInput {
            has_private: true,
            ..Default::default()
        },
    ));
    let mut p = crate::PackageSpecificationInfo::new(public.clone(), []).unwrap();
    for i in 1..crate::depset::MAX_DEPTH {
        p = crate::PackageSpecificationInfo::new(
            if i % 2 == 0 {
                public.clone()
            } else {
                private.clone()
            },
            [p],
        )
        .unwrap();
    }
    assert_eq!(p.groups().depth(), crate::depset::MAX_DEPTH);
    PublishedValueArenaBuilder::new()
        .audit_owner_provider_value(&crate::ProviderValue::PackageSpecificationInfo(p.clone()))
        .unwrap();
    assert!(crate::PackageSpecificationInfo::new(public, [p]).is_err());
}

#[path = "struct_projection_tests.rs"]
mod struct_projection_tests;

#[path = "owner_dependency_tests.rs"]
mod owner_dependency_tests;

#[path = "set_capture_tests.rs"]
mod set_capture_tests;
