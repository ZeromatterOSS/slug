# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-semantics-complete-loading-proof-r2`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete dependency-free 234-line rules_cc
`cc/common/semantics.bzl` producer freezes its eager constants/label, all 30
lazy functions, and the exact 43-field function-capturing `semantics` struct.
Add no production behavior and invoke nothing.

## Learned facts and decision

Base commit is `fc131d7aa` (`Add configuration field loading binding`). It adds
9 production and 59 proof lines. `.bzl` loading now exposes the exact required
positional-or-named string ABI; BUILD keeps the binding absent; every valid
positional/named form returns the same Slug-native fail-closed error before a
result exists. All 249 loading-library, 24 invalidation and 31 BUILD-loading
tests, locked analysis/core checks, CLI build, formatting and hygiene pass.
Independent review returned `ACCEPT`; no descriptor/schema/configured behavior
or retained type was added.

The first exact semantics attempt stopped during name resolution because lazy
`_get_coverage_attrs` referenced the then-absent `configuration_field`. The
accepted binding now satisfies compilation/freeze without invocation. Private
`cc_common.bzl` source order therefore returns to rules_cc 0.2.17
`cc/common/semantics.bzl`: 234 lines, SHA-256
`029254fd58eb8b3bf32a0f772e479b991a51ce21a6f6cc8a5739aadbce3900da`.
It has no loads. Eager rows bind two public Booleans, 30 lazy private functions,
one private canonical `Label`, and one public 43-field struct that captures 29
functions plus exact strings, Booleans, three lists and two empty dictionaries.
No function is invoked. The alternative toolchain-config branch still reaches
the dependency-free 622-line `cc_toolchain_config_lib.bzl`, so semantics remains
the smaller source-ordered frontier.

Therefore run only
`WP-4-7A-rules-cc-semantics-complete-loading-proof-r2`. Do not invoke any lazy
function or `configuration_field`; do not claim `compile.bzl`, configuration
library, legacy features, private/public `cc_common`, generated proxy, action
execution, configured C++, or toolchain feature semantics.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing accepted
Boolean/string/list/dictionary/struct, `.bzl` `Label`, lazy-function,
configuration-field name-resolution and module-freeze regressions cover every
eager evaluator shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only declaration-owned
generic aggregates, captured-function defining-module ownership and recursive
freeze before publication. Copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** complete source/hash and dependency-free owner; both public Boolean
  constants; private Windows label value/type/visibility; all 30 private lazy
  function types/visibility; exact 43-field `semantics` type, scalar/list/dict
  values and order, and all 29 captured-function pointer identities.
- **Slug-native:** realization through starlark-rust frozen values and one
  defining-module heap that owns the struct, aggregate children and captured
  functions; the accepted configuration-field binding remains uncalled.
- **Unsupported/deferred:** any function or `configuration_field` invocation;
  returned semantics, late-bound descriptors/resolution, `compile.bzl`, actions,
  configuration-library/legacy-feature construction, private/public `cc_common`,
  generated proxy, toolchain configuration and configured C++.

The frozen defining-module heap owns every lazy function and every value retained
by `semantics`; no evaluator borrow or foreign owner escapes. No production,
DICE, request, cache, async, fixture, oracle, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` only for
  the prerequisite acceptance and this packet's eventual terminal row.

At base `fc131d7aa` the Rust test authority is 12,863 lines, SHA-256
`1e5d3ba1ad2c5f05ac9fab656bc4ec0e1d56d249376296a50519973d73d47251`.
Its final ceiling is 13,413 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 550 proof and 550 total additions; deletions do not buy
budget. Embed/hash all 234 lines; evaluate dependency-free at exact owner
`@@rules_cc+//cc/common:semantics.bzl` with `platforms -> platforms+` mapping;
prove both public Boolean constants, private `@@platforms+//os:windows` label,
all 30 private function types/visibility, and the exact 43-field public struct.
Prove every captured function pointer-identical to its defining binding, exact
strings/Booleans, exact three list contents/order, both empty dictionaries, and
complete multiline `malloc_docs` bytes. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete field/function coverage, captured ownership, no-invocation
boundary, compatibility split, branch selection and Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source, incomplete binding/field coverage,
any manual invocation, lost captured identity, evaluator-borrowed value,
consumer/parent claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after semantics and re-audit
`compile.bzl` child source order against the toolchain-config branch.

## Immediate predecessor

Commit `fc131d7aa` accepts only the `.bzl` configuration-field binding/ABI and
fail-closed call boundary. It does not accept semantics, any successful
configuration-field call, late-bound values, `compile.bzl`, private `cc_common`,
toolchain config, or the generated compatibility proxy.
