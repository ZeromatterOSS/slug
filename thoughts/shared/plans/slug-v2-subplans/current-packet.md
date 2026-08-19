# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repository-ignore-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `b08b7f2e`
Rust base: `b08b7f2e`
Accepted predecessor: `WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry-2`

## Accepted predecessor completion

Commit `b08b7f2e` accepts the private observed nonregistry REPO-file sibling.
Against Rust base `12f68983`, `repo_file.rs` adds +170 production and +718
proof lines, +888 aggregate semantic, at 3,567 physical lines. Focused proof is
3/3; full bzlmod is 442 unit plus 193 integration; loading is 204/204 and query
is 121/121. Core retains only the documented 245/246 stale visibility wording
and 12/13 legacy snapshot-adapter Need baselines. Formatting, diff hygiene,
cleanup/Buck2 retention and independent review are accepted.

The implementation preserves source-first matching-family order, exact local
REPO event ownership, carrierless Need/typed outer and one local semantic
Result Arc plus the compact source epoch. No caller or upper owner was
activated.

## Docs-only design authority

Write only the canonical plan, this manifest, Stage 6 and the routing log at
<=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust, Cargo, BUILD,
fixtures and oracles are read-only during this design.

## Natural owner and frozen identity

`HostNonregistryRepositoryIgnoreKey` is the uniquely smallest complete next
owner. Its exact existing order is event-owning
`HostNonregistryRepoFileKey`, then `.bazelignore`
`RepositorySourceFileKey`, then the existing ignore parser. Both semantic
children now have accepted observed siblings. The parser's only additional
mutable edge is Windows `PathObservationKey::windows_long_path`, already owned
by `parse_ignore_file_observed` and returned as a compact epoch with
Need/typed-outer polarity. No lower carrier remains.

Freeze one private structural
`HostNonregistryRepositoryIgnoreObservationKey(HostNonregistryRepositoryIgnoreKey)`
and one private `ObservedHostNonregistryRepositoryIgnore`. Its carrier is
exactly one local
`Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>` plus one
compact `PathObservationEpoch`, with `Dupe`/`Allocative` and borrowed
result/epoch accessors. Add no export or caller.

## Shared driver, order and algebra

Use one Legacy/Observed driver and preserve exact repo -> source -> parser
order. Legacy selects only `HostNonregistryRepoFileKey`, then
`RepositorySourceFileKey`, and projects the exact local matcher Result Arc.
Observed selects only `HostNonregistryRepoFileObservationKey`, then
`RepositorySourceFileObservationKey`, and uses
`parse_ignore_file_observed`. Neither sibling computes the other; parser
grammar, path flavor and matcher construction stay shared and exact.

Observed repo Need/typed outer returns immediately, carrierless. Accept the
complete repo epoch before semantic inspection; repo semantic failure retains
the repo-only prefix and suppresses source work. After repo success, source
Need/typed outer returns immediately, carrierless. Union the accepted repo
prefix left-first with the complete source epoch before source semantic
inspection. Source error, Absent, Directory and Present retain the merged
repo+source prefix; Absent and Directory suppress parser work.

Present invokes `parse_ignore_file_observed`. Parser Need/typed outer returns
immediately, carrierless. Union the accumulated repo+source prefix left-first
with the complete parser epoch before parser semantic inspection. Parser error
and matcher success retain the full prefix. Equal duplicate demands preserve
the earliest exact Arc; conflicting values and operation mismatch are typed
outer. This sequential owner has no Need union.

Need is invalid/self-unequal. Complete outer is valid/equal by outer value.
Complete carrier is valid/equal by semantic Result plus epoch. Preserve all
existing DICE-invariant behavior and semantic error classes.

## Events, families and retention

Both ignore siblings remain eventless. Their matching REPO-file child remains
sole owner of its local REPO batch. Repository source and parser path
observations remain eventless. Need/outer/cancellation stores no parent state;
warm parent reuse emits nothing.

Retain exactly the local matcher Result Arc plus compact cumulative epoch. The
REPO/source child carriers, source bytes, logical path, parsed prefix Vec and
union scratch remain dependency-owned or compute-local. Add no second carrier
Arc, retained child collection, cache, interner, store, lock, task, direct Host
read, revision, certificate or event state.

Observed direct dependency rows contain only the observed REPO and source
families reached in exact order plus neutral parser path observations on
Windows. Legacy rows contain only legacy REPO/source families plus the same
neutral parser path observations when reached on Windows. Activate no
package preflight, closure, discovery, selected graph, registry preparation,
extension or public caller.

## Exact future Rust authority and caps

After independent design ACCEPT, write only
`app/slug_bzlmod_v2/src/repository_ignore.rs` from the 3,297-line
`b08b7f2e` baseline. Cap production at <=180 lines, proof at <=400,
aggregate semantic growth at <=580 and final physical size at <=3,900.
Touched helpers remain below 200 lines; the file is the cohesive owner/proof
exception.

## Required proof

Discriminate:

- distinct key equality/hash and Display; carrier accessors,
  `Dupe`/`Allocative`, Complete/Need/outer equality and validity;
- the production reducer at repo, source and parser positions, including
  carrierless Need/outer and later-child suppression;
- real repo/source Need and typed outer; repo semantic; source
  error/Absent/Directory/Present; parser syntax, absolute-path and
  platform-specific terminals with exact prior/merged/full prefixes;
- exact epoch iteration order and per-demand `Arc::ptr_eq`; earliest duplicate
  Arc, conflict and operation-mismatch behavior;
- on Windows, real WindowsLongPath Need/outer/append through the existing
  parser without inventing another owner;
- exact observed/legacy dependency rows and reverse-family isolation;
- exact child-owned REPO batch/order/text, parent/source/parser silence, warm
  suppression and poll-drop cancellation/same-DICE recovery;
- independent local and immutable REPO/.bazelignore A -> B -> absent ->
  directory -> A matcher restoration, with held Result/epoch handles and exact
  restored child-to-parent Arc identity; and
- zero preflight/closure/discovery/selected-graph/registry/extension/public
  activation.

Run focused ignore proof, full bzlmod, affected loading/query/core baselines,
fmt, diff-check, exact cap accounting and AI-cleanup/Buck2 retention review.
Reuse accepted evidence; add no fixture or Bazel oracle.

## Compatibility

Exact: current nonregistry REPO -> `.bazelignore` -> parser ordering, ignore
grammar/platform behavior, matcher values/errors and every legacy child event.
Slug-native: the private sibling, Result-Arc+epoch carrier and typed outer.
Unsupported/deferred: observed package preflight, closure, discovery/selected
graph; registry preparation/patches; extension-generated repositories,
rules_rust actions, M8/M7B and exact identity bytes.

## STOP and sole successor

STOP Rust/Cargo/BUILD/fixture/oracle writes during design. STOP a second Rust
file/key/caller/export, parser or legacy behavior drift, event-owner change,
extra retained state, direct Host read, upper/registry activation, cap excess,
M7A closure, M8/M7B/M9 or a second successor. REPLAN if the owner cannot fit
the frozen one-file envelope without weakening exact behavior.

After independent design ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-implementation`.
After independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.
