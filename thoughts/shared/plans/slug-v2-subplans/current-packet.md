# Current Slug V2 Packet

Packet: `WP-2A-m1-cquery-observed-publication-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `941db0d0`
Frozen semantic design: `895996d5`
Frozen correction: `7b7826e6`
Result: finish the retained in-owner observed cquery cutover with one authorized
parallel-workspace test-isolation replacement.

## Authority and caps

Write only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs` (new).

Against `941db0d0`, exclude only the original byte-identical 833-line test
relocation from semantic growth. The authorized four-line replacement counts
as +3 test semantic lines. Caps remain 160 production, 300 test and 460
aggregate semantic lines; physical caps remain 12,435 DICE, 1,200 child-test
and 13,635 combined.

## Required implementation

Retain the existing candidate and the complete `895996d5` contract: the sole
`CqueryCommandRoot` uses only accepted observed preparation, configured-analysis
and rdeps seed-package families; ordered roots and joined deps inspect full
batches with first typed outer > combined Need > first semantic > ordered
success; child keys remain sole event owners; cancellation/outer publish no
attempt; and no root, carrier, revision or retained state is added.

In only
`cquery_restores_structural_configuration_and_display_projection`, replace the
original `tempfile::tempdir()` line with exactly:

```rust
let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../target/slug-cquery-restores-structural-configuration");
fs::create_dir_all(&stable_parent).unwrap();
let workspace = tempfile::tempdir_in(stable_parent).unwrap();
```

This occurs before runtime construction. Preserve every assertion, every other
relocated body, parent fixtures/visibility and the nested include. Existing
`Path`/`fs` imports flow through `use super::*;`; add no import or fixture.
The cfg(test) activation audit is test growth and the existing nonrelocated
root-count test body remains unchanged.

## Compatibility and proof

Public cquery results, bytes, errors, exit classes and events remain exact.
Observed-family association, typed outer failure, selected-epoch ownership and
the stable test parent are Slug-native. All unsupported/deferred boundaries in
`895996d5` remain.

Require zero legacy package/configured-analysis activation for direct,
multi-root, deps and rdeps paths; exact output/error/events; cold child order
and warm suppression; mixed outer/Need/semantic ordering; semantic sidecars;
cancellation/recovery; configuration edit/restore; recursive/null/delegating/
platform/toolchain closure; exact selected Arc retry survival; no carrier or
revision; and build/query/aquery/one-shot nonactivation.

First run the corrected lifecycle test alone, then the full 16-test cquery
batch at default parallelism. Run the remaining focused cquery/native-demand
tests, complete core/analysis/loading suites, formatting, direct check,
diff/archive gates, exact accounting, Buck2 retention and AI cleanup scans,
and independent implementation review.

## STOP / REPLAN

STOP on any other file; any other relocated-body change; assertion weakening;
public API/syntax/output/error/event drift; legacy or second package/analysis
family; duplicate root/driver/event owner; carrier/revision invention; retained
store/collection/cache/interner/lock/task; direct Host read; Cargo/BUILD/
fixture/oracle/generated write; or cap excess. `REPLAN` if the corrected test
fails alone or under default parallelism, another body/file is required, or
the complete terminal algebra cannot remain bounded. Acceptance returns to one
docs-only next-owner audit and does not close M1.

## Immediate predecessor

`7b7826e6` freezes the sole test-harness correction after the original
implementation packet formally replanned in `3a51f1f9`. Production authority
remains the observed cquery design `895996d5` over accepted Rust base
`941db0d0`.
