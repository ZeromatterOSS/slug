# Current Slug V2 Packet

Packet: `WP-6-7A-post-owner-context-bootstrap-closure-owner-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Accepted implementation: `cb5073e0`
Result: audit the remaining bootstrap closure after accepting the immutable
configured-action owner, then select exactly one smallest next design or
evidence prerequisite without changing Rust.

## Exact authority and caps

Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`: <=180 net;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`:
   <=160 net; and
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs cap <=410 net. Rust, tests, fixtures, oracle outputs, Cargo/BUILD
metadata and every other plan are read-only.

## Accepted completion input

`cb5073e0` completes the first immutable analysis-owned configured-action row.
It moves intrinsic `ActionSpec` values into one configured-action slice, shares
one compact owner context per group, records explicit `SelectedToolchain`,
`SelectedPlatformOnly` and `UnresolvedDefault` states, and makes FileWrite
identity/aquery/REAPI consumers borrow the retained row rather than reconstruct
platform state from topology. Platform analysis remains matching-family and
precedes implementation/rule evaluation. No new DICE key, retained map, Host
read, lock, task, scanner or public named-group surface was added.

Measured accounting against `51127df8` is +397 production, +538 test and +935
aggregate semantic lines; physical size is 24,807 across the exact eleven-file
authority. Full analysis passes 4 library, 11 configured-target, 10 root,
21 Starlark-rule and 4 toolchain tests. The affected core and REAPI suites keep
only the already recorded inherited core baselines; workspace check, fmt,
diff-check, cap accounting, Buck2-retention/AI-cleanup and independent review
pass.

## Read-only audit

Trace only enough live code, accepted Stage 4-8 evidence, and the exact Stage
10 bootstrap closure to rank the remaining M7A owners:

- repository sources and the external rules_rust/provider/toolchain graph;
- bootstrap-required rule/provider semantics and toolchain registration/
  selection not already covered by the immutable row;
- required action kinds, Args/paramfiles, tools, runfiles and input-tree
  construction beyond bounded FileWrite;
- the corresponding normalized aquery shapes; and
- REAPI command/input-root, execution, cache and materialization behavior
  required by Stage 10.3/10.4.

For each candidate, identify the existing semantic owner, DICE dependency and
caller order, retained value/identity, error/Need/event boundary, exact Bazel
9.2 evidence, memory lifetime, and whether accepted lower producers are already
complete. Inspect one-shot or compatibility adapters only to prove they do not
own a second semantic path. Do not choose an umbrella packet merely because
several later consumers share the bootstrap closure.

The audit must return exactly one terminal:

1. one bounded docs-only design for the uniquely smallest complete natural
   owner, with measured future Rust/test allowlist, caps, compatibility and
   proof matrix;
2. one uniquely smaller just-in-time Bazel 9.2 evidence prerequisite when live
   ownership cannot be frozen discriminatingly from accepted evidence; or
3. formal `REPLAN` with the smallest missing owner/evidence boundary.

Any implementation requires an independently accepted design and may have at
most one immediate successor. M7A remains partial during this audit.

## Compatibility and STOP

Exact: accepted FileWrite/action declaration order, configured ownership,
default selected platform/toolchain/property semantics, diagnostics, text
aquery and REAPI wire behavior.

Slug-native: immutable configured-action/context rows, explicit execution
states, compact Arc sharing, structural configuration/path/action identity and
the private named-group representation.

Unsupported/deferred until selected by this audit: public named exec groups,
applied-aspect actions, broader action/rules_rust/input-tree semantics,
bootstrap aquery/REAPI execution breadth, one-shot snapshot migration, M7B
run/test/BEP breadth, and exact Bazel configuration/ActionKey bytes in M9.

STOP on Rust/tests/fixtures/oracles, generated evidence, direct implementation,
reopening the accepted owner for uniformity, a second action/publication owner,
Java/JVM delegation, bootstrap-only manifests or execution paths, M7A closure,
M8/M7B/M9 activation, cap excess, multiple successors, or a nondiscriminating
owner choice. Preserve the ordering M7A -> M8 -> M7B.
