use slug_build_api_v2::ModuleSourceOrigin;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;
use crate::bzl_environment::PreparedBzlEnvironment;
use crate::bzl_environment::references::EnvironmentReferencePlan;
use crate::bzl_environment::references::admit_state;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

// These are isolated descriptor values, not successful source/retention
// certificates. Actual producer/owner/event/Weak controls remain separate.
fn input() -> BzlEnvironmentInputs {
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let mut input = copy(&env.inputs);
    let mapping = Arc::from([pair("first", "first+"), pair("last", "last+")]);
    input.selections = Arc::from([ExportSelection {
        global: "global".into(),
        export: "export".into(),
        module: BzlModuleIdentity {
            label: CanonicalLabel::parse("@@dep+//pkg:defs.bzl").unwrap(),
            source: ModuleSourceOrigin::Workspace("/source/pkg/defs.bzl".into()),
            repository_mapping: mapping,
        },
    }]);
    input
}
fn pair(a: &str, c: &str) -> (ApparentRepoName, CanonicalRepoName) {
    (
        ApparentRepoName::new(a).unwrap(),
        CanonicalRepoName::new(c).unwrap(),
    )
}
fn copy(a: &BzlEnvironmentInputs) -> BzlEnvironmentInputs {
    BzlEnvironmentInputs {
        mode: a.mode,
        native_revision: a.native_revision,
        catalog: a.catalog.clone(),
        selections: a.selections.clone(),
        rows: a.rows.clone(),
        budget: a.budget,
        local: a.local,
        proof: a.proof.clone(),
        hash: a.hash,
        publication: a.publication.clone(),
    }
}
fn foreign(a: &BzlEnvironmentInputs) -> BzlEnvironmentInputs {
    let mut b = copy(a);
    let mut selections = a.selections.to_vec();
    for selection in &mut selections {
        selection.module.repository_mapping = selection.module.repository_mapping.to_vec().into();
    }
    b.selections = selections.into();
    b
}
fn selection(a: &mut BzlEnvironmentInputs) -> &mut ExportSelection {
    &mut Arc::make_mut(&mut a.selections)[0]
}
fn trace(a: &BzlEnvironmentInputs, b: &BzlEnvironmentInputs) -> (bool, Vec<(usize, usize)>) {
    let mut trace = Vec::new();
    let result = equal(Some(0), a, Some(0), b, |r, b| {
        trace.push((r, b));
        Ok(())
    })
    .unwrap();
    (result, trace)
}
fn cost(trace: &[(usize, usize)]) -> Budget {
    let mut budget = Budget::default();
    for &(r, b) in trace {
        budget.charge(r, b).unwrap();
    }
    budget
}
fn mapped_label(id: &str) -> CanonicalLabel {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new(id).unwrap());
    ApparentLabel::parse("//pkg:defs.bzl")
        .unwrap()
        .resolve(&mapping)
}

#[test]
fn complete_descriptor_differential_covers_order_and_every_identity_field() {
    let a = input();
    let mut variants = vec![copy(&a), foreign(&a)];
    let mut b = foreign(&a);
    b.mode = BzlEnvironmentMode::Build;
    variants.push(b);
    let mut b = foreign(&a);
    b.native_revision += 1;
    variants.push(b);
    let mut b = foreign(&a);
    b.selections = Arc::from([]);
    variants.push(b);
    let mut b = foreign(&a);
    selection(&mut b).global = "changed".into();
    variants.push(b);
    let mut b = foreign(&a);
    selection(&mut b).export = "changed".into();
    variants.push(b);
    for label in [
        "@@other+//pkg:defs.bzl",
        "@@dep+//other:defs.bzl",
        "@@dep+//pkg:other.bzl",
    ] {
        let mut b = foreign(&a);
        selection(&mut b).module.label = CanonicalLabel::parse(label).unwrap();
        variants.push(b);
    }
    for label in [
        CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap(),
        mapped_label("map-a"),
        mapped_label("map-b"),
    ] {
        let mut b = foreign(&a);
        selection(&mut b).module.label = label;
        variants.push(b);
    }
    for origin in [
        ModuleSourceOrigin::Workspace("/source/pkg/other.bzl".into()),
        ModuleSourceOrigin::builtin("catalog", "pkg/defs.bzl").unwrap(),
        ModuleSourceOrigin::builtin("other", "pkg/defs.bzl").unwrap(),
        ModuleSourceOrigin::builtin("catalog", "pkg/other.bzl").unwrap(),
    ] {
        let mut b = foreign(&a);
        selection(&mut b).module.source = origin;
        variants.push(b);
    }
    for mapping in [
        vec![],
        vec![pair("first", "first+")],
        vec![pair("last", "last+"), pair("first", "first+")],
        vec![pair("first", "first+"), pair("late", "last+")],
        vec![pair("first", "first+"), pair("last", "late+")],
    ] {
        let mut b = foreign(&a);
        selection(&mut b).module.repository_mapping = mapping.into();
        variants.push(b);
    }
    let mut two = foreign(&a);
    let mut selections = two.selections.to_vec();
    let mut other = selections[0].clone();
    other.global = "second".into();
    selections.push(other);
    two.selections = selections.into();
    let mut reversed = foreign(&two);
    Arc::make_mut(&mut reversed.selections).reverse();
    variants.push(two);
    variants.push(reversed);
    // Pairwise comparison includes None/Some mapping-id, both Builtin fields,
    // differing lengths/order and late fields after otherwise equal prefixes.
    for left in &variants {
        for right in &variants {
            assert_eq!(trace(left, right).0, left.descriptor_eq(right));
        }
    }
    assert!(!Arc::ptr_eq(&a.selections, &variants[1].selections));
    assert!(!Arc::ptr_eq(
        &a.selections[0].module.repository_mapping,
        &variants[1].selections[0].module.repository_mapping
    ));
    assert!(trace(&a, &variants[1]).0);
    assert!(!trace(&variants[variants.len() - 2], &variants[variants.len() - 1]).0);
}

