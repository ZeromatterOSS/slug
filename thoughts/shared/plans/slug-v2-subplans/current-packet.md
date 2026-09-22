# Current Slug V2 Work Packet

Packet: WP-7-77-m7a-workspace-status-artifacts-r1
Status: accepted

## Observable outcome and compatibility

Give real configured rule contexts immutable ctx.info_file / ctx.version_file
Files from one workspace-global DICE descriptor owner. Preserve status provenance,
shared root, owner=None, stable/volatile identity and metadata policy through
provider publication, target/exec and main/external contexts, freeze and GC.
These are descriptors, NOT produced status bytes or registered rule-owned actions.
They unblock the next native build-info transform registration owner.

Exact NOOP-path File fields: root.path bazel-out; basename/short_path
stable-status.txt or volatile-status.txt; path bazel-out/name; dirname bazel-out;
extension txt; owner None; is_source/is_directory/is_symlink false. Stable is
ordinary derived metadata, volatile constant metadata; the latter is retained
internal policy, never a fabricated content digest. Live-ctx retrieval guards,
immutable captured File lifetime and native structural equality/hash are admitted.
The existing File facade also gains the pinned is_symlink predicate for source
and configured Files (only declared Symlink outputs are true), not a status-only
attribute. DefaultInfo.files may publish status descriptors without declaring
them as rule-owned outputs or granting execution authority.
Slug-native: workspace-scoped structural identity, diagnostics and output naming
domain (matching these path strings does not claim Bazel configuration identity).
Unsupported/deferred: actual workspace-status producer action/bytes, status command,
host/user/time/environment observation, volatile refresh/cache policy, transform
callbacks and template actions, bare-provider rule returns, installed module tools,
full cc_toolchain and execution. Every executable/materialization route for status
Files must reject explicitly; no fake Source label, per-rule FileWrite or empty
status fallback. M7A partial; M8 unproved.

WP776 at614594c31 accepted native template providers with86 tests and Core/CLI
checks; its complete receipt remains in Git. Authentic bazel_cc_build_info.bzl
unconditionally registers stable/volatile transforms and redacted Symlink even
under nostamp. Its callbacks and genuine global inputs cannot be replaced by
eager/default content. This foundational owner is required before registration.

## Source and natural owner

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
SkyframeExecutor.java1473–1496; BlazeDirectories.java223–233,257–258;
WorkspaceStatusValue.java37–43,55–73; CachingAnalysisEnvironment.java349–364;
StarlarkRuleContext.java995–1003; Artifact.java294–308,400–406,435–471,
563–564,618–666,866–894,1251. Pinned stamping_test.sh32–46 supplies path
regressions; StarlarkRuleContextTest.java4773–4822 supplies metadata distinction.
execution_phase_tests.sh116–144 is deferred execution/cache evidence, not proved
by descriptors. No new oracle corpus/body or Java probe required.
General File.is_symlink is pinned to FileApi.java100–106 and retains source,
regular File, Directory and declared Symlink discrimination. DICE's existing
dependency/publication rules are inherited from docs/developers/dice.md; this
key has no mutable dependencies or injected semantic value.

BuildAPI owns WorkspaceStatusRoot over existing NormalizedAbsolutePath and
WorkspaceStatusArtifact(kind Stable/Volatile), plus the immutable descriptor pair.
Add distinct AnalysisArtifact and AnalysisArtifactRoot variants; do not relax
configured DerivedArtifactRoot or ActionOutput invariants. Equality/hash includes
normalized workspace and role, never displayed path alone or requesting config.
Analysis WorkspaceStatusArtifactsKey is keyed only by normalized workspace and
computes shared Arc<WorkspaceStatusArtifacts>. This pure descriptor computation
does no IO, injection, clock/env read or status-content inference; no dependency
can change these workspace-constant names/policies. Existing configured analysis
computes that key before synchronous rule evaluation and retains its result.
No locks, counters, global caches or alternate discovery paths. Future execution
must add real tracked producer inputs separately; this key cannot certify bytes.

Loading reuses the existing native File/root facades; owner/label are None, and
status Files have no rule declaration authority. Analysis guards getter access
with its existing context token, but captured values contain no token/heap borrow.
Existing full artifact publication carries complete workspace/role semantics.
DICE equality cutoff, cancellation, graph lifetime and Arc ownership bound release.
No mutable retained state; string projections and validation walks are scratch.

## Utility and dependency decision

Reuse Arc/Dupe/Allocative and the existing normalized Arc<PathBuf> workspace
primitive, preserving OS-native/non-Unicode identity. Explicitly add acyclic
BuildAPI -> slug_workspace_v2 internal dependency (Cargo and Bazel metadata),
rather than duplicate path normalization or weaken its type via opaque IDs.
This brings an already-used lower DICE-linked crate into BuildAPI's dependency
closure, but no new external dependency or DICE computation into BuildAPI.
Stage9 existing shared workspace/observation and structural-artifact dispositions
apply. No donor semantic code, performance or memory-reduction claim.

## Scope and routing

Base614594c31; codex/wp777-workspace-status-review. Independent design checkpoint
before edits, actual-diff/evidence final review before acceptance.
Worker1:
- app/slug_build_api_v2/src/workspace_status.rs + child tests; lib.rs;
  analysis_value.rs, artifact_root.rs; actions/runfiles_support.rs,
  actions/cpp_compile/input_sets.rs, runfiles/manifest.rs narrowly reject status
  at unadmitted consumer boundaries.
- app/slug_build_api_v2/Cargo.toml and BUILD.bazel internal dependency;
  root owns mechanical Cargo.lock update.
