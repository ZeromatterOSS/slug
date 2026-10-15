//! Native producer witness, not an arbitrary scalar coverage fixture.
use super::*;
use crate::bzl_environment::BuiltinCatalogSnapshot;
use crate::bzl_environment::BuiltinRepositoryMapping;
use crate::bzl_environment::BuiltinSourceCatalogKey;
use crate::bzl_environment::prepare_build_environment;

fn leaf_manifest(index: usize) -> BzlLoadManifest {
    let mut m = manifest();
    m.root.label =
        slug_identity_v2::CanonicalLabel::parse(&format!("@@coverage+//:leaf{index}.bzl")).unwrap();
    m.root.source = slug_build_api_v2::ModuleSourceOrigin::Workspace(
        format!("/coverage/leaf{index}.bzl").into(),
    );
    m.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        m.root.clone(),
        [0; 32],
    )]);
    m
}
fn joined_manifest(children: &[BzlLoadManifest]) -> BzlLoadManifest {
    let mut m = leaf_manifest(children.len());
    let mut reachable = vec![crate::BzlModuleSourceProvenance::new(
        m.root.clone(),
        [0; 32],
    )];
    for child in children {
        reachable.extend(child.reachable.iter().cloned());
    }
    m.direct_children = children
        .iter()
        .map(|c| c.root.clone())
        .collect::<Vec<_>>()
        .into();
    m.reachable = reachable.into();
    m
}

#[tokio::test]
async fn native_covered_owner_crosses_old_physical_overlap_with_full_logical_admission() {
    let engine = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let mut transaction = engine.updater().commit().await;
    let catalog = BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2);
    // Genuine mapping rows enlarge the complete descriptor bodies. Four
    // separately selected equal descriptors retain their original actual guards
    // and share the same complete six-source proof in this transaction.
    const MAPPING_ROWS: usize = 128;
    const FOREIGN_DESCRIPTORS: usize = 4;
    let entries = (0..MAPPING_ROWS)
        .map(|index| {
            (
                slug_identity_v2::ApparentRepoName::new(format!("coverage_{index}")).unwrap(),
                slug_identity_v2::CanonicalRepoName::new(format!("coverage_{index}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mapping = BuiltinRepositoryMapping::new(entries.into()).unwrap();
    let mut environments = Vec::new();
    for _ in 0..FOREIGN_DESCRIPTORS {
        environments.push(
            prepare_build_environment(&mut transaction, &catalog, &mapping)
                .await
                .unwrap(),
        );
    }
    for env in &environments[1..] {
        assert!(!Arc::ptr_eq(environments[0].inputs(), env.inputs()));
        assert_eq!(environments[0].inputs(), env.inputs());
        assert!(Arc::ptr_eq(
            environments[0].inputs().source_proof().unwrap(),
            env.inputs().source_proof().unwrap()
        ));
    }
    let manifests: Vec<_> = (0..environments.len()).map(leaf_manifest).collect();
    let children: Vec<_> = manifests
        .iter()
        .zip(&environments)
        .map(|(m, e)| owner_with_environment(m, "def leaf(): return 1", e, []).unwrap())
        .collect();
    let mut current_manifest = joined_manifest(&manifests);
    let mut current = owner_with_environment(
        &current_manifest,
        "def parent(): return 1",
        &environments[0],
        children,
    )
    .unwrap();
    // A bounded genuine native chain grows real manifests/row references. It
    // may not skip or pass if a quota prevents finding the required witness.
    let mut last = (0, 0, 0, 0, 0, 0, 0, 0);
    for level in 0..96 {
        let v = validity(&current);
        let base = admitted_base(v);
        let ordinary =
            EnvironmentReferencePlan::prepare(environment_publication::uses(&v.rows)).unwrap();
        let positive = complement(&ordinary);
        let old_physical = base
            .records
            .checked_add(ordinary.physical_budget().records)
            .unwrap();
        let old_logical = base.records.checked_add(ordinary.budget().records).unwrap();
        let covered_physical = base.records.checked_add(positive.records).unwrap();
        last = (
            level,
            v.rows.len(),
            base.records,
            old_logical,
            old_physical,
            covered_physical,
            v.references.construction_budget().records,
            environments[0].inputs().local_budget().records,
        );
        if old_physical > RECORD_LIMIT
            && old_logical <= RECORD_LIMIT
            && covered_physical <= RECORD_LIMIT
        {
            assert!(base.bytes + ordinary.physical_budget().bytes <= BYTE_LIMIT);
            assert!(base.bytes + positive.bytes <= BYTE_LIMIT);
            assert!(base.depth <= 256 && ordinary.physical_budget().depth <= 256);
            let full_error = EnvironmentReferencePlan::prepare_with_budget(
                environment_publication::uses(&v.rows),
                base,
                Budget::default(),
                PeakOwner::FunctionOwner,
            )
            .unwrap_err();
            assert!(full_error.contains("operation=charge kind=records-cap owner=function-owner"));
            assert!(full_error.contains("first_ledger=physical-forecast"));
            let before = Arc::strong_count(environments[0].inputs());
            let mut work = zero_shape();
            OwnerReferenceInputs::prepay(&mut work).unwrap();
            let covered = EnvironmentReferencePlan::prepare_covered_owner(
                OwnerReferenceInputs {
                    rows: &v.rows,
                    base,
                },
                Budget {
                    records: work.visits,
                    bytes: work.bytes,
                    depth: work.depth,
                },
            )
            .unwrap();
            assert_same_cache(&ordinary, &covered);
            assert_same_cache(&ordinary, &v.references);
            assert_eq!(credits(complement(&covered)), credits(positive));
            // current is the actual completed native constructor, including
            // its final table/payload/catalog/array/stable transfer checks and
            // strict raw owner audit. No coverage fixture synthesized it.
            assert!(current.retained_shape().visits <= 131072);
            assert!(current.retained_shape().bytes <= BYTE_LIMIT);
            drop(covered);
            assert_eq!(Arc::strong_count(environments[0].inputs()), before);
            drop(ordinary);
            return;
        }
        drop(ordinary);
        let next_manifest = parent_manifest(level + 1, &current_manifest);
        let next = owner_with_environment(
            &next_manifest,
            "def parent(): return 1",
            &environments[0],
            [current],
        )
        .expect("genuine native precursor refused before the required covered witness");
        current = next;
        current_manifest = next_manifest;
    }
    panic!(
        "no genuine physical-only native boundary witness within the bounded fixture; last(level,rows,base,logical,physical,covered,work,envlocal)={last:?}"
    );
}
