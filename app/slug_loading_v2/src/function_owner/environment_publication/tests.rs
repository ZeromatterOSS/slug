use slug_build_api_v2::published_value::PublishedOriginalBinding;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
use crate::function_owner::tests::manifest;
use crate::function_owner::tests::owner;
use crate::function_owner::tests::owner_with_environment;
use crate::function_owner::tests::parent_manifest;

fn validity(owner: &FunctionEvaluationOwner) -> &BzlInputValidity {
    owner.input_validity().as_any().downcast_ref().unwrap()
}
fn fields(s: PublicationShape) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
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
fn seal(mut b: PublishedValueArenaBuilder) -> PublishedValueArena {
    b.charge_bytes(
        std::mem::size_of::<Originals>()
            + 2 * std::mem::size_of::<usize>()
            + 2 * b.imports().len() * std::mem::size_of::<PublishedValueArena>(),
    )
    .unwrap();
    let imports = b.imports().to_vec();
    b.seal(Arc::new(Originals(imports))).unwrap()
}

#[test]
fn declaration_scratch_bound_and_precopy_refusals_are_positive() {
    use starlark_map::small_map::SmallMap;
    for count in [0usize, 1, 4, 5, 16, 128, 1024] {
        let mut set = SmallSet::<usize>::with_capacity(count);
        for i in 0..count {
            set.insert(i);
        }
        assert!(
            std::mem::size_of::<SmallSet<usize>>() <= std::mem::size_of::<SmallMap<usize, usize>>()
        );
        assert!(
            set.retained_storage_bytes_upper_bound().unwrap()
                + std::mem::size_of::<SmallSet<usize>>()
                <= descriptor_memo_scratch_bytes(count).unwrap()
        );
    }
    assert!(descriptor_memo_scratch_bytes(usize::MAX).is_err());
    let o = owner(&manifest(), "def f(): return 1", false, []).unwrap();
    let v = validity(&o);
    let env = &v.rows[0].key.environment;
    let before = Arc::strong_count(env);
    let mut full_work = PublicationShape {
        visits: 65536,
        ..zero_shape()
    };
    assert!(table_shape(&v.references, &mut full_work).is_err());
    let (_, count) = v.references.owner_table_budget().unwrap();
    let mut retained = zero_shape();
    let mut full_work = PublicationShape {
        bytes: 64 * 1024 * 1024,
        ..zero_shape()
    };
    assert!(admit(&v.rows, count, v.rows.len(), &mut retained, &mut full_work).is_err());
    assert_eq!(Arc::strong_count(env), before);
    // Exact and +1 joint scratch boundaries, without allocation or guard copies.
    let scratch = descriptor_memo_scratch_bytes(count).unwrap();
    let retained = PublicationShape {
        bytes: 64 * 1024 * 1024 - scratch,
        ..zero_shape()
    };
    let work = PublicationShape {
        bytes: scratch,
        ..zero_shape()
    };
    peak(retained, work).unwrap();
    assert!(
        peak(
            retained,
            PublicationShape {
                bytes: scratch + 1,
                ..work
            }
        )
        .is_err()
    );
    assert_eq!(Arc::strong_count(env), before);
}

#[tokio::test]
async fn original_environment_stream_declares_duplicate_and_foreign_catalog_guards() {
    let (a, mixed) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair().await;
    let m = manifest();
    let child = owner_with_environment(&m, "def f(): return 1", &mixed, []).unwrap();
    let pm = parent_manifest(1, &m);
    let parent = owner_with_environment(&pm, "parent", &a, [child.clone(), child.clone()]).unwrap();
    let v = validity(&parent);
    let original: Vec<_> = uses(&v.rows).collect();
    assert_eq!(original.len(), v.references.owner_table_budget().unwrap().1);
    assert!(Arc::ptr_eq(original[0], a.inputs()));
    assert!(Arc::ptr_eq(original[1], mixed.inputs()));
    assert!(Arc::ptr_eq(original[2], mixed.inputs()));
    assert!(Arc::ptr_eq(original[3], mixed.inputs()));
    let b_catalog = mixed.inputs().own_catalog().unwrap();
    assert_ne!(
        b_catalog.pointer(),
        a.inputs().own_catalog().unwrap().pointer()
    );
    assert_eq!(
        fields(mixed.inputs().intrinsic_shape()),
        fields(PublicationShape {
            // The declaration-array header is one fact, not another edge.
            edges: mixed.inputs().local_budget().records - 1,
            depth: 1,
            ..publication_shape(mixed.inputs().local_budget())
        })
    );
    let env_before = Arc::strong_count(mixed.inputs());
    let crate::bzl_environment::references::PayloadRef::Catalog(cb) = b_catalog else {
        unreachable!()
    };
    let catalog_before = Arc::strong_count(cb);
    let mut declared = Vec::new();
    append(&v.rows, &mut declared);
    assert_eq!(declared.len(), original.len() * 2);
    assert_eq!(Arc::strong_count(mixed.inputs()), env_before + 3);
    assert_eq!(Arc::strong_count(cb), catalog_before + 3);
    drop(declared);
    assert_eq!(Arc::strong_count(mixed.inputs()), env_before);
    assert_eq!(Arc::strong_count(cb), catalog_before);
    let old = v
        .references
        .physical_payloads()
        .iter()
        .map(|p| p.shared_count())
        .sum::<usize>()
        + v.rows.len();
    assert_eq!(v.shared_inputs.len(), v.rows.len());

    // The conservative body forecast still includes every legacy direct handle.
    let mut r = zero_shape();
    let mut w = zero_shape();
    assert_eq!(
        admit(&v.rows, original.len(), old, &mut r, &mut w).unwrap(),
        old + original.len() * 2
    );
    let ca = a.inputs().own_catalog().unwrap().physical_budget();
    let cb_budget = b_catalog.physical_budget();
    assert_eq!(
        r.visits,
        original.len() * 2 + ca.records + cb_budget.records
    );
    assert_eq!(
        r.bytes,
        original.len() * 2 * std::mem::size_of::<SharedPublicationInput>()
            + ca.bytes
            + cb_budget.bytes
    );
    // The full actual catalogs and SAME scratch ledger pass exactly, then
    // refuse +1 before any extra env/catalog guard clone. No credit is reused.
    let base = 64 * 1024 * 1024 - r.bytes - w.bytes;
    for (extra_byte, succeeds) in [(0, true), (1, false)] {
        let mut retained = PublicationShape {
            bytes: base + extra_byte,
            ..zero_shape()
        };
        let mut work = zero_shape();
        assert_eq!(
            admit(&v.rows, original.len(), old, &mut retained, &mut work).is_ok(),
            succeeds
        );
        assert_eq!(Arc::strong_count(mixed.inputs()), env_before);
        assert_eq!(Arc::strong_count(cb), catalog_before);
    }
}

