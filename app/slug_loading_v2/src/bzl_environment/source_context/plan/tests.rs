use slug_identity_v2::ApparentLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;

fn row(name: &str, entries: usize) -> BzlModuleSourceProvenance {
    BzlModuleSourceProvenance::new(
        BzlModuleIdentity {
            label: CanonicalLabel::parse(&format!("@@//pkg:{name}.bzl")).unwrap(),
            source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
                format!("/workspace/{name}.bzl").into(),
            ),
            repository_mapping: (0..entries)
                .map(|n| {
                    (
                        ApparentRepoName::new(format!("dep{n}")).unwrap(),
                        CanonicalRepoName::new(format!("dep{n}+")).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
                .into(),
        },
        [7; 32],
    )
}
fn foreign(source: &BzlModuleSourceProvenance) -> BzlModuleSourceProvenance {
    let mut out = source.clone();
    out.identity.repository_mapping = source.identity.repository_mapping.to_vec().into();
    out
}
// Exact old label-key-first union, independent from the optimized helper.
fn reference<'a>(
    rows: impl IntoIterator<Item = &'a BzlModuleSourceProvenance>,
) -> Result<Vec<&'a BzlModuleSourceProvenance>, ()> {
    let mut out: Vec<&BzlModuleSourceProvenance> = Vec::new();
    for row in rows {
        if let Some(prior) = out.iter().find(|p| p.identity.label == row.identity.label) {
            if **prior != *row {
                return Err(());
            }
        } else {
            out.push(row);
        }
    }
    Ok(out)
}
#[test]
fn source_plan_matches_complete_eq_and_ordered_union_across_all_fields_and_collisions() {
    let a = row("a", 3);
    let equal = foreign(&a);
    let mut digest = a.clone();
    digest.source_digest[31] ^= 1;
    let mut path = a.clone();
    path.identity.source = slug_build_api_v2::ModuleSourceOrigin::Workspace("/other/a.bzl".into());
    let mut builtin = a.clone();
    builtin.identity.source =
        slug_build_api_v2::ModuleSourceOrigin::builtin("catalog", "a.bzl").unwrap();
    let mut mapping = foreign(&a);
    let mut pairs = mapping.identity.repository_mapping.to_vec();
    pairs.last_mut().unwrap().1 = CanonicalRepoName::new("changed+").unwrap();
    mapping.identity.repository_mapping = pairs.into();
    let mut id_a = a.clone();
    id_a.identity.label =
        ApparentLabel::parse("//pkg:a.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("a").unwrap(),
            ));
    let mut id_b = id_a.clone();
    id_b.identity.label =
        ApparentLabel::parse("//pkg:a.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("b").unwrap(),
            ));
    let b = row("b", 3);
    let cases = [
        &a, &equal, &digest, &path, &builtin, &mapping, &id_a, &id_b, &b,
    ];
    for x in cases {
        for y in cases {
            let mut work = Budget::default();
            let actual = accounting::same_label(&x.identity.label, &y.identity.label, &mut work)
                .unwrap()
                && accounting::same_tail(x, y, &mut work, || {}).unwrap();
            assert_eq!(actual, x == y);
            let inputs = [x, y, x, y];
            let expected = reference(inputs);
            let mut plan = Plan::new(inputs.len(), Budget::default()).unwrap();
            let actual = inputs
                .into_iter()
                .try_for_each(|v| plan.observe_indexed(v, |_| 0));
            assert_eq!(actual.is_ok(), expected.is_ok());
            if let Ok(expected) = expected {
                assert_eq!(plan.rows, expected);
            }
        }
    }
    assert_eq!(
        reference([&id_a, &id_b]).unwrap().len(),
        2,
        "mapping IDs are distinct canonical keys"
    );
}
#[test]
fn source_plan_skips_actual_mapping_scans_only_for_guarded_aliases_or_successful_rows() {
    let a = row("a", 64);
    let alias = a.clone();
    let equal = foreign(&a);
    let mut scans = 0;
    accounting::same_tail(&a, &alias, &mut Budget::default(), || scans += 1).unwrap();
    assert_eq!(scans, 0);
    assert!(accounting::same_tail(&a, &equal, &mut Budget::default(), || scans += 1).unwrap());
    assert_eq!(
        scans, 64,
        "a first foreign mapping executes all pair comparisons"
    );
    let guards = Arc::strong_count(&a.identity.repository_mapping);
    let mut plan = Plan::new(4, Budget::default()).unwrap();
    plan.observe(&a).unwrap();
    let before = plan.work.records;
    plan.observe(&equal).unwrap();
    assert!(plan.work.records - before > 2 * 64);
    let before = plan.work.records;
    plan.observe(&equal).unwrap();
    assert_eq!(
        plan.work.records - before,
        2,
        "successful foreign allocation becomes an actual row hit"
    );
    assert_eq!(plan.successful.len(), 2);
    assert_eq!(plan.rows.len(), 1);
    assert_eq!(Arc::strong_count(&a.identity.repository_mapping), guards);
    plan.work.records = RECORD_LIMIT - 2;
    plan.observe(&a).unwrap();
    assert_eq!(plan.work.records, RECORD_LIMIT);
    let mut denied = Plan::new(2, Budget::default()).unwrap();
    denied.observe(&a).unwrap();
    denied.work.records = RECORD_LIMIT - 1;
    let retained = denied.successful.len();
    assert!(denied.observe(&equal).is_err());
    assert_eq!(
        denied.successful.len(),
        retained,
        "failed foreign comparisons never memoize"
    );
}
#[test]
fn source_plan_joint_peak_precedes_growth_and_full_unique_output_remains_retained() {
    use std::mem::size_of;
    for count in [0, 1, 4, 17, 65, 257] {
        let inputs = (0..count)
            .map(|i| row(&format!("r{i}"), 0))
            .collect::<Vec<_>>();
        let mut plan = Plan::new(count, Budget::default()).unwrap();
        for input in &inputs {
            plan.observe(input).unwrap();
        }
        let actual = plan.index.retained_storage_bytes_upper_bound().unwrap()
            + plan
                .successful
                .retained_storage_bytes_upper_bound()
                .unwrap()
            + plan
                .index
                .values()
                .map(|v| v.capacity() * size_of::<usize>())
                .sum::<usize>()
            + plan.rows.capacity() * size_of::<&BzlModuleSourceProvenance>();
        assert!(accounting::scratch_bytes(count).unwrap() >= 2 * actual);
    }
    assert!(accounting::scratch_bytes(usize::MAX).is_err());
    let scratch = accounting::scratch_bytes(1).unwrap();
    assert!(
        Plan::new(
            1,
            Budget {
                bytes: BYTE_LIMIT - scratch,
                ..Budget::default()
            }
        )
        .is_ok()
    );
    assert!(
        Plan::new(
            1,
            Budget {
                bytes: BYTE_LIMIT - scratch + 1,
                ..Budget::default()
            }
        )
        .is_err()
    );
    let a = row("a", 32);
    let raw = BzlLoadManifest {
        root: a.identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([a.clone()]),
        fingerprint: [0; 32],
    };
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let guards = Arc::strong_count(&a.identity.repository_mapping);
    let mut full = output::Output::new(Budget::default());
    let inherited =
        crate::source_table::inherited::prepare(1, 1, std::iter::empty(), &env, &mut full.work)
            .unwrap();
    full.forecast(&raw, &env, &[&a], inherited).unwrap();
    let mut identity = Budget::default();
    identity.identity(&a.identity).unwrap();
    assert!(
        full.retained.records >= 4 * identity.records,
        "raw root/raw row/two unique views all retain complete identities"
    );
    assert!(full.retained.bytes > 4 * identity.bytes);
    let mut exact = output::Output::new(Budget {
        bytes: BYTE_LIMIT - full.work.bytes,
        ..Budget::default()
    });
    let inherited =
        crate::source_table::inherited::prepare(1, 1, std::iter::empty(), &env, &mut exact.work)
            .unwrap();
    exact.forecast(&raw, &env, &[&a], inherited).unwrap();
    assert_eq!(exact.work.bytes, BYTE_LIMIT);
    let mut refused = output::Output::new(Budget {
        bytes: BYTE_LIMIT - full.work.bytes + 1,
        ..Budget::default()
    });
    let inherited =
        crate::source_table::inherited::prepare(1, 1, std::iter::empty(), &env, &mut refused.work)
            .unwrap();
    assert!(refused.forecast(&raw, &env, &[&a], inherited).is_err());
    assert_eq!(
        Arc::strong_count(&a.identity.repository_mapping),
        guards,
        "forecast refusal owns no source clones"
    );
}
#[test]
fn canonical_source_context_preserves_raw_unique_views_and_complete_fingerprint() {
    let a = row("a", 3);
    let b = row("b", 3);
    let c = row("c", 3);
    let raw =
        |root: &BzlModuleSourceProvenance, rows: Vec<BzlModuleSourceProvenance>| BzlLoadManifest {
            root: root.identity.clone(),
            direct_children: Arc::from([]),
            reachable: rows.into(),
            fingerprint: [9; 32],
        };
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let child_raw = raw(&b, vec![b.clone(), a.clone()]);
    let child = BzlModuleSourceContext::new(&child_raw, &env, std::iter::empty()).unwrap();
    let parent_raw = raw(&c, vec![c.clone(), a.clone()]);
    let context =
        BzlModuleSourceContext::new(&parent_raw, &env, [&child, &child].into_iter()).unwrap();
    let expected = reference([&c, &a, &b, &a, &b, &a]).unwrap();
    assert_eq!(context.raw_load_manifest(), &parent_raw);
    assert!(context.environment_bindings().is_empty());
    assert_eq!(
        context.effective_reachable().as_ref(),
        expected.into_iter().cloned().collect::<Vec<_>>()
    );
    assert_eq!(
        context.sources().iter().map(|(_, p)| p).collect::<Vec<_>>(),
        context.effective_reachable().iter().collect::<Vec<_>>()
    );
    let mut hash = Sha256::new();
    hash.update(b"slug-bzl-effective-source-context-v1\0");
    hash.update(parent_raw.fingerprint);
    hash.update(0u64.to_be_bytes());
    hash.update(3u64.to_be_bytes());
    for p in [&c, &a, &b] {
        crate::bzl_module::fingerprint_complete_identity(&mut hash, &p.identity);
        hash.update(p.source_digest);
    }
    assert_eq!(context.fingerprint().as_slice(), hash.finalize().as_slice());
    let foreign_raw = raw(&c, vec![foreign(&c), foreign(&a)]);
    let equal = BzlModuleSourceContext::new(&foreign_raw, &env, [&child].into_iter()).unwrap();
    assert_eq!(context, equal);
}

