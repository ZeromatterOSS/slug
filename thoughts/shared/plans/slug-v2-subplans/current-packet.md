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

The latest accepted component borrows AST literal fragments during private
`CompilerExprUtil::reduces_to_string` collection, removing two per-fragment owned
copy sites. Traversal/rejection/order and the returned owned concatenated String
are unchanged; existing frozen heap allocation owns compiled values.
This is Slug-native scratch ownership, with admitted behavior unchanged.
Next: select a proved redundant copy at existing string/compiler owners through
a bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Local compiler memory contract

Accepted component: private `CompilerExprUtil::reduces_to_string` in
`starlark-rust/starlark/src/eval/compiler/expr.rs` borrows AST literal fragments
in `Vec<&str>` before constructing its existing owned concatenated result.
Preserve traversal, left-association, early rejection, reverse/concat and frozen
heap allocation. No borrowed fragment escapes or enters compiled instructions.
The exact source/type change removes two per-fragment owned-copy sites; vector,
final String and frozen storage remain. No RSS, byte-capacity or speed claim.

Root owns edits, serial Cargo and commits. Allowlist: compiler `expr.rs`, sibling
`literal_concat_memory_tests.rs`, compact receipt and matching Stage9 row.
Private design/evidence stays under ignored `target/wp899/literal-concat-borrow-*`.
The bounded design and three DESIGN/FINAL reviews cover AST ownership, optimizer
semantics and evidence; pins live in the [acceptance receipt](wp899-literal-concat-borrow-2026-10-05.json).
No new helper/API/cache/interner/dependency/key/unsafe or retained representation.

Controls exercise the actual private helper's ordered Unicode/NUL/empty/long
left chains, rejection boundaries and owned result after AST drop. Compile/freeze
functions, then invoke after source/module handles drop; cover mixed dynamic
fallback and actual BazelInternal carriers. Reuse constant-folding/bytecode,
GC/frozen/interner and recently accepted compiler scratch controls.

Resolve pinned nightly-2025-09-14 tools through `rustup which`. Serial offline
Starlark default no-run Cargo JSON, exact artifact preflight and focused execution;
one default Loading/Core/CLI all-target check. Preparation60s/test30s operation
caps; goldens need CARGO_MANIFEST_DIR and no regeneration. Format changed Rust;
plan/archive/diff checks. Preserve passing evidence unless relevant inputs change.
Semantic/ownership disagreement revisits design; invocation repairs stay narrow.
Full WP899/M7A and source/integration gates remain unwaived and unaccepted.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
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
