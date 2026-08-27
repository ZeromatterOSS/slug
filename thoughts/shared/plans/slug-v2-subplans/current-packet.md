# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-toolchain-config-lib-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete dependency-free 622-line rules_cc
`cc/cc_toolchain_config_lib.bzl` producer freezes all 13 public provider
declarations and all 21 lazy functions with exact ownership/visibility. Add no
production behavior and invoke no exported callable.

## Learned facts and decision

Base commit is `9cc0d4ace` (`Prove complete C++ semantics freeze`). It adds 363
proof lines and no production. The exact 234-line source freezes both public
Booleans, the private canonical Windows label, all 30 private functions, the
exact 43-field name/value mapping, 29 captured-function identities, scalar and
dictionary values, and exact list contents/order. No lazy function or
`configuration_field` is invoked. A focused review correction removed an
incorrect struct-iteration-order claim: exact field mappings remain, while
Slug constructor-order iteration is explicitly Slug-native. All 250 loading
units, 24 invalidation tests and 31 BUILD-loading tests, locked analysis/core
checks, CLI build, formatting and hygiene pass. Independent rereview returned
`ACCEPT`.

After accepted paths, action names, helper, semantics, CcInfo and cc_internal,
the compile branch's first incomplete child is the 666-line
`cc/private/compile/cc_compilation_helper.bzl`. The live toolchain branch instead
reaches rules_cc 0.2.17 `cc/cc_toolchain_config_lib.bzl`: 622 lines, SHA-256
`f8418490663f7e188fa060265b215e80b154a5d190b32ef75fcb6d0254017808`.
It has no loads. Its eager surface declares 13 public providers; 14 public and
seven private functions freeze lazily, with no exported provider or helper
invocation.

Therefore run only
`WP-4-7A-rules-cc-toolchain-config-lib-complete-loading-proof`. Do not invoke
any exported provider or function; do not claim constructor schemas at call
time, returned providers, validation/diagnostics, toolchain features/actions,
the 666-line compilation helper, `compile.bzl`, private/public `cc_common`, the
generated proxy, configured C++ or action execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing provider
declaration/doc/schema and lazy-function freeze regressions cover every eager
evaluator shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only declaration-owned
provider/function identities and defining-module recursive freeze before
publication. Copy no Zig code, representation, algorithm, diagnostic or
behavior.

- **Exact:** complete source/hash, dependency-free owner, 13 public provider
  callable identities with exact exported names/source label, 14 public
  function types/visibility, and seven private function types/visibility.
- **Slug-native:** realization through starlark-rust frozen provider/function
  values owned by one defining-module heap.
- **Unsupported/deferred:** invocation of every exported provider/function,
  returned values, validation and diagnostics, toolchain feature/action
  construction, the compilation helper/parent modules, configured C++ and
  action execution.

The frozen defining-module heap owns every provider callable and function; no
evaluator borrow or foreign owner escapes. No production, DICE, request, cache,
async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `9cc0d4ace` the Rust test authority is 13,226 lines, SHA-256
`5721652ef783cf8adfe4ddce7e36f1dd74f86e194267ab0cf281bbb945951419`.
Its final ceiling is 14,076 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 850 proof and 850 total additions; deletions do not buy
budget. Embed/hash all 622 lines; evaluate dependency-free at exact owner
`@@rules_cc+//cc:cc_toolchain_config_lib.bzl` with an empty repository mapping.
Prove all 13 provider values are public `provider_callable` objects with exact
source label/exported names. Prove all 14 public functions are callable and all
seven private functions are callable but absent from public lookup. Prove the
module exposes exactly the named 27 public declarations plus no public private
helper. Invoke no provider/function and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete provider/function inventory, identity/visibility,
no-invocation boundary, compatibility split, branch selection and Zabel's
guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source, incomplete declaration coverage,
any exported callable invocation, lost provider identity, evaluator-borrowed
value, consumer/parent claim, unpinned source, copied Zabel content, dirty
authority, allowlist escape or cap/function violation. Stop after this library
and re-audit the 666-line compile helper against the toolchain consumers.

## Immediate predecessor

Commit `9cc0d4ace` accepts only complete eager `semantics.bzl` freeze with no
lazy-function or `configuration_field` call. It does not accept this library,
any provider/function invocation, returned toolchain values, compile/helper
modules, private/public `cc_common`, configured C++ or actions.
