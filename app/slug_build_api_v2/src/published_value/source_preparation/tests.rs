use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use super::*;
use crate::CallableModuleIdentity;
use crate::CallableModuleSource;
use crate::CallableSourceAuthorityData;

fn data() -> CallableSourceAuthorityData {
    let identity = CallableModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:source.bzl").unwrap(),
        workspace_path: "/workspace/pkg/source.bzl".into(),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("selected+").unwrap(),
        )]),
    };
    CallableSourceAuthorityData {
        root: identity.clone(),
        direct_children: Arc::from([identity.clone()]),
        reachable: Arc::from([CallableModuleSource {
            identity,
            digest: [3; 32],
        }]),
        fingerprint: [4; 32],
    }
}
fn peak(data: &CallableSourceAuthorityData) -> usize {
    // Independent reference to the reviewed existing complete native Q, plus
    // both Vec-to-Arc transfer arrays. Final elements/payload stay in Q.
    CallableSourceAuthority::prepare(data.clone())
        .unwrap()
        .byte_cost()
        + std::mem::size_of_val(data.direct_children.as_ref())
        + std::mem::size_of_val(data.reachable.as_ref())
}
fn transfer(data: &CallableSourceAuthorityData) -> usize {
    std::mem::size_of_val(data.direct_children.as_ref())
        + std::mem::size_of_val(data.reachable.as_ref())
}
fn prepared(b: &mut PublishedValueArenaBuilder) -> CallableSourceAuthority {
    let data = data();
    b.begin_source_authority(peak(&data) - transfer(&data), transfer(&data), 0)
        .unwrap()
        .finish(data)
        .unwrap()
}
fn bind(b: &mut PublishedValueArenaBuilder, source: CallableSourceAuthority) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    let dependencies = b.append_edges(&[]).unwrap();
    b.set_node(
        id,
        PublishedNode::Code(PublishedCode {
            source,
            name: "".into(),
            definition: crate::CallableDefinitionSpan { start: 0, end: 1 },
            dependencies,
            globals: Arc::from([]),
        }),
    )
    .unwrap();
    id
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
fn seal(b: PublishedValueArenaBuilder) -> PublishedValueArena {
    let imports = b.imports.clone();
    b.seal(Arc::new(Originals(imports))).unwrap()
}

#[test]
fn same_allocation_skips_only_q_while_equal_foreign_and_other_builders_pay_in_full() {
    let mut b = PublishedValueArenaBuilder::new();
    let authority = prepared(&mut b);
    let q = authority.byte_cost();
    let before = (b.bytes, b.visits);
    bind(&mut b, authority.clone());
    let first = (b.bytes - before.0, b.visits - before.1);
    assert_eq!(b.sources.len(), 1);
    assert_eq!(b.sources[0].fact_count(), authority.fact_count());
    assert!(b.sources[0].shares_storage_with(&authority));
    let before = (b.bytes, b.visits);
    bind(&mut b, authority.clone());
    let second = (b.bytes - before.0, b.visits - before.1);
    assert_eq!(first.0 - second.0, 4 * size_of::<CallableSourceAuthority>());
    assert_eq!(first.1 - second.1, authority.fact_count() + 2);
    for other in [
        authority.clone(),
        CallableSourceAuthority::new(authority.data().clone()).unwrap(),
    ] {
        let mut unprepared = PublishedValueArenaBuilder::new();
        let before = (unprepared.bytes, unprepared.visits);
        bind(&mut unprepared, other);
        assert_eq!(unprepared.bytes - before.0, first.0 + q);
        assert_eq!(unprepared.visits - before.1, first.1);
        assert_eq!(unprepared.sources[0], authority);
        assert!(unprepared.source_preparations.paid.is_empty());
    }
    let separate = CallableSourceAuthority::new(authority.data().clone()).unwrap();
    assert_eq!(separate, authority);
    assert!(!separate.shares_storage_with(&authority));
    let before = b.bytes;
    bind(&mut b, separate);
    assert_eq!(b.bytes - before, first.0 + q);
    assert_eq!(b.sources.len(), 2);
}

