//! Actual proof work and guarded identity, not source names or content profiles.
use super::super::tests::manifest;
use super::*;

fn authority(
    code: &CodeMap,
    cache: &mut SourceAuthorities<'_>,
    builder: &mut PublishedValueArenaBuilder,
    remaining: &mut usize,
) -> CallableSourceAuthority {
    let digest = cache.digest(code, builder, remaining).unwrap();
    let mut value = manifest("@@root//:root.bzl", [0; 32]);
    let mut reachable = value.reachable.to_vec();
    for n in 0..24 {
        reachable.push(slug_loading_v2::BzlModuleSourceProvenance::new(
            manifest(&format!("@@root//:other{n}.bzl"), [0; 32]).root,
            [0; 32],
        ));
    }
    reachable.push(slug_loading_v2::BzlModuleSourceProvenance::new(
        manifest("@@root//:leaf.bzl", digest).root,
        digest,
    ));
    value.reachable = reachable.into();
    convert(&value, &mut cache.mappings, builder, remaining).unwrap()
}

#[test]
fn repeated_actual_member_checks_cost_one_lookup_and_distinct_equal_allocations_recheck() {
    let code = CodeMap::new("same.bzl".into(), "def actual(): return 1\n".into());
    let mut cache = SourceAuthorities::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    let a = authority(&code, &mut cache, &mut builder, &mut remaining);
    let b = authority(&code, &mut cache, &mut builder, &mut remaining);
    assert_eq!(a, b);
    assert!(!std::ptr::eq(a.data(), b.data()));
    let before = builder.charged_visits();
    cache
        .verify_source(&a, &code, &mut builder, &mut remaining)
        .unwrap();
    let first = builder.charged_visits() - before;
    assert!(first > a.data().reachable.len());
    let before = builder.charged_visits();
    for _ in 0..32 {
        cache
            .verify_source(&a.clone(), &code.clone(), &mut builder, &mut remaining)
            .unwrap();
    }
    assert_eq!(builder.charged_visits() - before, 32);
    let before = builder.charged_visits();
    cache
        .verify_source(&b, &code, &mut builder, &mut remaining)
        .unwrap();
    assert_eq!(builder.charged_visits() - before, first);
    let independent = CodeMap::new("same.bzl".into(), code.source().to_owned());
    assert_ne!(code.id(), independent.id());
    let before = builder.charged_visits();
    cache
        .verify_source(&a, &independent, &mut builder, &mut remaining)
        .unwrap();
    assert!(builder.charged_visits() - before >= first);
}

#[test]
fn changed_source_failure_is_not_cached_and_membership_never_supplies_a_root_proof() {
    let module = starlark::environment::Module::new();
    let code = CodeMap::new("same.bzl".into(), "original source".into());
    let mut cache = SourceAuthorities::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    let a = authority(&code, &mut cache, &mut builder, &mut remaining);
    cache
        .verify_source(&a, &code, &mut builder, &mut remaining)
        .unwrap();
    let changed = CodeMap::new("same.bzl".into(), "changed source".into());
    let before = builder.charged_visits();
    assert!(
        cache
            .verify_source(&a, &changed, &mut builder, &mut remaining)
            .is_err()
    );
    assert!(builder.charged_visits() - before > a.data().reachable.len());
    assert!(!cache.memo.member(&a, &changed));
    // A prior successful hit cannot bypass a later failed source proof.
    let before = builder.charged_visits();
    assert!(
        cache
            .verify_source(&a, &code, &mut builder, &mut remaining)
            .unwrap_err()
            .contains("previously failed")
    );
    assert_eq!(builder.charged_visits(), before);
    let mut cache = SourceAuthorities::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let untrusted = module.heap().alloc("module-looking metadata");
    assert!(cache.memo.root(untrusted, &code).is_none());
    assert!(
        cache
            .authority(Some(untrusted), &code, &mut builder, &mut remaining)
            .is_err()
    );
    assert!(cache.memo.roots.is_empty());
}