#[test]
fn source_conflict_diagnostics_preserve_all_rejections_and_first_difference() {
    let original = row("conflict", 3);
    let mut origin = original.clone();
    origin.identity.source =
        slug_build_api_v2::ModuleSourceOrigin::Workspace("/other/source.bzl".into());
    let mut digest = original.clone();
    digest.source_digest[0] ^= 1;
    let mut length = original.clone();
    length.identity.repository_mapping = Arc::from([]);
    let mut entry = foreign(&original);
    let mut pairs = entry.identity.repository_mapping.to_vec();
    pairs[2].1 = CanonicalRepoName::new("changed+").unwrap();
    entry.identity.repository_mapping = pairs.into();
    for (row, tag) in [
        (&origin, "source origin"),
        (&digest, "source digest"),
        (&length, "repository mapping length"),
        (&entry, "repository mapping entry 2"),
    ] {
        let mut plan = Plan::new(2, Budget::default()).unwrap();
        plan.observe(&original).unwrap();
        let error = plan.observe(row).unwrap_err();
        assert!(
            error.contains(
                "conflicting complete environment source provenance for @@//pkg:conflict.bzl"
            ),
            "{error}"
        );
        assert!(error.ends_with(tag), "{error}");
        assert_eq!(plan.rows, [&original]);
        assert_eq!(plan.successful.len(), 1, "no failed row is memoized");
    }
}