#[test]
fn retained_source_and_transfer_scratch_preserve_full_facts_seal_import_and_reseal() {
    for arrays in [false, true] {
        let mut data = data();
        if arrays {
            let q = peak(&data) - transfer(&data);
            let mapping = slug_identity_v2::RepositoryMapping::new(
                slug_identity_v2::RepositoryMappingId::new("tail-source-mapping-id").unwrap(),
            );
            let mut tail = data.reachable[0].clone();
            tail.identity.label = slug_identity_v2::ApparentLabel::parse("//pkg:source.bzl")
                .unwrap()
                .resolve(&mapping);
            data.reachable = Arc::from([tail]);
            assert_eq!(
                peak(&data) - transfer(&data) - q,
                mapping.id().as_str().len()
            );
        } else {
            data.direct_children = Arc::from([]);
            data.reachable = Arc::from([]);
        }
        let p = peak(&data);
        let a = transfer(&data);
        assert_eq!(
            a,
            if arrays {
                size_of::<CallableModuleIdentity>() + size_of::<CallableModuleSource>()
            } else {
                0
            }
        );
        let mut paid = PublishedValueArenaBuilder::new();
        let authority = paid
            .begin_source_authority(p - a, a, 19)
            .unwrap()
            .finish(data)
            .unwrap();
        let q = authority.byte_cost();
        assert_eq!(p, q + a);
        assert_eq!(paid.bytes - paid.producer_scratch_bytes, q);
        assert!(
            paid.bytes >= p + 19,
            "complete construction peak stays paid"
        );
        let weak = Arc::downgrade(&authority.data().direct_children);
        let expected = authority.data().clone();
        bind(&mut paid, authority.clone());
        let paid = seal(paid);
        let mut ordinary = PublishedValueArenaBuilder::new();
        bind(
            &mut ordinary,
            CallableSourceAuthority::new(expected.clone()).unwrap(),
        );
        let ordinary = seal(ordinary);
        assert_eq!(paid.0.local_shape.bytes, ordinary.0.local_shape.bytes);
        assert_eq!(paid.shape().bytes, ordinary.shape().bytes);
        assert_eq!(paid.0.local_shape.visits, ordinary.0.local_shape.visits);
        assert_eq!(paid.0.local_shape.visits, authority.fact_count() + 4);
        assert_eq!(paid.sources()[0].data(), &expected);
        assert!(
            paid.value(PublishedNodeId::new(0).unwrap())
                .unwrap()
                .publication_eq(&ordinary.value(PublishedNodeId::new(0).unwrap()).unwrap())
        );
        let mut imported = PublishedValueArenaBuilder::new();
        imported.import(paid.clone()).unwrap();
        assert_eq!(imported.import_bytes, paid.shape().bytes);
        let resealed = seal(imported);
        assert_eq!(resealed.imports()[0].sources()[0].data(), &expected);
        assert_eq!(
            resealed.shape().bytes - resealed.0.local_shape.bytes,
            paid.shape().bytes,
            "import replays retained Q without another A subtraction"
        );
        drop((authority, paid, ordinary, expected));
        assert!(weak.upgrade().is_some());
        drop(resealed);
        assert!(weak.upgrade().is_none());
    }
}
#[test]
fn proof_growth_refuses_before_guard_clones_or_credit_and_capacity_mutation() {
    for count in [0, 4] {
        let warm = || {
            let mut b = PublishedValueArenaBuilder::new();
            for _ in 0..count {
                drop(prepared(&mut b));
            }
            b
        };
        let d = data();
        let p = peak(&d);
        let a = transfer(&d);
        let mut probe = warm();
        let before = probe.bytes;
        drop(probe.begin_source_authority(p - a, a, 7).unwrap());
        let required = probe.bytes - before;
        let mut conservative = warm();
        let before = conservative.bytes;
        drop(conservative.begin_source_authority(p, 0, 7).unwrap());
        assert_eq!(
            required,
            conservative.bytes - before,
            "same complete P and proof peak"
        );
        assert_eq!(
            probe.producer_scratch_bytes - conservative.producer_scratch_bytes,
            a
        );
        for extra in [0, 1] {
            let mut b = warm();
            let before = (
                b.source_preparations.paid.len(),
                b.source_preparations.paid.capacity(),
                b.source_preparations.credit,
            );
            let refs = Arc::strong_count(&d.direct_children);
            b.charge_bytes(BYTE_LIMIT - b.bytes - required + extra)
                .unwrap();
            let result = b.begin_source_authority(p - a, a, 7);
            if extra == 0 {
                let authority = result.unwrap().finish(d.clone()).unwrap();
                assert_eq!(b.bytes, BYTE_LIMIT);
                assert_eq!(b.source_preparations.paid.len(), before.0 + 1);
                assert_eq!(authority.data(), &d);
            } else {
                assert!(matches!(
                    result.err().unwrap(),
                    PublishedValueError::ByteBudget(_)
                ));
                assert_eq!(
                    (
                        b.source_preparations.paid.len(),
                        b.source_preparations.paid.capacity(),
                        b.source_preparations.credit
                    ),
                    before
                );
                assert_eq!(Arc::strong_count(&d.direct_children), refs);
                assert!(b.failed_budget);
                assert!(b.begin_source_authority(p - a, a, 0).is_err());
            }
        }
    }
}
#[test]
fn underpayment_invalid_data_pending_work_and_abandonment_poison_without_success() {
    let d = data();
    let a = transfer(&d);
    let q = peak(&d) - a;
    let mut underpaid = PublishedValueArenaBuilder::new();
    assert!(matches!(
        underpaid
            .begin_source_authority(q - 1, a + 1, 0)
            .unwrap()
            .finish(data()),
        Err(PublishedValueError::Invalid(
            "source authority underpayment"
        ))
    ));
    assert_eq!(underpaid.bytes - underpaid.producer_scratch_bytes, q - 1);
    assert!(
        underpaid.bytes >= peak(&d),
        "adequate total P cannot excuse Q > R"
    );
    assert!(underpaid.source_preparations.paid.is_empty());
    assert!(underpaid.charge_bytes(0).is_err());

    let mut invalid = data();
    invalid.root.repository_mapping = vec![
        (
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("selected+").unwrap()
        );
        256
    ]
    .into();
    invalid.direct_children = vec![invalid.root.clone(); 255].into();
    invalid.reachable = Arc::from([]);
    let mapping = invalid.root.repository_mapping.clone();
    let mut b = PublishedValueArenaBuilder::new();
    assert!(matches!(
        b.begin_source_authority(BYTE_LIMIT / 2, 0, 0)
            .unwrap()
            .finish(invalid),
        Err(PublishedValueError::Limit("source facts"))
    ));
    assert_eq!(Arc::strong_count(&mapping), 1);
    assert!(b.source_preparations.paid.is_empty());
    assert!(b.charge_visit().is_err());

    for abort in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        let authority = prepared(&mut b);
        let preparation = b
            .begin_source_authority(
                peak(authority.data()) - transfer(authority.data()),
                transfer(authority.data()),
                0,
            )
            .unwrap();
        if abort {
            preparation.abort();
        } else {
            drop(preparation);
        }
        assert_eq!(b.source_preparations.paid.len(), 1);
        assert!(b.prepaid_source_bytes(&authority).is_err());
    }
    let mut b = PublishedValueArenaBuilder::new();
    for _ in 0..NODE_LIMIT - 2 {
        b.charge_visit().unwrap();
    }
    let mut preparation = b
        .begin_source_authority(peak(&data()) - transfer(&data()), transfer(&data()), 0)
        .unwrap();
    assert!(preparation.charge_visit().is_err());
    assert!(preparation.finish(data()).is_err());
    assert!(b.source_preparations.paid.is_empty());
    assert_eq!(b.source_preparations.credit, 0);
    assert!(b.charge_bytes(0).is_err());
}

