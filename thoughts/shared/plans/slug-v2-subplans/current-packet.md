# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-public-migration-audit`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: audit the smallest loading/public consumer of the accepted private
root-host request-revision family and select one bounded implementation
successor. This packet is documentation-only.

## Fixed predecessor

Commit `207fe438` accepts the callerless one-file Host vertical designed in
`94324880` after the pinned Bazel 9.2 ordering evidence in `2ffad088`.
`WorkspaceRuntime` owns one `RequestRevisionRuntime` over its retained
`Arc<Dice>`; all five live production updater/commit sites share its async,
nonreentrant publication owner. The private root key consumes a Slug-native
revision plus the existing typed `PathObservationKey`, retains an exact
one-demand source certificate, final-reobserves under the publication owner,
and discards a stale provisional terminal before bounded retry. It retains no
transaction, evaluator, accepted semantic snapshot, worker, or manual cache.

The accepted proof covers contained and missing paths, relevant/irrelevant
overlay identity, serial V1/V2/warm/A-B-A/restoration, two genuinely
overlapping post-demand requests with V1 discarded and V2-only acceptance,
exact observation/commit/retry counters, one-waiter and last-waiter
cancellation, forced observation/injection/publication/nonprogress failures,
lock-state assertions, idle cleanup, and no leaked gate or publishable failure.
Focused `cargo test -p slug_core_v2 request_revision -- --nocapture` passes
7/7; `cargo check -p slug_core_v2` passes. The full suite passes 210 unit and
12 integration tests when two independently reproducible inherited failures
are skipped:

- `direct_external_query_uses_host_route_native_materialization_and_apparent_output`
  expects an older external-repository visibility diagnostic; and
- `loaded_custom_rule_reaches_analysis_and_declares_an_action` reaches the
  pre-existing legacy configured-analysis `Needs` boundary.

Fixing either would enter repository or public/loading work forbidden by the
implementation packet. Strict `clippy -D warnings` stops first in unchanged
`allocative_derive`; local `--all-targets --no-deps` finishes with no
`request_revision` warning. The targeted Bazel Rust test cannot analyze
because the checkout has no matching `rules_rust` toolchain. Diff hygiene and
two independent ownership/cleanup reviews pass.

Formatted accounting partitions every `#[cfg(test)]` range: 456/560 net
production lines, 648/700 in-module test lines, and 1,104/1,520 total net
lines. No cap correction was consumed.

## Audit question

Which single live loading/public path can first consume
`RequestRevisionRuntime::read_host_file` without widening the accepted
one-file Host family, duplicating DICE ownership, or changing unrelated
repository/materialization behavior?

The audit must inspect the live checkout and map exact symbols from public or
daemon request entry through `WorkspaceRuntime`, loading/Bzlmod/query
adapters, and the terminal producer. It must compare the already accepted
direct root exported-source/filegroup source-read path, including its exact
`BuildCommandRootKey`/source-terminal chain and Need/error/publication
ordering, against at least the root `MODULE.bazel` read, selected package
BUILD-file read, one `.bzl` load, and the loading-query adapter, then select
the smallest dependency-safe candidate
or record a prerequisite `REPLAN`.

For every candidate, record:

- the current snapshot/input keys, host-read owner, and complete Need/error
  ordering;
- whether the request is exactly one contained Host file-bytes demand or would
  require a directory/glob union, repository namespace, or materialization;
- the immutable semantic overlay fields that belong in root identity and the
  presentation/event fields that must remain outside DICE;
- the exact command/server caller, provisional terminal/effect boundary, and
  cancellation lifetime;
- whether the private `pub(super)` seam is sufficient and which production
  updater/commit sites can race it;
- the accepted oracle or pinned-source evidence that discriminates the
  candidate, and any demonstrated evidence gap; and
- a future exact file allowlist, production/test/ledger caps, focused proof,
  compatibility classification, STOPs, and `REPLAN` triggers.

## Compatibility and authority

The predecessor remains exact only for serial Host file
present/bytes/absence/error behavior and oracle-backed invalidation, warm reuse,
and restoration. Overlay identity, overlapping-request isolation, revision
numbers, final reobservation, no-mixed-epoch publication, and provisional
suppression are Slug-native. Directory/glob certificates,
repository/materialized sources, public overlapping commands, and historical
filesystem reads remain unsupported/deferred until separately admitted.

This audit cannot convert a Slug-native property into Bazel parity, treat the
Bazel client lock as concurrent-request evidence, or count a callerless private
proof as M1 completion. The current manifest remains the scheduling authority;
the compact predecessor above plus Git retains the implementation evidence.

## Allowlist and caps

Edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 220 current-packet, 220 Stage 2, and 480 total net
ledger lines. No Rust, Cargo/BUILD, oracle fixture, generated evidence, or
other ledger file is authorized. Read-only source inspection and focused
existing-test discovery are permitted; do not generate or update an oracle.

STOP on any code write, public API activation, command/server behavior change,
new DICE key/store, second graph, snapshot replacement, directory/glob union,
repository/materialization session, source observation, watcher claim,
historical host read, JVM work, evidence generation, or cap excess.

`REPLAN` if no one-file Host candidate preserves current Need/error/output
ordering within a bounded migration, if the first consumer inherently requires
multi-file or repository-session concurrency, or if accepted evidence cannot
discriminate the selected boundary.

## Acceptance and immediate successor

Accept only after independent review confirms the live symbol/call-chain
inventory, one uniquely smallest candidate, exact/Slug-native/deferred
classification, bounded file/cap/proof contract, and no hidden competing
publication path. Then activate only that candidate's implementation packet;
do not combine audit and Rust implementation.