#[test]
fn cached_native_guards_outlive_original_handles_and_keep_code_map_identity() {
    let mut cache = SourceAuthorities::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    let (key, id, digest) = {
        let code = CodeMap::new("released.bzl".into(), "guarded actual source".into());
        let a = authority(&code, &mut cache, &mut builder, &mut remaining);
        cache
            .verify_source(&a, &code, &mut builder, &mut remaining)
            .unwrap();
        (
            member_key(&a, &code),
            code.id(),
            cache.memo.digest(&code).unwrap(),
        )
    };
    let proof = cache.memo.members.get(&key).unwrap();
    assert_eq!(proof._code.id(), id);
    assert_eq!(proof._code.source(), "guarded actual source");
    assert_eq!(cache.memo.digests.get(&id).unwrap().digest, digest);
    let code = proof._code.clone();
    let authority = proof._authority.clone();
    let before = builder.charged_visits();
    cache
        .verify_source(&authority, &code, &mut builder, &mut remaining)
        .unwrap();
    assert_eq!(builder.charged_visits() - before, 1);
}

#[test]
fn cache_quota_refuses_before_guard_or_table_growth_and_poisons_builder_reuse() {
    let code = CodeMap::new("quota.bzl".into(), "x".into());
    let mut cache = SourceAuthorities::default();
    let mut limited = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    let a = authority(&code, &mut cache, &mut limited, &mut remaining);
    let digest_entries = cache.memo.digests.len();
    let used = limited.charged_bytes();
    limited
        .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - used - 1023)
        .unwrap();
    assert!(
        cache
            .verify_source(&a, &code, &mut limited, &mut remaining)
            .is_err()
    );
    assert!(cache.memo.members.is_empty());
    assert!(cache.memo.roots.is_empty());
    assert_eq!(cache.memo.digests.len(), digest_entries);
    assert!(limited.charge_visit().is_err());
    let fresh = CodeMap::new("quota.bzl".into(), "y".into());
    assert!(
        cache
            .digest(&fresh, &mut limited, &mut remaining)
            .unwrap_err()
            .contains("previously failed")
    );
    assert_eq!(cache.memo.digests.len(), digest_entries);
    assert!(limited.charge_visit().is_err());
}

#[test]
fn memo_scratch_keeps_full_entry_admission_and_refuses_before_reserved_growth() {
    let mut memo = SourceMemo::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    memo.preflight(3, &mut builder, &mut remaining).unwrap();
    for n in 0..3 {
        let code = CodeMap::new(format!("source{n}.bzl"), "x".into());
        memo.record_digest(&code, [n; 32]);
    }
    assert_eq!(builder.charged_bytes(), 3 * 1024);
    assert_eq!(builder.charged_visits(), 3);
    assert_eq!(remaining, 65533);
    builder
        .charge_bytes(
            slug_build_api_v2::published_value::BYTE_LIMIT - builder.charged_bytes() - 1023,
        )
        .unwrap();
    assert!(memo.preflight(1, &mut builder, &mut remaining).is_err());
    assert_eq!(memo.reserved_entries, 3);
    assert_eq!(memo.digests.len(), 3);
    assert!(builder.charge_bytes(0).is_err());
}

#[test]
fn host_step_and_memo_entry_limit_poison_native_reuse_without_byte_pressure() {
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 0;
    assert!(
        step(&mut builder, &mut remaining)
            .unwrap_err()
            .contains("visit limit")
    );
    assert_eq!(builder.charged_bytes(), 0);
    assert_eq!(builder.charged_visits(), 1);
    assert!(builder.charge_visit().is_err());
    assert!(builder.charge_bytes(0).is_err());

    let mut builder = PublishedValueArenaBuilder::new();
    let mut memo = SourceMemo::default();
    memo.reserved_entries = slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
    let mut remaining = 65536;
    assert!(
        memo.preflight(1, &mut builder, &mut remaining)
            .unwrap_err()
            .contains("entry limit")
    );
    assert_eq!(builder.charged_bytes(), 0);
    assert_eq!(builder.charged_visits(), 0);
    assert_eq!(remaining, 65536);
    assert!(memo.digests.is_empty());
    assert!(memo.members.is_empty());
    assert!(memo.roots.is_empty());
    assert!(builder.charge_bytes(0).is_err());
    assert!(memo.preflight(0, &mut builder, &mut remaining).is_err());
}
