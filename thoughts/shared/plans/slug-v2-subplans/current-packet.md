# Current Slug V2 Packet

Packet: `WP-2A-m1-routed-repository-policy-proof-cap-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `181964f0`
Rust base: `e4ee0a8e`
Retained unaccepted candidate: the dirty two-file routed REPO/ignore Rust diff
Result: correct only the proof contract and test/physical caps before resuming
the same implementation; do not change Rust in this packet.

## Authority

Write exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`;
  and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Against scheduling base `181964f0`, cap canonical-plan growth at 40 net
lines, Stage 2 at 140, this manifest at 200, the routing log at 30, and the
aggregate at 410. The two dirty Rust files are retained evidence but are not
writable during this design packet. STOP on Cargo, BUILD, fixture, oracle,
generated-file, or any other edit.

## Formal REPLAN evidence

The unaccepted implementation keeps the accepted two route-local natural
owners and passes `cargo check`, 406/406 bzlmod unit tests, the full bzlmod,
loading, and query suites, and the unchanged core 234/235 plus runtime 12/13
baselines. Independent latest-diff review finds no production ownership,
carrier, event, memory, or family-isolation defect.

Measured against Rust base `e4ee0a8e`, `repo_file.rs` is +119 production
and +157 tests at 2,557 physical lines; `repository_ignore.rs` is +157
production and +201 tests at 3,141 physical lines. Aggregate semantic growth
is +634 and combined physical size is 5,698. The original production and total
caps pass, but only 13 and 9 test lines remain.

The sole route-level composite proof covers success, exact selected Result
Arcs, both family directions, warm suppression, upper-key nonactivation, and a
real pending computation drop. It does not prove successor recovery, complete
route-level semantic prefixes, exact legacy parity, or edit/delete/recreate and
A/B/A. A real routed parser Need is Windows-only, while typed epoch outer is a
fail-closed corruption algebra rather than a constructible valid Host epoch.
Completing those discriminators in 22 lines would require weakening assertions
or an unauthorized seam. The frozen cap STOP therefore fired, and independent
review accepts formal REPLAN rather than cap excess or nondiscriminating proof.

## Frozen correction design

Retain the two-file candidate and every production semantic decision from
design `7f60a5c4`: structural crate-private observed siblings, one
matching-family driver per legacy/observed pair, one semantic Result Arc plus
one Arc-backed epoch, union before semantic inspection, Need/typed-outer/
semantic polarity, exact left-first Result Arcs, observed REPO local Complete
batch ownership, eventless ignore parent, and no upper lookup activation.

The implementation retry may write only:

- `app/slug_bzlmod_v2/src/repo_file.rs`; and
- `app/slug_bzlmod_v2/src/repository_ignore.rs`.

Production behavior and ownership are frozen. Permit only test-only helper
restructuring and focused route proof. Do not add a production key, branch,
injection hook, retained field, public export, dependency, event owner, Host
read, lock, task, cache, store, interner, or collection.

Route-level real-compute proof must cover:

1. policy-before-source, REPO-source-before-ignore-source-before-parser, and
   cold child-before-parent activation/event order;
2. exact legacy/observed semantic and event parity for success, missing,
   wrong-kind/source, REPO parse/evaluation, and ignore parse errors;
3. exact empty/REPO/source/full decisive epochs and `Arc::ptr_eq` membership;
4. source Need, real polled cancellation, no partial batch, successor recovery,
   warm suppression, edit/delete/recreate, and A/B/A; and
5. both family-isolation directions and zero upper lookup/package activation.

Keep corruption- and platform-only proof honest and separate. Test the
left-first equal duplicate plus mismatch/conflict typed outer directly at the
route union/key-value algebra, including Complete-only validity and no carrier
on Need/outer. Test WindowsLongPath parser operation, Need, and typed-operation
outer through the existing parser seam under its platform guard; do not invent
a Unix runtime path or add a production test hook. Existing lower observed
source/parser tests may supply kind/symlink/UTF-8 tables only when the new
route test proves that the route driver preserves the corresponding terminal
and reached prefix.

Correct only test and physical caps. Keep production caps at +120 for
`repo_file.rs` and +160 for `repository_ignore.rs`. Raise test caps to
+280 and +360 respectively; raise physical caps to 2,700 and 3,350; cap
aggregate semantic growth at 920 and combined physical size at 6,050. These
limits provide at most 123 and 159 additional test lines over the measured
candidate without authorizing production growth.

## Validation and successor

The retry must run the route semantic/outer/cancellation tests individually,
then their default-parallel batch; full `slug_bzlmod_v2`,
`slug_loading_v2`, and `slug_query_v2`; the established core library and
runtime baselines; fmt, check, diff-check, exact accounting, Buck2 retention
scan, AI cleanup categories 1-9, and independent latest-correction review.

This docs-only packet ends only on independent design `ACCEPT`. Then schedule
exactly one
`WP-2A-m1-routed-repository-policy-observation-implementation-retry` using
Rust base `e4ee0a8e`, semantic design `7f60a5c4`, and the accepted
correction design. After implementation acceptance, return directly to the
docs-only external package source/load frontier design. Do not activate query
or close M1.

## STOP / REPLAN

STOP on Rust writes during design; production semantic or ownership changes in
the retry; another file; public export; upper lookup/package/loading/query
activation; a third family; mixed source families; reconstructed Result Arcs;
semantic inspection before union; partial carrier; moved/duplicate events;
retained scratch; a Unix-only fiction for Windows parser behavior; cap excess;
multiple successors; or M1 closure.

`REPLAN` again if discriminating proof requires a production seam, another
owner/file, platform behavior cannot be tested at the existing parser boundary,
or the corrected caps still cannot contain the route proof.