// The optimized row path is compared with the real general table producer,
// not with a second copy of its mapping-body accounting implementation.
fn dispatched_table(
    raw: &crate::source_table::Rows,
    child: Option<&BzlModuleSourceContext>,
    dispatch: bool,
) -> (crate::source_table::CertifiedSourceTable, Budget, Vec<bool>) {
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let capacity = raw.len() + child.map_or(0, |c| c.sources().len());
    let mut work = Budget::default();
    let inherited = crate::source_table::inherited::prepare(
        capacity,
        raw.len(),
        child.into_iter(),
        &env,
        &mut work,
    )
    .unwrap();
    let mut table =
        crate::source_table::prepare::Plan::with_bank(raw.len(), inherited, &mut work).unwrap();
    let mut decisions = Vec::new();
    for (name, source) in &**raw {
        table.prefix(source, &mut work).unwrap();
        let prepare = if dispatch {
            let needed = table.needs_mapping_preparation(&mut work).unwrap();
            decisions.push(needed);
            needed
        } else {
            true
        };
        if prepare {
            for (a, c) in &*source.identity.repository_mapping {
                table.mapping(a, c, &mut work).unwrap();
            }
        } else {
            for (a, c) in &*source.identity.repository_mapping {
                table.mapping_completed(a, c, &mut work).unwrap();
            }
        }
        table.filename(name, &mut work).unwrap();
    }
    table.prepay_finish(&mut work).unwrap();
    (table.finish(raw).unwrap(), work, decisions)
}
fn dispatch_shape(
    s: slug_build_api_v2::published_value::PublicationShape,
) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
}
fn dispatch_bytes() -> usize {
    // &Entry is a thin reference; the test uses &() without naming the private
    // bank representation. Option/index/flag and result alignment remain real.
    std::mem::size_of::<(Option<(usize, bool)>, &(), bool)>()
}
#[test]
fn completed_mapping_row_dispatch_preserves_general_full_facts_and_exact_work() {
    use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
    let seed = row("seed", 17);
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let manifest = BzlLoadManifest {
        root: seed.identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([seed.clone()]),
        fingerprint: [9; 32],
    };
    let child = BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let mut inherited = row("inherited", 17);
    inherited.identity.repository_mapping = seed.identity.repository_mapping.clone();
    let first = row("new", 17);
    let mut repeated = row("repeat", 17);
    repeated.identity.repository_mapping = first.identity.repository_mapping.clone();
    let foreign = foreign(&first);
    let empty = row("empty", 0);
    let raw: crate::source_table::Rows = [inherited, first, repeated, foreign, empty]
        .into_iter()
        .enumerate()
        .map(|(n, p)| (CompactString::from(format!("row{n}")), p))
        .collect::<Vec<_>>()
        .into();
    let guards = Arc::strong_count(&seed.identity.repository_mapping);
    let (general, old, _) = dispatched_table(&raw, Some(&child), false);
    let (optimized, new, decisions) = dispatched_table(&raw, Some(&child), true);
    assert_eq!(decisions, [false, true, false, true, true]);
    let skipped =
        raw[0].1.identity.repository_mapping.len() + raw[2].1.identity.repository_mapping.len();
    assert_eq!(skipped, 34);
    assert_eq!(new.records + skipped, old.records + 3 * raw.len());
    assert_eq!(new.bytes, old.bytes + dispatch_bytes() * raw.len());
    assert_eq!(new.depth, old.depth);
    assert!(new.records < old.records);
    assert_eq!(general, optimized);
    let a = &general.declarations()[0];
    let b = &optimized.declarations()[0];
    assert_eq!(
        dispatch_shape(a.retained_shape()),
        dispatch_shape(b.retained_shape())
    );
    assert_eq!(
        dispatch_shape(a.intrinsic_shape()),
        dispatch_shape(b.intrinsic_shape())
    );
    assert_eq!(a.shared_inputs().len(), raw.len());
    assert_eq!(b.shared_inputs().len(), raw.len());
    for input in [a, b] {
        let mut audit = PublishedValueArenaBuilder::new();
        audit
            .audit_shared_input_at_depth(
                &child.certified_sources().declarations()[0].shared_inputs()[0],
                2,
            )
            .unwrap();
        for (index, leaf) in input.shared_inputs().iter().enumerate() {
            assert!(leaf.shared_inputs().is_empty());
            assert_eq!(
                dispatch_shape(leaf.intrinsic_shape()),
                dispatch_shape(leaf.retained_shape())
            );
            let before = audit.charged_visits();
            audit.audit_shared_input_at_depth(leaf, 2).unwrap();
            let expected = if index == 0 || index == 2 {
                2
            } else {
                2 + leaf.intrinsic_shape().visits
            };
            assert_eq!(
                audit.charged_visits() - before,
                expected,
                "ordered source child {index}"
            );
        }
    }
    for (x, y) in a.shared_inputs().iter().zip(b.shared_inputs()) {
        assert_eq!(
            dispatch_shape(x.retained_shape()),
            dispatch_shape(y.retained_shape())
        );
    }
    drop(general);
    drop(optimized);
    assert_eq!(Arc::strong_count(&seed.identity.repository_mapping), guards);
    // An empty first-new mapping adds a real fixed classification, no saving.
    let raw: crate::source_table::Rows = Arc::from([("empty".into(), row("empty_only", 0))]);
    let (_, old, _) = dispatched_table(&raw, None, false);
    let (_, new, decisions) = dispatched_table(&raw, None, true);
    assert_eq!(decisions, [true]);
    assert_eq!(new.records, old.records + 3);
    assert_eq!(new.bytes, old.bytes + dispatch_bytes());
}

