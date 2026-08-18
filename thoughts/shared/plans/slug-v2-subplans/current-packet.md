# Current Slug V2 Packet

Packet: `WP-2A-m1-routed-repository-policy-observation-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `4381bc61`
Rust base: `e4ee0a8e`
Semantic design: `7f60a5c4`
Proof/cap correction: `4381bc61`
Result: complete and accept only the retained two-file routed REPO/ignore
candidate with the corrected discriminating proof.

## Authority and caps

Write exactly:

- `app/slug_bzlmod_v2/src/repo_file.rs`; and
- `app/slug_bzlmod_v2/src/repository_ignore.rs`.

Against Rust base `e4ee0a8e`, cap `repo_file.rs` at 120 production plus
280 test lines and 2,700 physical lines; cap `repository_ignore.rs` at 160
production plus 360 test lines and 3,350 physical lines. Aggregate semantic
growth is capped at 920 and combined physical size at 6,050. Current retained
candidate accounting is +119 production/+157 tests at 2,557 and +157
production/+201 tests at 3,141: +634 semantic and 5,698 physical combined.

## Frozen implementation contract

Preserve the retained candidate's structurally distinct crate-private
`HostRouteRepoFileObservationKey` and
`HostRouteRepositoryIgnoreObservationKey`, one mode-aware driver per
legacy/observed pair, and matching legacy versus observed
`HostRepositorySourceFile{,Observation}Key` activation. Neither sibling may
compute the other family, `ExternalRepositoryPackageLookupKey`, or an upper
loading key.

Each observed carrier retains exactly one semantic Result Arc of the legacy
value type plus one Arc-backed `PathObservationEpoch`, is
`Allocative`/cheaply cloneable, and exposes only borrowed crate-visible
accessors. Need has no carrier. Typed source/parser/epoch outer remains outer.
Semantic policy/source/parse/evaluation/ignore errors remain inside a Complete
carrier and only Complete values are valid/equal.

Preserve policy before routed `REPO.bazel` source before evaluation. Policy
projection failure is semantic with an empty epoch and no source activation.
Every completed source epoch is retained before semantic inspection. Missing
source produces the legacy empty value; source/evaluation errors retain that
source prefix.

Preserve routed REPO before routed `.bazelignore` source before parser
observations. Union completed epochs left-first with
`PathObservationEpoch::from_shared` before semantic inspection. Equal
duplicates retain the earlier exact Arc; mismatch/conflict is typed outer.
Semantic REPO retains only its prefix. Missing/directory ignore source keeps
legacy empty behavior. Parser-specific operations join last; parser semantic
errors retain the reached full prefix, while parser Need/outer has no carrier.

The legacy and observed routed REPO keys remain the sole local Complete batch
owners for their respective families. Ignore parents store no batch.
Source/parser children keep existing ownership. Need, typed outer, and
cancellation publish no parent batch. Preserve cold child-before-parent order,
semantic-error batches, cancellation discard, recovery, and warm suppression.

Retain no route graph, parser vector, prefix list, queue, store, cache,
interner, lock, task, or direct Host read. Evaluation buffers, union inputs,
and parser scratch remain compute-local. Completed keys retain only the
semantic Result Arc, Arc-backed epoch, and existing DICE-owned REPO event
batch. Routed semantics/events remain exact; observed identity/carrier/outer
mechanics remain Slug-native; upper external loading/query stays deferred.

## Corrected proof

Production semantics are frozen. Permit only test-only helper restructuring
and focused proof beyond the retained candidate.

Real route computations must prove:

1. policy-before-source, REPO-source-before-ignore-source-before-parser, and
   exact cold child-before-parent activation/event order;
2. exact legacy/observed semantic and event parity for success, missing,
   wrong-kind/source, REPO parse/evaluation, and ignore parse errors;
3. empty/REPO/source/full decisive epochs and exact demand/value/
   `Arc::ptr_eq` membership;
4. source Need, a genuinely polled cancellation, no partial batch, successor
   recovery, warm suppression, edit/delete/recreate, and A/B/A; and
5. both family-isolation directions and zero upper lookup/package activation.

Keep corruption/platform proof separate: exercise left-first equal duplicates,
mismatch/conflict typed outer, Complete-only validity, and no Need/outer carrier
at the route union/key-value algebra. Exercise WindowsLongPath parser
operation, Need, and typed-operation outer only through the existing
platform-guarded parser seam. Do not invent a Unix path or production test
hook. Existing lower source/parser tables may supply file-kind, symlink, UTF-8,
and parser cases only when route proof preserves their corresponding terminal
and decisive prefix.

Run serially:

1. route semantic/outer/cancellation tests individually, then their
   default-parallel batch;
2. full `slug_bzlmod_v2`, `slug_loading_v2`, and `slug_query_v2`;
3. established `slug_core_v2` library/runtime checks, recording only the
   unchanged inherited baselines; and
4. fmt, `cargo check -p slug_bzlmod_v2`, `git diff --check e4ee0a8e`,
   exact accounting, Buck2 retention scan, AI cleanup categories 1-9, and
   independent latest-correction review.

After `ACCEPT`, commit this Rust packet and schedule exactly one docs-only
external package source/load frontier design. Do not activate query or close
M1.

## STOP / REPLAN

STOP on any other file; Cargo, BUILD, fixture, oracle, generated-file, or
public-export write; production semantic/ownership change; a production test
hook; upper lookup/package/loading/query activation; a third family; mixed
source families; reconstructed Result Arcs; inspection before union; partial
carrier; moved/duplicate event ownership; retained scratch/new state; false
platform proof; cap excess; multiple successors; or M1 closure.

`REPLAN` if discriminating proof requires a production seam, another
owner/file, the existing platform parser seam cannot cover its declared
algebra, or corrected caps still cannot contain the proof.

## Immediate predecessor

`4381bc61` independently accepts the formal proof-cap correction after the
sound retained candidate exhausted its original 22-line test headroom. It
keeps production semantics/caps fixed, separates real computation from
corruption/platform algebra, and authorizes only this bounded retry.
