# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-observation-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit base: pending docs commit / Rust `b3eba6df`

Result: design only the minimum same-crate handoff for the accepted terminal
legacy/observed source-observation owner. Linux under WSL is the first and only
platform validation target for the successor implementation; Windows and macOS
work remain deferred.

## Active docs-only design contract

The implementation at `b3eba6df` is complete and remains read-only in this
packet. It leaves the legacy source-observation value/view/result/key and the
observed key/carrier/typed outer private to
`root_apparent_repository_source_observation.rs`, with zero production callers.

Design only the minimum `pub(super)` surface required by a future runtime
sibling:

- the existing semantic certificate, borrowed view/disposition, their read-only
  accessors, opaque semantic error, concrete Result alias, legacy key and its
  existing three-argument `Option<Self>` constructor;
- the existing observed key and constructor, Result-Arc+epoch carrier with
  borrowed concrete accessors, and one opaque field-private typed outer; and
- exactly one test-only sibling smoke in `runtime/mod.rs` proving that another
  runtime module can name both keys, associated Values, concrete Result/view,
  carrier accessors and opaque outer without constructing private state or
  computing either key.

Keep every field, semantic error kind, observed outer inner/variant, driver
mode, reducer and helper private. Rename the current observed outer enum to a
private inner and wrap it only at the observed Key projection. Add no
constructor, conversion, inspector, alias beyond the existing semantic Result,
crate-root export, adapter, copied carrier, semantic caller or compute edge.

Preserve exact legacy and observed key identity, root-name rejection and
Display. For `/workspace`, `@first`, `pkg/file.bzl`, the observed Display remains
exactly:

`observed-HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }`.

The future sibling smoke may construct only keys, assert their existing
Display/root rejection, and use nonexecuted exact function-pointer/type checks.
It must not construct a certificate, error, carrier or outer; inspect a private
kind/variant; call `compute`; or activate package, command or bootstrap work.
Existing three observed tests, legacy tests, helpers and semantic assertions
remain frozen except the minimum wrapper/source-shape spelling.

## Consumer-frontier decision

Do not publish or activate the raw terminal carrier. Existing public
`RootRepositoryRoute` and its package source/load owners admit only builtin or
direct-local roots and cannot represent the extension-generated
`@rust_toolchains` chain. Exact generated BUILD loading also still requires a
separate owner for canonical deleted-package policy, `REPO.bazel` plus
`.bazelignore`, and ordered `BUILD.bazel` then `BUILD` selection with complete
epoch composition.

After the visibility implementation is accepted, return only to
`WP-6-7A-generated-repository-package-publication-frontier-audit`. That audit
must choose the smallest policy/lookup/source/load owner or prerequisite; it
must not assume that raw-source publication or the current narrow public route
is sufficient.

## Authority, caps and Linux-first validation

This packet may edit only canonical/current/Stage 6/routing documentation. All
Rust, tests, fixtures, oracles, Cargo/BUILD, APIs, callers and other plans are
read-only.

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`,
  baseline 1,866 physical lines/tests at 562/SHA-256
  `a4b89ce073f70454be89cf17df35fc52d513210d0b075733902be58ee897e993`;
- test-only `app/slug_core_v2/src/runtime/mod.rs`, baseline 251 physical lines/
  SHA-256
  `c52a11c0e082e76cb604ea30798600f07ddbf023b7abfd96f590d515335093a4`.

Design within <=90 owner production, <=40 owner proof, <=80 sibling proof and
<=210 aggregate additions; physical ceilings are 1,970/340. Add no production
helper, owner test or `rustfmt::skip`; add exactly one sibling smoke below 100
lines. Both files remain cohesive and the owner stays below 2,000 lines. This
is visibility-only and does not trigger hot-path representation work.

The successor must run serially under Ubuntu 24.04 WSL: exact sibling smoke;
the three observed source-observation tests; protected legacy source-
observation and observed source-path/source-input suites; full core with only
the byte-identical accepted query diagnostic baseline; separate runtime with
only the `c8d2d0b5`-identical accepted failure and 12 passes; direct commands
check; formatting; exact two-file baseline/SHA/allowlist/accounting/physical/
visibility/wrapper/source-shape/no-skip and diff hygiene. Add no Windows or
macOS gate, platform abstraction or conditional implementation in this packet.

Legacy semantic values, errors, staging, lower events and equality remain
**exact** Bazel 9 compatibility. Observed Result-Arc+transaction-local epoch
identity/invalidation and same-crate opaque visibility remain **Slug-native**. Package policy,
lookup/source/load, public command/bootstrap activation, other platforms and
exact identity bytes remain **unsupported/deferred** for this packet.

STOP Rust/tests now, public/crate-root exposure, private field/kind/variant
exposure, new key/carrier/adapter, compute/caller/event/semantic/equality/
retention/lifecycle change, package/public/bootstrap activation, Cargo/BUILD,
fixture/oracle, cap/format/test waiver, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or baseline drift.

Design ACCEPT may schedule exactly
`WP-6-7A-host-root-apparent-repository-source-observation-observation-carrier-visibility-implementation`,
then return to the generated repository package/publication frontier audit.
M7 remains partial and M7A -> M8 -> M7B remains.

## Historical implementation contract

## Goal and authority

Implement only the callerless private observed owner for root apparent
repository source observation. Preserve the legacy result/view/error surface,
exact path-first conditional-Host-observation staging and every public path.

Rust authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`.
Every second Rust/API/export/caller file, fixture/oracle, Cargo/BUILD and the
four orchestration records are read-only during implementation.