#[test]
fn split_identity_walk_keeps_independent_full_callback_tape_and_error_order() {
    use std::mem::size_of;
    let mut mapped = row("mapped", 3);
    mapped.identity.label =
        ApparentLabel::parse("//pkg:mapped.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("last-map-id").unwrap(),
            ));
    let mut builtin = row("builtin", 2);
    builtin.identity.source =
        slug_build_api_v2::ModuleSourceOrigin::builtin("catalog", "path.bzl").unwrap();
    for source in [row("empty", 0), row("ordinary", 3), mapped, builtin] {
        let id = &source.identity;
        let label = &id.label;
        // Literal original prefix/entry expressions, independent of BOTH new
        // walkers and the wrapper that now composes those walkers.
        let prefix = size_of::<BzlModuleIdentity>()
            .saturating_add(label.package().repo().as_str().len())
            .saturating_add(label.package().package().as_str().len())
            .saturating_add(label.target().as_str().len())
            .saturating_add(label.mapping_id().map_or(0, |v| v.as_str().len()))
            .saturating_add(id.source.retained_text_bytes().unwrap())
            .saturating_add(2 * size_of::<usize>());
        let mut expected = vec![(1, prefix)];
        for (a, c) in &*id.repository_mapping {
            expected.push((
                1,
                size_of::<(ApparentRepoName, CanonicalRepoName)>()
                    .saturating_add(a.as_str().len())
                    .saturating_add(c.as_str().len()),
            ));
        }
        let run = |split: bool, stop: Option<usize>| {
            let mut tape = Vec::new();
            let mut visit = |r, b| {
                tape.push((r, b));
                if stop == Some(tape.len() - 1) {
                    Err("ordered stop".to_owned())
                } else {
                    Ok(())
                }
            };
            let result = if split {
                accounting::identity_prefix(id, &mut visit)
                    .and_then(|()| accounting::mapping_entries(id, |r, b, _, _| visit(r, b)))
            } else {
                accounting::identity(id, &mut visit)
            };
            (result, tape)
        };
        for split in [false, true] {
            let (result, tape) = run(split, None);
            assert!(result.is_ok());
            assert_eq!(tape, expected);
            for stop in 0..expected.len() {
                let (result, tape) = run(split, Some(stop));
                assert_eq!(result, Err("ordered stop".to_owned()));
                assert_eq!(
                    tape,
                    &expected[..=stop],
                    "prefix/entry error stops before the next field"
                );
            }
        }
    }
}

