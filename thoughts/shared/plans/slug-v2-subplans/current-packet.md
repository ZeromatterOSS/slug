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

The latest accepted component omits successful query integer-conversion scratch.
Numeric AST values convert directly; target literals borrow the unchanged parser.
Rejected raw values/capacities, fallback parsing, diagnostics/spans and owners remain.
Next: select a proved redundant local copy or existing interner reuse through a
bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Accepted query integer-conversion scratch contract

QueryExpression::java_integer_literal converts Integer(u64) directly via i32
TryFrom, allocating the identical decimal CompactString only on failure. Its
positive range equals prior decimal i32 parsing, without narrowing u64 AST values.
TargetLiteral borrows the original str for unchanged Rust i32 parsing, with the
original CompactString clone only on failure. Signs/leading zeroes/Unicode/NUL/
whitespace/overflow keep original interpretation. Every other kind retains Display,
CompactString construction and parsing, including numeric fallback displays.
Normal argument validators/configured specs/generic depth/count/deps retain order,
negative constraints, error priority and exact raw errors/spans. AST source pointers/
capacities and owned final diagnostics remain. No helper/API/interner/cache/key/
DICE/identity/unsafe/dependency/retained representation or donor import. Slug-native
memory implementation preserves inherited slots; no widened Bazel/Java parity.

Three DESIGNs/FINALs accept identical 200-line tests: extended baseline 2/2 and
candidate 8/8, including six unchanged configured depth/reverse/count/order/error
and signed-integer/validator controls. 2,054 numeric boundary/sample values,
21 literal cases/four fallback AST displays and two numeric fallback displays pass.
Nineteen raw errors and seventeen complete loading/configured diagnostics match
byte-for-byte, with exact capacities/spans and unchanged AST/debug/payload owners.
Two long tight/spare input errors and clones retain 47,104-byte/capacity outputs
after AST/source/original-error drop. The original 191-line baseline pass/hash
remains; nine additive fallback lines address DESIGN review before production,
then identical extended variants were freshly compiled/preflighted. Sixteen
protected sources, serial pinned offline lib/integration artifacts, Core/CLI
alltargets and format/archive/plan/diff checks pass. No failures/assets/goldens.

