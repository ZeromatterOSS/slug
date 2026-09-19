# Current Slug V2 Work Packet

Packet: WP-7-49-m7a-runfiles-generation-leases-r1
Status: accepted

## Outcome and basis

Allow the native runfiles manifest preparation API to return materialized-repository
source paths backed by exact native generation ownership. Held accepted results keep those
generations alive across later requests and runtime shutdown. WP748 (0f0570d2f) supplies
manifest bytes and complete observed metadata but rejects every Materialization namespace.
This packet replaces that refusal only when the selected native materializer supplies an
owned generation. Custom/virtual materialization paths without that ownership still fail.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a Artifact.java:362-364 and SourceManifestAction.java:294-315
require ordinary Artifact.getPath targets; RunfilesSupport.java:467-543 and
SymlinkTreeHelper.java:167-187 establish backing artifacts and physical links. Existing
exact byte algorithms remain unchanged. The generation lease/lifetime is Slug-native
resource ownership, not a claim to reproduce Bazel's repository storage layout. Existing
Core repository_io tests generated_file_effect_sessions_conflict_replace_reuse_restore_and_discard_post_io,
retained_session_epochs_acceptance_reuse_and_old_roots_are_exact, and
retained_native_bridge_authority_lifecycle_and_structural_errors_are_exact establish the
native generation owner and observation semantics. Read docs/developers/dice.md and reuse
the native driver's completion/validation boundary; no compute may run under its mutex.

## Contract and ownership

Split the retained DICE manifest input value from the public prepared result. DICE owns only
existing build/source facts, exact owner coordinates, observation epoch and certificate,
with structural equality and Allocative. The public command-owned prepared wrapper holds
an Arc of those semantic inputs plus a private native-generation lease. Preserve its useful
API methods; it is not a DICE value, and filesystem owners never participate in key/value
equality. All manifest bytes and lookup maps remain call-scoped scratch.

NativeCommandRoot::complete obtains the lease after terminal observation association and
selected-snapshot preparation, before final source validation. Supply the private completion
context with the selected full repository requests and result epoch. RepositoryMaterializer
owns lease construction under one short synchronous lock: require the validated active
session, exact selected full request and active success/result, canonical source repository,
matching observation instance and generation root, and the actual owned Arc<TempDir> from
that session's accepted/provisional roots. A historical root or matching integer alone is
not authority. Require the normalized requested path to lie component-wise beneath the
exact owned root. Preserve existing observed symlinks whose real target lies outside the
root: they remain covered by the source certificate; a lease owns the requested artifact
root, not all mutable target content. No filesystem reopen/canonicalization or side scan
may create authority. Reject absent/unselected/stale/forged/virtual/root-mismatched requests.

Deduplicate lease rows by exact native instance and share them across wrapper clones. Host
sources require no generation. Complete-only semantic DICE results remain reusable;
command completion reacquires authority for the current exact selection on every request.
Retry/rejection/unwind drops attempt wrappers; accepted wrappers retain only required native
root Arcs. Runtime drop cannot remove roots held by accepted wrappers; after runtime and all
leases are dropped, existing TempDir cleanup releases them. No new global registry, cache,
interner, persistent directory layout or materializer eviction policy. Reuse Stage 9's Arc,
Allocative, SmallMap/SmallSet and existing WP730/731 observed-source ownership rows.

A lease guarantees root lifetime only while its owner exists, not ongoing source freshness
or historical reads. The driver still validates the complete source frontier before
acceptance. This metadata boundary creates no runfiles outputs and does not activate Build.
Durable runfiles publication must subsequently stage persistent materialized-source backing
or introduce a durable generation owner that survives command/result/process exit. It must
never publish permanent links to TempDirs whose only owner is this prepared result.

## Scope and proof

Allow repository_io.rs module wiring and new repository_io/source_generations.rs with focused
child tests; runfiles_manifest.rs and paths/tests for the semantic/native split; dice.rs
completion-context fields/accessor and construction wiring; requested_artifacts test child
wiring plus new native generation integration tests. Root owns current/canonical and Stage 7/
bootstrap owner notes. No Bzlmod provider/schema, source observation/kernel semantics, REAPI,
CLI, persistent storage, action execution or output publication changes. Existing large
repository_io.rs/dice.rs files receive only narrow wiring; put new ownership logic and tests
in focused children. Estimate 250-450 new production and 300-550 proof lines, plus moving
existing manifest methods to the semantic input owner as needed. Estimates trigger review.

