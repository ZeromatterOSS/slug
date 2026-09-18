# Current Slug V2 Work Packet

Packet: WP-7-31-m7a-source-action-staging-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and authority

Stage all declared ordinary source inputs of one retained typed Spawn selected by
configured owner and action ordinal from the requested roots' ValidatedActionClosure. Compute
WP-7-30 facts in the same native request, project exact REAPI Directory topology,
and transfer missing content using WP-7-29 bounded verified readers. This advances
the ordinary source/compiler/tool transfer row in bootstrap-readiness for the
production CLI closure. It does not admit typed Execute, Action/AC publication,
generated/tree inputs, built-in catalog adaptation or a successful build result.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
MerkleTreeComputer.java FileNode builder always sets executable=true; existing
WP-7-28 canonical source execution paths and WP-7-27 exact SHA-256/size evidence
apply. SpawnAction input/tool ownership and CommandLines parameter expansion are
inherited from retained Stage 6 Spawn and WP-7-23. Exact content/protobuf/path
projection for admitted sources; Slug-native canonical/configuration identity;
unsupported action/input shapes fail closed, never silently omitted.

## Owners and invariants

Core adds an opt-in native staging root depending on the existing observed build
root. A staging-only constructor admits a main singleton rule through the existing
observed multi-branch implementation with one target; ordinary build dispatch and
its constructor stay unchanged. The previous neutral singleton terminal can lack
an epoch for rules and cannot be used for staging. The staging root then selects
a configured owner/action from its validated closure before computing source facts. The staging key Eq/Hash
includes the underlying build root, configured owner key and action ordinal. It
does not analyze a detached action or accept raw path/digest rows. Traverse SpawnSpec inputs/tools, including retained depsets and
FilesToRun files and executable. Explicitly inspect invocation and provider support:
reject path/string/shell executables, runfiles support, unused-input pruning and
Derived file/tree inputs until their required owners are implemented. No omission
may be interpreted as a complete tree. Canonical source labels dedupe using
existing compact structural sets; all selected source facts are WP-7-30 keys.
Configured analysis remains content-independent. Need propagates; errors are
invalid and abort staging before acceptance. No production driver change.

The immutable prepared set retains the producer's validated evaluation, selected
configured owner/action, Arc source facts and unioned full route/path/digest epoch.
The epoch unions the full predecessor build-root observed epoch (including its
staging-only main singleton dispatch) with every WP-7-30 source fact epoch, retaining
equal-result Arcs through from_shared. Any existing build source certificate
must be an associated subset; a missing or conflicting association fails closed.
The new certificate covers that complete union, never merely the narrower build
source certificate or digests. Equality is structural; no rendered
path identity, new interner/cache, lock across compute or command-injected semantic
facts. DICE owns retained state; transfer/request scratch drops with its operation.
This prepared set authorizes this staging operation only, not Execute or final
build publication. Later execution must revalidate the full frontier after transfer.

Core opens the observed physical source path as a regular handle, nonblocking on
Unix to reject a replacement FIFO without waiting. REAPI transport remains
path-agnostic. Source handles are transfer-owned and never retained in DICE.
Upload verifies actual bytes/EOF against the observed digest; a stale or mutated
source fails without finishing a write. No retry against a different source.

REAPI accepts only the opaque prepared set. Build a complete source-only input
tree using artifact execution paths, exact digest/size and executable=true for the
new ordinary source entry kind; preserve accepted inline FileWrite/param-file wire
bytes. Compose forced virtual param files through the existing owner. Reject
same-path conflicting content, source/param collisions and file/directory prefix
conflicts before any network call. Deduplicate equal source labels/digests.
FindMissingBlobs covers source, directory and inline digests once. Upload only
missing values, each digest once; missing source content uses the verified reader,
metadata/virtual bytes use the shared cache leaf. CAS presence avoids source open.
No Action identity, AC request, Execute or output materialization is issued.

## Scope and proof

Allowlist: new Core runtime/source_staging.rs, source_staging/tests.rs and shared
test_workspace.rs; bounded module/reexports and staging request method in
runtime/dice.rs and mod.rs. Core staging owns source handle opening. REAPI
input_tree.rs plus new source_staging.rs/tests and lib reexport; existing executor
wire gates remain protected. A new source_staging/tests.rs NativeLink selector
supplies the public staging wire proof. Minimal Cargo/BUILD dev dependency additions
if required for the real Core fixture. Canonical/manifest, Stage 7 and Stage 9.

Native tiny configured rule proves validated action selection, retained source
inputs/tools/executable, same-runtime warm/content A/B/A, rejected derived and
unsupported invocation/support, conflicting requested roots and missing input.
Accepted state remains unchanged on failed preparation. Existing source fact
namespace/symlink/final-validation proofs are reused; new set frontier is checked
against native acceptance, including a build-only observation absent from the
predecessor source certificate. Focused Merkle tests inspect exact source FileNode mode,
paths, topology and virtual-file conflicts. Public NativeLink staging proof uses
real prepared Core sources, three-byte upload chunks and downloaded bytes/Directory
inspection; reuse of an already prepared set after removing a source proves CAS
hits avoid reopening without authorizing Execute or publication. A fresh native
preparation must reject that missing source; a stale-byte source miss fails
transfer verification. No compiler action or broad suite.