- app/slug_loading_v2/src/subrule_invocation.rs File/root wiring and
  subrule_invocation/workspace_status_tests.rs; artifact_root.rs.
Worker2:
- app/slug_analysis_v2/src/workspace_status.rs + child tests; lib.rs;
  dice.rs compute/pass-through only; starlark_rule.rs context getter/field/freeze;
  starlark_rule/context_lifecycle_tests.rs and cargo_runfiles_tests.rs context
  initialization; analysis_value.rs test hook and
  analysis_value/workspace_status_tests.rs.
Root:
- Analysis tests/workspace_status.rs and tests/starlark_rule.rs hook.
- Core runtime/action_prerequisites.rs plus child tests,
  action_prerequisites/symlink.rs, source_input.rs plus existing child tests,
  source_staging/tests.rs pure descriptor refusal regression,
  dice.rs narrow output-only branch, configured_output.rs,
  runfiles_manifest.rs plus existing child tests,
  action_chain_staging/runfiles.rs and symlink.rs;
  REAPI action_chain/binding.rs explicit refusal and focused tests.
- REAPI Cargo.toml dev-only workspace dependency for the direct no-RPC binding
  rejection test; no new production dependency. Existing Bazel target does not
  build library unit tests, so no production Bazel edge is added for this test.
- Cargo.lock mechanical internal edge; Stage6/Stage9/bootstrap/scheduling.
Any additional file requires root reassessment. Large parents receive only closed
variant/wiring/refusal branches; new ownership is in children. Estimate350–650
production and400–800 test lines, no copied fixture trees.

## Gates

Prove descriptor sharing within one DICE across main/external, targets/configs and
target/exec; different workspaces remain unequal despite equal paths; stable vs
volatile and internal metadata differ; all public File/root projections and
native dict/depset equality/hash; real provider forwarding and configuration A/B/A.
Key repeat/normalized-path equivalence and separate workspace tests must use real
DICE, not an injected pair. Lifecycle tests distinguish expired ctx retrieval
from captured File survival, lower/rematerialize/source-heap disposal and GC.
Refuse status as source input, executable action prerequisite, runfiles/source
materialization, REAPI input, declared rule output and unsupported Cpp discovery
input. Verify failures precede writes/RPCs; ordinary source/derived gates remain.

Serial freshly-resolved pinned nightly offline/locked Cargo JSON no-run for
BuildAPI/Loading/Analysis/Core and focused REAPI as needed, exact batched
nonignored preflight plus actual counts. Protect accepted root/File/action-input
and lifecycle tests, WP776 native/helper proof, and source/prerequisite/runfiles
rejections. CLI direct-dependent check includes all exhaustive match consumers.
No daemon/Bazel/compiler action/network required. Preparation60s/operation;
runtimes expected seconds, not user deadlines. Rustfmt/diff/archive/status checks.
REPLAN if descriptor identity requires actual mutable status inputs or retained
lifetime/execution integrity cannot remain separate; ordinary scoped corrections
are not REPLAN. Do not claim the real status producer or transform gate completed.

## Validation receipt

Independent design and final reviews ACCEPT. Final review completed04:18Z;
packet branch started03:58:37Z (~20min elapsed, including overlapping review).
Review wall time was not separately instrumented. No REPLAN required.

2026-09-22, base614594c31. Freshly resolved nightly-2025-09-14; serial
offline/locked Cargo JSON no-run, separate exact nonignored preflight and actual
execution. All commands exit0 after the documented test-only corrections.
87/87 final selected tests, summed runtime2.265s:

| Owner | Selected / passed | Receipt under target/wp777 | Runtime |
|---|---:|---|---:|
| BuildAPI |16/16|tests-1790050141783457875.receipt.json|0.067s|
| Loading |15/15|tests-1790050199954281106.receipt.json|0.061s|
| Analysis library |7/7|tests-1790050246772747396.receipt.json|0.021s|
| Analysis integration |18/18|tests-1790050510464319813.receipt.json|1.653s|
| Core |27/27|tests-1790050385127411937.receipt.json|0.443s|
| REAPI |4/4|tests-1790050448799197606.receipt.json|0.020s|

Final no-run receipts: BuildAPI1 6.886s; Loading2 4.122s; Analysis2
(library+integration)8.834s and3 (formatted integration)6.379s; Core2 31.128s;
REAPI1 52.932s. CLI check1 exit0 20.145s includes Core, REAPI, Commands,
Query and Server. Nine compile attempts totaled218.362s; no operation exceeded
60s. Initial Analysis integration lacked an import; initial Core test fixture
needed the new exhaustive variant. Loading test top-level conditions were moved
inside functions before final replay. No production redesign or waived failure.
The earlier integration14 replay is superseded by18, not counted twice.

Source review and semantic tests cover every named gate: real DICE pointer
sharing, full workspace/role identity including non-Unicode paths, public File
fields/hash/depsets, main/external target/exec and config A/B/A, DefaultInfo
forwarding, expired ctx versus captured File lifetime, heap disposal/freeze/GC,
and refusal before source observation, execution, materialization or RPC.
Protected genuine roots, template providers and unchanged toolchain helper
construction/publication remain green. No oracle corpus growth, daemon, Bazel,
compiler action, CLI invocation or network replay. Rustfmt, diff, archive and
plan-status checks pass. Skill-guided Arc/Dupe/Allocative reuse adds no interner,
cache, lock or evaluator borrow. No performance claim.

Observable gate advanced: genuine workspace-global status descriptors in native
ctx. Remaining: real tracked status producer/bytes, constant-metadata execution
policy, authenticated transform callback ownership and template actions, bare
provider returns and installed module tools. M7A remains partial; M8 unproved.
