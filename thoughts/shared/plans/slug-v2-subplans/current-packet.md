# Current Slug V2 Work Packet

Packet: WP-7-51-m7a-files-to-run-inputs-r1
Status: accepted

## Outcome and basis

Ordinary requested Build executes a generated tool through executable-attribute FilesToRun
and explicit FilesToRun tools, with source File and generated File/Directory runfiles available in
the remote sandbox. Transfer uses logical mappings and verified same-attempt producers;
tool intermediates need not be locally published. Preserve WP750 binary publication.
Predecessor: 7f9e1a4e8, accepted WP750 contract in that commit's current-packet.md.

Demanded by: pinned rules_rust 0.73 rust/private/rustc.bzl:1895-1948 wrapper invocations
and cargo/private/cargo_build_script.bzl:663-684 executable runner plus explicit script
FilesToRun. Saved authenticated target/wp714/actions.json contains 510 Rustc, 375
ExtractCargoTomlEnvVars and 66 CargoBuildScriptRun actions consuming RunfilesTree artifacts.
This is capability demand, not proof that the full live production root executes.
Virtual Args paramfiles at rustc.bzl:1167-1168 and cargo_build_script.bzl:353-357 do not
establish standalone ArgsWrite demand. Bootstrap Symlink/ExecutableSymlink and Cargo's
_runfiles_map callback remain separate concrete residuals. F3 stays closed.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
exec/SpawnInputExpander.java:85-130,202-211 expands logical runfiles mappings, null empty
files and tree children/empty roots, excluding automatic MANIFEST. Its tests
SpawnInputExpanderTest.java:189-273,356-388 cover regular links, root links/workspace
sentinel and tree children. remote/merkletree/MerkleTreeComputer.java mapping and file
node construction establish regular FileNodes with executable=true. Reuse WP747/748
layout/manifest algorithms, WP749 source-generation lifetime and WP750 typed results.
No fresh Bazel probe is required. Read docs/developers/dice.md and Stage 9 existing
runfiles/depset/observed-source utility dispositions; no donor semantics are imported.

## Contract

Logical mapping and regular-file/tree expansion are exact for admitted Bazel surfaces.
Structural artifact/configuration identities, native validation, transport routing and
execution paths remain Slug-native. Exact ActionKeys, Run, arbitrary Symlink outputs,
source directories, nested RunfilesTree, unresolved symlinks, Filesets, archived trees, path-mapping modes
and Windows remain unsupported/deferred. Do not claim M7A completion or M8.

Core's validated prerequisite forest owns exact artifact-to-producer binding and ordering.
Expand all FilesToRun occurrences in executable, inputs and tools; deduplicate structurally
through existing artifact collections. Require provider.support to match the exact retained
RunfilesTree producer support, never just a matching path. The provider's executable and
support tree must be represented. Generic source-only staging remains source-only.
Admit direct retained RunfilesTree inputs only through their exact supported producer.
Preserve cycle detection, shared scalar FileWrite representative bindings and whole-plan
unsupported-kind rejection. Existing support edges schedule every raw constituent, including
obscured sources and derived backing. Existing ActionChainStagingKey observes all sources,
retains full certificates/events, and performs exact support preflight; no alternate graph,
filesystem readback, semantic side table or new DICE key is introduced.

REAPI prepares request-local binding templates from the borrowed native plan. Expand each
RunfilesTree at its declared exec path using support.layout().entries(), including authored
MANIFEST and _repo_mapping. Never inject automatic physical MANIFEST or host absolute link
paths. Empty logical entries become empty inline file bytes. Ordinary targets bind certified
source digests/readers or exact same-session producer results; generated Directory targets
project their verified regular file children at logical paths, preserving an empty root but
not nested empty producer directories. All consumer file modes follow existing executable=true
projection. Virtual tree metadata remains a typed local result, never an Execute/Directory
output. Validate its exact completed output before consuming the mapping.

Whole-plan preflight reserves runfiles root namespaces against ordinary inputs, paramfiles
and outputs, and validates known logical entry paths/types/conflicts before any connection.
Runtime verified directory children undergo existing canonical input-tree validation. Exact
repeated artifact inputs deduplicate; conflicting paths or leaf-prefix placements reject.
Retain separate generated-CAS provenance: missing/corrupt generated data cannot be repaired
from equal-digest source/local/empty bytes. Local manifest bytes may be uploaded only from
actual completed local results. Keep session identity and ordinal gates, per-step and final
native source validation, and selected generation leases. No local output publication is
required for a tool-only tree; requested outputs still use existing staging/publication.

All new bindings/maps/bytes are action/session scratch, released on failure/cancel/finish.
Reuse borrowed plan, Arc support/layout identities, compact retained collections and existing
ordered REAPI scratch maps. Avoid copying depset graphs or replanning the whole forest per
ordinary input. No new service cache, filesystem owner, public API or lock is required.

## Scope and validation

Build API: runfiles.rs retained-artifact importer and focused tests admit generated Directory
in direct/transitive files and link targets, preserving nested RunfilesTree rejection.
Core: runtime/source_staging.rs, action_prerequisites.rs with focused child helpers/tests,
action_chain_staging.rs/runfiles.rs only if needed for preflight; focused requested_artifacts
runfiles tests. REAPI: action_chain.rs and focused binding/runfiles-input/test children.
CLI: reuse shared fixture for one public wrapper proof if necessary; compile CLI/server direct
dependents. Root owns canonical, Stage 7/bootstrap notes and this manifest. Expected 300-600
production and 400-800 test lines; review responsibility boundaries if exceeded. Keep new
binding expansion out of the central transport module. No fixture corpus growth/new oracle.

