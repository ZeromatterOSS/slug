# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repository-ignore-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `9c0a5473`
Rust base: `b08b7f2e`
Accepted design: `9c0a5473`

## Objective and exact authority

Implement the accepted private nonregistry repository-ignore observation owner.
Write only `app/slug_bzlmod_v2/src/repository_ignore.rs` from its 3,297-line
`b08b7f2e` baseline. Cap production at <=180 lines, proof at <=400,
aggregate semantic growth at <=580 and final physical size at <=3,900.
Touched helpers remain below 200 lines; the file is the cohesive owner/proof
exception. Every other Rust, Cargo, BUILD, fixture, oracle, caller and public
file is read-only.

## Frozen identity and retained value

Add one private structural
`HostNonregistryRepositoryIgnoreObservationKey(HostNonregistryRepositoryIgnoreKey)`
and one private `ObservedHostNonregistryRepositoryIgnore`. Its DICE value is
`SourcePreparationOutcome<Result<ObservedHostNonregistryRepositoryIgnore,
ObservedPathFrontierError>>`. The carrier retains exactly one local
`Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>` plus one
compact `PathObservationEpoch`, derives `Dupe`/`Allocative`, and exposes
borrowed result/epoch accessors. Add no export or caller.

## Shared driver and exact order

Use one Legacy/Observed driver and preserve exact repo -> source -> parser
order. Legacy selects only `HostNonregistryRepoFileKey`, then
`RepositorySourceFileKey`, and moves the exact local matcher Result Arc.
Observed selects only `HostNonregistryRepoFileObservationKey`, then
`RepositorySourceFileObservationKey`, and uses
`parse_ignore_file_observed`. Neither sibling computes the other family.
Parser grammar, platform behavior and matcher construction remain shared and
exact. Both modes retain the same neutral Windows long-path parser dependency
when that parser path is reached.

Repo Need/typed outer returns immediately, carrierless. Accept the complete
repo epoch before semantic inspection; repo semantic failure retains the
repo-only prefix and suppresses source work. After repo success, source
Need/typed outer is carrierless. Union the accepted repo prefix left-first with
the complete source epoch before source semantic inspection. Source
error/Absent/Directory/Present retains repo+source; Absent/Directory suppress
parser work.

Present invokes `parse_ignore_file_observed`. Parser Need/typed outer is
carrierless. Union the accumulated repo+source prefix left-first with the
complete parser epoch before parser semantic inspection. Parser error/success
retains the full prefix. Equal duplicates preserve the earliest exact Arc;
conflicting values and operation mismatch are typed outer. This sequential
owner has no Need union.

Need is invalid/self-unequal. Complete outer is valid/equal by outer value.
Complete carrier is valid/equal by semantic Result plus epoch. Preserve every
existing legacy DICE invariant and error class.

## Events, family isolation and memory

Both ignore siblings remain eventless. The matching REPO-file child remains
sole owner of its local REPO batch; repository source and parser observations
remain eventless. Semantic Complete includes the child batch exactly as before;
Need/outer/cancellation stores no parent state; warm reuse emits nothing.

Observed dependency rows contain observed REPO/source families plus neutral
Windows parser observations when reached. Legacy rows contain legacy
REPO/source families plus that same neutral parser observation. Retain no child
carrier/source bytes/parser scratch, second collection, cache/interner/store,
lock/task, direct Host read, revision, certificate or event state. Activate no
package preflight, closure, discovery, selected graph, registry preparation,
extension or public caller.

## Required proof and validation

Discriminate:

- distinct key equality/hash and Display; carrier accessors,
  `Dupe`/`Allocative`, Complete/Need/outer equality and validity;
- the production reducer at repo/source/parser positions, carrierless
  Need/outer and later suppression;
- real repo/source Need and typed outer; repo semantic; source
  error/Absent/Directory/Present; parser syntax/absolute/platform terminals
  with exact prior/merged/full prefixes;
- exact epoch iteration order and per-demand `Arc::ptr_eq`, duplicate-first,
  conflict and operation mismatch;
- on Windows, real WindowsLongPath Need/outer/append through the existing
  parser;
- exact observed/legacy dependency rows, reverse-family isolation and the
  neutral Windows parser edge in both modes;
- exact child-owned REPO batch/order/text, parent/source/parser silence, warm
  suppression and poll-drop same-DICE recovery;
- independent local and immutable REPO/`.bazelignore` A -> B -> absent ->
  directory -> A matcher restoration with held Result/epoch handles and exact
  restored child-to-parent Arc identity; and
- zero preflight/closure/discovery/selected-graph/registry/extension/public
  activation.

Run focused ignore proof, full bzlmod, affected loading/query/core baselines,
`cargo fmt --all -- --check`, `git diff --check b08b7f2e`, exact cap
accounting and AI-cleanup/Buck2 retention review. Reuse accepted evidence; add
no fixture or Bazel oracle because semantic grammar, platform policy, errors
and events are unchanged.

## Compatibility and STOP

Exact: existing nonregistry REPO -> `.bazelignore` -> parser ordering,
grammar/platform behavior, matcher values/errors and every legacy child event.
Slug-native: private sibling, Result-Arc+epoch carrier and typed outer.
Unsupported/deferred: package preflight/closure/discovery/selected graph;
registry preparation/patches; extension-generated repositories, M8/M7B and
exact identity bytes.

STOP a second Rust file/key/caller/export, parser or legacy behavior drift,
event-owner change, extra retained state, direct Host read, upper/registry
activation, cap excess, M7A closure, M8/M7B/M9 or a second successor. REPLAN if
the owner cannot fit the frozen one-file envelope without weakening exact
behavior.

After independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.