#[test]
fn row_dispatch_preflight_precedes_copy_and_incomplete_new_mapping_cannot_finish() {
    let source = row("incomplete", 3);
    let raw: crate::source_table::Rows = Arc::from([("source".into(), source.clone())]);
    let guards = Arc::strong_count(&source.identity.repository_mapping);
    // Measure the real complete precursor on a default ledger, independently
    // asserting the query's fixed operation/type cost before using its total.
    let mut measured = Budget::default();
    let mut measured_plan = crate::source_table::prepare::Plan::new(1, &mut measured).unwrap();
    measured_plan.prefix(&source, &mut measured).unwrap();
    let before_query = measured;
    assert!(
        measured_plan
            .needs_mapping_preparation(&mut measured)
            .unwrap()
    );
    assert_eq!(measured.records, before_query.records + 3);
    assert_eq!(measured.bytes, before_query.bytes + dispatch_bytes());
    drop(measured_plan);
    for bytes in [false, true] {
        for extra in [0, 1] {
            // Seed BEFORE construction; the same cumulative ledger pays every
            // Plan::new, prefix and query operation. No allowance replacement.
            let mut work = if bytes {
                Budget {
                    bytes: BYTE_LIMIT - measured.bytes + extra,
                    ..Budget::default()
                }
            } else {
                Budget {
                    records: RECORD_LIMIT - measured.records + extra,
                    ..Budget::default()
                }
            };
            let mut plan = crate::source_table::prepare::Plan::new(1, &mut work).unwrap();
            plan.prefix(&source, &mut work).unwrap();
            assert_eq!(
                work.records,
                if bytes {
                    before_query.records
                } else {
                    RECORD_LIMIT - 3 + extra
                }
            );
            assert_eq!(
                work.bytes,
                if bytes {
                    BYTE_LIMIT - dispatch_bytes() + extra
                } else {
                    before_query.bytes
                }
            );
            let result = plan.needs_mapping_preparation(&mut work);
            assert_eq!(result.is_ok(), extra == 0);
            if extra == 0 {
                assert!(result.unwrap());
            }
            assert_eq!(
                work.records,
                if bytes {
                    measured.records
                } else {
                    RECORD_LIMIT + extra
                }
            );
            assert_eq!(
                work.bytes,
                if bytes {
                    BYTE_LIMIT + extra
                } else {
                    measured.bytes
                }
            );
            assert_eq!(
                Arc::strong_count(&source.identity.repository_mapping),
                guards
            );
        }
    }
    let mut work = Budget::default();
    let mut incomplete = crate::source_table::prepare::Plan::new(1, &mut work).unwrap();
    incomplete.prefix(&source, &mut work).unwrap();
    assert!(incomplete.needs_mapping_preparation(&mut work).unwrap());
    // A mistaken private caller cannot publish a fresh leaf via completed path.
    for (a, c) in &*source.identity.repository_mapping {
        incomplete.mapping_completed(a, c, &mut work).unwrap();
    }
    incomplete.filename(&raw[0].0, &mut work).unwrap();
    assert!(incomplete.prepay_finish(&mut work).is_err());
    assert!(incomplete.finish(&raw).is_err());
    assert_eq!(
        Arc::strong_count(&source.identity.repository_mapping),
        guards
    );
}

