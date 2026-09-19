# Current Slug V2 Work Packet

Packet: WP-7-36-m7a-generated-action-prerequisites-r1
Status: accepted

## Outcome and demand

A consumer selected from the validated configured action closure resolves each
retained generated File/Directory input to its declared producer and returns a
stable prerequisite-first action plan. A tiny public rule's source -> generated
file/tree -> consumer chain is reachable; missing owners/outputs, wrong kinds,
cycles and unsupported action/input shapes reject the complete plan.

Demanded by: bootstrap-readiness's LALRPOP and cache-leaf proto build-script out_dir
inputs to downstream Rustc. WP-7-35 accepted verified tree outputs at 351486c8c;
Core source staging still rejects Derived artifacts. Resolving the actual producer
is necessary before generated content can be staged or actions scheduled. This
packet implements that owner-preserving prerequisite, not a generated-byte or
execution claim. Source staging and SourceActionTransport remain source-only.

Pinned source: Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a,
ArtifactFunction.compute/getGeneratingActionKey and ActionExecutionFunction's
collectInputs/getInputDepKeys before execution; Actions sharing remains the
accepted scalar FileWrite closure contract. Exact: resolve retained artifact
owner/output identity and require prerequisites. Slug-native: existing configured
identity, deterministic planning order and error rendering. Unsupported/deferred:
execution scheduling/results, generated CAS binding, tree expansion, discovered
inputs, symlinks/runfiles, publication, CLI activation, bootstrap and exact ActionKey.

## Ownership and algorithm

ValidatedActionClosure remains the sole producer of conflict-free owner-complete
configured actions. Add a borrowed planning projection on BuildCommandEvaluation
for one selected owner/action. Its result borrows that evaluation and exposes only
ordered configured actions plus their direct producer dependencies and declared
artifact inputs; it conveys no Execute authority. No new DICE key/cache or retained
semantic fact: the projection is derived solely from the already DICE-owned closure,
like existing FileWrite semantic views. No filesystem, repository, source digest,
remote cache or request mutation occurs. Edit/restoration uses existing configured
DICE dependencies and native acceptance; a held plan remains a view of that result,
never permission to execute it later.

Build phase-local compact indices for exact retained artifact owner plus full
ActionOutput (path AND kind); use the same Analysis-owned configured-key projection
as artifact production, including execution-platform preference. Never infer a
producer from execution path alone or match a prefix/descendant of a tree. Preserve
source artifact leaves without content observation. Keep whole-tree inputs as one
artifact edge; no traversal/expansion of runtime tree contents.

Typed Spawn ordinary inputs admit whole-tree Directory artifacts in direct lists
and top-level depsets, matching pinned StarlarkActionFactory Artifact inputs.
Tools/executable restrictions and the legacy helper remain unchanged; no runtime
tree expansion is admitted. Reachable supported actions are typed Spawn and the accepted scalar FileWrite
shape. Spawn input discovery visits inputs, tools and the artifact/FilesToRun
executable, preserving stable first occurrence. Reject unused-input pruning,
runfiles support, raw executable strings/shells, unadmitted derived kinds and every
reachable unsupported action before returning a plan. Reuse the source-staging
collector by separating its declared-artifact collection from its source-only
restriction; the old source-only guard remains effective.

Resolve duplicate scalar FileWrite actions through the validated closure's existing
sharing relation: one canonical execution action per physical output/configuration,
while exact artifact lookup first proves the requested declared owner and output.
Do not conflate declared owners across configurations or toolchain execution-platform
preferences; physical sharing occurs only through the closure's admitted relation.
Index construction must reject ambiguous retained ownership rather than choose one.

Iterative DFS (no call-stack recursion) follows retained input order, deduplicates
shared prerequisites, detects visiting-state cycles including self-reference, and
returns prerequisite-first indices. Scratch indices/DFS state and result vectors
are projection-owned and freed on error/drop; actions borrow the retained closure.
Declared artifact leaves are cloned into projection-owned inputs by the existing
collector, without copying depset graphs. Use existing compact SmallMap/SmallSet
utilities (Stage 9 retained-utility disposition); no new interner,
DICE locks, global state, Rust dependencies or bytes in DICE. Complexity is linear
in closure output index size plus reachable declared inputs/actions, apart from
existing compact-map and closure execution-representative lookup costs. No per-edge
rescan of the full closure.

## Scope and evidence

Allowlist: Core runtime/configured_action_closure.rs, new action_prerequisites.rs
and focused tests, runtime/dice.rs/mod.rs delegation/exports, source_staging.rs
collector extraction; Analysis key.rs minimal public projection method reusing
analysis_value::analysis_configured_key and starlark_rule.rs typed input admission. Canonical/current manifest, Stage 6/7
owner paragraphs and bootstrap-readiness status correction only.

Tests: pure diamond/shared producer, File and Directory identity, same path in
different configurations, missing owner/output and wrong-kind, unsupported
reachable action, self/two-action cycle, shared FileWrite representative. Public
BUILD/defs native evaluation proves a multi-action generated chain and same-DICE
producer/input edit/restoration; original source staging still rejects its Derived
input. Reuse accepted root-conflict/lifecycle evidence, run focused affected
conflict and source-staging controls. No transport/Bazel/build action invocation.
Pinned no-run preparation separately capped at 60s, exact selector preflight;
Core tests and direct REAPI/CLI compilation; rustfmt/diff/plan/archive checks.
Tests expected under a few seconds; none requiring >30s are planned. Receipts
in target/wp736. Independent design and final review before atomic checkpoint.

REPLAN if resolving ownership requires a new semantic identity, a producer outside
the validated closure must be synthesized, or this projection is used to authorize
execution/publication. M7A remains partial and M8 unproved. Future generated-input
execution must bind verified producer results and whole-tree CAS manifests within
one native request and revalidate the full source frontier.

## Acceptance receipt

Baseline 351486c8c; review/wp736-generated-prerequisites. The intervening
instruction-only commit 8b043b0e8 implements the user's explicit request for more
parallel agents and is already pushed. Independent design/correction/final review
ACCEPT covers this planning boundary only. No runtime generated-input admission.

Eight exact Core selectors pass: four pure prerequisite/ownership/tool/cycle gates,
one public native list/depset/generated-chain A/B/A plus tool/executable negatives,
one protected source-staging gate and two protected conflict/sharing gates. The
native final selector takes 0.363s; the longest focused group takes 0.854s. Passing
unchanged selectors are reused from earlier correction runs. No build action,
backend, network, Bazel oracle or broad suite was run for this packet.

Pinned nightly-2025-09-14 no-run compile and CLI dependency-chain check pass,
covering Analysis/Core/REAPI/server consumers. Six separate preparations including
two corrected compiler failures total 71.257s; longest 25.341s, under the 60s
preparation cap. Exact selection/runtime/compile receipts live in target/wp736
(focused.receipt, focused-final.receipt, native-final.receipt and no-run JSON).
Rustfmt, diff, plan and archive checks pass. Packet wall/review time was not
continuously recorded; parallel test/review work overlapped root implementation.

Corrected failures: test contexts must be unique per execution group; typed Spawn
ordinary inputs needed the reviewed directory admission; SmallMap indexing and a
test snapshot type annotation required compiler corrections. Negative Analysis
checks use the existing observed preparation API: the general build wrapper
surfaced a dirty activation-closure error instead of the semantic rejection after
the mutation sequence. That wrapper's error-reporting lifecycle is not repaired or
claimed here. Observed preparation rejects directory tools/executables exactly,
and all Derived inputs remain rejected by source staging.