#[tokio::test]
async fn ordinary_foreign_bodies_keep_strict_certified_import_and_last_drop() {
    let (a, mixed) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair().await;
    let m = manifest();
    let first = owner_with_environment(&m, "def f(): return 1", &a, []).unwrap();
    let same = owner_with_environment(&m, "def f(): return 1", &a, []).unwrap();
    let foreign = owner_with_environment(&m, "def f(): return 1", &mixed, []).unwrap();
    assert_eq!(first, same);
    assert_eq!(first, foreign);
    let hash = |o: &FunctionEvaluationOwner| {
        let mut h = DefaultHasher::new();
        o.hash_fields(&mut h);
        h.finish()
    };
    assert_eq!(hash(&first), hash(&foreign));
    let full_env = mixed.inputs().intrinsic_shape();
    let full_catalog = publication_shape(mixed.inputs().own_catalog().unwrap().physical_budget());
    for reverse in [false, true] {
        let run = |other: &FunctionEvaluationOwner| {
            let mut b = PublishedValueArenaBuilder::new();
            for o in if reverse {
                [other, &first]
            } else {
                [&first, other]
            } {
                b.audit_owner_shape(o).unwrap();
            }
            b.local_shape()
        };
        let shared = run(&same);
        let distinct = run(&foreign);
        assert_eq!(
            distinct.visits - shared.visits,
            full_env.visits + full_catalog.visits + 4 // Two newly followed child occurrences.
        );
        assert_eq!(
            distinct.edges - shared.edges,
            full_env.edges + full_catalog.edges + 2
        );
        assert!(distinct.bytes - shared.bytes >= full_env.bytes + full_catalog.bytes);
    }
    let weak_env = Arc::downgrade(mixed.inputs());
    let weak_proof = Arc::downgrade(mixed.inputs().source_proof().unwrap());
    let crate::bzl_environment::references::PayloadRef::Catalog(cb) =
        mixed.inputs().own_catalog().unwrap()
    else {
        unreachable!()
    };
    let weak_catalog = Arc::downgrade(cb);
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_owner_shape(&foreign).unwrap();
    let arena = seal(b);
    for import_first in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        if import_first {
            b.import(arena.clone()).unwrap();
        }
        b.audit_owner_shape(&foreign).unwrap();
        b.import(arena.clone()).unwrap();
        let retained = seal(b);
        assert!(retained.shape().visits >= arena.shape().visits);
    }
    let mut b = PublishedValueArenaBuilder::new();
    b.import(arena.clone()).unwrap();
    let retained = seal(b);
    drop(a);
    drop(mixed);
    drop(first);
    drop(same);
    drop(foreign);
    drop(arena);
    assert!(weak_env.upgrade().is_some());
    assert!(weak_catalog.upgrade().is_some());
    assert!(weak_proof.upgrade().is_some());
    drop(retained);
    // The DICE state processor releases cached keys on a detached OS thread.
    // Bound the wait for actual zero owners; an ownership cycle still fails.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak_env.strong_count() != 0
        || weak_catalog.strong_count() != 0
        || weak_proof.strong_count() != 0
    {
        assert!(
            std::time::Instant::now() < deadline,
            "environment/proof/catalog survived engine teardown: {}/{}/{}",
            weak_env.strong_count(),
            weak_proof.strong_count(),
            weak_catalog.strong_count()
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(weak_env.upgrade().is_none());
    assert!(weak_catalog.upgrade().is_none());
    assert!(weak_proof.upgrade().is_none());
}
