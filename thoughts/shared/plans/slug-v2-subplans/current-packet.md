# Current Slug V2 Work Packet

Packet: WP-7-38-m7a-validated-output-publication-r1
Status: accepted

## Outcome and compatibility

Execute the existing selected FileWrite/Spawn prerequisite chain, privately download
and verify all selected action outputs, and publish those File/Directory artifacts
only after Core's complete source frontier passes final validation. Intermediate
producer outputs remain CAS-only. Public library operation only; general scheduling
and CLI activation stay deferred. This advances bootstrap's generated file/tree
materialization gate. Baseline e90aeaaaf owns verified chain execution and manifests.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
RemoteExecutionService.java:1350-1384 completes downloads before moving outputs;
915-922 uses 0555 regardless of producer executable bit; OutputPermissions.java
READONLY=0555. FileSystemUtils.moveFile:454-489 does not promise atomic group moves.
AbstractActionInputPrefetcher:681-728 chmods temporary files, and 365-397 restores
tree-directory modes while shared package parents remain writable. Exact: verified
file content and default 0555 file/tree permissions. Slug-native: existing configured
paths, preserving all verified manifest empty directories, per-declared-output atomic
replacement and error text. No command-wide filesystem atomicity claim. Deferred:
symlink artifacts, writable-output policy, Windows/non-Linux publication, general
scheduling/recovery, CLI/watch activation, bootstrap and exact ActionKey.

The explicit atomic unit is one declared File or whole declared Directory. Finish
all downloads/modes/preflight before any artifact rename. CAS, staging, source retry
or cancellation before publication leaves old artifact outputs intact. A final
filesystem commit error (or a later bookkeeping error) may leave earlier artifacts
replaced, but returns no AcceptedCommand or success events. No rollback promise.
This matches the bounded Bazel output-group model; a whole-bin snapshot would add
an unneeded O(all existing outputs) owner and is not selected. Stage 7 watch's future
whole-generation publication remains a separate contract.

## Ownership and request lifecycle

Existing ActionChainStagingKey/evaluation owns selected action and its complete
source/build certificate. No new semantic DICE key, retained output contents or
side semantic cache. Use the validated plan's last action and its structural
configuration, never caller-supplied destination/RemoteExecutionResult provenance.
ConfiguredOutputOwner retains configuration projection collision ownership and
configured_output_root spelling. New confined staging reuses its structural register
but does NOT use existing path-following claim_sidecar I/O.

Core exposes ActionOutputStaging with read-only declared outputs and methods to
create a regular file or directory at one output index plus a canonical tree-relative
path (empty path denotes the declared root). It exposes no arbitrary filesystem
root or publication method to transports. Exclusively created hidden sibling entries and
owned directory descriptors survive transfer; successful sealing checks every
selected output exists with its declared kind, applies 0555 file/tree permissions,
and finishes file writes before returning a publishable pending object. Ordinary
package parents remain writable. No output-file content is accumulated in RAM.

Add ActionChainOutputTransport: ActionChainTransport with an async stage_outputs
callback receiving &mut Session and &ActionOutputStaging. REAPI implements it using
only the current private session's selected verified result and fixed CacheClient.
Reconcile exact output paths/kinds, reconstruct trees from the bounded manifest,
and stream every file with read_blob_verified into freshly created staging files.
Callback bytes are provisional until final digest/size success. Any late failure
drops the entire staged set. Missing CAS never reads local outputs or reruns a
producer. Existing metadata-only operation and detached legacy materializer remain
unchanged. New execute_and_publish_action_chain_with_repository_environment shares
chain ordering/prechecks through a private publication policy; avoid duplicating
the complete driver. ActionChainResult gains optional published-output metadata.

NativeCommandRoot gets a default no-op synchronous publication callback. Add a
finalize_native_with_publication variant, preserving the old no-op wrapper/tests.
Under the existing RequestRevision owner: compare DICE version, reobserve full
certificate, commit selected DICE revision, then invoke the synchronous publication
callback before releasing the owner. No downloads, Execute, DICE compute or Starlark
run under this lock. Publication errors are distinct from observation errors. The
selected DICE revision may already advance on publication failure; abort restores
native accepted bookkeeping/events, but does not claim filesystem rollback. Audit
fallible native bookkeeping after callback and test its failure classification.
Only the new publishing operation has a pending output capability to commit.
An older AcceptedCommand cannot authorize later publication. Source/version retry
never calls the callback and discards the pending stage before a fresh attempt.

## Filesystem boundary

Linux GNU only for the new publisher (the existing nix renameat2 binding). Use existing nix directory-relative operations:
anchor workspace, walk/create bazel-out/configuration/bin and marker parents with
O_DIRECTORY|O_NOFOLLOW, and verify canonical configuration marker bytes through
no-follow regular-file handles. A collision never writes selected artifacts. Reject
preexisting symlink/special-file roots, parents and artifact leaves. Stage as hidden direct siblings of each destination, outside declared artifact
paths, using owned descriptor-backed paths only where tempfile requires a path.
Linux requires write permission on a directory moved between parents; same-parent
rename preserves sealed 0555 roots and old output modes without any chmod window. Do not follow an unvalidated destination pathname.
Canonical segments reject empty, dot/dotdot, slash/backslash and NUL components;
file/tree-relative APIs prevent output-root escape and type/ancestor collisions.
Paths cap at 256 components; descriptor traversal uses O(depth) handles.

