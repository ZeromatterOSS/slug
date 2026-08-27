# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-semantics-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete dependency-free 234-line rules_cc
`cc/common/semantics.bzl` producer freezes its eager constants/label, all 30
lazy functions, and the exact 43-field function-capturing `semantics` struct.
Add no production behavior and invoke nothing.

## Learned facts and decision

Base commit is `9e312f958` (`Prove complete action names freeze`). It adds 328
proof lines and no production, byte-verifies all 220 action-name lines, and
exhaustively proves 33 public constants, the 33-field `ACTION_NAMES` mapping,
seven ordered lists, and all seven pointer-identical `ACTION_NAME_GROUPS` fields.
Focused proof, 248 library tests, 24 invalidation tests, 31 BUILD-loading tests,
locked analysis/core checks, CLI build, formatting and hygiene pass. Independent
review returned `ACCEPT`.

Private `cc_common.bzl` source order remains in the 2,295-line
`cc/private/compile/compile.bzl`. After now-complete Skylib paths, action names
and helper children, the first incomplete child is rules_cc 0.2.17
`cc/common/semantics.bzl`: 234 lines, SHA-256
`029254fd58eb8b3bf32a0f772e479b991a51ce21a6f6cc8a5739aadbce3900da`.
It has no loads. Its eager rows bind two public Booleans, 30 lazy private
functions, one private canonical `Label`, and one public 43-field struct that
captures 29 of those functions plus exact strings, Booleans, lists and empty
dictionaries. Every eager expression uses accepted exact evaluator shapes. The
alternative toolchain-config branch now reaches the dependency-free 622-line
`cc_toolchain_config_lib.bzl`, so semantics is the smaller source-ordered
frontier.

Therefore run only
`WP-4-7A-rules-cc-semantics-complete-loading-proof`. Do not claim function
invocation, `compile.bzl`, the configuration library, legacy features,
private/public `cc_common`, generated proxy, action execution, configured C++,
or toolchain feature semantics.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing accepted
Boolean/string/list/dictionary/struct, `.bzl` `Label`, lazy-function and module-
freeze regressions cover every eager evaluator shape; no fresh oracle is needed.
Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
guides only declaration-owned generic aggregates, captured-function defining-
module ownership and recursive freeze before publication. Copy no Zig code,
representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash and dependency-free owner; both public Boolean
  constants; private Windows label value/type/visibility; all 30 private lazy
  function types/visibility; exact 43-field `semantics` type, scalar/list/dict
  values and order, and all 29 captured-function pointer identities.
- **Slug-native:** realization through starlark-rust frozen values and one
  defining-module heap that owns the struct, aggregate children and captured
  functions.
- **Unsupported/deferred:** any function invocation or returned semantics;
  `compile.bzl`, compile/link actions, configuration-library/legacy-feature
  construction, private/public `cc_common`, generated proxy, toolchain
  configuration and configured C++.

The frozen defining-module heap owns every lazy function and every value retained
by `semantics`; no evaluator borrow or foreign owner escapes. No production,
DICE, request, cache, async, fixture, oracle, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `9e312f958` the Rust authority is 12,804 lines, SHA-256
`72c9d73c961bcbcaac256dc7b9daaafb05797ae418b1c6097bd5162199e0bba9`.
Its final ceiling is 13,354 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 550 proof and 550 total additions; deletions do not buy
budget. Embed/hash all 234 lines; evaluate dependency-free at exact owner
`@@rules_cc+//cc/common:semantics.bzl`; prove both public Boolean constants, the
private `@@platforms+//os:windows` label, all 30 private function types and
visibility, and the exact 43-field public struct. Prove every captured function
pointer-identical to its defining binding, exact strings/Booleans, exact three
list contents/order, exact empty dictionaries, and the complete multiline
`malloc_docs` bytes. Invoke nothing and add no fixture or oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete field/function coverage, captured ownership, compatibility
split, branch selection and Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, copied/narrowed source, incomplete binding/field coverage, any manual
invocation, lost captured identity, evaluator-borrowed value, consumer/parent
claim, unpinned source, copied Zabel content, dirty authority, allowlist escape
or cap/function violation. Stop after semantics and re-audit `compile.bzl` child
source order against the toolchain-config branch.

## Immediate predecessor

Commit `9e312f958` completes action names. It does not complete semantics,
`compile.bzl`, private `cc_common`, toolchain config, or the generated
compatibility proxy.
