# Current Slug V2 Work Packet

Packet: WP-7-140-m7a-which-aggregate-budget-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain `cquery //:probe --output=label` past
the first post-WP839 `repository_ctx.which` aggregate-budget stop, then
report its first new stop. An offline Bazel 9.2 oracle must discriminate
more than 64 distinct ordered candidates across several `which` calls while
each call remains within WP838's 64-component PATH limit. This packet does
not claim full rules_cc `local_config_cc`, C++ toolchain, root Build or M7A
closure.

The admitted Unix first-regular-executable-or-None lookup, absolute PATH
component normalization, order and resulting path spelling remain **exact**
within WP838's existing slice. Rust Host path observations, finite input
limits and the enlarged aggregate candidate budget are **Slug-native**;
the budget must not be described as a Bazel cap. Windows/PATHEXT, relative
PATH search, unrestricted tool lookup, later compiler forms and full
toolchain behavior remain **unsupported/deferred**. No existing exact slice
is widened into a parity claim.

## Source and observed stop

Pinned Bazel 9.2
`StarlarkBaseExternalContext.java:2304-2353` searches PATH in order and has
no invocation-wide candidate cap; its repository-context test at
`StarlarkRepositoryContextTest.java:235-256` covers order, missing tools and
relative-entry filtering. Pinned rules_cc 0.2.17
`cc/private/toolchain/unix_cc_configure.bzl:68-87,272-283,332-340,376-417`
orders GCC lookup and two `-v` calls before `gcov`, `llvm-cov`,
`llvm-profdata`, `ar` and eight further non-overridden tools: at most 13
distinct program names in this phase. WP839 at `5cf098264` accepts the
source-ordered GCC execute slice and its offline oracle.

WP838's context permits at most 64 normalized candidates in one `which`
call (`repository_rule_context.rs:737-779`), while selected Loading's
`prepared_which` map permits only 64 distinct candidates across the whole
repository-rule invocation
(`module_extension_repository_file_effect.rs:1490-1493,1620-1646`). The
authentic smoke after WP839 stopped at the conflated diagnostic
`repository_ctx.which repeated or exceeded its candidate limit`. A
read-only model of the then-current 657-byte/25-component host PATH yields
15 unique absolute directories and 129 distinct candidates across those
13 tools, with the 65th during `ld`; this is not a captured daemon trace.
The context emits `WhichNeed` only for a missing prepared candidate, so a
repeated-key failure would contradict the normal replay contract and must
trigger investigation rather than a blind cap lift.

## Owner and decision

Keep WP838's selected repository-file effect owner, raw PATH
`RepositoryEnvironmentCellKey` verification, ordered
`ResolvedPathObservationKey` candidate dependencies and normalized-path
prepared-map replay. Keep WP839's execute transcript, request/prefix/call
index/nonce validation and transient Need/abort behavior unchanged. No
command-side filesystem lookup, new DICE key, global cache or lock is
admitted. Before widening the limit, split the repeated-key and
aggregate-limit branches into distinct static, path-free diagnostics; the
authentic retry must identify the actual arm without exposing PATH or other
host data.

If the limit arm is confirmed, raise only the selected-invocation distinct
candidate budget from 64 to `13 × 64 = 832`: 13 source-ordered tool names
times the already admitted maximum of 64 normalized candidates per call.
Keep per-call program/PATH byte, component and candidate limits exactly as
WP838 accepted them. The map remains evaluation scratch (not DICE-retained)
and is released on terminal, Need restart and cancellation. It remains the
existing deterministic `SmallMap`; no new retained representation or
Buck2/V1 import is warranted. Replaying Starlark for each new candidate
increases deep-clone and evaluator cost, so record elapsed authentic time
and do not claim a speedup. If this cost is impractical, reconsider a
source-ordered batched DICE observation that stops at the first hit, not an
unbounded cap or an observation of paths after the first hit.

The observed paths and raw PATH cell must still structurally invalidate
same-DICE success and post-host error results under executable-mode,
create/delete/recreate and PATH A→B→A changes. Duplicate normalized
candidates must be de-duplicated in insertion order. No result may outlive
the immutable request/observation certificate or publish from a partially
replayed execute transcript. This changes scratch budget and diagnostic
only; no semantic identity domain, output path token or process policy
changes.

## Proof, validation and scope

Extend the existing
`repository-rule-which-normalized-path` fixture with one independent
generated repository/anchored query row: loop over 65 distinct reserved
absent names (`"slug_v2_which_absent_" + str(i)`) under its single
normalized absolute PATH component, assert each lookup is `None`, then
assert `which("sh")` returns `/bin/sh`. This crosses the 65th aggregate
candidate without changing the original normalization row. Document the
conditional host assumption that the reserved names are absent under
`/bin`. Generate the new Bazel 9.2 expected
row with fresh ignored-RC roots and independently verify it; run rebuilt
Slug against the same fixture. Keep fixture provenance, exact output and
no-download/no-credential hygiene; add no copied upstream asset or
nondiscriminating row. Existing WP838 and WP839 fixtures remain protected.

