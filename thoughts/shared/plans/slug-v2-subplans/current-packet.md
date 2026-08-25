# Current Slug V2 Packet

Packet: `WP-6-7A-generated-repository-route-capability-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: design accepted 2026-08-24 / Rust `846ef196`

Result: implement the doc-hidden `RootRepositorySource::Generated` route
capability so core's accepted generated-route view can construct a public
`RootRepositoryRoute` and drive the existing routed bzlmod package owners
unchanged. Linux under WSL is the only platform target.

## Active implementation contract

Design record: "Generated-repository package-policy-lookup design accepted
(2026-08-24)" in the Stage 6 owner plan.

Change only:

1. `app/slug_bzlmod_v2/src/host_module.rs`:
   - add `RootRepositorySource::Generated { repo_spec, local_path_policy }`
     with the same derives/manual impls as the existing variants;
   - add one `#[doc(hidden)]` constructor restricted to nonroot apparent and
     canonical names, non-`bazel_tools` canonical, and
     `HostRepositoryLocalPathPolicy::LocalUnsupported`;
   - extend `source_capability()` and every exhaustive match (hash/debug/eq
     arms) with the Generated arm delegating to the existing
     `from_repo_spec` path; and
   - preserve exact Display/identity of existing routes.
2. One test-only proof in
   `app/slug_core_v2/src/runtime/generated_repository_definition.rs`'s test
   module proving that a Generated-view-shaped route constructs, hashes,
   displays, produces its source capability, and names/drives the existing
   routed keys (`HostRouteRepoFileKey`, `ExternalRepositoryPackageLookupKey`)
   in nonexecuted type/function checks — without activating any production
   caller or compute edge.

Everything else is frozen: no third file, no driver edit beyond the capability
match arm, no new lookup key family, no public activation, no Cargo/BUILD,
fixture or oracle change. Existing builtin/direct-local behavior remains
byte-exact.

Caps against `846ef196`: <=60 production, <=80 proof, <=140 aggregate.
Add no `rustfmt::skip`.

## Compatibility classes

Legacy routed semantic values/errors/order/equality remain exact. The observed
Result-Arc+epoch association, opaque handoff and the new doc-hidden variant
remain Slug-native. Public publication, bootstrap activation, other platforms
and exact identity bytes remain unsupported/deferred.

## Validation

Serial on Ubuntu 24.04 WSL: the new sibling proof; focused routed repo-file/
ignore/lookup suites (`slug_bzlmod_v2 --lib`); protected root package suites;
full Bzlmod; full core with only the byte-identical accepted query diagnostic
baseline; separate runtime with only the accepted
`PathObservationEpochKey`/configured-analysis-Needs failure; direct commands
check; formatting; exact two-file allowlist/accounting/cap/no-skip gates;
`git diff --check`.

STOP a third file, production caller/compute-edge activation, semantic/event/
equality drift, private exposure beyond the named variant/constructor, fixture/
oracle growth, cap or format waiver, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or baseline drift.

After ACCEPT, return to the Stage 6 owner plan for scheduling; M7 remains
partial and M7A -> M8 -> M7B remains.