fn copy_fixture() -> (
    BzlLoadManifest,
    BzlModuleSourceContext,
    PreparedBzlEnvironment,
) {
    let mut source = row("copy", 17);
    source.identity.source =
        slug_build_api_v2::ModuleSourceOrigin::Workspace("/workspace/café.bzl".into());
    let raw = BzlLoadManifest {
        root: source.identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([source]),
        fingerprint: [8; 32],
    };
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let child = BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    (raw, child, env)
}
fn original_mapping_volume(id: &BzlModuleIdentity) -> (usize, usize) {
    (
        id.repository_mapping.len(),
        id.repository_mapping
            .iter()
            .map(|(a, c)| {
                std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>()
                    + a.as_str().len()
                    + c.as_str().len()
            })
            .sum(),
    )
}
#[test]
fn inherited_copy_forecast_preserves_full_retained_volume_and_genuine_context_fingerprint() {
    let (raw, child, env) = copy_fixture();
    let mut work = Budget::default();
    let bank =
        crate::source_table::inherited::prepare(1, 1, std::iter::once(&child), &env, &mut work)
            .unwrap();
    let before = work;
    let volume = bank
        .copy_volume(&raw.root.repository_mapping, &mut work)
        .unwrap();
    assert_eq!(volume, Some(original_mapping_volume(&raw.root)));
    assert_eq!(work.records, before.records + 8);
    let mut old = output::Output::new(Budget::default());
    old.mapping_copy(&raw.root, None).unwrap();
    let mut hit = output::Output::new(Budget::default());
    hit.mapping_copy(&raw.root, volume).unwrap();
    assert_eq!(
        (old.retained.records, old.retained.bytes, old.retained.depth),
        (hit.retained.records, hit.retained.bytes, hit.retained.depth)
    );
    let (records, bytes) = original_mapping_volume(&raw.root);
    assert_eq!((old.work.records, old.work.bytes), (records, bytes));
    assert_eq!(
        (hit.work.records, hit.work.bytes),
        (3, bytes + std::mem::size_of::<(usize, usize, Budget)>())
    );
    let guards = Arc::strong_count(&raw.root.repository_mapping);
    let parent = BzlModuleSourceContext::new(&raw, &env, std::iter::once(&child)).unwrap();
    assert_eq!(parent, child);
    assert_eq!(parent.fingerprint(), child.fingerprint());
    assert_eq!(
        (
            parent.retention().records,
            parent.retention().bytes,
            parent.retention().depth
        ),
        (
            child.retention().records,
            child.retention().bytes,
            child.retention().depth
        )
    );
    assert_eq!(parent.raw_load_manifest(), &raw);
    assert_eq!(parent.effective_reachable(), child.effective_reachable());
    let table_weak = Arc::downgrade(child.sources());
    let mapping_weak = Arc::downgrade(&raw.root.repository_mapping);
    let mut audit = slug_build_api_v2::published_value::PublishedValueArenaBuilder::new();
    let source_leaf = &child.certified_sources().declarations()[0].shared_inputs()[0];
    audit.audit_shared_input_at_depth(source_leaf, 2).unwrap();
    let prior = audit.charged_visits();
    audit
        .audit_shared_input_at_depth(
            &parent.certified_sources().declarations()[0].shared_inputs()[0],
            2,
        )
        .unwrap();
    assert_eq!(audit.charged_visits() - prior, 2);
    assert!(Arc::strong_count(&raw.root.repository_mapping) > guards);
    drop(bank);
    drop(child);
    drop(raw);
    drop(env);
    assert!(
        table_weak.upgrade().is_none(),
        "only exact mapping leaf propagates, no upstream table"
    );
    assert!(mapping_weak.upgrade().is_some());
    drop(parent);
    assert!(
        mapping_weak.upgrade().is_some(),
        "audit still owns the pure mapping guard"
    );
    drop(audit);
    assert!(mapping_weak.upgrade().is_none());
}
#[test]
fn copy_forecast_full_constructor_uses_cumulative_work_and_retained_boundaries() {
    let (raw, child, env) = copy_fixture();
    let rows = raw.reachable.iter().collect::<Vec<_>>();
    let mut measured = output::Output::new(Budget::default());
    let bank = crate::source_table::inherited::prepare(
        2,
        1,
        std::iter::once(&child),
        &env,
        &mut measured.work,
    )
    .unwrap();
    measured.forecast(&raw, &env, &rows, bank).unwrap();
    let work = measured.work;
    let retained = measured.retained;
    drop(measured);
    let guards = Arc::strong_count(&raw.root.repository_mapping);
    let descriptors = child.certified_sources().descriptor_strong_count();
    for retained_limit in [false, true] {
        for bytes in [false, true] {
            for extra in [0, 1] {
                let total = if retained_limit { retained } else { work };
                let seed = if bytes {
                    Budget {
                        bytes: BYTE_LIMIT - total.bytes + extra,
                        ..Budget::default()
                    }
                } else {
                    Budget {
                        records: RECORD_LIMIT - total.records + extra,
                        ..Budget::default()
                    }
                };
                let mut out = output::Output::new(if retained_limit {
                    Budget::default()
                } else {
                    seed
                });
                if retained_limit {
                    out.retained = seed;
                }
                // Both the temporary bank and complete output use this one ledger;
                // retained seed precedes every header/prefix/query, never replaces it.
                let bank = crate::source_table::inherited::prepare(
                    2,
                    1,
                    std::iter::once(&child),
                    &env,
                    &mut out.work,
                )
                .unwrap();
                let result = out.forecast(&raw, &env, &rows, bank);
                assert_eq!(result.is_ok(), extra == 0);
                let actual = if retained_limit {
                    out.retained
                } else {
                    out.work
                };
                if extra == 0 {
                    assert_eq!(
                        if bytes { actual.bytes } else { actual.records },
                        if bytes { BYTE_LIMIT } else { RECORD_LIMIT }
                    );
                } else if retained_limit {
                    assert!(
                        result
                            .unwrap_err()
                            .contains("effective source metadata retained")
                    );
                    assert!(actual.records > RECORD_LIMIT || actual.bytes > BYTE_LIMIT);
                }
                assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
                assert_eq!(
                    child.certified_sources().descriptor_strong_count(),
                    descriptors
                );
            }
        }
    }
    // Explicit retained-first aggregate: old header1 then root prefix1 fit;
    // the completed root mapping volume is the next actual admission.
    let (m, b) = original_mapping_volume(&raw.root);
    let mut prefix = Budget::default();
    accounting::identity_prefix(&raw.root, |r, b| prefix.charge(r, b)).unwrap();
    let headers = std::mem::size_of::<BzlModuleSourceContext>()
        + std::mem::size_of::<BzlLoadManifest>()
        + 8 * std::mem::size_of::<usize>();
    for bytes in [false, true] {
        let mut out = output::Output::new(Budget::default());
        out.retained = if bytes {
            Budget {
                bytes: BYTE_LIMIT - headers - prefix.bytes - b + 1,
                ..Budget::default()
            }
        } else {
            Budget {
                records: RECORD_LIMIT - 1 - prefix.records - m + 1,
                ..Budget::default()
            }
        };
        let bank = crate::source_table::inherited::prepare(
            2,
            1,
            std::iter::once(&child),
            &env,
            &mut out.work,
        )
        .unwrap();
        let error = out.forecast(&raw, &env, &rows, bank).unwrap_err();
        assert!(error.contains("effective source COPY retained"), "{error}");
        assert!(error.contains(&format!("requested_records={m} requested_bytes={b}")));
        assert!(out.work.records < RECORD_LIMIT && out.work.bytes < BYTE_LIMIT);
        assert_eq!(
            if bytes {
                out.retained.bytes
            } else {
                out.retained.records
            },
            if bytes {
                BYTE_LIMIT + 1
            } else {
                RECORD_LIMIT + 1
            }
        );
        assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
        assert_eq!(
            child.certified_sources().descriptor_strong_count(),
            descriptors
        );
    }
    // The same helper also attributes an earlier generic retained header failure.
    let mut out = output::Output::new(Budget::default());
    out.retained.records = RECORD_LIMIT;
    let bank =
        crate::source_table::inherited::prepare(2, 1, std::iter::once(&child), &env, &mut out.work)
            .unwrap();
    assert!(
        out.forecast(&raw, &env, &rows, bank)
            .unwrap_err()
            .contains("effective source output retained")
    );
    assert_eq!(out.retained.records, RECORD_LIMIT + 1);
    assert!(out.work.records < RECORD_LIMIT);
    assert_eq!(
        child.certified_sources().descriptor_strong_count(),
        descriptors
    );
    assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
}
#[test]
fn copy_forecast_reports_actual_aggregate_and_checked_partial_states() {
    let (raw, child, env) = copy_fixture();
    let mut paid = Budget::default();
    let bank =
        crate::source_table::inherited::prepare(1, 1, std::iter::once(&child), &env, &mut paid)
            .unwrap();
    let volume = bank
        .copy_volume(&raw.root.repository_mapping, &mut paid)
        .unwrap();
    let (m, b) = volume.unwrap();
    let control = std::mem::size_of::<(usize, usize, Budget)>();
    let guards = Arc::strong_count(&raw.root.repository_mapping);
    for record_overflow in [true, false] {
        let mut out = output::Output::new(paid);
        out.retained = if record_overflow {
            Budget {
                records: usize::MAX - m + 1,
                bytes: 7,
                depth: 2,
            }
        } else {
            Budget {
                records: 5,
                bytes: usize::MAX - b + 1,
                depth: 2,
            }
        };
        let before = out.retained;
        let error = out.mapping_copy(&raw.root, volume).unwrap_err();
        assert!(error.contains("effective source COPY retained"));
        assert_eq!(
            out.retained.records,
            if record_overflow {
                before.records
            } else {
                before.records + m
            }
        );
        assert_eq!(
            out.retained.bytes, before.bytes,
            "record overflow never reaches bytes; byte overflow never commits bytes"
        );
        assert_eq!(
            (out.work.records, out.work.bytes),
            (paid.records + 3, paid.bytes + control)
        );
        assert_eq!(out.retained.depth, 2);
    }
    // Fixed work control refuses BEFORE attempting retained aggregate.
    let mut out = output::Output::new(Budget {
        records: RECORD_LIMIT - 2,
        ..Budget::default()
    });
    let error = out.mapping_copy(&raw.root, volume).unwrap_err();
    assert!(error.contains("effective source COPY control work"));
    assert_eq!(
        (out.work.records, out.work.bytes),
        (RECORD_LIMIT + 1, control)
    );
    assert_eq!(out.retained.records, 0);
    // Byte-only WORK admission follows successful retained aggregate, preserving
    // both actual attempted ledgers rather than inventing an entry position.
    let mut out = output::Output::new(Budget {
        bytes: BYTE_LIMIT - control - b + 1,
        ..Budget::default()
    });
    let error = out.mapping_copy(&raw.root, volume).unwrap_err();
    assert!(error.contains("effective source COPY work"));
    assert!(error.contains(&format!("requested_records=0 requested_bytes={b}")));
    assert_eq!((out.work.records, out.work.bytes), (3, BYTE_LIMIT + 1));
    assert_eq!((out.retained.records, out.retained.bytes), (m, b));
    // Metadata/direct retained charge uses the SAME real wrapper and actual
    // Budget mutations as the production descriptor/capacity/metadata sites.
    let mut out = output::Output::new(paid);
    out.retained = Budget {
        records: RECORD_LIMIT,
        bytes: 0,
        depth: 0,
    };
    assert!(
        out.retained_charge("effective source metadata retained", 1, 9)
            .unwrap_err()
            .contains("metadata retained")
    );
    assert_eq!(
        (out.retained.records, out.retained.bytes),
        (RECORD_LIMIT + 1, 9)
    );
    assert_eq!(
        (out.work.records, out.work.bytes),
        (paid.records, paid.bytes)
    );
    assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
}
