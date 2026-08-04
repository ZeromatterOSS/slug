# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-package-preflight-helper-refactor`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private one-file behavior-preserving preflight extraction
Evidence: accepted package horizon in `1d5edc7c`; accepted cycle-boundary design
in the owner plan; and all five existing `direct_include_horizon` regressions.
Add no oracle or fixture.

Edit exactly `app/slug_bzlmod_v2/src/source_preparation.rs`. The formatted net
addition may not exceed **20 production lines, 12 test lines, or 32 total
lines**. Add one private, non-key helper with this ownership shape:

```text
preflight_direct_local_include_package_horizon(
    ctx: &mut DiceComputations,
    route: RootRepositoryRoute,
    requests: &[NonrootIncludeRequest],
) -> <DirectLocalIncludePackageHorizonKey as Key>::Value
```

`DirectLocalIncludePackageHorizonKey::compute` remains the sole owner of
`DirectLocalModuleInspectionKey`. Preserve its exact key identity, inspection
Need forwarding, typed inspection/inspection-compute mapping, absent-as-empty
request projection, and complete-only equality/validity. After deriving the
accepted route and request slice, it awaits the helper.

Move unchanged into the helper: parse every request before package work; retain
raw label/`LogicalSpan`; rebind package identity to `route.canonical_repo()`;
deduplicate only first-seen package dependencies; request the complete
`ExternalRepositoryPackageLookupKey` group; preserve lookup-compute failures;
and call the existing source-order mixed terminal/Need finisher. The helper
returns the same route plus ordered occurrence value and owns no event data.

Tighten only the existing structural test to prove the private helper exists and
the accepted key calls it. The existing five focused tests remain the semantic
evidence for parse-before-lookup, dependency dedupe, occurrence retention,
typed failures and source chains, multi-kind Need union, mixed source order,
lifecycle/pruning, and policy/horizon event ownership.

Stops: no second DICE key, type alias, carrier, error, cache boundary, route or
inspection recomputation, cycle detection or unsupported result, closure owner,
nested fragment path/source/bytes, visited set, recursion, evaluator/default/
event/public activation change, key/value/error/equality change, fixture/oracle,
second file, or cap breach. `REPLAN` on any such expansion. Run only formatting,
the focused `direct_include_horizon` library tests, diff/scope/cap gates, and an
independent latest-diff review. Do not run Bazel.