#[test]
fn workspace_path_component_equality_and_non_utf8_remain_typed() {
    let paths = [
        "/a//b/./defs.bzl",
        "/a/b/defs.bzl",
        "./a/defs.bzl",
        "a/defs.bzl",
        "/a/../b/defs.bzl",
        "/b/defs.bzl",
        "/a/b/other.bzl",
    ];
    for a in paths {
        for b in paths {
            let mut left = input();
            let mut right = foreign(&left);
            selection(&mut left).module.source = ModuleSourceOrigin::Workspace(a.into());
            selection(&mut right).module.source = ModuleSourceOrigin::Workspace(b.into());
            assert_eq!(trace(&left, &right).0, left.descriptor_eq(&right));
        }
    }
    let mut left = input();
    let mut right = foreign(&left);
    selection(&mut left).module.source = ModuleSourceOrigin::Workspace(paths[0].into());
    selection(&mut right).module.source = ModuleSourceOrigin::Workspace(paths[1].into());
    assert_ne!(paths[0].len(), paths[1].len());
    assert!(trace(&left, &right).0);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for byte in [0xfe, 0xff] {
            selection(&mut left).module.source = ModuleSourceOrigin::Workspace(
                std::ffi::OsString::from_vec(vec![b'/', b'a', b'/', byte]).into(),
            );
            selection(&mut right).module.source = left.selections[0].module.source.clone();
            assert!(trace(&left, &right).0);
            selection(&mut right).module.source = ModuleSourceOrigin::Workspace(
                std::ffi::OsString::from_vec(vec![b'/', b'b', b'/', byte]).into(),
            );
            assert!(!trace(&left, &right).0);
        }
    }
}

#[test]
fn short_circuits_and_actual_slice_guards_skip_only_unvisited_work() {
    let a = input();
    let mut b = foreign(&a);
    let full = trace(&a, &b).1;
    let mut payload_trace = Vec::new();
    assert!(
        !equal(Some(0), &a, Some(1), &b, |r, b| {
            payload_trace.push((r, b));
            Ok(())
        })
        .unwrap()
    );
    assert_eq!(payload_trace, full[..1]);
    b.mode = BzlEnvironmentMode::Build;
    assert_eq!(trace(&a, &b).1, full[..2]);
    b = foreign(&a);
    b.native_revision += 1;
    assert_eq!(trace(&a, &b).1, full[..3]);
    assert_eq!(trace(&a, &copy(&a)).1, full[..4]);
    b = foreign(&a);
    b.selections = Arc::from([]);
    assert_eq!(trace(&a, &b).1, full[..5]);
    b = foreign(&a);
    selection(&mut b).global = "unrelated-global".into();
    let early = trace(&a, &b);
    selection(&mut b).export = "x".repeat(200_000).into();
    selection(&mut b).module.repository_mapping = vec![pair("late", "unvisited+"); 100].into();
    assert_eq!(
        trace(&a, &b),
        early,
        "late fields cannot impose an eager forecast scan"
    );
    let mut shared_mapping = foreign(&a);
    selection(&mut shared_mapping).module.repository_mapping =
        a.selections[0].module.repository_mapping.clone();
    assert!(trace(&a, &shared_mapping).0);
    assert!(cost(&trace(&a, &shared_mapping).1).bytes < cost(&full).bytes);
}

