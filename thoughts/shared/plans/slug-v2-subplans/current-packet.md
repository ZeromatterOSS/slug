# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-loading-proof-authority-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `ea0d1d41`
Accepted Rust base: `a4dd40d6`
Accepted semantic design: `1a217e2a`
Result: correct only the loading proof authority that predates the accepted
core observed-package-load consumer, then resume the same implementation.

## Exact docs authority and retained candidate

Write exactly:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
2. this manifest: <=180 net;
3. `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`:
   <=120 net; and
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs net <=370 against `ea0d1d41`. The existing dirty
`runtime/{dice.rs,tests/build_command_tests.rs}` candidate is retained but is
not writable during this design. STOP all Rust, Cargo, BUILD, fixture, oracle,
export, caller and public-behavior changes.

## Formal REPLAN and frozen correction

The accepted loading proof at
`app/slug_loading_v2/src/host_package_load_tests.rs` still asserts that core
does not name `RepositoryPackageLoadObservationKey`. The accepted external
singleton design requires the opposite: the observed core branch must consume
the accepted observed package-load carrier before target-kind classification.
Full `slug_loading_v2` therefore fails 137/138 on a current-packet regression,
not an inherited baseline. The proof file is outside the active two-file Rust
authority, so implementation must stop rather than hide the type, weaken the
observed family, or edit an unauthorized file.

Freeze exactly one future proof correction in
`app/slug_loading_v2/src/host_package_load_tests.rs`: replace

```rust
assert!(!core.contains("RepositoryPackageLoadObservationKey"));
```

with

```rust
assert!(core.contains("RepositoryPackageLoadObservationKey"));
```

Keep the adjacent positive query assertion and every other loading proof byte
unchanged. This line records the now-accepted two public consumers; the
retained core integration proof remains the discriminating authority for
external-only observed activation, matching-family selection, later-child
suppression and zero query/multi-build/one-shot activation. The correction is
test-only, line-neutral and semantic-neutral.

## Retry authority, caps and compatibility

After independent design ACCEPT, schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry` with the
original two-file Rust authority plus only the exact loading assertion above:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=260 production net and <=11,220
   physical;
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=360 test net
   and <=3,350 physical; and
3. `app/slug_loading_v2/src/host_package_load_tests.rs`: zero net and <=3,439
   physical.

Aggregate semantic remains <=620; combined physical becomes <=18,009. Preserve
the complete accepted `1a217e2a` owner, admission, route/package/revision/source
order, prefix/certificate, repository-selection, event, family, memory,
compatibility, proof and STOP contract. Exact public values/errors/events and
legacy behavior remain exact. The observed sibling/carrier/certificate
association remains Slug-native. Multi-build, one-shot, broader actions,
external globs and identity bytes remain unsupported/deferred.

Require the focused external public test, 33/33 build-command group, full
loading 138/138, full core with only the recorded stale visibility-wording
baseline, formatting/diff, exact caps, retention/cleanup and independent final
review. STOP on any other relocated/loading byte, production redesign, new
owner/state/event, behavior/family/order drift, cap excess, new failure or M1
closure. REPLAN again if the exact assertion is insufficient. After retry
ACCEPT return only to one docs-only remaining M1 owner audit.
