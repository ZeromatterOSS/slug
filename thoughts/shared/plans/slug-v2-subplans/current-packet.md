# Current Slug V2 Work Packet

Packet: WP-7-48-m7a-runfiles-manifests-r1
Status: accepted

## Outcome and basis

Produce deterministic source and repository mapping manifest bytes for retained binary
runfiles through a native observed preparation owner. WP747 (0662733cc) supplies typed
layout and complete constituent metadata. This packet supplies the path/byte prerequisite
for coupled virtual-result integration and complete runfiles publication; ordinary Build
continues rejecting runfiles until those effects can be admitted together.

Pinned Bazel 9.2 commit 8220c6198837d5c13d53fea211cf3282aa12408a:
SourceManifestAction.java:249-315,369-424 owns nested-tree failure, sorted absolute target
entries, conditional escaping and empty-entry trailing space; generated unresolved symlinks
require metadata and remain explicitly unsupported here. RepoMappingManifestAction.java:
186-348 owns repository presence, first package mapping per repo, sort/filter, compact groups
and CSV lines. StringEncoding.java:25-59 establishes UTF-8 filesystem/BUILD bytes represented
as Latin-1 internal strings. Correct WP747's UTF-16 interpretation: for Slug valid Unicode,
internal byte ordering and encoding correspond to UTF-8 bytes, including BMP/supplementary
order. Paths/configuration directory spelling remain Slug-native; admitted manifest
formatting and mapping algorithms are exact under these supplied path identities.

## Owners and contract

Build API retains RunfilesSupport, RunfilesSupportActionSpec and package mapping depsets.
Pure encoders consume those owners and layout with a typed artifact-to-absolute-target
resolver. No filesystem access, action execution or remote ActionResult in the encoders.
Preserve empty entries, authored root MANIFEST, structured diagnostics and exact artifact
identity. Reject nested runfiles trees and Error-policy obscuring diagnostics; preserve Warn
policy diagnostics for the later event owner. Reject generated unresolved Symlink targets
rather than substituting their output path. Repository presence uses raw runfiles artifact
owners, normal-symlink presence and root-symlink first segments before layout filtering.
Compact merging requires producer-owned shared-group identity plus identical retained
mapping contents and matching canonical name prefix through the last '+', never just
similar text. Canonical/apparent names and workspace prefix supply the three CSV fields.

Core adds RunfilesManifestPreparationKey(build root, selected configured owner), following
SourceStagingKey's observed-root/Need/native-driver contract. Selection comes from the
validated action closure and its retained support actions, not caller-invented metadata.
Resolve raw source constituents through SourceArtifactInputObservationKey and union the
full observed frontier with build observations. Retain that frontier/certificate and exact
source metadata; source link targets use observed requested_path rather than real_path,
matching Artifact.getPath without canonicalizing symlink targets. Reject immutable virtual
materialization namespaces that do not establish a native on-disk target. Generated target resolution
requires exact artifact owner/output lookup in the validated closure and structural
configuration through configured_output_root. No string-derived owner or lookup against
current output files. Target projection is metadata only; collision registration and
publication remain the existing effect owner's responsibility.

The new prepared value retains Arc build/source facts, owner coordinate, observation epoch
and certificate, with Allocative and structural equality. Derived paths, layouts and byte
buffers are call-scoped projection scratch. No new retained cache/interner, file handle, source-byte
buffer, evaluator reference, output filesystem mutation or lock across compute. Existing
DICE dependencies own configuration, repository mappings and source invalidation; incomplete
and failed preparation cannot cut off later success. Native command acceptance validates
the full source frontier; later execution must validate again before publication. This
metadata API does not authorize historical filesystem reads or execution. Errors follow
the selected SourceStaging API (outer preparation errors), not Build-terminal parity. Reuse the Stage 9
SmallMap/SmallSet, shared Arc/Allocative and WP730/731 observation rows; no donor import.

## Scope and evidence

Allow Build API runfiles/layout.rs and tests for corrected ordering, new
runfiles/manifest.rs plus child tests, runfiles.rs/lib.rs exports and an action-spec encoder
method if necessary. Core new runtime/runfiles_manifest.rs plus child helpers/tests, wiring
in runtime/dice.rs and mod.rs, and requested_artifacts test child wiring/new manifest tests.
Root owns scheduling/Stage 7/bootstrap notes. No REAPI, CLI, action planner activation,
source observation semantics, provider schema or dependencies change. Existing large files
receive module/export wiring only. Estimate 500-850 production and 400-700 proof lines;
review substantive growth rather than enforcing estimates as caps.