Focused Loading tests must distinguish duplicate from cap diagnostics;
the 65th candidate succeeds, the 832nd is admitted and the 833rd fails
closed; first hit/None and normalized dedup/order remain unchanged. Cover
a candidate beyond the old limit changing miss→executable→miss in the
same DICE with fresh request observations, plus a `which` after two
transcript-replayed GCC probes without relaunching identical calls. Reuse
existing `which` and execute tests for unchanged per-call limits and
transience. Acceptance requires the authentic cquery to advance past the
confirmed aggregate-limit arm to its first new stop; any different result is
diagnostic only and must be attributed, with the candidate preserved or
`REPLAN` as appropriate. This smoke is strictly required because the offline
row cannot establish rules_cc progression. Record before/after
host resources and do not repeat an identical resource failure.

Resolve the pinned toolchain; serialize Cargo. Use JSON `--no-run`, exact
selector preflight and focused Loading tests; rebuild `slug_cli_v2` after
Rust changes before Slug fixture/authentic replay. Clean stale `slugd`
before/after daemon-sensitive checks. Run format, diff, plan-status,
archive and fixture hygiene. Independent DESIGN review must accept this
bounded owner/budget before implementation, then FINAL review the actual
diff and evidence. The selected Loading owner file exceeds 2,000 lines,
but the production edit is one local guard/diagnostic branch; a wider
physical split would obscure this packet's ownership and is deferred.

Allowlist: this manifest and canonical Current; Loading's
`module_extension_repository_file_effect.rs` with colocated tests; the
named fixture's `fixture.toml`, `expected/oracle.json`, `workspace/MODULE.bazel`
and `workspace/repo.bzl`; compact Stage 5/9 notes only after acceptance.
No change to `repository_rule_context.rs`, Bzlmod/Core, DICE keys, sandbox
launcher, generic path text, compiler flags, other fixtures or cache
transport. A test-only Loading helper inside the allowed file is fine; a
production data-structure/key change requires `REPLAN`.

`REPLAN` if the repeated-key diagnostic occurs, if 832 cannot cover the
source-bounded calls, if measured replay cost becomes impractical, or if
candidate observation/invalidation fails. A later stop requiring different
repository/path/execute authority is successor scope, not permission to
widen this packet. Do not work around the bound by direct host reads or
silently admit further repository commands.

## Acceptance receipt (2026-09-24)

The diagnostic-only Loading build and three focused tests passed before a
rebuilt authentic retry stopped on the distinct aggregate-limit message,
not the repeated-key branch; the new offline row also failed on that same
arm under the old 64 budget. With the budget at `13 * 64 = 832`, pinned
Loading JSON no-run completed in 15.85 seconds, exact-selector preflight
selected four tests, and all four passed (0.00, 0.19, 0.01 and 0.01 seconds).
They cover both diagnostics, 65th/832nd admission and 833rd rejection,
beyond-old-cap same-DICE miss→executable→miss restoration, and `which` after
two prepared identical `-v` execute records. No DICE key or retained value
shape changed.

The existing fixture gained one independent row and no new files. Fresh
ignored-RC Bazel 9.2 generation and independent verification passed in
4.13/4.14 seconds with both query rows successful; the original normalized
`sh` expected row is byte-for-byte unchanged. The rebuilt Slug CLI (11.17
seconds) passed both rows in a fresh root at
`/tmp/slug-wp840-fixture-zGKXtx` (6.30 seconds). No user Bazel RC or
credential contents were read or copied. This is an oracle for the admitted
ordered lookup, not a claim that Bazel has the Slug-native 832 budget.

The required authentic Rust-toolchain `cquery //:probe --output=label` from
`/tmp/slug-wp797-authentic-2YQmSt` exited 2 after about 52 seconds at
`repository execute policy: generated GCC probe prefix has an unexpected
effect`, rather than either `which` diagnostic. Pinned rules_cc source
orders `_get_tool_paths` before three template effects and the next
linker-discovery execute request; Core rejects effects beyond its admitted
symlink/symlink/`empty.cc` prefix before launching. This is a source-order
attribution of a distinct later boundary, not a captured owner, argv or
proof that the linker probe ran. Committed memory was 49,553,244/49,522,112
kB before/after against a 48,427,772 kB commit limit; no `ENOMEM` occurred.
No stale `slugd` remained. Independent DESIGN and FINAL reviews returned
**ACCEPT**. Later prefix/template authority, full `local_config_cc` and M7A
closure remain separate work.

## Immediate predecessor

Independent DESIGN review accepted this diagnostic-first bounded contract on
2026-09-24 after making authentic progression a mandatory acceptance gate.

WP839 at `5cf098264` admitted guarded GCC execute and a low-commit
Linux/GNU launcher. Core runner 18/18, fresh Bazel/Slug offline evidence
and independent FINAL review passed. Authentic cquery consumed two GCC
`-v` calls before this separate `which` stop; later probes and M7A remain
open. The full WP839 contract and resource-correction receipt remain in
that commit's `current-packet.md`.