#[test]
fn unused_paid_guard_drops_with_the_builder_and_forecasts_overflow_before_copy() {
    let d = data();
    let weak = Arc::downgrade(&d.direct_children);
    let mut b = PublishedValueArenaBuilder::new();
    let authority = b
        .begin_source_authority(peak(&d) - transfer(&d), transfer(&d), 0)
        .unwrap()
        .finish(d)
        .unwrap();
    drop(authority);
    assert!(weak.upgrade().is_some());
    let retained = b.charged_local_bytes() - b.producer_scratch_bytes;
    let unused = seal(b);
    assert!(weak.upgrade().is_none());
    assert!(
        unused.shape().bytes >= retained,
        "unused R remains conservative and retained"
    );
    for (r, a, m, reason) in [
        (1, usize::MAX, 0, "source preparation bytes"),
        (1, 0, usize::MAX, "source preparation bytes"),
        (usize::MAX, 1, 0, "producer scratch bytes"),
    ] {
        let mut b = PublishedValueArenaBuilder::new();
        assert_eq!(
            b.begin_source_authority(r, a, m).err().unwrap(),
            PublishedValueError::Limit(reason)
        );
        assert_eq!((b.bytes, b.producer_scratch_bytes), (0, 0));
        assert!(!b.source_preparations.header_admitted);
        assert!(b.source_preparations.paid.is_empty());
        assert!(b.charge_bytes(0).is_err());
    }
}
