# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-load-bridge-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: bridge design accepted 2026-08-24 / Rust `b42b004c`

Result: implement one same-crate core bridge child so the external
exported-source build branch loads packages from extension-generated
repositories, with byte-exact preservation of all existing diagnostics.
Linux under WSL is the only platform target.

## Active implementation contract

Design record: "Generated-package load bridge design accepted (2026-08-24)" in
the Stage 6 owner plan.

Change only:

1. `app/slug_core_v2/src/runtime/dice.rs`:
   - add private `GeneratedPackageRouteKey` +
     `GeneratedPackageRouteObservationKey` (root apparent rejection at
     construction; Display `generated-package-route:{workspace}:@{apparent}`
     and observed prefix), with typed semantic error kinds
     Missing / ContextMismatch / Definition / Compute and no private-inner
     exposure;
   - driver: canonical-apparent-mapping child (context root) ->
     root-apparent-definition child -> require Generated view with RepoSpec and
     LocalUnsupported -> construct via `for_generated_repo_spec`; Need
     immediate/carrierless; left-first epoch union; eventless parent; one
     local Result Arc + compact epoch retained;
   - fallback polarity in `drive_external_exported_source_build_branch`: the
     public route child runs first; only Unknown/Unsupported kinds (legacy
     error kind or observed outer result) fall back to the bridge; all other
     terminals keep today's exact bytes; bridge failure yields new typed
     `BuildCommandErrorKind::GeneratedRoute`; downstream children unchanged.

Everything else frozen: no query activation, no public export, no second key
family, no direct-local/builtin drift, no Cargo/BUILD or fixture change.
A new private sibling module for the bridge is permitted only if dice.rs size
demands it — record which choice was made.

Caps against `b42b004c`: <=120 production, <=240 proof, <=360 aggregate. No
`rustfmt::skip`.

## Compatibility classes

Legacy build semantics exact. Slug-native: observed carrier/epoch association,
doc-hidden route construction, new private key family. Unsupported/deferred:
query-path publication, other platforms, exact identity bytes.

## Validation

Serial on Ubuntu 24.04 WSL: new bridge proofs (rejection polarity,
mapping-Missing, definition-error pass-through, Generated success, fallback
byte-exactness for direct-local/builtin/unknown); protected external-build
suites; full core with only the accepted query diagnostic baseline; separate
runtime with only the accepted PathObservationEpochKey baseline; Bzlmod full
green; direct commands; formatting; allowlist/accounting/caps/no-skip;
diff hygiene.

STOP a third owner/key family, query/public activation, private-inner exposure
beyond pub(super), semantic/event/equality/lifecycle drift, fixture growth
without demonstrated gap, cap/format waiver, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or baseline drift.

After ACCEPT, return to the Stage 6 owner plan for scheduling; M7 remains
partial and M7A -> M8 -> M7B remains.
