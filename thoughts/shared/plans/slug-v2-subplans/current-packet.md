# Current Slug V2 Work Packet

Packet: WP-7-199-m7a-request-selected-source-profile-r1
Status: Local string ownership and memory optimization; source integration deferred; full WP899 unaccepted

## Active scope and observable outcome

The user's current direction is local string interning and basic memory
optimization, with offline validation. Use the orchestration and memory-reuse
skills at existing string owners. Remove proved redundant copies or reuse existing
interners without changing semantic identity, source authority or natural owners.
Windows work remains minimal stubs/deferred until it can be tested effectively.
Other checkout directories are reference material.

The latest accepted component transfers owned raw load-name Strings into the
existing repository BUILD child variants. It removes one name copy per child on
both routes, preserving original pointers/capacities, order, typed errors and
observations. Existing async child consumers retain normal owned strings.
This is Slug-native scratch ownership, with admitted behavior unchanged.
Next: select a proved redundant copy or existing interner reuse at local string
owners through a bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Local load-name memory contract

Accepted component: `prepare_repository_package_child_loads` consumes the
existing owned raw load-name vector and transfers each String into unchanged
Canonical/Resolved child variants. Its sole caller no longer needs the vector.
Preserve root/canonical routing, order, duplicates, typed first error, exact
observations and later canonical validation. Resolution borrows a name only
within the synchronous call; async consumers retain normal owned strings.
No DICE key/identity/hash/dependency/API/cache/interner/IO/locking changes.

Root owns edits, serial Cargo and commits. Allowlist: Loading `bzl_module.rs`
private helper/caller/test hook, sibling `repository_child_load_memory_tests.rs`,
compact receipt and matching Stage9 row. The oversized production owner remains
cohesive for this minimal delta; new tests are split into a sibling file.
Private design/evidence stays under ignored `target/wp899/repository-child-load-move-*`.
Three DESIGN/FINAL reviews accepted ownership transfer, input semantics and
meaningful evidence; pins live in the [receipt](wp899-repository-child-load-move-2026-10-05.json). Keep external `.bzl` preparation and source decoding unchanged.

Direct helper Root/Canonical controls check original String pointers/capacities
and exact bytes/order/variant; baseline copies fail, candidate transfers pass.
A semantic control covers pass-through/duplicate/empty input and first typed
root error plus observations. Reuse genuine existing injected route fixtures
and protected normal wrapper lifecycle/prevalidation/projection controls.
Only raw name buffers transfer; vectors, AST/parser and retained storage remain.
No quantified memory/RSS/speed/source-fit claim.

