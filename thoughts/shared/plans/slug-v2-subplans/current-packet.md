# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-path-input-observation-proof-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
REPLAN and candidate base: pending docs commit / `c8d2d0b5`

## Goal and authority

Retain the current one-file source-path observation candidate and complete only
its proof-cap correction plus fresh serial validation. Preserve the accepted
path-first Legacy/Observed owner, API, driver, semantic/error/event/epoch/
retention/lifecycle contract, assertion set and all test/helper identities.

Rust authority is only proof lines 481+ of
`app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`.
Production lines 1..=480, every second file, API/driver/helper/test names and
counts, fixtures/oracles/Cargo/BUILD/exports/callers and orchestration docs are
read-only during proof correction.

The retained candidate exceeds only the former proof and aggregate addition
caps: proof is +623/-6 against <=620 and aggregate is +861/-63 against <=860.
Production is +238/-57 within <=240. This formal REPLAN raises only proof and
aggregate ceilings; it changes no implementation, assertion, test, helper,
formatting or activation contract.

## Frozen retained candidate

Entry candidate is 1,687 physical lines with full SHA-256
`bba8073d34fc9cf13d6c8c9b2572a30bbf8d96764d948509980735a110ad4371`.
Production lines 1..=480 are byte-frozen at SHA-256
`2fd574628625d9f09ff248f784801e93e97e7f629d73d56404feb5ee7966f9ba`;
proof begins exactly at `#[cfg(test)]` line 481. Accounting against
`c8d2d0b5` is +238/-57 production, +623/-6 proof and +861/-63 aggregate.

Freeze the complete production/API/driver byte sequence and accepted contract:

- pure requested-path normalization precedes every child; invalid Path is a
  semantic empty-epoch Result with no child edge;
- Legacy requests exactly legacy source input with empty epoch; Observed
  requests exactly observed source input;
- Need is immediate and the opaque child outer maps to carrierless Source;
  child DICE failure remains semantic Compute with empty epoch;
- every child-complete Source/InvalidSource/success retains the exact legacy
  child Result Arc and forwards the child epoch unchanged;
- there is one shared driver, one pure finisher, no second child/merge/union/
  rebuild/fallback/mismatch and no direct Host read;
- the parent is eventless, dependency vectors are exact, warm rows are
  batchless and every lower batch stays child-owned; and
- the carrier retains only the local Result Arc plus compact epoch; child
  carrier/path/view/event scratch dies before publication, with DICE-owned
  serialization and lawful poll-drop/recovery.

The private observation key/carrier/Source outer, equality/validity and exact
Display remain unchanged. Visibility, caller activation and upper source-
observation/public/bootstrap work remain absent.

## Frozen proof contract

Retain exactly the three named tests:

- `observed_root_apparent_repository_source_path_input_identity_finisher_and_terminal_algebra`;
- `observed_root_apparent_repository_source_path_input_real_families_events_and_parity`;
- `observed_root_apparent_repository_source_path_input_lifecycle_cancellation_and_nonactivation`.

Their entry spans are respectively 107, 132 and 148 lines. Preserve every
existing assertion and the entry candidate's proof-helper count/names/spans;
add/remove/rename no test or helper, grow no helper/test beyond its entry span,
and keep every span below 200. Preserve the accepted source-input visibility
smoke and all legacy tests/source-shape assertions. Add no `rustfmt::skip`.

The proof remains authoritative for exact key/path identity, invalid-path no-
child precedence, Need/outer/compute/Source/InvalidSource/success Arc+epoch
algebra; Main/Builtin/selected success, generated/mapping/missing Source
terminal legacy semantic parity, complete lower event vectors and warm
batchlessness; held parent/child mapping/definition/policy A-B-A, lawful
same-Result/different-epoch invalidation, same-transaction child=parent subset-
global epoch associations, no cross-transaction pairing, cancellation/
recovery and source-observation/public/bootstrap nonactivation.

No Rust edit is required merely to raise the cap. Within proof lines 481+, only
a semantic-neutral import/layout/source-scan repair demonstrated by the fresh
gates is permitted, and it must preserve the exact assertion set, helper/test
names and counts, and the frozen span ceilings. REPLAN rather than changing an
assertion, adding a helper/test family or touching production.

## Corrected caps and validation

Corrected caps are <=240 production additions, <=640 proof additions, <=880
aggregate additions and <=1,750 physical lines. Relative to the entry candidate
there is exactly 17 proof-addition, 19 aggregate-addition and 63 physical-line
headroom; the two unused production additions are frozen and cannot transfer.
Deletions do not authorize replacement breadth.

Run every gate fresh and serially; no result predating this REPLAN is
admissible:

1. the exact three observation tests;
2. protected source-input visibility smoke, legacy source-path tests and the
   observed-source-input suite;
3. full `cargo test -p slug_core_v2`;
4. direct `cargo check -p slug_commands_v2`;
5. `cargo fmt --all -- --check`; and
6. exact one-file allowlist, entry full-file hash before any permitted proof
   repair, frozen production-prefix hash after it, production/proof/aggregate
   accounting, physical/helper/test/name/span/no-skip/source-shape checks and
   `git diff --check`.

Reuse the accepted Bazel 9.2 source/path-capability evidence and Buck2 DICE
lifecycle concepts. Add no fixture or oracle.

## Compatibility and stops

Path normalization, requested/relative-path identity, source-input projection,
Main/Builtin/selected values, generated/source errors and terminal order,
equality/invalidation and lower events remain **exact** Bazel 9 compatibility.
The private Result-Arc+transaction-local epoch carrier/outer remains
**Slug-native**. Carrier visibility, source observation, public command/
bootstrap activation and exact Bazel configuration/output/ActionKey bytes
remain **unsupported/deferred**.

STOP production-prefix/full-entry hash precondition drift, production/API/
driver/semantic or assertion change, test/helper name/count/span change, second
file/key/child/owner/adapter, visibility/export/caller/source-observation work,
event/epoch/equality/retention/lifecycle/cancellation drift, private/malformed
injection, new test/helper family, new `rustfmt::skip`, cap/format/test waiver,
stale validation, Cargo/BUILD, fixture/oracle, milestone closure, M8/M7B or
exact identity work. REPLAN before production change, assertion change or cap
widening.

## Terminal

ACCEPT requires the complete fresh serial gates above and returns only to a
docs-only source-path carrier-visibility/source-observation consumer audit. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted design `54c444d2` authorizes the one-file owner from Rust base
`c8d2d0b5`. The retained implementation fits production and physical caps but
exceeds the former proof/aggregate additions by 3/1 respectively; this REPLAN
corrects only those measured limits.
