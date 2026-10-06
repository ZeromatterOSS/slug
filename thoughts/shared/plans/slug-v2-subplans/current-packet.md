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

The latest accepted component reuses existing strings for unchanged ASCII
lower()/upper(). Changed ASCII and every non-ASCII input preserve original
conversion and final heap allocation. Both 96 KiB fixtures share their inputs
with zero additional public heap accounting. Next: select a proved redundant
local copy or existing interner reuse through a bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Accepted unchanged ASCII case input reuse contract

Native lower()/upper() return the existing StringValue only when every byte is
ASCII and already in the requested case. Changed ASCII and every non-ASCII input
retain the original Rust conversion through the same heap allocator. Public
string type, zero visible arguments, speculative annotation, tracing/freezing,
interner code and final ownership remain unchanged. No new helper, API, interner,
cache, key, unsafe code, dependency or retained representation is introduced.

Three DESIGNs/FINALs accept exact source proof and the same corrected tests.
Baseline has two semantic passes and one intended pointer-sharing failure;
candidate passes 8/8, including 32,768 ASCII comparisons, explicit Unicode and
four byte-identical error records. Mutable results survive explicit GC, freezing
and source/module/handle drop; speculative execution and internal string carrier
controls pass. Default Loading/Core/CLI alltargets compile offline. Preserved
E0597 and repr-assumption failures were test-only corrections; fresh corrected
builds and exact artifact preflights precede execution. Format/archive/plan/diff
hygiene passes. The owner receives only two method changes and a test hook,
with a 188-line sibling.

Each 98,304-byte fixture changes from an unshared result to sharing its input;
observed public Heap::allocated_bytes() growth changes from 204,736/409,536 to
0/0. These are fixed-fixture arena observations, excluding exact allocator or
payload counts, retained capacity, aggregate/peak/RSS/speed and source-fit claims.
All fallback/final storage remains. Full WP899/M7A remains open.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
| Native lower()/upper() reuse unchanged ASCII inputs; all changed ASCII/Unicode conversion, public types/errors, GC/frozen ownership and speculative behavior preserved. Three DESIGNs/FINALs; baseline two semantic passes/one intended sharing failure, candidate 8/8 and 32,768 ASCII comparisons plus default dependents. Both 96 KiB fixtures share with zero additional public heap accounting; fixture observations only | [ASCII case input reuse](wp899-ascii-case-reuse-2026-10-05.json) |
| Manifest temporary SmallSets borrow full canonical labels; seven scratch clone sites removed, first-seen/full structural identity/fingerprints/final owned and frozen outputs preserved. Three DESIGNs/FINALs; identical baseline 3/3 and candidate 8/8 as 6 lib + 2 BUILD, default Loading/Core/CLI checks. Explicit scratch drop resolves preserved compiler E0505; no allocator/capacity/RSS/speed claim | [Manifest label borrowing](wp899-manifest-label-borrow-2026-10-05.json) |
| Loading Label collectors format directly and external workspace_root copies into final heap; three scratch construction sites removed, bytes/identity/mapping/final ownership unchanged. Three DESIGNs/FINALs; identical baseline 2/2 and candidate 7/7, default Loading/Core/CLI checks. Source proof only; no allocation/capacity/RSS/speed claim | [Label renderer scratch removal](wp899-starlark-label-render-2026-10-05.json) |
| Compiler method calls move owned receiver instead of deep-cloning; optimizer order/IR/spans/args/fallback/frozen ownership preserved. Three DESIGNs/FINALs; baseline 2 semantic passes/1 pointer failure; 3 Symbol buffers share owners (15,384 content bytes). Fifteen gates as 14 preserved + 1 reviewed stale golden correction, independently baseline-reproduced; default Loading/Core/CLI checks pass. Generic IR spare capacity remains possible; no aggregate/RSS/speed claim | [Method receiver transfer](wp899-method-receiver-move-2026-10-05.json) |
| Apparent-label formatting borrows existing repository bytes; root spelling, flags, errors, identity and final owned output preserved. Three DESIGNs/FINALs; baseline 2 semantic passes/1 pointer failure, candidate 10 controls as 3+4+3 and default Loading/Core/CLI checks. A 20,491-byte repository shares its owner at the sink. One temporary String removed; no aggregate/RSS/speed claim | [Apparent-label display borrowing](wp899-apparent-label-display-borrow-2026-10-05.json) |
| Attribute-query rendering transfers tight heap strings from consuming temporary atoms; spare-capacity strings retain original cloning and final capacity. Three revised DESIGNs/FINALs; baseline3 semantic passes/1 tight-pointer failure, candidate12/12 and default Core/CLI checks. Unconditional move rejected; natural concat8193B tempcap12288B/finalcap8193B unchanged. Callback/order/errors/final ownership preserved; no aggregate/RSS/speed claim | [Guarded candidate transfer](wp899-attr-candidate-render-move-2026-10-05.json) |
| Rule output templates borrow existing attribute/target strings and temporary dedup keys; all six shapes/four placeholders, exact order/errors and owned outputs preserved. Three DESIGNs/FINALs; baseline 1 semantic pass/2 pointer failures, candidate 9 controls as5+3+1 and default Core/CLI checks. Reviewed stale DICE assertion correction preserves changed source identity and equivalent output facts. Scratch copies only; no aggregate/RSS/speed claim | [Output-template borrowing](wp899-output-template-borrow-2026-10-05.json) |
| Compiler typo matching borrows existing frozen names; reversed-local/module/global order, duplicates, ties and owned diagnostics preserved. Three DESIGNs/FINALs, baseline 1 semantic pass/2 pointer failures; candidate 22/22 and default dependent checks. Fixed32x1024B global payloads now borrowed, owner matches0→32 and nested visible matches0→14. Cold scratch only, vectors/matching/final errors remain | [Scope-name borrowing](wp899-scope-name-borrow-2026-10-05.json) |
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