Acceptance proves two successful scratch-construction sites bypassed only. Invalid
and fallback raw strings, final diagnostics and AST owners remain. No pointer/
allocator witness, quantified memory/payload/count, aggregate/peak/RSS/speed/source-
fit claim. Windows minimal/deferred. Full WP899/M7A integration open and unwaived.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
| Query integer slots omit successful numeric formatting/target-literal clone scratch; exact range/signed parsing, original error bytes/capacity/ownership, fallback parsing and caller order/spans remain. Three DESIGNs/FINALs; same extended baseline 2/2, candidate 8/8, 2,054 numeric/21 literal/four fallback/two numeric-display cases, nineteen raw/seventeen public equal errors, retained tight/spare error clones and default Core/CLI. Additive fallback test extension preserved. Source scratch-site proof only, no allocator/quantified memory/RSS/speed claim | [Query integer scratch](wp899-query-integer-scratch-2026-10-06.json) |
| Query negative-token diagnostics borrow exact input slices through existing final join; scanning/UTF8/quote/three-token rules, public bytes/spans/priority and owned output remain. Three DESIGNs/FINALs; same tests baseline 2/2, candidate 6/6, thirty contexts/twelve negatives, eleven equal errors and two long retained error clones; default Core/CLI checks. Source copy-site proof only, no quantified memory/allocator/RSS/speed claim | [Query error token borrowing](wp899-query-error-token-borrow-2026-10-06.json) |
| Compiler type expression-statement lowering transfers its owned sole argument under the unchanged exact guard; earlier branches, inner IR/spans/frozen owners and rejected full IR/pointers stay. Three DESIGNs/FINALs; corrected same-test baseline two semantic passes/one ownership failure, candidate 10/10, three Symbol buffers per compiled/owned freeze-optimized route (15,384 content bytes each), nine rejection shapes/seven equal errors/default dependents. Fixture static-checking correction and two independently baseline-proved single-span golden corrections preserved. Generic IR capacity caveat; no retained/aggregate/RSS/speed claim | [Type statement argument transfer](wp899-type-statement-arg-move-2026-10-06.json) |
| Native two-element join writes borrowed parts directly into existing heap concat3; zero/singleton and larger buffers, exact bytes/errors/iterator cleanup/capacity-overflow/frozen ownership remain. Three revised DESIGNs/FINALs; identical corrected baseline 3/3, candidate 11/11, 2,053 comparisons, 31 equal traces, 33 equal errors and default dependents. Initial capacity proposal rejected and fixture E0716 corrected; source scratch-bypass proof only, OOM sequencing excluded, no quantified memory/aggregate/RSS/speed claim | [Two-element join](wp899-join-two-direct-2026-10-06.json) |
| Query attribute labels borrow canonical/apparent owners instead of temporary deep clone/Arc construction; full repo projection, exact spelling/final capacity/owned output/identity remain. Three DESIGNs/FINALs; identical baseline 2/2, candidate 13/13, twenty attribute cases and retained output after source drops, eleven existing graph/DOT controls and default dependents. Source clone-site proof only; formatting/final copying remain, no quantified memory/aggregate/RSS/speed claim | [Query attribute-label borrowing](wp899-query-attribute-label-borrow-2026-10-06.json) |
| DOT class-label conversion consumes tight buffers only at/above the pinned compact heap floor; short/spare original copying, exact bytes/final capacities/graph identity/release remain. Three revised DESIGNs/FINALs; identical baseline 2/2 and candidate 7/7, fourteen byte/capacity cases, eight equal DOT outputs, five existing graph controls and default Core/CLI checks. Initial heap-floor proposal rejected before implementation; source/library copy-bypass proof only, large 32-bit fallback excluded and no quantified memory/aggregate/RSS/speed claim | [DOT class-label conversion](wp899-dot-class-label-move-2026-10-06.json) |
| Compiler list addition folding transfers owned elements after both unchanged per-operand 1000 guards; ordered fallback/tree/spans/frozen constant conversion stay. Three DESIGNs/FINALs; same corrected baseline two semantic passes/one ownership failure, candidate 12/12, fifteen Symbol buffers transferred across four routes, four admitted boundary combinations/eight rejection checks/four equal diagnostics and default dependents. Independently baseline-proved five-span golden correction; output collection/capacity caveats remain, no aggregate/RSS/speed claim | [List addition element transfer](wp899-list-add-arg-move-2026-10-06.json) |
| Compiler type equality folding transfers the owned type(x) argument after unchanged guards, in both operand orders; rejected operands/pointers, helper IR/spans/frozen type-name ownership and ordered fallback stay. Three DESIGNs/FINALs; identical baseline two semantic passes/one ownership failure, candidate 19/19, three Symbol buffers transferred per orientation (15,384 content bytes each), ten rejection cases, five equal diagnostics and default dependents. Generic IR capacity caveat; no aggregate/RSS/speed claim | [Type equality argument transfer](wp899-type-equality-arg-move-2026-10-05.json) |
| Terminal type-is/format optimizations transfer the sole owned expression only after every unchanged guard succeeds; every rejected helper preserves full args/pointers, optimizer order/spans/interner/foreign frozen owners stay. Three DESIGNs/FINALs; identical baseline two semantic passes/one ownership failure, candidate 16/16, three Symbol buffers transferred per route (15,384 content bytes each), seventeen rejection checks, five equal diagnostics and default dependents. Generic IR capacity caveat; no aggregate/RSS/speed claim | [Terminal call argument transfer](wp899-terminal-call-arg-move-2026-10-05.json) |
| Compiler len/type sole arguments move under unchanged exact shape guards; optimizer order/helper folding/spans/fallback/frozen owners preserved. Three DESIGNs/FINALs; corrected same-test baseline two semantic passes/one ownership failure, candidate 15/15, three Symbol buffers transferred per route (15,384 content bytes each), twelve negative shapes, seven equal diagnostics and default dependents. Initial test message correction preserved; generic IR spare capacity caveat, no aggregate/RSS/speed claim | [Builtin argument transfer](wp899-builtin-arg-move-2026-10-05.json) |
| Compiler double-not bool fold transfers already-owned inner IR instead of deep-cloning; exact guard/tree/spans/constant/fallback/frozen ownership preserved. Three DESIGNs/FINALs; identical baseline two semantic passes/one pointer failure, candidate 13/13, three Symbol buffers transferred (15,384 content bytes), two equal diagnostics and default dependents. Generic IR spare capacity caveat; no aggregate/RSS/speed claim | [Double-not ownership transfer](wp899-double-not-move-2026-10-05.json) |
| Split/rsplit reuse complete input substrings by equal byte length at final list allocation; all splitting decisions, parameters/order, lists/scratch and partial owned strings preserved. Three DESIGNs/FINALs; same tests baseline two semantic passes/one sharing failure, candidate 14/14, 77,750 comparisons per variant, six equal errors, 42 sharing routes and default dependents. Two 128 KiB fixtures share with zero observed added arena accounting; fixture observations only | [Split/rsplit input reuse](wp899-split-input-reuse-2026-10-05.json) |
| Capitalize/title join lower/upper in unchanged ASCII whole-input reuse; exact first-position/word-boundary guards preserve all changed ASCII/Unicode fallback, errors and GC/frozen/speculative behavior. Three DESIGNs/FINALs; identical baseline two semantic passes/one new sharing failure, candidate 11/11, 74,898 comparisons per variant, eight equal errors and default dependents. Both new 96 KiB fixtures share; zero public arena growth is fixture accounting only | [ASCII capitalize/title reuse](wp899-ascii-title-case-reuse-2026-10-05.json) |
| Native splitlines reuses complete nonempty inputs without CR/LF; list and actual split-line allocations, bytes/parameters/errors, GC/frozen and speculative behavior preserved. Three DESIGNs/FINALs; baseline two semantic passes/one sharing failure, candidate 11/11, 5,602 character comparisons per variant and default dependents. Four large ASCII/Unicode cases share; observed zero added arena accounting excludes allocator/aggregate/speed claims | [Splitlines input reuse](wp899-splitlines-input-reuse-2026-10-05.json) |
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
