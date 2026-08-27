# Current Slug V2 Packet

Packet: `WP-4-5-7A-registration-target-pattern-syntax`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Result: complete the one shared absolute target-pattern syntax vocabulary and
retain every fact required by later repository mapping, package lookup and
registration expansion. Add focused Bazel 9.2 oracle evidence. Do not load a
package or activate a new registration/query/build expansion.

## Learned facts and decision

Commit `0cd339800` accepts the final raw MODULE registration type and the
selected canonical-owner/final-mapping projection. The current direct analysis
adapter intentionally fails package and recursive registrations closed. The
next shared boundary is `slug_identity_v2::TargetPattern`, already consumed by
query, build, cquery and aquery.

That parser currently recognizes only `:all` package wildcards and bare
`/...` recursion. It stores `:all` under a `PackageAll` shape that cannot
represent `:*` or `:all-targets`, does not retain the wildcard spelling needed
for Bazel's explicit-target conflict rule, and cannot distinguish rules-only
from all-target recursive forms.

Pinned Bazel 9.2 `TargetPattern.Parser` recognizes package `:all` as rules-only
and `:*` / `:all-targets` as all-targets. Recursive `...` and `...:all` are
rules-only; `...:*` and `...:all-targets` include all targets. An absolute
package wildcard remains a candidate until a loaded package is checked: an
existing legal target named `all`, `*` or `all-targets` wins over wildcard
expansion and emits a warning. Parsing itself performs no package lookup.

## Required implementation

1. Replace the misleading `PackageAll` syntax variant with one package-
   wildcard candidate carrying apparent repository, package and an explicit
   three-value suffix enum (`all`, `*`, `all-targets`). Expose only the small
   syntax/policy accessors later loading needs.
2. Extend recursive syntax with an optional retained suffix. Bare `...` and
   `...:all` report rules-only; `...:*` and `...:all-targets` report all-targets.
   Reject any other target after `...` at parse time.
3. Preserve exact display spelling for every admitted absolute form, apparent
   repository identity and existing direct-label normalization. Do not store a
   wildcard as an `ApparentLabel` or canonical label.
4. Mechanically update exhaustive query/build consumers. Existing `:all` and
   bare recursive paths retain their accepted behavior. Newly represented
   wildcard forms must stop at an explicit existing unsupported boundary; this
   packet does not inspect packages, resolve ambiguity or expand them.
5. Add a focused Bazel 9.2 oracle fixture for package and recursive rules-only/
   all-target spellings plus legal `all` and `all-targets` target conflicts.
   Unit regressions cite the pinned parser and `TargetPatternTest` source.
6. Prove parser equality/display, rules-only classification, invalid recursive
   suffixes and consumer fail-closed exhaustiveness. No DICE key or retained
   semantic graph value changes in this packet.

## Architecture, compatibility and guidance

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`TargetPattern.java`, `TargetPatternTest.java` and the focused oracle are exact
authority. The syntax enum records unresolved facts; loading later owns
repository mapping, target-conflict lookup, warning, lexical expansion and
family policy.

Zabel is peer guidance only. Its useful choices are one shared target-pattern
leaf, a three-value package-wildcard suffix, a separate contextual resolution
stage and retaining ambiguity until a loaded package exists. Slug does not copy
its Zig types, borrowed-lifetime model, diagnostics or compatibility claims.
Unlike Zabel's owner that retains raw text separately, Slug's command parser
must retain the recursive suffix in the enum so `Display` does not invent a
different request spelling.

- **Exact:** absolute package/recursive syntax classification, suffix spelling,
  rules-only versus all-target distinction, invalid recursive-target rejection
  and the unresolved explicit-target conflict fact.
- **Slug-native:** Rust enum layout, accessor names and current-consumer
  unsupported terminals.
- **Unsupported/deferred:** selected repository mapping, package lookup,
  wildcard conflict resolution/warnings, package or recursive expansion,
  stable dedupe, family filters, configured providers/settings, option-based
  registrations, rule/toolchain implementations and actions.

## Allowlist and validation

Base is `0cd339800`. Change only:

- `app/slug_identity_v2/src/pattern.rs`;
- `app/slug_identity_v2/src/lib.rs` (public syntax enum export only);
- `app/slug_identity_v2/tests/pattern.rs`;
- `app/slug_query_v2/src/loading_environment.rs` (exhaustive fail-closed
  adapter/proof only);
- `app/slug_core_v2/src/runtime/dice.rs` (mechanical exhaustive adapters/proof
  only); and
- one new focused fixture under
  `tests/v2_oracle/fixtures/registration-target-pattern-syntax/`.

Caps are 240 production, 750 proof and 990 total additions; deletions do not buy
budget. Each new helper/test is at most 100 lines. Add no dependency, retained
raw-text copy, interner, DICE key, package read, traversal, expansion, mapping
copy, configured value or global state.

Run the Bazel-only focused oracle against pinned Bazel 9.2, all
`slug_identity_v2` tests, focused query/core consumer tests, locked core check
and locked CLI build serially. Run format, diff, scope/cap/helper, archive and
utility/no-DICE audits. Do not claim Slug oracle parity for deferred expansion.

STOP and `REPLAN` for a second parser, a canonical label standing in for a
wildcard, loss of exact suffix spelling, package lookup, repository mapping,
new query/build expansion, registration activation, a changed accepted `:all`
or bare-recursive result, allowlist/cap escape or a new retained utility.

## Immediate predecessor

Commit `0cd339800` accepts `WP-4-5-7A-selected-registration-pattern-retention-
owner`; commits `17ea1f751` and `e4cba54aa` record its allowlist and cap replans.
The canonical subplan supplies the complete declaration/view/parser/expansion
architecture. This packet implements only sequence step 2.
