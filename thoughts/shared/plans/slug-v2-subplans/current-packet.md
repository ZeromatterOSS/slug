# Current Slug V2 Packet

Packet: `WP-4-6-8-bazel-tools-test-closure-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the exact embedded Bazel 9.2 `@bazel_tools//tools/test`
source/package closure and its repository-routing/DICE/configured-edge
ownership before any retained TestRunner action or Test command activation.

## Active prerequisite contract

Audit the Bazel 9.2 embedded-tools manifest/source for the complete
`@bazel_tools//tools/test` package and every transitively referenced file,
BUILD target, load, runfile, and `@platforms` edge required by the admitted
single POSIX Starlark test. Pin verbatim-content provenance and the exact
repository identity/apparent-to-canonical mapping. Design how existing
repository package/source and configured-node DICE owners observe, invalidate,
and expose this closure without synthetic labels, command-owned files, direct
filesystem bypass, or a second graph.

Classify changed behavior as exact, Slug-native, or unsupported/deferred.
Exact claims are limited to verbatim Bazel 9.2 content and source-proven
package/edge semantics. Slug-native repository/configuration/path bytes remain
explicit. TestProvider/TestRunner representation, Test execution/results,
generated-input REAPI, multi-output materialization, BEP, Windows, coverage,
shards/runs, flaky retry, and exact Bazel identity bytes remain deferred.

This packet is design-only. It may edit only:

- canonical and current-packet scheduling;
- owner bookkeeping in Stage 4, Stage 6, and Stage 8;
- Stage 9 only if the reuse/import decision changes; and
- at most one new or existing Bazel-only evidence fixture/source ledger if
  pinned source alone cannot discriminate the closure.

Cap bookkeeping at 300 net lines and optional evidence at six files/500 net
text lines. Add no production or test Rust, Cargo manifest/lock change, DICE
key, dependency, public wire/schema, JVM/Java artifact, Test command behavior,
TestProvider/TestRunner model, REAPI execution/materializer, BEP event,
client/local executor, Stage 10/CI change, or ported V1 test orchestration.

Required proof:

- exact pinned Bazel 9.2 file/package/load/runfile/`@platforms` closure with
  immutable source references and verbatim-content policy;
- one repository-routing/DICE ownership diagram in prose, including equality,
  invalidation, apparent/canonical mapping, missing/wrong-kind/cycle terminals,
  and no lock across a DICE compute;
- explicit configured-edge and compact retained-data decisions, with Buck2/V1
  inspect/adopt/reject status and no weak identity hash;
- a successor allowlist/caps/stops plus focused source/package/edge and
  create/edit/delete/recreate lifecycle validation; and
- source/structure checks, archive active-layout checks, `git diff --check`,
  and independent Sol design review.

Stop and `REPLAN` if the closure requires broader embedded-tools generation,
unavailable upstream content, JVM/Java semantics, unbounded external
repository/glob breadth, or a second semantic graph. One bounded correction is
allowed; a second material miss is `REPLAN`. At `ACCEPT`, schedule only
the reviewed analysis/loading implementation or a smaller named prerequisite,
commit, and continue.
## Predecessor decision record

Historical context only; this section grants no files, actions, scope, or
scheduling authority.

The accepted executable FileWrite Run vertical established client-only launch,
bounded daemon authorization, resolved REAPI materialization, and strict
provider/runfiles/path guards. A subsequent Test handoff audit found that Bazel
9.2 creates a distinct `TestProvider` and non-shareable `TestRunnerAction`;
it is not Run plus a command-owned exit code. That action consumes the
executable, runfiles tree, setup/XML tools, and test environment and owns logs,
XML, cache status, timeout, shard/run, undeclared-output, and infrastructure
failure state. Test result analysis separately owns aggregate status and exit.

Slug loading retains test capability, attributes, and implicit
`@bazel_tools//tools/test` labels, but configured analysis retains only the
rule-declared FileWrite action and built-in providers. The configured-node
boundary fails closed on this external repository until its real routing,
content, and edge topology is owned. Treating labels as content-free tokens
would omit semantic DICE/action-identity inputs; using the Run client or a
direct-local executor would bypass the accepted action/REAPI boundary.

The Test handoff and TestRunner semantic proposals therefore ended `REPLAN`
without fixture or production changes. The active prerequisite above is the
sole current authority.