Discriminators: executable attribute and explicit tools support; mixed generated File and
Directory (including empty), observed Host/materialized sources, empty logical files, mapping
manifest and authored MANIFEST vs absent automatic MANIFEST; exact support/producer identity,
shared/deduplicated bindings and namespace/prefix conflict rejection before connection;
actual remote consumer bytes/modes, zero tool-only publication; cold/warm/A/B/A identity/cache
behavior with accepted source-generation lifetime; generated CAS missing/corrupt and equal-digest
local provenance controls. Reuse existing lifecycle/freshness/publication controls where
unchanged. Existing WP750 binary Build must still work.

Independent design and final review. Pinned nightly-2025-09-14 offline compile/no-run separately
from exact-selector tests; compilation cap 60s per operation, focused tests expected a few
seconds, >30s demands strict necessity. Shared-target Cargo and ancestor-observing native tests
strictly serial. Root coordinates all execution, workers may author tests but must not run
Cargo concurrently. Rebuild CLI before wrapper smoke. Use supervised fresh NativeLink only
for named real-transfer proofs, with cleanup receipts; no broad suite. Format/diff/plan/archive
checks and receipts target/wp751. Commit and authorized main push after acceptance.
REPLAN only for a new unbounded ownership/compatibility prerequisite; routine corrections
remain within this contract. Keep WP746 baseline diagnostic defect separately open.

Independent design review: ACCEPT. Exact support/producer ownership, source-file boundary,
logical mapping, pre-connection conflict rejection and CAS provenance are frozen.

Implementation discovery: the shared RetainedRunfiles importer still rejected generated
Directory artifacts despite existing layout/remote tree semantics. Admit that same typed
artifact through the existing visitor (no new semantic owner); pinned StarlarkRuleContext.java
1059-1143 adds Artifact lists/depsets and symlink targets without a regular-file restriction, and pinned
SpawnInputExpander.java104-122 owns their remote expansion. Source directories stay closed.

Independent design amendment review: ACCEPT. Shared Directory importer admission completes
the existing typed contract; no Analysis API or new observation owner is needed.

## Acceptance evidence

Implementation completes remote FilesToRun consumers through the existing exact prerequisite
forest. Core rejects mismatched or incomplete support (including missing manifests before
retained-input traversal), preserves source-only staging, and retains canonical shared
producer bindings. REAPI binding is a focused child using exact artifact lookup and plan
ordinals; no new public API or dependency was introduced. The shared runfiles importer now
admits generated Directory in all existing file/link input channels. Physical publication
and remote logical input topology remain distinct.

Twenty-four unique portable checks pass: 12 Core covering new provider occurrences/malformed
support plus preserved ownership/cycles/source-only/native runfiles controls (1.748s total),
11 REAPI covering logical aliases/empty files/empty directories, typed producer results,
source/local/generated provenance, namespace preflight and existing requested/local result
controls (initial batch 2.672s, final namespace 0.476s), and the revised Build API importer
regression (0.001s). Exact selection receipts are in target/wp751. Initial namespace failure
revealed the generated-Directory importer prerequisite; the valid-control assertion was
improved to expose the error and the reviewed importer correction closed it.

Three unique real-backend checks pass: new implicit/explicit FilesToRun consumers use six
shared remote producers/consumers with cold/warm/B/restored-A hits 0/6/2/6, correct logical
bytes/modes, authored versus absent automatic MANIFEST, no tool-only publication, and an
execution-only fresh-workspace control (final 7.883s); missing/corrupt generated CAS data
blocks consumer Execute despite equal source bytes (1.955s); and the inherited WP750 binary
publication/source-backing/shutdown-lifetime proof remains passing (1.405s). Every supervised
backend exited 143 and its temporary root was removed. The 7.9s consumer proof is an infrequent
checkpoint test; no test exceeded 30s and no broad suite or new Bazel process ran.

Pinned offline Core/REAPI/provider no-run preparation and the direct CLI/server build pass.
Compilation was separate from test execution. Two compiler corrections used existing owners:
public RetainedArtifactInputs replaced an inaccessible depset visitor, and AnalysisArtifact
source keys replaced a test-only identity-crate import. The latter correction was validated
by the final CLI build, REAPI compile, native namespace check and full positive wire proof.
Source-map ordering remains deterministic because the scratch hash map is lookup-only.
Failed and passing receipts remain under target/wp751; accepted Rust hashes are recorded.

Proof growth exceeds the estimate because isolated tests cover importer channels, exact
provider occurrences/forgeries, binding provenance and supervised real-transfer faults;
review found these separate responsibilities cohesive. The transport module shrank while
new binding logic stayed in its focused child. No performance speedup is claimed. Formatting,
diff, plan and archive checks pass. Final independent review: ACCEPT, confirming exact support ownership, certified source
binding, logical expansion, generated CAS integrity and the named validation.

M7A remains partial and M8 unproved. Next production-demanded prerequisites include bootstrap
Symlink/ExecutableSymlink and Cargo's _runfiles_map callback. Full production closure,
source directories, nested runfiles trees, Run, broader aquery/ActionKeys and Windows remain
unproved/deferred. WP746's baseline diagnostic defect remains separately open.