Resolve pinned nightly-2025-09-14 tools via `rustup which`. Serial offline Loading
lib no-run Cargo JSON, exact artifact preflight and focused execution; default
Core/CLI alltarget check. Preparation60s/test30s caps; format changed Rust and run
plan/archive/diff checks. Preserve passing evidence unless inputs change.
Full WP899/M7A and source/integration gates remain unwaived/unaccepted.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
| Repository BUILD preparation transfers raw load-name Strings into existing children on both routes; original buffers/capacities preserved and one copy per child removed. Three DESIGNs/FINALs, baseline pointer failures, candidate 8/8 and default Core/CLI checks pass; no quantified memory/speed claim | [Repository child load-name transfer](wp899-repository-child-load-move-2026-10-05.json) |
| Literal-concatenation compiler helper borrows AST fragments; two intermediate owned-copy sites removed, final String/frozen ownership unchanged. Three DESIGNs/FINALs, corrected baseline 3/3, candidate 15/15 and default dependent checks pass; no quantified memory/speed claim | [Literal concatenation borrowing](wp899-literal-concat-borrow-2026-10-05.json) |
| Percent-s compiler parser borrows unescaped segments before existing frozen interning. Fixed returned temporary capacity 1,572,868B→0; escaped buffers/final storage remain. Three DESIGNs/FINALs, 27 controls, 58,824 comparisons and default dependent checks pass | [Percent-s scratch borrowing](wp899-percent-s-borrow-2026-10-05.json) |
| Dot-format compiler helper borrows unescaped segments before existing frozen interning. Fixed returned temporary capacity 1,572,868B→0; escaped scratch/final frozen storage remain. Three DESIGNs/FINALs, 19 unique controls (16+3 corrected goldens), 112,347 differential comparisons and default dependent checks pass | [Format scratch borrowing](wp899-dot-format-borrow-2026-10-05.json) |
| Limited replacement omits untouched suffix from temporary payload; zero/no-match input reuse. Fixed temporary 1,048,577B→1B, same final output; no capacity/RSS/speed claim | [Replacement prefix](wp899-replace-prefix-2026-10-05.json) |
| Local load-name String moved into existing DICE key; one unused scratch clone removed | [Load-name move](wp899-local-load-move-2026-10-05.json) |
| Host decoder borrows immutable source bytes; initial root BUILD load-discovery AST released earlier, owned attempt source retained | [Host source borrowing](wp899-host-source-borrow-2026-10-05.json) |
| Whitespace split helpers borrow input parts; fixed returned temporary payload capacity 524,288B→0 per direction. Final heap strings remain owned | [Whitespace split](wp899-whitespace-split-2026-10-05.json) |
| External `.bzl` decoding borrows existing bytes, removing one decoded copy; parser String remains owned | [External source borrowing](wp899-bzl-source-borrow-2026-10-05.json) |
| Parsed `.bzl` retains/clones existing workspace source Arc; parser-owned copies remain | [Source sharing](wp899-bzl-source-sharing-2026-10-05.json) |
| Lexer literal capacity grows with decoded literal instead of whole remaining source; fixed retained capacity 16,775,168B→16,384B | [Lexer growth](wp899-lexer-string-growth-2026-10-05.json) |
| Archive namespace scratch uses one ordered index; fixed prior-path work 8,386,560→0, excluding tree comparisons | [Archive namespace](wp899-archive-namespace-2026-10-05.json) |
| Injected DICE history reuses full-equal penultimate payload while preserving version metadata; measured 32×1MiB fixture 32→1 payload owners and 74.2% lower peak RSS | [History payload reuse](memory-payload-reuse-2026-10-03.json) |

Each receipt proves only its named component/fixture. None establishes historical
allocation attribution, actual source fit or whole-workload performance.

## Deferred full WP899 acceptance requirements

The full objective remains the normal full-caller request → apparent imported
owner → selected first-query checkpoint → tools/runtime/ordered Label data → ONE
restored opaque source view. Require normal Root and independent immutable-writer
vertical reads plus genuine actual-root source/admission evidence. A missing
source/alias/layout/fit terminal records a prerequisite and cannot accept the
complete runtime-profile consumer.

Full acceptance requires the inherited exact declaration/mapping and current
resource-bank authority, DICE full equality/invalidation, request/cancel/history
isolation, provisional and final source validation, exact caller/raw restoration,
engine retirement, clone-reader/descriptor-credit lastDrop, and coupled integration
FINAL. Preserve AliasDeny/TreeOnly/opaque loader boundaries and existing source,
resource and supervisor ceilings. Controlled fixtures and diagnostics cannot
substitute for actual-root source admission. Runtime/backend/query plus
locked-generate/bootstrap closure remains deferred and unproved.

The full frozen API, semantic-owner and proof/source-service envelope is inherited
unwaived from commit `dfea1798826170fb4db0eb5cfda0574eb39b6538`, exact path
`thoughts/shared/plans/slug-v2-subplans/current-packet.md`, lines 515–1105.
Completed observations and superseded launch instructions are history, never
current authorization. Global source attempts 16 and 17 are spent, with their
terminal receipts; no pending launch or automatic retry is selected.
See [pinned topic index](../plan-history.md#october-5-wp899-scheduling-compaction).
