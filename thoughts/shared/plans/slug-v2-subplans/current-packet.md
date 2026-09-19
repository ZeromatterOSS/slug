# Current Slug V2 Work Packet

Packet: WP-7-35-m7a-source-output-trees-r1
Status: accepted

## Outcome, demand and classification

A public authored rule declares a directory output; a source-only typed Spawn
executes through WP-7-34's native operation and returns a verified regular-file
output tree, including nested and empty directories. This advances the build-script
out_dir obligation observed for LALRPOP and REAPI proto generation in bootstrap-
readiness. It does not admit generated/tree inputs, closure scheduling, local
materialization, CLI activation, the compiler closure or bootstrap.

Pinned oracle: Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a,
StarlarkActionFactory.declareDirectory, DefaultInfo and RemoteExecutionService ActionResultMetadata/
parseDirectory and pinned REAPI Directory/Tree/OutputDirectory messages already in
slug_reapi_cache_v2. Read the pinned Git objects, not the sibling checkout's HEAD.
Exact: admitted directory declaration kind/owner, regular-file contents, protocol
size/SHA-256, Directory references and output-path/type matching. Slug-native:
existing configured/artifact path identity and Command wire profile, bounded tree
resource policy. Unsupported: sibling declaration, symlinks/node properties,
root-digest-only directory results, excessive tree resources and generated inputs.
No Bazel ActionDigest/ActionKey parity claim and no Java semantic delegation.

## Declaration owner

Expose ctx.actions.declare_directory(filename) through the existing active analysis
call token and AnalysisActionSink. Explicit None sibling may be accepted; a supplied
File sibling fails closed. The synchronous sink remains the producer: package-
relative validated path plus the existing Derived artifact owner and Directory kind.
A phase-local compact declaration map rejects file/directory conflicts at the same
path and permits repeated same-kind declarations. It is owned by the existing
analysis sink, shared across its subrule calls, dropped with analysis, and never a
new graph cache/key. Expose File.is_directory from the retained kind. DefaultInfo.files
retains File and Directory artifacts; executable and generated-input restrictions
remain unchanged. Declared tree
contents are not available in analysis and no Args directory expansion is admitted.
Existing configured/source observation dependencies own edit/restoration invalidation.

## REAPI ownership and validation

SourceSpawnReapiPlan admits File and Directory output kinds; sorts each legacy field
and the combined output_paths; preserves all current source/virtual input-output
collision checks across both kinds. RunfilesTree/Symlink remain rejected. Closure
conflict validation and Core execution-representative selection remain authoritative.

The shared executor matches exact file/directory result sets and rejects duplicate,
wrong-type, missing or extra outputs and all symlink fields. For each output directory,
require a Tree digest and present Tree.root, fetch through verified CAS reads, decode the Tree and verify
child Directory references by canonical message SHA-256. If root_directory_digest is
supplied it must match the Tree root. Validate component names and unique names across
files/directories; require sorted protocol node lists, reject missing child references,
symlinks and unsupported node properties. Preserve empty directories and repeated
subtree occurrences; identical serialized child directories may deduplicate.
NativeLink includes the root again among children; an identical root copy is
allowed and deduplicated after content/digest comparison, still charged to budgets.
Do not trust is_topologically_sorted as validation. Unreferenced Tree children fail
closed. Bound aggregate wire Tree bytes (16 MiB), aggregate expanded entries including
roots (100,000), aggregate expanded path bytes (16 MiB) per action, and depth (256)
per path. Reject oversized declared digests before download and check remaining
wire budget in the sink. Charge each expanded path before allocation/queueing.
Before prost decoding, scan length-delimited Directory/node/digest records and cap their
aggregate count at 100,000 as well, rejecting unsupported properties/symlinks. This
bounds allocation from many tiny duplicate/empty records before expanded checks.
Reject unmodeled protobuf fields in Tree, Directory, FileNode, DirectoryNode and
Digest before prost can discard them and alter Directory hashing.
Use explicit resource errors rather than stack recursion or unbounded DAG expansion.
These are admitted implementation resource limits, not user test deadlines.

Return explicit directory metadata alongside ActionResult files: Tree/root digests,
regular file paths/digests/executable bits and empty/nested directory topology.
Operation-owned Tree decode buffers are dropped after producing the bounded manifest;
metadata drops on Core retry/unwind or caller drop and none enters DICE. Tree file
blobs use read_blob_verified with a discard sink, so their content is not retained
in output_blobs. Existing standalone output-file buffers remain unchanged; no large-
output readiness claim. The graph-independent cache leaf remains unchanged. All tree
file blobs are hash/size verified before native acceptance; no file is written locally. Preserve regular file metadata in both
standalone and tree outputs. Existing cache validation helpers must not falsely
report a directory-bearing result as complete from a files-only inventory.
The existing detached materialize_outputs must fail before writes on directory
results; FileWrite behavior stays protected. No new local publication API.

