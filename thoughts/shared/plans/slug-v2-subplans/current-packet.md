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

The latest accepted component removes unescaped prefix/suffix copies from the private
`parse_percent_s_one` compiler helper. Validated percent escapes use owned scratch;
the existing compiler immediately interns both segments in its frozen heap.
Preserve optimizer grammar/rejection, runtime tuple/error behavior, Unicode and
GC/freeze ownership. Returned `Cow<str>` values are synchronous compiler scratch.
This is Slug-native memory ownership, with admitted behavior unchanged.
Next: select a proved redundant copy at existing string/compiler owners through
a bounded design checkpoint.

No network-dependent source observation, actual-source retry, cap growth,
Bazel invocation, linked CLI/runtime activation or broader integration is
selected during this scope. Source-admission and integration gates remain
unwaived; memory controls cannot substitute for them.

## Local compiler memory contract

Root owns edits, serial Cargo, integration and commits. Allowlist:
`starlark-rust/starlark/src/values/types/string/interpolation.rs`, sibling
`percent_s_memory_tests.rs`, compact receipt and matching Stage9 row. Compiler
`expr.rs`, runtime interpolation and accepted dot-format parser remain unchanged.
Private exact-helper probes stay under ignored `target/wp899`.

Keep the two parse loops and rejection decisions. Track character byte indices;
copy literal fragments only after validated `%%` needs decoding. The `%s` capture
ends the prefix and starts the suffix at ASCII boundaries. A private finishing
helper borrows an unescaped fragment or appends it to owned escape scratch.
Existing `alloc_str_intern` immediately copies/interns outputs; no input borrow
enters compiled instructions. Add no cache, interner, dependency, unsafe code,
DICE key or retained representation.

Independent DESIGN and FINAL reviews cover parser decisions, borrowed lifetimes,
compiler interning and meaningful evidence. New controls discriminate nonempty
input-pointer sharing, empty/Unicode/escaped segments, exact optimizer rejection,
and retained compiled functions invoked after original source/module handles drop.
Keep ordinary formatter fallback successes/errors and existing compiler bytecode,
speculative execution, allocation/interner and carrier controls.

Validation uses the pinned nightly-2025-09-14 toolchain resolved with `rustup which`.
Prepare default Starlark tests with offline Cargo `--no-run --message-format=json`,
then preflight exact selectors and execute focused tests against that unchanged
artifact. Golden controls need `CARGO_MANIFEST_DIR` set to the Starlark crate
fixture directory. New selectors need their own preflight; rebuilding invalidates it.
Serialize Cargo; preparation is capped at 60s and focused execution at 30s. Check
default Loading/Core/CLI all targets offline, format Rust, and run plan/archive/diff
checks. Preserve passing results unless relevant inputs change. Invocation errors
get narrow repairs; semantic or ownership disagreement requires revisiting design.

The exact-helper differential probe measures returned temporary String or
owned-Cow capacities only. Input, final frozen storage, transient peaks, RSS,
speed and source fit remain outside the claim. It does not replace real compiler
and GC/freeze controls. Full WP899/M7A remains unaccepted.

## Accepted local memory boundaries

| Owner / narrow accepted result | Receipt |
|---|---|
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
