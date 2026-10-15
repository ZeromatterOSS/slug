use slug_build_api_v2::published_value::PublishedOriginalBinding;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
use crate::function_owner::tests::manifest;
use crate::function_owner::tests::owner;
use crate::function_owner::tests::owner_with_environment;
use crate::function_owner::tests::parent_manifest;

fn fields(s: PublicationShape) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
}
fn complete(input: &dyn ImmutablePublicationInput) {
    let mut expected = input.intrinsic_shape();
    for child in input.shared_inputs() {
        let c = child.retained_shape();
        expected.nodes += c.nodes;
        expected.edges += c.edges;
        expected.bytes += c.bytes;
        expected.visits += c.visits;
        expected.depth = expected.depth.max(c.depth + 1);
    }
    assert_eq!(fields(input.retained_shape()), fields(expected));
}
#[derive(Debug, Allocative)]
struct Raw {
    inputs: Vec<SharedPublicationInput>,
}
impl StructuralFunctionOwnerPart for Raw {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, h: &mut dyn Hasher) {
        h.write_u8(0);
    }
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 2 * std::mem::size_of::<usize>()
            + self.inputs.capacity() * std::mem::size_of::<SharedPublicationInput>()
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            bytes: self.retained_bytes(),
            edges: self.inputs.len(),
            visits: 1 + self.inputs.len(),
            depth: 1,
            nodes: 0,
        }
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.inputs
    }
}
fn raw(input: SharedPublicationInput) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        Arc::new(Raw {
            inputs: vec![input],
        }),
        Arc::new(Raw { inputs: vec![] }),
    )
    .unwrap()
}
#[derive(Debug)]
struct Originals(Vec<PublishedValueArena>);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.0
    }
}
fn seal(mut builder: PublishedValueArenaBuilder) -> PublishedValueArena {
    builder
        .charge_bytes(
            std::mem::size_of::<Originals>()
                + 2 * std::mem::size_of::<usize>()
                + 2 * builder.imports().len() * std::mem::size_of::<PublishedValueArena>(),
        )
        .unwrap();
    let imports = builder.imports().to_vec();
    builder.seal(Arc::new(Originals(imports))).unwrap()
}
#[test]
fn actual_key_diamonds_raw_extraction_preserve_positive_bodies_and_last_fields() {
    let cm = manifest();
    let child = owner(&cm, "def child(): return 1", false, []).unwrap();
    let pm = parent_manifest(1, &cm);
    let parent = owner(&pm, "parent", false, [child.clone(), child.clone()]).unwrap();
    let validity: &BzlInputValidity = parent.input_validity().as_any().downcast_ref().unwrap();
    let row = validity.rows[0].clone();
    let child_row = validity.rows[1].clone();
    assert!(Arc::ptr_eq(&row.children[0], &child_row.key));
    assert!(Arc::ptr_eq(&row.children[1], &child_row.key));
    assert_eq!(row.shared_inputs().len(), 3);
    complete(row.as_ref());
    complete(row.key.as_ref());
    complete(row.key.environment.as_ref());
    assert!(row.intrinsic_shape().visits < row.shape.visits);
    let weak = Arc::downgrade(&row);
    let extracted = SharedPublicationInput::new(row.clone());
    drop(parent);
    drop(child);
    drop(child_row);
    drop(row);
    let root = raw(extracted);
    assert!(weak.upgrade().is_some());
    let mut builder = PublishedValueArenaBuilder::new();
    builder.audit_owner_shape(&root).unwrap();
    let arena = seal(builder);
    let mut builder = PublishedValueArenaBuilder::new();
    builder.import(arena.clone()).unwrap();
    builder.audit_owner_shape(&root).unwrap();
    let retained = seal(builder);
    drop(root);
    drop(arena);
    assert!(weak.upgrade().is_some());
    drop(retained);
    assert!(weak.upgrade().is_none());

    let a = owner(&cm, "same", false, []).unwrap();
    let b = owner(&cm, "changed last source field", false, []).unwrap();
    assert!(a.stable_key().eq_part(b.stable_key()));
    assert_ne!(
        owner(&pm, "parent", false, [a]).unwrap(),
        owner(&pm, "parent", false, [b]).unwrap()
    );
}
#[tokio::test]
async fn genuine_foreign_catalog_and_reused_proof_are_complete_without_outer_siblings() {
    let (a, mixed) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair().await;
    let actual = owner_with_environment(&manifest(), "def f(): return 1", &mixed, []).unwrap();
    let validity: &BzlInputValidity = actual.input_validity().as_any().downcast_ref().unwrap();
    let row = validity.rows[0].clone();
    let env = mixed.inputs();
    let proof = env.source_proof().unwrap();
    assert_eq!(env.shared_inputs().len(), 2);
    assert!(proof.shared_inputs().len() >= 1);
    complete(row.as_ref());
    complete(row.key.as_ref());
    complete(env.as_ref());
    complete(proof.as_ref());
    let crate::bzl_environment::references::PayloadRef::Catalog(own) = env.own_catalog().unwrap()
    else {
        unreachable!()
    };
    assert!(proof.nested_payloads().iter().all(|p| match p {
        crate::bzl_environment::references::Payload::Catalog(c) => !Arc::ptr_eq(c, own),
        _ => false,
    }));
    let weak_env = Arc::downgrade(env);
    let weak_proof = Arc::downgrade(proof);
    let weak_catalog = Arc::downgrade(own);
    let inputs = vec![
        SharedPublicationInput::new(row.clone()),
        SharedPublicationInput::new(row.key.clone()),
        SharedPublicationInput::new(env.clone()),
        SharedPublicationInput::new(proof.clone()),
    ];
    drop(row);
    drop(actual);
    drop(a);
    drop(mixed);
    // Each root is independently raw-audited after the original owner/exports
    // are gone, so no validity sibling can close an omitted declaration.
    let roots: Vec<_> = inputs.into_iter().map(raw).collect();
    let mut builder = PublishedValueArenaBuilder::new();
    for root in &roots {
        builder.audit_owner_shape(root).unwrap();
    }
    let arena = seal(builder);
    for first in [true, false] {
        let mut builder = PublishedValueArenaBuilder::new();
        if first {
            builder.import(arena.clone()).unwrap();
        }
        for root in &roots {
            builder.audit_owner_shape(root).unwrap();
        }
        builder.import(arena.clone()).unwrap();
        seal(builder);
    }
    drop(roots);
    assert!(weak_env.upgrade().is_some());
    drop(arena);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak_env.strong_count() != 0
        || weak_proof.strong_count() != 0
        || weak_catalog.strong_count() != 0
    {
        assert!(
            std::time::Instant::now() < deadline,
            "raw publication closure leaked"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(weak_env.upgrade().is_none());
    assert!(weak_proof.upgrade().is_none());
    assert!(weak_catalog.upgrade().is_none());
}
#[test]
fn row_key_metadata_refuses_before_arrays_or_actual_guard_clones() {
    let actual = owner(&manifest(), "source", false, []).unwrap();
    let v: &BzlInputValidity = actual.input_validity().as_any().downcast_ref().unwrap();
    let env = &v.rows[0].key.environment;
    let before = Arc::strong_count(env);
    let mut full = PublicationShape {
        visits: 65536,
        ..zero_shape()
    };
    let mut reads = 0;
    assert!(
        prepare(
            zero_shape(),
            1,
            std::iter::once_with(|| {
                reads += 1;
                env.retained_shape()
            }),
            &mut full
        )
        .is_err()
    );
    assert_eq!(reads, 0);
    assert_eq!(Arc::strong_count(env), before);
    let mut full = PublicationShape {
        bytes: 64 * 1024 * 1024,
        ..zero_shape()
    };
    assert!(
        prepare(
            zero_shape(),
            1,
            std::iter::once_with(|| env.retained_shape()),
            &mut full
        )
        .is_err()
    );
    assert_eq!(Arc::strong_count(env), before);
}
