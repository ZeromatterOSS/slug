use super::super::tests::part;
use super::*;
use crate::published_value::builder::feature_proof::basic_tests::features;
use crate::published_value::builder::import_accounting::tests::leaf;
use crate::published_value::builder::retained_facts::local_visits;
use crate::published_value::builder::retained_facts::tests::code;
use crate::published_value::builder::retained_facts::tests::seal;
use crate::published_value::builder::retained_facts::tests::source;

fn insert(b: &mut PublishedValueArenaBuilder, row: PublishedNode) {
    let id = b.reserve().unwrap();
    b.set_node(id, row).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
}
fn native_arena(values: Vec<AnalysisValue>) -> (PublishedValueArena, Vec<usize>) {
    let count = values.len();
    let mut b = PublishedValueArenaBuilder::new();
    let mut visits = Vec::new();
    for value in values {
        let atom = b.native_atom(value).unwrap();
        visits.push(atom.retained_visits());
        insert(&mut b, PublishedNode::Native(atom));
    }
    (seal(b, count), visits)
}
fn draft(arenas: &[PublishedValueArena]) -> PublishedValueArenaBuilder {
    let mut b = PublishedValueArenaBuilder::new_strict_shared_audit();
    for arena in arenas {
        b.import(arena.clone()).unwrap();
    }
    b
}
fn summary(b: &mut PublishedValueArenaBuilder) -> PublicationClosureCensus {
    let total = b.complete_shape().unwrap().visits;
    super::super::collect(b, total).unwrap()
}
fn channel_arena() -> (PublishedValueArena, usize, usize, usize) {
    let mut b = PublishedValueArenaBuilder::new();
    let native = b.native_atom(AnalysisValue::string("value")).unwrap();
    let first = native.retained_visits();
    insert(&mut b, PublishedNode::Native(native));
    let files = b
        .files_to_run_atom(crate::FilesToRunProvider::empty())
        .unwrap();
    let second = files.retained_visits();
    insert(
        &mut b,
        PublishedNode::Provider(PublishedProvider {
            identity: crate::ProviderIdentity::builtin("FilesToRunProvider"),
            has_schema: false,
            names: Arc::from([]),
            values: PublishedEdgeRange { start: 0, len: 0 },
            files_to_run: Some(files),
        }),
    );
    let provider = b
        .provider_value_atom(crate::ProviderValue::FilesToRunProvider(
            crate::FilesToRunProvider::empty(),
        ))
        .unwrap();
    let third = provider.retained_visits();
    insert(
        &mut b,
        PublishedNode::ConfiguredTarget {
            identity: crate::AnalysisTargetIdentity::null(
                slug_identity_v2::CanonicalLabel::parse("@@//pkg:target").unwrap(),
            ),
            providers: Arc::from([
                PublishedTargetProvider::Native {
                    provider,
                    view: PublishedEdge::Local(PublishedNodeId::new(1).unwrap()),
                },
                PublishedTargetProvider::Occurrence(PublishedEdge::Local(
                    PublishedNodeId::new(1).unwrap(),
                )),
            ]),
        },
    );
    code(&mut b, source(2));
    // Independent existing full canonical pass, never a fabricated certificate.
    let (facts, _) = local_visits(
        &b.rows, &b.edges, &b.slots, &b.imports, 0, 3, &mut 0, &mut 0, &mut 0, &mut false,
    )
    .unwrap();
    assert_eq!(facts.native, first + second + third);
    let arena = seal(b, 3);
    assert_eq!(arena.0.local_shape.visits, facts.total);
    (arena, facts.native, facts.source, facts.other())
}
#[path = "tests/bounds.rs"]
mod bounds;
#[path = "tests/channels.rs"]
mod channels;
#[path = "tests/lifetime.rs"]
mod lifetime;
#[path = "tests/models.rs"]
mod models;