## Exact private types and identity

Add private
`HostRootApparentRepositorySourceObservationObservationKey`, nominally wrapping
`HostRootApparentRepositorySourceObservationKey`. Its exact constructor is
`fn new(NormalizedAbsolutePath, ApparentRepoName, PathBuf) -> Option<Self>` and
delegates to the legacy constructor. Preserve root rejection, all three key
fields in structural equality/hash and exact `observed-{legacy Display}`. For
`/workspace`, `@first`, `pkg/file.bzl`, Display is:

`observed-HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }`.

Add private `ObservedHostRootApparentRepositorySourceObservation` with exact
`Debug, Clone, PartialEq, Eq, Allocative, Dupe`, private fields
`Arc<HostRootApparentRepositorySourceObservationResult>` and
`PathObservationEpoch`, and borrowed `result()`/`observations()` accessors.

Add private
`HostRootApparentRepositorySourceObservationObservationError` with exact
`Debug, Clone, PartialEq, Eq, Allocative`, manual Dupe and the sole variant:

`SourcePath(HostRootApparentRepositorySourcePathInputObservationError)`.

It is carrierless and exposes no lower variant. Add no alias, visibility,
export, conversion, inspector or caller.

## Exact driver and finisher

Add `RootApparentRepositorySourceObservationMode::{Legacy, Observed}` and one
shared driver returning
`SourcePreparationOutcome<Result<(Arc<HostRootApparentRepositorySourceObservationResult>, PathObservationEpoch), HostRootApparentRepositorySourceObservationObservationError>>`.
Factor exactly one pure completed-child finisher; reuse the existing value/view,
error kinds and certificate construction rather than duplicating them.

The first stage is always source path:

- Legacy computes `HostRootApparentRepositorySourcePathInputKey` and pairs every
  completion with `PathObservationEpoch::empty()`.
- Observed computes only
  `HostRootApparentRepositorySourcePathInputObservationKey`. Need returns
  immediately. Its opaque outer maps to parent `SourcePath` with no carrier.
  DICE compute failure remains the legacy semantic SourcePathCompute Result
  with empty epoch. Success supplies the exact child Result Arc and epoch.

The finisher maps completed path semantic failure to SourcePath, inconsistent
successful view to InvalidSourcePath and lawful Main to success with no second
child. All retain the exact predecessor Arc and forward the path epoch.

Only lawful Input continues to the unchanged
`HostRepositorySourceObservationKey`. This child has no observed counterpart or
epoch carrier. Need returns immediately and is carrierless. DICE failure maps to
ObservationCompute with the path prefix. Completed semantic failure maps to
Observation, inconsistent/wrong-polarity success to InvalidObservation and a
lawful observation to success. Every row retains the exact predecessor and,
where present, observation Arc and forwards the path epoch unchanged.

There is no second epoch, union, merge, OperationMismatch, rebuild, fallback,
parallel join or direct Host read. Legacy Key delegates to the driver, asserts
empty epoch and returns the existing outcome. Observed Key projects only Need,
the typed SourcePath outer or the local Result-Arc+epoch carrier. Both keys keep
`complete_eq` equality and `is_complete` validity.

## Events, retention and lifecycle

The parent emits no event batch. Legacy dependencies are legacy path then
conditional Host observation; observed dependencies are observed path then the
same conditional Host observation. SourcePath/InvalidSourcePath/Main suppress
the second edge. Lower events remain lower-owned in exact order and every warm
parent row is batchless.

The carrier retains only its local Result Arc plus compact path epoch. The
Result already owns exact predecessor/optional observation Arcs. Observed path
carrier, views, disposition/input/path clones, event/tracker and finisher scratch
die before publication. Add no store/cache/interner/task/lock or command borrow.
DICE owns serialization and equality cutoff. Poll-drop publishes no partial
carrier; same-DICE recovery recomputes lawfully.

