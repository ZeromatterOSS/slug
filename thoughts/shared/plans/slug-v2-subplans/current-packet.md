# Current Slug V2 Work Packet

Packet: WP-7-39-m7a-artifact-owned-output-groups-r1
Status: accepted

## Outcome and basis

Retain OutputGroupInfo as owner-bearing artifact depsets and admit its ordinary
Starlark constructor and provider round trips. This removes a concrete prerequisite
for requested-output scheduling: current groups are path-only Depset<String>, the
constructor always rejects, and dependency fields materialize strings. Inferring
producers from those paths would discard owner/configuration/kind. The authentic
rules_rust rust/private/rustc.bzl:2153-2168 constructs these groups for metadata and
other compiler outputs. Baseline 0ec1eb5e7 accepts selected-action execution and
publication; selecting all requested build artifacts remains separate work.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
OutputGroupInfoApi.java:41-52 supplies kwargs-only construction;
OutputGroupInfo.java:575-584 sorts names and converts values through
StarlarkRuleConfiguredTargetUtil.java:227-235 (list/tuple Artifact sequences become
stable-order depsets; existing Artifact depsets retain topology/order). No field
name whitelist or artifact-kind filter exists in this constructor. Empty fields
remain present. OutputGroupInfo.java:284-306,374-391 canonicalizes every field to
the common stable-order empty depset only when ALL groups are empty; mixed groups
retain their supplied empty depsets. OutputGroupInfo.java:329-357,385-391,555-562
owns attr/index/membership/iteration. StructImpl.java:97-131 and
Depset.java:550-560 own provider/field/depset-identity equality and hashing.
Duplicate providers returned by one rule fail (StarlarkRuleConfiguredTargetUtil:
270-274); internal aspect/rule merge is separate and actually unions all equal
names despite its stale comment (OutputGroupInfo.java:182-204).

Exact admitted behavior: kwargs, sequence/depset normalization, artifact typing,
field presence/access, membership/index type behavior, UTF-16-sorted field iteration,
empty normalization, builtin identity and depset identity/sharing preservation.
Existing Slug-native structural configured/artifact identities, Rust representation,
rendered repr/diagnostics and hash bytes remain native. Public dir() inherits the
Starlark runtime's Rust Unicode sorting, distinct from UTF-16 provider iteration;
hash/equality consistency is required. No JVM or source-byte/path inference. Unsupported/deferred: implicit
hidden runfiles groups, transitive validation propagation, internal aspect/group
merge, requested-output scheduling/flags, CLI activation and bootstrap acceptance.
Constructor accepts arbitrary names including _validation_transitive; configured
return of that private override explicitly rejects until its allowlist/propagation
owner exists (RuleConfiguredTargetBuilder.java:367-381). _validation can be retained
as declared data but does not claim complete ordinary-build validation semantics.

## Ownership and representation

The existing ConfiguredNodeResult/ProviderCollection remains the DICE semantic owner.
Replace only OutputGroupInfo's path-only groups with checked AnalysisDepset values,
private immutable group storage and accessors. Accept Empty or Artifact element
metadata and every AnalysisArtifact kind, including source, File, Directory,
Symlink and RunfilesTree. Keep the FileDepset alias for unrelated runfiles strings.
Constructor returns Result; prevent unchecked insertion via public mutable fields.
Provide typed builtin lookup and checked ProviderOccurrence conversion so fresh,
forwarded dependency and nested provider values use one builtin identity.

Do not flatten, rebuild, stringify or rebase retained artifact depsets. Reuse the
existing graph-aware AnalysisValueLowerer and materializer and their occurrence
memoization. Preserve source labels, derived owners/configurations/path/kind,
transitive graph topology/order and cross-field/cross-provider sharing. All-empty
canonicalization is the specified exception. OutputGroupInfo publication equality
must use the incoming shared PublicationEqState across all groups, not ordinary
AnalysisDepset occurrence equality or one new state per group. No new DICE key,
interner, side cache or transport state. Reuse existing compact map/Arc/depset
owners under Stage 9; retained metadata belongs to the configured graph, evaluator
values to that evaluator, lowering scratch to the call, with no escaped heap loans.

## Starlark and analysis boundary

Put the fresh/frozen StarlarkOutputGroupInfo value and constructor in a dedicated
loading provider child module; provider.rs gets minimal callable/key wiring.
Construction is also valid during .bzl loading when inputs are valid. Normalize
list/tuple Files immediately through the existing depset builder, reject every
wrong element/value before creating the provider, and preserve supplied depsets.
Validate all fields before all-empty normalization; empty groups cannot hide invalid
values. Fresh and dependency-rematerialized instances share this value class and
attr/index/in/iteration/dir/equality/hash behavior. Missing fields differ from present
empty fields; nonstring membership is false and nonstring indexing errors. Preserve
arbitrary valid-Unicode group names and compare names by UTF-16 for retained field
order and iteration; public dir() retains the inherited native ordering above.

