# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-action-names-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete dependency-free 220-line rules_cc
`cc/action_names.bzl` producer freezes its 33 string constants, exact
`ACTION_NAMES` struct, seven ordered action-name lists, and exact
`ACTION_NAME_GROUPS` struct. Add no production behavior and invoke nothing.

## Learned facts and decision

Base commit is `63d4bda76` (`Prove complete compilation outputs freeze`). It
adds exactly 450 proof lines and no production, embeds/hash-checks all 226
compilation-output lines, reconstructs the complete helper/internal/LTO closure,
and proves five imported pointers, sentinel/output providers, all lazy types and
the exact source-owned empty output without manual invocation. Focused proof,
247 library tests, 24 invalidation tests, 31 BUILD-loading tests, locked
analysis/core checks, CLI build, formatting and hygiene pass. Independent review
returned `ACCEPT`, including captured-helper closure ownership.

Private `cc_common.bzl` source order next enters the 2,295-line
`cc/private/compile/compile.bzl`. Its accepted Skylib-paths first child is
complete; the first incomplete child is rules_cc 0.2.17
`cc/action_names.bzl`: 220 lines, SHA-256
`e52d16474bd3ad3a0e0a4cd0cb1ad60b968ac5b0b2bcb0b1cffe85aedf80ed9d`.
It has no loads or functions. Its eager rows bind 33 public string constants,
one 33-field struct, seven ordered lists (including one list concatenation), and
one seven-field struct. Every expression uses already accepted exact evaluator
shapes. The deferred toolchain-config branch's 1,387-line `legacy_features.bzl`
also loads this producer first, so this is the smallest source-ordered frontier
shared by both audited branches.

Therefore run only
`WP-4-7A-rules-cc-action-names-complete-loading-proof`. Do not claim
`compile.bzl`, legacy features, toolchain-config constructors, private/public
`cc_common`, generated proxy, action execution, configured C++, or toolchain
feature semantics.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing accepted
string, list, list-addition, struct and module-freeze regressions cover every
evaluator shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only declaration-owned
generic structs/lists and defining-module recursive freeze. Copy no Zig code,
representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash and dependency-free owner; all 33 exported
  constant names/string values; the exact `ACTION_NAMES` field/value mapping;
  all seven exported list contents/order; the exact `ACTION_NAME_GROUPS`
  field/list mapping and retained aggregate identities.
- **Slug-native:** realization through starlark-rust frozen strings, lists and
  structs owned by the defining module's frozen heap.
- **Unsupported/deferred:** any consumer behavior; `compile.bzl`, compile or
  link actions, legacy-feature/config-library construction, private/public
  `cc_common`, generated proxy, toolchain configuration and configured C++.

The frozen defining-module heap owns both structs and every list; no evaluator
borrow or foreign owner escapes. No production, DICE, request, cache, async,
fixture, oracle, hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `63d4bda76` the Rust authority is 12,476 lines, SHA-256
`ed691b14328bf8cc7dede1195b5ee2bf2d5a50470ead804a9541b52377a4e5c4`.
Its final ceiling is 12,926 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 450 proof and 450 total additions; deletions do not buy
budget. Embed/hash all 220 lines; evaluate dependency-free at exact owner
`@@rules_cc+//cc:action_names.bzl`; prove all 33 public constants and exact
values, all 33 `ACTION_NAMES` fields, all seven list contents/order, all seven
`ACTION_NAME_GROUPS` fields and their pointer-identical exported lists. Prove
the two aggregate types and public visibility. Invoke nothing and add no fixture
or oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete mappings/order, defining-module ownership, compatibility split,
the shared-branch selection and Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, copied/narrowed source, incomplete constant/field/list coverage, lost
aggregate identity, evaluator-borrowed value, consumer/parent claim, unpinned
source, copied Zabel content, dirty authority, allowlist escape or cap/function
violation. Stop after action names and re-audit `compile.bzl` child source order
against the toolchain-config branch.

## Immediate predecessor

Commit `63d4bda76` completes compilation outputs. It does not complete action
names, `compile.bzl`, private `cc_common`, toolchain config, or the generated
compatibility proxy.