#[test]
fn exact_work_and_joint_boundaries_refuse_before_the_next_visited_step() {
    let a = input();
    let b = foreign(&a);
    let (_, steps) = trace(&a, &b);
    let needed = cost(&steps);
    let run = |base: Budget, mut work: Budget| {
        equal(Some(0), &a, Some(0), &b, |r, b| {
            work.charge(r, b)?;
            admit_state(
                PeakOwner::FunctionOwner,
                PeakPhase::ReferenceDescriptorCompare,
                base,
                Budget::default(),
                Budget::default(),
                work,
            )
        })
    };
    assert!(
        run(
            Budget::default(),
            Budget {
                records: RECORD_LIMIT - needed.records,
                ..Budget::default()
            }
        )
        .unwrap()
    );
    assert!(
        run(
            Budget::default(),
            Budget {
                records: RECORD_LIMIT - needed.records + 1,
                ..Budget::default()
            }
        )
        .is_err()
    );
    let base = Budget {
        bytes: BYTE_LIMIT - needed.bytes,
        ..Budget::default()
    };
    assert!(run(base, Budget::default()).unwrap());
    let error = run(
        Budget {
            bytes: base.bytes + 1,
            ..base
        },
        Budget::default(),
    )
    .unwrap_err();
    assert!(error.contains(
        "owner=function-owner phase=reference-descriptor-compare first_ledger=physical-forecast"
    ));
    assert!(
        run(
            Budget::default(),
            Budget {
                bytes: BYTE_LIMIT - needed.bytes,
                ..Budget::default()
            }
        )
        .unwrap()
    );
    assert!(
        run(
            Budget::default(),
            Budget {
                bytes: BYTE_LIMIT - needed.bytes + 1,
                ..Budget::default()
            }
        )
        .is_err()
    );
    assert!(bytes(usize::MAX, 1).is_err());
    assert!(
        run(
            Budget::default(),
            Budget {
                bytes: usize::MAX,
                ..Budget::default()
            }
        )
        .is_err()
    );
    for refused_step in 1..=steps.len() {
        let mut calls = 0;
        let error = equal(Some(0), &a, Some(0), &b, |r, b| {
            assert_eq!((r, b), steps[calls]);
            calls += 1;
            if calls == refused_step {
                Err("refuse before comparison".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "refuse before comparison");
        assert_eq!(calls, refused_step);
    }
}

#[test]
fn unrelated_retention_fields_are_not_descriptor_comparison_work() {
    let a = input();
    let mut b = foreign(&a);
    let before = trace(&a, &b);
    // Isolated read-set discriminator, not a valid new publication certificate.
    // These retained fields are deliberately outside descriptor_eq's read set.
    b.local = Budget {
        records: 65_000,
        bytes: 63 * 1024 * 1024,
        depth: 200,
    };
    b.budget = b.local;
    b.hash = u64::MAX;
    assert_eq!(trace(&a, &b), before);
    assert_eq!(
        (b.local.records, b.local.bytes, b.local.depth),
        (65_000, 63 * 1024 * 1024, 200)
    );
}

#[test]
fn refused_pair_cannot_commit_a_foreign_descriptor_memo_or_retain_its_guard() {
    // Actual successful empty bootstrap producers need no source fabrication.
    let a = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let b = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let inputs = [&a.inputs, &b.inputs, &b.inputs];
    let normal = EnvironmentReferencePlan::prepare(inputs.into_iter()).unwrap();
    let collided = EnvironmentReferencePlan::prepare_indexed(
        inputs.into_iter(),
        |_| 0,
        Budget::default(),
        Budget::default(),
        PeakOwner::FunctionOwner,
    )
    .unwrap();
    assert_eq!(normal, collided);
    assert_eq!(normal.ordinals.as_ref(), &[0, 0, 0]);
    assert_eq!(
        (normal.physical.records, normal.physical.bytes),
        (collided.physical.records, collided.physical.bytes)
    );
    let total = normal.construction.records;
    drop(normal);
    drop(collided);
    let pair_work = cost(&trace(&a.inputs, &b.inputs).1);
    // After equality: successful memo commit, then the repeated incoming ref
    // and its paid successful memo lookup. The same three-input table is used.
    let pre_pair = total - pair_work.records - 3;
    let guards = (Arc::strong_count(&a.inputs), Arc::strong_count(&b.inputs));
    let hashes = std::cell::Cell::new(0);
    let refused = EnvironmentReferencePlan::prepare_indexed(
        inputs.into_iter(),
        |h| {
            hashes.set(hashes.get() + 1);
            h
        },
        Budget {
            records: RECORD_LIMIT - pre_pair - 1,
            ..Budget::default()
        },
        Budget::default(),
        PeakOwner::FunctionOwner,
    );
    assert!(refused.is_err());
    assert_eq!(
        hashes.get(),
        2,
        "refusal precedes the later would-be memo hit"
    );
    assert_eq!(
        (Arc::strong_count(&a.inputs), Arc::strong_count(&b.inputs)),
        guards
    );
}
