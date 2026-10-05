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

The latest accepted component copies the native nonstring format suffix directly
into the final owned heap string. The prefix and single renderer callback remain
unchanged; a fixed 1MiB suffix reduces observed initial scratch capacity from
1,048,589B to 13B. This is Slug-native scratch ownership with admitted behavior
unchanged. Next: select a proved redundant copy or existing interner reuse at
local string owners through a bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Accepted formatter memory contract

Native `format_one` reserves prefix plus rendering slack, keeps the prefix push
and single `collect_str` call, then uses existing `alloc_str_concat` to copy
scratch and untouched suffix into final owned storage. String/identity arms,
percent tuple/errors, generic formatting and compiler own frozen interning stay
unchanged. No helper/API/interner/cache/key/unsafe/dependency or retained
representation change. Prefix/render scratch and final output remain; larger
renderers may grow scratch. No total peak/arena/RSS/speed/source-fit claim.

Three DESIGN/FINAL reviews accept the component. Same-test baseline semantic and
retained/carrier controls pass2; only initial capacity assertion fails1. Candidate
30/30 includes custom prefix/count/str-vs-repr/final-byte observations, builtin
conversions, Unicode/NUL, tuple/errors, retained compiled functions after source
and module drops, GC/freeze, actual carriers and protected protocol/bytecode/
interner/empty-format controls. Default Loading/Core/CLI alltarget check passes.
Pinned nightly-2025-09-14, serial offline preparation60s/tests30s, fresh artifact
exact preflight; goldens CARGO_MANIFEST_DIR and regeneration absent.

Root owns edits, serial Cargo and commits. Keep future changes bounded at existing
string owners with reviewed design and focused discriminators. Join estimates
and scope-name audits remain unselected. Full WP899/M7A/source and integration
gates remain unwaived/unaccepted.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
| Native nonstring formatting copies untouched suffix directly into final owned string; prefix and renderer callback preserved. Three DESIGNs/FINALs, same-test baseline 2 semantic passes/1 capacity failure, candidate 30/30 and default dependent checks. Fixed 1MiB suffix initial scratch capacity 1,048,589B→13B; render growth/final storage/peak/RSS/speed excluded | [Formatter suffix direct copy](wp899-format-suffix-direct-2026-10-05.json) |
| Empty-segment native formatting reuses existing StringValue, including percent single-tuple delegation; compiler own frozen interning unchanged. Three DESIGNs/FINALs, baseline pointer/arena failures, candidate 23/23 and default dependent checks pass. Fixed 64KiB cases share inputs with zero arena growth; chunks/padding excluded from exact payload/RSS/speed claims | [Empty-format input reuse](wp899-empty-format-reuse-2026-10-05.json) |
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