AnalysisValueLowerer recognizes the wrapper both in reachable-depset discovery and
generic provider lowering; the materializer handles builtin occurrences and typed
OutputGroupInfo through the same wrapper and existing depset() path. Rule return
assembly canonicalizes the builtin occurrence into checked typed OutputGroupInfo,
including forwarded providers. User providers merely named OutputGroupInfo stay
separate. Duplicate builtin returns remain errors, never implicit merges. Existing
DefaultInfo, ToolchainInfo and user-provider lowering must remain unchanged.

## Scope and evidence

Allowlist: slug_build_api_v2/src/providers/mod.rs (or cohesive output-group child)
and its existing providers tests; slug_loading_v2/src/provider.rs plus new
provider/output_group.rs and focused child tests, and the one old loading test that
asserted constructor rejection; slug_analysis_v2/src/analysis_value.rs minimal
lowering/materialization wiring and existing typed-group test conversion;
starlark_rule.rs minimal typed return conversion; new focused analysis integration
test child module and minimal module wiring in tests/starlark_rule.rs to reuse its
existing configured/DICE test harness. Existing huge
provider/analysis files gain only delegation, not another embedded value/test owner.
Canonical/current manifest, Stage 6 and bootstrap readiness owner status only.
No action selection, scheduler, REAPI, Core or command/daemon edits.

Independent design/final review required for this retained public provider boundary.
Parallel writers: Build API retained owner/tests; loading evaluator wrapper/tests;
root analysis handoff/integration tests; separate reviewer. Keep edits disjoint and
Cargo preparation serialized. Evidence: lists/tuples/depsets, shared transitive
inputs, all-empty versus mixed-empty orders, arbitrary/reserved/Unicode names,
empty versus missing, wrong values/elements and duplicates, fresh/dependency
operations, source and different configured owners with the same output spelling,
Directory/other artifact-kind retention, forwarded/nested-provider round trips,
user/builtin distinction, publication alias changes and configured A/B/A invalidation.
Reuse source-pinned provider/depset evidence; no new Bazel execution is needed.

Pinned preparation uses no-run Cargo JSON, separately capped at 60s; exact selector
preflight and focused tests expected under a few seconds. Tests exceeding roughly
30s require strict necessity. Compile named direct dependents Core and CLI as
appropriate after Build API/loading/analysis tests; no broad suite or remote probe.
Changed Rust formatting, diff/plan/archive checks, independent final ACCEPT, then
commit/fast-forward main/push using existing authorization. Receipts target/wp739.
Design checkpoint started 2026-09-19 01:50:38 UTC. M7A remains partial; M8 unproved.
REPLAN only for a necessary new semantic owner or contradiction with pinned provider
semantics; ordinary invocation/compiler/test corrections remain in this packet.

## Acceptance receipt

Independent design and final reviews ACCEPT. Checkpoint elapsed approximately
16 minutes from the recorded design start through final review; no REPLAN.

Nineteen unique focused cases pass: Build API 6/6 (0.002s), loading 6/6 (0.010s),
analysis materializer/lowering 4/4 (0.004s), duplicate/private-override and protected
recursive-provider integration 2/2 in the initial 0.153s group, and corrected
configured forwarding/A/B/A 1/1 (0.082s). Source artifacts are checked by the
retained owner, loading constructor and typed materializer. The configured test
always declares both File alternatives plus a Directory and changes only selected
groups; two forwarding edges preserve the original owner, nested builtin identity,
cross-provider depset sharing and A/B/A publication equality. No actions execute.

Initial compile failures were corrected SmallMap indexing in tests and a missing
structural-hash trait bound. Integration fixture corrections separated a shadowed
builtin alias into its own loaded module, replaced unsupported ctx.files access
with declared artifacts, and asserted the exact root-package relative output path.
Those failure receipts remain available; no production provider failure is waived.
The loading selector's old module prefix was corrected before execution. Unchanged
passing checks are reused after fixture-only edits.

Pinned no-run preparations for Build API/loading/analysis and the Core/CLI check
pass. Ten preparation invocations, including corrected compiles, total 162.027s
(maximum 37.481s); these were separate from test runtime and each below the 60s
preparation cap. Exact selector preflights, changed-Rust formatting, diff, plan and
archive checks pass. Receipts are in target/wp739: build-api-focused,
loading-output-groups-r2, analysis-unit, analysis-integration (two passing controls),
analysis-integration-r5 and dependents-check, plus compile/preflight evidence.
Observable gate: typed OutputGroupInfo constructor and configured round trips.
Requested scheduling/implicit groups/CLI activation remain open; M7A partial,
M8 unproved.