Held proof must cover mapping/policy/requested-path/source-content A-B-A. Path-
semantic changes alter Result and path epoch; source-content changes alter the
retained Host-observation Result without inventing a second epoch; a lawful
same-Result/different-path-epoch row invalidates the carrier. Each recovered
path epoch associates only with its same-transaction parent and global epoch.
Require Arc identity only for an exact same-transaction Reused row and compare
separate transactions semantically.

## Exact proof

Add exactly three tests:

- `observed_root_apparent_repository_source_observation_identity_staging_and_terminal_algebra`;
- `observed_root_apparent_repository_source_observation_real_order_events_and_parity`;
- `observed_root_apparent_repository_source_observation_lifecycle_cancellation_and_nonactivation`.

The identity test proves key/hash/Display/root rejection, accessors, Complete-
only equality/validity, pure finisher terminals, exact Arc+epoch forwarding and
no merge. It must not fabricate the lower opaque outer: reuse accepted lower
proof and require static private-producer/parent-mapping source shape.

The real-order test proves observed parent -> observed path -> conditional Host
observation, legacy parent -> legacy path -> the same conditional child,
first-terminal/Need suppression, exact Main/Builtin/Request success and error
legacy semantic parity, retained Arcs, lower event vectors and warm silence.

The lifecycle test proves the held A-B-A and epoch associations above, lower
carrier/scratch release, poll-drop recovery and zero legacy-source-observation,
public `RootRepositoryRouteKey`/observation, command and bootstrap activation.
Use only lawful real child values; add no malformed epoch/private injection.
Preserve the accepted sibling smoke and every legacy test/assertion.

## Baseline, caps and executable validation

Entry is 932 physical lines with `#[cfg(test)]` at 340 and SHA-256
`57f95be85cceb9a02d04ecff400b9c526837daecf2b54aa111afe3366650396a`.
Caps are <=300 production additions, <=720 proof additions, <=1,020 aggregate
additions and <=2,000 physical lines; at most six production/eight proof
helpers, exactly three new tests, driver below 180 and every changed helper/test
below 200. Add no `rustfmt::skip` and allow no formatter/cap/test waiver. The
one file remains the cohesive value/error/view/reducer/test owner and is not a
demonstrated hot path or retained-representation change.

Before editing, verify the entry SHA and capture both accepted baseline
diagnostics. Then run post-edit gates serially:

1. `cargo test -p slug_core_v2 observed_root_apparent_repository_source_observation_ -- --nocapture`;
2. `cargo test -p slug_core_v2 root_apparent_repository_source_observation -- --nocapture`;
3. `cargo test -p slug_core_v2 observed_root_apparent_repository_source_path_input_ -- --nocapture`;
4. `cargo test -p slug_core_v2 observed_root_apparent_repository_source_input_ -- --nocapture`;
5. full `cargo test -p slug_core_v2`; only the byte-identical accepted library
   query diagnostic baseline may fail;
6. separately `cargo test -p slug_core_v2 --test runtime`; only the accepted
   `c8d2d0b5`-identical `PathObservationEpochKey`/configured-analysis-Needs
   failure may remain while the other 12 tests pass;
7. `cargo check -p slug_commands_v2`;
8. `cargo fmt --all -- --check`; and
9. exact one-file allowlist, entry SHA, production/proof/aggregate/physical,
   helper/test/driver spans and counts, dependency/source-shape/no-skip/
   nonactivation checks plus `git diff --check`.

The two known failures are baseline accounting, not waivers. Compare exact
diagnostic bytes; any changed/additional failure is a STOP. Reuse accepted
source-path/Host-observation and Bazel source evidence; add no fixture/oracle.

## Compatibility and stops

Existing source-observation values/views/errors, child order, Result identity,
equality/invalidation and lower event behavior remain **exact** Bazel 9
compatibility. The private observed key/carrier/typed outer and Result-Arc+
transaction-local path epoch are **Slug-native**. Carrier visibility, any
caller/public-command/bootstrap activation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

STOP second file/key/owner/adapter, visibility/export/caller/public-route
change, observed Host-observation variant/accessor, second epoch/merge/mismatch/
rebuild, legacy semantic/order/error/event/equality/retention/lifecycle drift,
retained scratch/task/lock, private/malformed injection, fixture/oracle,
helper/test/cap/format waiver, changed/additional validation failure, milestone
closure, M8/M7B or exact identity work. REPLAN before widening or baseline
drift.

## Terminal

ACCEPT returns only to a docs-only terminal source-observation carrier/
publication consumer-frontier audit. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `597df31b` proves the callerless terminal owner is the uniquely
smallest slice and freezes the one-prefix/no-merge contract and entry baseline.