Independent design/final review. Compile pinned nightly separately, --no-run JSON,
<=60 seconds each operation. Exact preflight, focused tests expected subsecond or
few seconds; >few-second tests infrequent and >~30s require strict necessity.
Reuse unaffected passing evidence. Named Core/REAPI and CLI direct compile
coverage; changed Rust formatting, archive, plan and diff checks. A new owner or
unsupported production prerequisite outside this slice requires an explicit design
revision, not a fallback or an assertion waiver.

Predecessor WP-7-30 accepted/pushed a8041e6f9: nine focused gates, independent
final ACCEPT; source facts preserve full observation provenance. M7A partial,
M8 unproved. Raw local WP-7-31 receipts will live under target/wp731.


## Acceptance receipt

Independent corrected design and final review ACCEPT. Gate advanced: validated
requested closure → complete declared source input set → native observed facts →
REAPI Merkle tree and verified missing-content transfer. Typed Action/Execute and
build publication remain closed; this is not bootstrap execution acceptance.

Baseline a8041e6f9; review/wp731-source-action-staging. Pinned direct
nightly-2025-09-14 Cargo/rustc/rustdoc; compile-only JSON and exact executable
selection. Raw receipts target/wp731, not committed. No Cargo/BUILD dependency
or upstream copied fixture changes; the tiny authored rule/workspace is shared by
Core and REAPI tests and uses existing direct-File allow_single_file behavior.

- Core library preparations: ten bounded operations, one compiler failure for
  test imports/API spelling/shadowing; remaining preparations exit 0. Initial
  preparation 22.245s; final preparation 12.225s. No operation reached 60s.
- Core exact selectors runtime::dice::source_staging::tests::{
  build_only_observations_survive_the_narrow_source_certificate,
  native_source_staging_tracks_sources_and_rejects_conflicting_roots} both pass.
  The synthetic frontier proof passed in the initial two-test batch; unchanged
  passing evidence retained. Final native selector preflight 1, pass 1 in 0.515s.
  It proves real singleton selection, complete build-only MODULE provenance and
  accepted Arc association, same-runtime source A/B/A, deduplication, rejected
  Derived file/tree, shell, unused-input pruning and FilesToRun support shapes,
  invalid action ordinal, warm individual roots followed by combined output
  equivalence rejection, missing input and directory/FIFO handle rejection.
  Combined-root and missing-source failures preserve accepted semantic state.
- Native fixture corrections supplied the unrelated local built-in MODULE
  closure and used the existing direct-File attr projection. The first real
  staging attempt exposed a missing epoch in the neutral singleton terminal.
  Independent design review accepted a staging-only observed-key constructor
  and reuse of the observed multi-branch owner for one target; ordinary build
  dispatch is unchanged. One later assertion was corrected to the existing
  shared-output UnsupportedEquivalence error family. No negative was waived.
- REAPI library/test preparations exit 0 in 21.052s, 4.709s, 9.254s and 2.647s.
  Seven exact ordinary selectors pass: source_staging::tests::
  source_merkle_nodes_are_executable_and_paths_are_structural and executor::tests::
  file_write_plan_owns_canonical_nul_safe_reapi_objects (2/2, 0.008s);
  integration forced_param_files_merge_exact_bytes_and_merkle_topology,
  forced_param_files_reject_input_collisions_atomically and
  raw_file_write_execution_rejects_before_transport (3/3, 0.004s);
  typed_actions_reject_command_input_tree_and_execution_projection and
  typed_action_execution_rejects_before_transport (2/2, 0.003s).
- Public ignored wire selector source_staging::tests::
  nativelink_closure_sources_merkle_and_verified_upload: exact ignored listing
  verified, final 1/1 pass in 0.591s; fresh NativeLink setup/test/cleanup 0.673s,
  process terminal 143, temporary store removed. Socket use required the normal
  reviewed sandbox exception. The real Core-prepared sources, Directory and
  forced parameter bytes upload with three-byte chunks and download verified;
  source nodes are executable. Source/parameter collision rejects before upload.
  Reusing an old prepared set after deletion hits CAS without open, while a new
  request rejects deletion. A newly prepared digest is missing before a file
  mutation; upload then returns the local digest-mismatch error and a subsequent
  verified read cannot accept remote bytes. The initial 0.626s wire run failed
  only an extra assertion that an interrupted digest must remain absent: this
  NativeLink configuration can advertise interrupted bytes. Independent review
  accepted the stronger client-verification proof rather than claiming server
  absence. WP-7-29's no-finish-on-mismatch proof remains unchanged.
- Direct dependent cargo check -p slug_cli_v2 exit 0 in 10.493s, covering Core,
  REAPI and public consumers. Changed Rust format, plan status, archive and diff
  checks pass. No compiler action, Slug daemon or broad suite ran.

Ten unique focused gates proved. All test batches, including failed diagnostic
runs, took under one second; total test execution was 3.371s. Separate compilation
operations totaled 131.780s, including the test compiler correction; continuous
packet/review elapsed time was not recorded. No workflow speedup claim.

Residual: generated/tree inputs, catalog sources, runfiles, non-artifact
executables, conditional spilling and typed execution remain unsupported here.
The exposed interrupted-backend-content behavior must be resolved by a trusted
verifying CAS or equivalent integrity gate before execution admission; CAS presence
alone cannot establish valid content. Later execution must also revalidate the
full source/build frontier after transfer. M7A remains partial and M8 unproved.
