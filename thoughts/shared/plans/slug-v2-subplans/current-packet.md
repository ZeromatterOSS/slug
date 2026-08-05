# Current Slug V2 Packet

Packet: `WP-6-m2-action-query-identity-boundary-design`
Milestone: M2 configured action-query handoff
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: adjudicate the remaining identity prerequisites after the accepted root
action closure, without authorizing implementation.
Predecessor: accepted root toolchain context `1533569f`, internal semantic
string-setting configurations `dfc1705e`, retained recursive action closure
`afd2a606`, and the existing Bazel 9.2 cquery/aquery evidence.

This is a documentation-only design packet. Reconcile the accepted evidence
with the live owners for all four facts required by an exact action-query
handoff:

- authoritative Bazel `BuildOptions` / configured-target identity;
- configured artifact paths;
- selected execution-platform identity on each action; and
- Bazel ActionKey identity, distinct from a REAPI execution digest.

Decide whether one future atomic vertical can own all four facts and the
`aquery` consumer without invented semantics. If it cannot, return `REPLAN`
and name the exact serial prerequisite chain, ownership, invalidation,
equality, evidence, and stop gates. The accepted action closure already owns
recursive reachability and target-local actions; do not redesign it.

Documentation allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- scheduling synchronization in this manifest and the canonical plan
- one terminal routing-log row after review

Cap the design record at 340 formatted documentation lines. No production,
test, fixture, oracle-record, dependency, or generated-file change is
authorized.

Stop on any Rust implementation; a new DICE key/cache/global/lock/interner;
partial or hard-coded configuration checksums; exposure of `first-build`;
configuration/aquery formatting or command wire changes; configured output
paths; platform or action-key implementation; reuse of REAPI digests as Bazel
ActionKeys; execution, scheduling, or cache behavior; general flag/transition
diagnostics; V1/Buck configuration semantics; or any claim not supported by
the accepted Bazel 9.2 evidence and pinned source.
