# Current Slug V2 Packet

Packet: `WP-2A-m1-routed-repository-policy-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `e4ee0a8e`
Rust base: `e4ee0a8e`
Result: freeze only the observed route-local REPO/ignore producer boundary
required before external package lookup; do not implement or activate lookup,
loading, or query.

## Authority and accounting

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

This packet is documentation-only. Net growth is capped at 40 canonical, 180
manifest, 180 Stage 2, 30 routing-log, and 410 aggregate lines. Rust, Cargo,
BUILD, fixtures, oracles and generated files are not writable.

## Audit result and natural owner

The resumed external package source/load audit formally `REPLAN`s before
`RepositoryPackageSourceKey`. `ExternalRepositoryPackageLookupKey` computes
`HostRouteRepositoryIgnoreKey` before BUILD marker paths. The routed ignore
key computes `HostRouteRepoFileKey`, then
`HostRepositorySourceFileKey(.bazelignore)`, then parser observations.
`HostRouteRepoFileKey` computes
`HostRepositorySourceFileKey(REPO.bazel)` and is the sole owner of its local
Complete REPO event batch. These route-local producers retain no complete path
epoch.

Absorbing the work into the package-key identity would duplicate route-wide
REPO/ignore evaluation per package, move REPO event ownership, and still miss
`DirectLocalModuleSupport` include horizons that re-enter
`ExternalRepositoryPackageLookupKey`. The uniquely smaller prerequisite is
one observed sibling for each existing route-local producer in its current
owner: `HostRouteRepoFileObservationKey` in `repo_file.rs`, followed by
`HostRouteRepositoryIgnoreObservationKey` in `repository_ignore.rs`.
Both remain crate-private; no `lib.rs` export is needed.

## Frozen key and carrier contract

Give each legacy/observed pair one private mode-aware driver. Legacy computes
only `HostRepositorySourceFileKey`; observed computes only the accepted
`HostRepositorySourceFileObservationKey`. Neither sibling computes the other
or constructs an upper package/lookup key.

Each observed value is
`SourcePreparationOutcome<Result<Carrier, ObservedPathFrontierError>>`.
A Complete carrier retains exactly one semantic Result Arc of the legacy value
type plus one Arc-backed `PathObservationEpoch`, is `Allocative` and cheaply
cloneable, and exposes only borrowed crate-visible accessors. Need returns
immediately without a carrier. Typed source/parser/union outer errors remain
outer. Semantic policy, source, parse, evaluation and ignore errors remain
inside the Result and are valid/equal exactly when Complete.

The routed REPO driver preserves the live order: policy projection first, then
the selected routed `REPO.bazel` source, then evaluation. A pre-source policy
error completes with an empty epoch. In observed mode, append no reconstructed
value: forward the source sibling's exact epoch before inspecting source
semantics. Missing source evaluates as the existing empty REPO value.
Source/evaluation semantic errors retain the decisive source prefix.

The routed ignore driver preserves exact order: observed routed REPO carrier
first, observed `.bazelignore` source second, and ignore parser observations
third. Union with stable `PathObservationEpoch::from_shared` in that left-first
order before inspecting each semantic terminal. Equal duplicates keep the
earlier exact Arc; mismatch/conflict is typed outer. Missing/directory ignore
source preserves existing empty behavior. Parser path operations, including
WindowsLongPath variants where selected, join last. A semantic terminal retains
only the prefix through its decisive child; Need/outer retains none.

## Events, memory and compatibility

The legacy routed REPO key remains its family's local batch owner. The observed
routed REPO sibling owns exactly one corresponding local Complete batch and
stores none on Need, typed outer, or cancellation. The routed ignore siblings
store no batch. Parents add no batch; source/parser children keep their existing
ownership. Preserve cold child-before-parent event order and warm suppression.

Retain no route graph, parser vector, prefix list, queue, store, cache, interner,
lock, task or direct Host read. Evaluation buffers, union inputs and parser
scratch are compute-local. After completion, only the semantic Result Arc,
Arc-backed epoch, and the existing DICE-owned local REPO event batch survive.
Apply the Buck2 retained-state scan and AI cleanup categories 1-9.

Routed REPO/ignore values, errors, ignored-prefix behavior, UTF-8 modes and
events remain exact. Structural observed identity, carrier association and
typed outer are Slug-native. External package lookup/source/load, recursive
external `.bzl`, loading query, multi-build, one-shot evaluation and exact
identity bytes remain deferred.

## Future implementation envelope and proof

After independent design ACCEPT, schedule one implementation from Rust base
`e4ee0a8e` writing exactly:

- `app/slug_bzlmod_v2/src/repo_file.rs`; and
- `app/slug_bzlmod_v2/src/repository_ignore.rs`.

Against that base, cap `repo_file.rs` at 120 production plus 170 test lines
and 2,600 physical lines; cap `repository_ignore.rs` at 160 production plus
210 test lines and 3,200 physical lines; cap aggregate semantic growth at 660
and combined physical size at 5,800. Current physical bases are 2,281 and
2,783. Keep tests colocated; neither file needs a split under these caps.

Proof must discriminate:

- structural key identity, exact legacy semantic/event parity, Complete-only
  validity/equality, and both directions of family nonactivation;
- policy-before-source, REPO-source-before-ignore-source-before-parser order,
  empty and decisive semantic prefixes, exact demand/value/`Arc::ptr_eq`
  membership, equal-duplicate first Arc, and union mismatch/conflict;
- source/parser Need, injected typed outer, semantic errors, cancellation with
  no batch, recovery, and cold child-before-parent event order;
- missing/directory/regular/special/symlink REPO and ignore files, ignored
  prefix behavior, parser path observations, UTF-8/evaluation errors, warm
  suppression, edit/delete/recreate and A/B/A; and
- no upper lookup/package activation plus compact post-return retention.

Run focused routed REPO/ignore tests (cancellation isolated and default
parallel), full `slug_bzlmod_v2`, downstream `slug_loading_v2` and
`slug_query_v2`, established core baselines, fmt/check/diff/accounting,
retention/cleanup, and independent latest-diff review.

## STOP / REPLAN and successor

STOP on implementation in this packet; any future third Rust file or public
export; upper lookup/package/loading/query activation; computing both source
families; reconstructed Result Arcs; semantic inspection before union; partial
carrier on Need/outer; moved/duplicate event authority; retained scratch or a
new store/cache/interner/lock/task/Host read; compatibility drift; cap excess;
multiple successors; or M1 closure.

`REPLAN` if the existing route producer cannot remain the REPO event owner,
the parser epoch cannot compose after the two source epochs, another owner/file
is required, or the frozen caps cannot build. After accepted implementation,
return directly to one docs-only external package source/load frontier design;
do not activate query or close M1.

## Immediate predecessor

`e4ee0a8e` accepts the observed routed Host path/source siblings at +330
production, +5 glue, +513 focused tests and +4 lib lines (+852 aggregate),
with 12,582/513/399 = 13,494 physical lines. Focused proof passes 6/6, full
bzlmod passes 405 unit plus 193 integration tests, loading 194 and query 120
pass, and the two established core baselines are unchanged. Independent review
accepts carrier completeness, family/event isolation, cancellation, retention,
caps and cleanup.
