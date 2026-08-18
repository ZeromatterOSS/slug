# Current Slug V2 Packet

Packet: `WP-2A-m1-external-package-source-load-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `2a8dd968`
Rust base: `2a8dd968`
Result: audit the remaining external package lookup/source/load frontier and
freeze exactly one bounded observed-owner design, a uniquely smaller
prerequisite, or formal `REPLAN`.

## Authority

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Do not write Rust, Cargo metadata, BUILD files, fixtures, oracles, generated
files, or public/query callers. Docs caps are 40 net canonical, 220 Stage 2,
180 manifest, 30 routing, and 470 aggregate.

## Accepted predecessor

Commit `2a8dd968` accepts the route-local REPO/ignore observed producers from
semantic design `7f60a5c4` and proof-cap correction `4381bc61`. Final
accounting against `e4ee0a8e` is +119 production/+279 tests at 2,679 physical
lines in `repo_file.rs`, +157 production/+357 tests at 3,297 physical lines in
`repository_ignore.rs`, and +912 semantic/5,976 physical combined.

Focused routed tests pass 3/3 plus the union discriminator; full bzlmod,
loading, and query pass. The established core 234/235 and runtime 12/13
baselines remain unchanged. Fmt, check, diff/accounting, Buck2 retained-state,
AI cleanup, Windows-only proof portability, and independent latest-diff review
all pass. The accepted carriers retain only one semantic Result Arc and one
Arc-backed epoch; no store, lock, task, direct Host read, or event owner moved.

## Audit scope

Start from the live external stack, not the earlier carrierless audit:

- `ExternalRepositoryPackageLookupKey` policy, accepted routed-ignore, and
  ordered `BUILD.bazel`/`BUILD` marker-path edges;
- both consumers of that lookup: `RepositoryPackageSourceKey` and
  `DirectLocalIncludePackageHorizonKey`;
- direct-local module support, selected BUILD source, and exact FileBytes;
- `ExternalBzlModuleEvalKey` recursive `.bzl` source/load closure; and
- `RepositoryPackageLoadKey` BUILD parse/load/evaluation and event ownership.

Inventory every legacy/observed path, route-policy, source, lookup, module,
and package edge. Identify the first natural producer that can retain one
semantic Result Arc plus every exact reached path Result Arc without computing
a second family or moving an existing child event batch. Constructor syntax,
direct-local include re-entry, recursive `.bzl` order, Need/typed-outer/
semantic precedence, cancellation, and compact dependency-owned lifetimes are
part of completeness.

The terminal decision must be exactly one of:

1. freeze one bounded source/load observed design with exact future files,
   semantic/physical caps, proof, validation, retention, and one successor;
2. select one uniquely smaller observed prerequisite and freeze that design;
   or
3. record formal `REPLAN` with the conflicting ownership evidence.

Public `RootQueryCommandKey` remains legacy and unactivated. Multi-build,
one-shot publication, root-package loading, and identity-byte breadth remain
deferred. Exact external bytes/values/errors/labels/events stay exact;
structural siblings, carriers, and typed outer handling are Slug-native.

## STOP / REPLAN

STOP on implementation, public activation, a second legacy/observed subtree,
partial or reconstructed epochs, moved/duplicate event ownership, retained
scratch or a new store/cache/lock/task/Host read, missing future files/caps,
multiple successors, or M1 closure.

`REPLAN` if no single producer or bounded composition can cover direct package
selection, direct-local include re-entry, and recursive external `.bzl`
loading without family or event duplication. After independent design review,
schedule only its one bounded implementation or prerequisite; do not activate
query or close M1.
