use super::*;
use crate::AnalysisValue;

#[derive(Debug, allocative::Allocative)]
struct Part;
impl StructuralFunctionOwnerPart for Part {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, state: &mut dyn std::hash::Hasher) {
        state.write_u8(1);
    }
    fn retained_bytes(&self) -> usize {
        1
    }
}
fn evaluation_owner() -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(FunctionOwnerDomain::BzlLoad, Arc::new(Part), Arc::new(Part))
        .unwrap()
}

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
pub(in crate::published_value) fn seal(builder: PublishedValueArenaBuilder, count: usize) -> PublishedValueArena {
    let imports = builder.imports().to_vec();
    builder
        .seal(Arc::new(Originals { count, imports }))
        .unwrap()
}
fn atom(builder: &mut PublishedValueArenaBuilder, value: AnalysisValue) -> PublishedNodeId {
    let id = builder.reserve().unwrap();
    let native = builder.native_atom(value).unwrap();
    builder.set_node(id, PublishedNode::Native(native)).unwrap();
    builder
        .set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
    id
}
pub(in crate::published_value) fn source(entries: usize) -> CallableSourceAuthority {
    use crate::callable_graph::*;
    let identity = CallableModuleIdentity {
        label: slug_identity_v2::CanonicalLabel::parse("@@//pkg:source.bzl").unwrap(),
        source: crate::ModuleSourceOrigin::Workspace("/workspace/pkg/source.bzl".into()),
        repository_mapping: (0..entries)
            .map(|i| {
                (
                    slug_identity_v2::ApparentRepoName::new(format!("alias{i}")).unwrap(),
                    slug_identity_v2::CanonicalRepoName::new(format!("selected{i}+")).unwrap(),
                )
            })
            .collect::<Vec<_>>()
            .into(),
    };
    CallableSourceAuthority::new(CallableSourceAuthorityData {
        root: identity.clone(),
        direct_children: Arc::from([]),
        environment_bindings: Arc::from([]),
        reachable: Arc::from([CallableModuleSource {
            identity,
            digest: [0; 32],
        }]),
        fingerprint: [0; 32],
    })
    .unwrap()
}
pub(in crate::published_value) fn code(
    builder: &mut PublishedValueArenaBuilder,
    source: CallableSourceAuthority,
) -> PublishedNodeId {
    let id = builder.reserve().unwrap();
    let dependencies = builder.append_edges(&[]).unwrap();
    builder
        .set_node(
            id,
            PublishedNode::Code(PublishedCode {
                source,
                name: "code".into(),
                definition: crate::CallableDefinitionSpan { start: 0, end: 1 },
                dependencies,
                globals: Arc::from([]),
            }),
        )
        .unwrap();
    id
}
fn count(
    rows: &[Option<PublishedNode>],
    edges: &[PublishedEdge],
) -> Result<usize, PublishedValueError> {
    let slots = vec![None; rows.len()];
    local_visits(
        rows,
        edges,
        &slots,
        &[],
        0,
        0,
        &mut 0,
        &mut 0,
        &mut 0,
        &mut false,
    )
    .map(|r| r.0.total)
}

#[test]
fn identical_retained_graphs_ignore_extra_producer_visits_and_import_lookups() {
    let mut prior = PublishedValueArenaBuilder::new();
    atom(&mut prior, AnalysisValue::integer(17));
    let prior = seal(prior, 1);
    let build = |extra| {
        let mut b = PublishedValueArenaBuilder::new();
        b.import(prior.clone()).unwrap();
        atom(&mut b, AnalysisValue::integer(4));
        if extra {
            for _ in 0..127 {
                b.charge_visit().unwrap();
            }
            b.import(prior.clone()).unwrap();
        }
        let work = b.charged_visits();
        (seal(b, 1), work)
    };
    let (a, work_a) = build(false);
    let (b, work_b) = build(true);
    assert!(work_b > work_a);
    assert_eq!(a.0.local_shape.visits, b.0.local_shape.visits);
    assert_eq!(a.shape().visits, b.shape().visits);
    assert!(
        a.value(PublishedNodeId::new(0).unwrap())
            .unwrap()
            .publication_eq(&b.value(PublishedNodeId::new(0).unwrap()).unwrap())
    );
    let mut limited = PublishedValueArenaBuilder::new();
    for _ in 0..NODE_LIMIT {
        limited.charge_visit().unwrap();
    }
    assert!(
        limited
            .seal(Arc::new(Originals {
                count: 0,
                imports: vec![]
            }))
            .is_err()
    );
}

#[test]
fn certificate_scratch_and_scan_refuse_before_growth_or_partial_publication() {
    // Near-cap declared byte credits exercise the same scratch preflight without
    // allocating a large source string or an actual large hash table.
    let mut visits = 0;
    let mut bytes = BYTE_LIMIT;
    let mut failed = false;
    assert!(
        local_visits(
            &[],
            &[],
            &[],
            &[],
            0,
            0,
            &mut visits,
            &mut bytes,
            &mut 0,
            &mut failed
        )
        .is_err()
    );
    assert!(failed);
    assert_eq!(bytes, BYTE_LIMIT);
    assert_eq!(visits, 1);
    let row = Some(PublishedNode::Cell { contents: None });
    let mut visits = NODE_LIMIT - 1;
    let mut bytes = 0;
    let mut failed = false;
    assert!(
        local_visits(
            &[row],
            &[],
            &[None],
            &[],
            0,
            0,
            &mut visits,
            &mut bytes,
            &mut 0,
            &mut failed
        )
        .is_err()
    );
    assert!(failed);
    assert_eq!(visits, NODE_LIMIT);
    assert!(
        local_visits(
            &[],
            &[],
            &[],
            &[],
            0,
            0,
            &mut visits,
            &mut bytes,
            &mut 0,
            &mut failed
        )
        .is_err()
    );
    assert!(count(&[None], &[]).is_err());
    let invalid = PublishedEdgeRange { start: 2, len: 1 };
    assert!(count(&[Some(PublishedNode::List(invalid))], &[]).is_err());
}

#[path = "metadata_tests.rs"]
mod metadata_tests;
#[path = "native_tests.rs"]
mod native_tests;
#[path = "source_tests.rs"]
mod source_tests;

#[path = "source_certificate_tests.rs"]
mod source_certificate_tests;

#[path = "name_count_tests.rs"]
mod name_count_tests;

#[path = "reference_count_tests.rs"]
mod reference_count_tests;