Pure tests discriminate conditional source versus target escaping, spaces/backslashes/newlines,
empty trailing space, raw UTF-8 and supplementary order, nested/error/warn handling, symlink
rejection, mapping presence before filtering, empty apparent omission, root workspace naming,
package dedup, ordering and compact enabled/disabled/equal-content-different-group behavior.
Native tests reuse the hermetic configured binary fixture and public preparation wrapper:
exact source/generated/manifest target paths, complete source certificate, source symlink
requested-path spelling, content and selected-layout A/B/A immutability, deletion/recreation,
configuration separation, external host source/mapping paths, bad selection/missing producer
rejection, immutable virtual namespace rejection, no published outputs and
continued requested-execution rejection. Reuse predecessor layout/selection controls.
Upstream tests are source regressions; no fresh Bazel process or new fixture directory tree.

Independent design and final review. Pinned nightly-2025-09-14, offline no-run preparation
capped at 60s per operation; shared Cargo and ancestor-observing native tests serial.
Exact selector preflight; aim for subsecond pure tests and a few seconds of native tests.
Tests over roughly 30s require strict necessity; 12s remains only an estimate. Format,
diff, plan and archive checks precede checkpoint commit, fast-forward and authorized push.
Receipts target/wp748. Replan if retained mapping groups cannot reproduce compact semantics
or generated targets require a new ownership decision. Run, Windows, arbitrary backend
symlink outputs, virtual action results and filesystem publication remain deferred. M7A
remains partial, M8 unproved; WP746's baseline diagnostic defect remains open.

Independent design review: ACCEPT. Native path ownership, namespace rejection, selected
preparation errors, byte encoding and planned discriminators were checked.

## Acceptance evidence

The pure encoders preserve exact typed resolver identity, conditional source/target escapes,
empty-entry separators and UTF-8 bytes/order. Repository presence uses raw files and links;
compact output preserves producer groups and exact mapping content. Shared mapping slices
use pointer equality before structural comparison, and call-scoped borrowed relevance rows
avoid repeated filtering of one immutable slice. No pointer enters semantic identity.

Core's new observed preparation owner validates each raw backing/support artifact against
its exact declared owner/output/configuration, observes all raw sources and retains the
complete accepted frontier. Its held values render without consulting current filesystem
state. Generated output files remain absent and requested execution still rejects runfiles.
Host sources, including external local-path repositories, are admitted; Materialization
sources explicitly lack retained native-generation lifetime authority and remain deferred.

Pinned offline API/Core no-run preparation passed in 46.071s, then the final correction
rebuild passed in 19.405s, both within separate 60s operation caps. The PATH rustup launcher
was unavailable (snap); the installed pinned binaries were checked directly against
rust-toolchain (rustc 1.91.0-nightly 02c7b1a7a). No executable proof uses a stale CLI binary.
Final exact-selector preflights and all 18 checks pass: ten API tests in 0.003s and eight Core
tests in 2.666s. No backend, daemon or fresh Bazel process ran. Initial Core evidence was
7 pass/1 failure because the new assertion expected source FileDigest at requested_path;
PathFileDigestObservationKey owns the digest at real_path (path_file_digest.rs:201). The
assertion was corrected to require that exact namespaced demand; requested-path manifest
and ReadLink/retargeting assertions remain. No source observation semantics were changed.

New implementation modules total 668 lines, plus small exports/wiring and the layout-order
correction. New proof modules total 920 lines plus wiring, exceeding the estimate because
pure byte/identity negatives and the native lifecycle/frontier proof cover distinct owners.
They remain focused children, with no large production-file expansion. Formatting, diff,
plan and archive checks pass. Receipts and exact pinned-source hashes are in target/wp748.
Independent final review: ACCEPT. Exact observed/generated ownership, byte algorithms,
all 18 focused checks and structural gates were verified; acceptance is metadata-only.

Next integrate virtual runfiles support results and complete backing/link publication under
native freshness validation, first supplying retained native generation authority where
materialized repository paths are needed. RunfilesTree stays rich metadata, never a fake
remote Directory result. This checkpoint closes path/byte preparation only; M7A remains
partial, M8 unproved and the WP746 baseline diagnostic defect remains open.