## Scope, proof and release boundary

Allowlist: build_api providers/mod.rs and tests/providers.rs for retained directory
files; loading subrule_invocation.rs and focused existing action-sink tests;
analysis starlark_rule.rs and focused declaration tests; REAPI source_spawn.rs/tests,
command.rs, executor.rs, cas.rs, action_cache.rs, lib.rs, new output_tree.rs/tests,
source_execution/tests.rs and focused integration tests. Minimal direct-consumer
struct/API corrections only if the explicit result type requires them. Canonical,
manifest and Stage 7/6 owner paragraphs. Prefer existing dependencies and fixtures.
Large analysis/loading files gain only delegation and bounded declaration ownership;
Tree validation lives in a separate module. No CLI/server activation or scheduler.

Reuse WP-7-34's source/certificate mutation, cancellation, revision retry and events
proof; this packet changes output decoding, not the native lifecycle. New tests use
one tiny authored rule/script and public Core operation: File.is_directory and owner/
path, repeated declarations/type conflicts, invalid paths/sibling, cold Execute and
AC replay with nested/empty tree contents and exact bytes/digests; source A/B/A
restores action identity. Pure decoder negatives cover corrupt/missing Tree or file
bytes, malformed names, duplicates/order, unresolved directory references, root
mismatch, symlink/properties, extra/wrong result types and resource limits. No compiler
action, copied ruleset, broad suite or fresh Bazel run. Reuse pinned source evidence.
Protect FileWrite wire and regular source execution. All runtime gates expected
under a few seconds; >~30 seconds requires strict necessity and none are planned.
Pinned compile preparation separately capped at 60 seconds with no-run JSON, exact
selector preflight, direct CLI/Core/REAPI/server compile coverage, format/diff and
plan/archive checks. Receipts target/wp735. Design and final independent review.

REPLAN if generated-input semantic ownership is needed, tree metadata cannot be
verified without a cache-leaf semantic dependency, declaration requires a new DICE
owner, or output handling invokes local publication. Keep M7A partial and M8 unproved.

Predecessor WP-7-34 accepted/pushed e50fcd7cb: request-owned source-only Spawn
execution with all-input CAS completion verification and post-Execute native
acceptance. Eight Core gates, policy and two NativeLink gates; longest batch 1.438s.

Local publication is deferred for a concrete ownership gap: an old held accepted
result currently has no generation token and cannot safely choose an arbitrary
output root. Future publication must claim the selected target configuration under
Core's workspace owner and serialize a one-use accepted-generation check with
acceptance/final renames. Pinned Bazel OutputPermissions.READONLY is 0555 regardless
REAPI is_executable; per-file temp+move permits a reported partial multi-file failure,
never a successful build claim. This packet does not reuse the detached materializer.

## Validation receipt

Candidate base e50fcd7cb, branch review/wp735-source-output-trees. Pinned nightly
2025-09-14; receipts and no-run JSON in target/wp735. Exact selector preflights
precede ordinary execution; ignored wire selectors each list exactly once and use
fresh verifying NativeLink storage with supervised backend termination/removal.
The final wire harness reuses tools/v2_oracle_lib/nativelink.py's configuration.

- Four tree decoder/result-shape gates, two protected source Spawn gates and the
  unsupported output projection pass. Root-duplicate/unknown-field corrections
  rerun the two affected decoder gates in 0.004s; unchanged passing gates reused.
- Public directory declaration/package owner/type conflict/restoration and
  executable rejection: 0.469s. Four retained provider gates: 0.002s.
- Eight REAPI/cache/materializer integration gates: 0.003s. Existing escaped
  context/action lifetime gate: 0.494s.
- NativeLink tree cold/cache/A-B-A/nested/empty/missing-descendant: 1.093s;
  source execution/incomplete CAS: 0.778s; protected FileWrite: 0.378s.
- Direct server tests and REAPI integration compile; CLI check covers Core,
  Loading, Analysis, build_api and REAPI. Final CLI check: 7.733s. Fifteen recorded
  compile preparations including corrected compiler failures total 117.935s;
  longest 20.522s. All preparation was separate from test execution.
- Rustfmt, git diff --check, plan status and archive checks pass. Runtime groups
  stayed below 1.32s. Total packet wall/review time was not continuously recorded.

Corrected failures: fixture canonical-package spelling; both Starlark and retained
DefaultInfo guards; NativeLink root duplication. Temporary harness omitted
capabilities and used a noop slow store; the repository configuration restored the
required incomplete-upload behavior without changing production code or assertions.
Design correction review ACCEPT covers retained directories, schema-aware scanning
and identical root duplication. Independent final review ACCEPT confirms only the
operation-owned manifest boundary; M7A remains partial and M8 remains unproved.