Use an authored tiny innate repository_rule/ctx.file fixture under the existing hermetic
native runfiles Workspace. Public prepare A/warm-A/B/restored-A; exact selected canonical
source identity and manifest requested path, changed/restored digests, immutable held values,
complete frontier and continued planner rejection. Drop runtime while retaining accepted
prepared handles and assert source generations still exist with their bytes; drop final
handles and assert cleanup. Restored source contents need not reuse a physical temp root.
Materializer unit tests discriminate selected full request/result, wrong instance/root/path,
stale token, virtual source without an owned root, accepted/provisional lifetime, dedup,
rejection/discard cleanup, and allowed external symlink real target. Reuse WP748 Host/external
manifest lifecycle controls and existing native materializer acceptance/abort controls.

Independent design and final review. Pinned nightly-2025-09-14; no-run offline compilation
separate from runtime, 60s per preparation; shared Cargo and ancestor-observing native tests
serial. Exact-selector preflight and focused runtime expected a few seconds; >30s requires
strict necessity. Format/diff/plan/archive checks and checkpoint commit/authorized main push.
Receipts target/wp749. Replan if exact native ownership cannot be obtained from the selected
completion boundary without placing effects in DICE or weakening source validation.
M7A partial and M8 unproved; ordinary runfiles execution/publication, durable backing and
WP746's baseline diagnostic defect remain open.

Independent design review: ACCEPT. Requested-path containment preserves existing observed
symlink semantics; selected native authority and command-only lifetime remain explicit.

## Acceptance evidence

RunfilesManifestInputs remains the sole semantic DICE value. Each native attempt constructs
a fresh PreparedRunfilesManifests wrapper; completion binds its private deduplicated lease
to the selected snapshot's exact requests/results and the active materializer's owned roots.
The source namespace alone cannot create native authority. Source observations and final
validation are unchanged. Host paths require no lease; materialized paths now prepare only
with exact native ownership. Metadata preparation still creates no runfiles outputs.

Four new lease tests prove shared clone/dedup lifetime, accepted/discarded generation
retention, final-owner cleanup, stale/unselected/duplicate/different full requests, forged
result identity, wrong canonical repository/instance, component-wise escaped requested paths
(including a second source after dedup), historical roots, mismatched roots, unowned virtual
generations and external symlink targets. The new public innate ctx.file fixture proves
A/warm-A/B/restored-A paths, bytes/digests and frontiers, immutable held manifests, no build
outputs, native root survival after runtime shutdown, and cleanup after final prepared Arc
drop. Reused WP748 Host/external/symlink/hidden-source controls and existing native driver
retry/drop plus materializer bridge/generated-effects controls protect inherited behavior.

All 14 exact selectors passed on the first runtime run in 2.790s; exact preflight passed.
Pinned offline Core no-run preparation initially failed at 14.695s on an ambiguous test
SmallMap key conversion, corrected with an explicit CompactString; production was unchanged.
The final preparation passed in 25.128s, within the separate 60s cap. No backend, daemon,
network repository fetch or fresh Bazel process ran. Formatting, diff, plan and archive
checks pass. Receipts/source hashes: target/wp749. The new lease module is 146 lines; the
existing manifest module holds the bounded semantic/native split, with narrow context/module
wiring in large owners. New proof children total 491 lines. No new dependencies, retained
semantic cache or persistent filesystem layout. Independent final review: ACCEPT, verifying exact selected native generation ownership,
all 14 focused checks, runtime-drop survival and final-owner cleanup.

This closes temporary native generation authority for prepared manifest results. Next
provide durable materialized-source backing and integrate virtual runfiles results with
complete confined link/backing-output publication under final native source validation.
An accepted metadata lease is not durable publication authority. M7A remains partial and
M8 unproved; ordinary runfiles execution and the WP746 baseline diagnostic defect remain open.