Preflight all destination parents/leaves, then recheck before publication. Existing
regular files/directories are replaced with renameat2(RENAME_EXCHANGE); absent
artifacts use RENAME_NOREPLACE. Never remove an old artifact first and never copy
across filesystems. Unrelated siblings remain untouched; a tree replacement removes
stale children as one artifact. Configuration root/parent identity changes fail
closed. The supported writer model is one coordinated Slug publisher per workspace;
concurrent external/legacy mutation of that output namespace is unsupported, with
no-follow operations still required to prevent escape. No cross-process writer
lock or crash-durability guarantee is added.

Retired outputs remain at the pending object's hidden names after swaps; cleanup
tracks their saved old identities, never the staged-root handles now naming published artifacts.
Cleanup occurs outside the revision-owner critical section, including error/unwind
paths. Confined cleanup must handle read-only directories and unlink symlinks
without following them; it removes only request-owned hidden entries whose identities still match.
Cleanup failure cannot retroactively turn a successful namespace commit into a
failed publication. No detached tasks. Metadata/pending handles are attempt-owned;
accepted published metadata owns no staged buffers or open handles. Reuse compact
Arc/SmallMap utilities (Stage 9); no new dependency/interner/cache. Work is linear in
selected manifest entries and retired selected trees, with bounded transfer memory;
no full-bin scan/copy and no performance claim.

## Scope and evidence

Allowlist: Core new runtime/action_output_staging.rs and platform module plus focused tests; minimal
configured_output.rs delegation/secure claim ownership, action_chain_execution.rs
and its tests, dice.rs NativeCommandRoot/driver callback, request_revision.rs hook
and focused lifecycle tests, mod.rs exports. REAPI new action_chain/output_staging.rs
and tests plus action_chain.rs trait implementation delegation, lib.rs if needed;
minimal visibility in action_chain/tests.rs to reuse the existing public fixture.
Canonical/current manifest, Stage 7 and bootstrap-readiness status only. No CLI or
server execution activation, old materializer migration or arbitrary output import.
Large dice.rs gets only delegation/hook wiring; new filesystem logic lives separately.

Parallel ownership: root filesystem owner; Core lifecycle worker; REAPI transfer
worker; independent design/final reviewer. Required evidence: selected File+Directory
publication with correct bytes/default 0555 even producer false mode, empty/nested
dirs; same-root tree replacement removes stale children and preserves unrelated
siblings; configuration A/B/A roots and structural collision; missing/corrupt late
file after earlier staging leaves old artifact group intact; source mutation during
output download causes fresh retry with no stale publication; failure/unwind stage
cleanup; symlink root/parent/leaf and outside sentinels; failed final rename returns
no acceptance and explicitly reports possible partial publication. Reuse WP737 chain
and full-frontier controls, WP735 manifest guards, existing revision lifecycle proof.
Tiny public NativeLink gate plus focused Core/filesystem tests; no fresh Bazel oracle.

Pinned no-run preparation separately capped at 60s, exact selector preflight, focused
runtime groups expected under a few seconds. Any >30s runtime test needs strict
necessity; none planned. Compile Core/REAPI and CLI direct dependent; rustfmt,
diff/plan/archive checks. Receipts target/wp738. Independent design/final review
before commit/fast-forward main/push, already authorized. M7A partial and M8 unproved.
REPLAN if publication requires whole-generation atomicity, new semantic identity,
external producers, cross-process coordination or broader CLI scheduling.


## Acceptance receipt

Independent design and final reviews ACCEPT. A measured Linux EACCES failure
corrected the initial cross-parent staging layout to the reviewed hidden-sibling
layout; the corrected readonly-tree replacements pass. Review also corrected
private-prefix depth accounting and preserved publication diagnostics when abort
restoration fails. No broader scheduling or milestone acceptance is claimed.

Pinned Core/REAPI `cargo test --lib --no-run --message-format=json` preparations
and `cargo check -p slug_cli_v2` pass. Six preparation invocations total 99.884s,
maximum 29.624s, separately bounded from tests. Final focused evidence comprises
27 unique passing cases in 8.982s: Core filesystem 8/8 (0.497s), publication
lifecycle 6/6 (1.621s), unchanged protected chain/revision/configuration controls
9/9 (1.431s), REAPI schema 1/1 (0.003s), and three fresh NativeLink cases
(1.500s, 1.999s, 1.931s). The wire cases cover selected-only file/tree publication,
late missing/corrupt downloads preserving old outputs, and source-change retry.
All ordinary and ignored selectors were checked exactly. Backend processes exited
and fresh roots were removed. Initial failing filesystem receipts remain attributed
to the corrected layout; no failed gate is waived.

Receipts: `target/wp738/core-{filesystem,lifecycle}-r2.receipt`,
`core-protected.receipt`, `reapi-schema-r2.receipt`, the three `nativelink_*.receipt`,
and compile JSON/receipts. Changed Rust formatting, diff, plan and archive checks
pass. The observable gate advanced is validated selected-output materialization;
M7A remains partial, M8 unproved, and general build/CLI activation remains open.
