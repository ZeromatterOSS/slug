# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-create-linkstamp-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 44-line rules_cc
`cc/private/link/create_linkstamp.bzl` module over its accepted helper child.
Prove its complete imported/provider/function surface without invocation.

## Learned facts and decision

Commit `da0d9a5a5` accepts the complete 137-line
`create_linking_context_from_compilation_outputs.bzl` source over five actual
complete children. It proves all seven imports, its function and exact
seven-public/eight-all inventories without invocation. Focused, all 270
loading-library, 24/31 integration, locked analysis/core, CLI, format,
diff/source and archive-baseline gates pass within 0/279/279; independent
review returned `ACCEPT`.

Rules_cc 0.2.17 `cc/private/cc_common.bzl` next loads complete
`cc/private/link/create_linkstamp.bzl`, 44 lines, SHA-256
`8d5fc394e31c5f0eb8a84f5020f35e71f90cdbf89591e44d1c0da8a8899e6000`.
Its sole child, `cc/common/cc_helper_internal.bzl`, is accepted. The module
retains one public imported function, constructs one private provider, and
defines one public lazy function. It has no other top-level
binding shape and invokes no function at top level.

Run only `WP-4-7A-rules-cc-create-linkstamp-complete-loading-proof`. Do not
invoke `create_linkstamp` or the provider, inspect callable defaults, construct
a linkstamp, register an action, or claim the `cc_common.bzl` consumer.

## Generic architecture, authorities and compatibility

This is a generic Starlark loader/evaluator proof with authenticated BCR
rules_cc as a demanding integration corpus; it is not a C++-specific parser or
an alternate implementation of rules_cc. Bazel 9 C++ rule definitions and
their `.bzl` modules come from rules_cc. The rules_cc
`cc/private/cc_internal.bzl` bridge is likewise loaded as Starlark, but the
value it exports is obtained from the Bazel host capability behind
`cc_common.internal_DO_NOT_USE()`; low-level host primitives therefore remain
distinct from BCR-owned rule/module logic.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child, provider-declaration and lazy-function regressions
cover every top-level binding shape. Invocation-oriented upstream tests are
skipped because configured linkstamp/action behavior is an unsupported later
phase. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only peer guidance:
its generic-evaluator/Bazel-host split and producer-owned frozen module
lifetime support this architecture, but no Zig code, representation,
algorithm, diagnostic or behavior may be copied.

- **Exact:** complete source/hash/owner/mapping, including the provider and
  field documentation as authenticated source bytes; child label and mapping;
  imported pointer identity/visibility; provider identity/type/visibility;
  function type/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining its child heap and owning its provider and function.
- **Unsupported/deferred:** retained/extractable provider or field
  documentation and runtime schema inspection; provider/function invocation
  and values; linkstamp inputs, compilation, headers, paths,
  toolchain/configuration behavior; action registration, ActionKey and
  execution; `cc_common.bzl` and configured C++ consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this
is test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `da0d9a5a5` the Rust test authority is 24,990 lines, SHA-256
`52b3e39ec8203d483c4cb87181ba5d0b204a6009ef38600d6768ac2e2fd24354`.
Its final ceiling is 25,240 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 250 proof and 250 total additions; deletions do not buy
budget. Embed/hash all 44 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:create_linkstamp.bzl`, path
`/rules_cc/cc/private/link/create_linkstamp.bzl`, with empty owner mapping and
the accepted helper at actual identity
`@@rules_cc+//cc/common:cc_helper_internal.bzl` carrying
`bazel_skylib -> bazel_skylib+`.

Prove public `wrap_with_check_private_api` pointer-identical to its actual child
export. Prove private `_LinkstampInfo` parses and freezes from the exact source,
has exact defining-module provider identity/type, and is absent publicly. Do
not claim retained documentation or inspect its runtime schema. Prove public
`create_linkstamp` is type `function`. Assert exact two-public/three-all name
sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/provider/function inventory, defining identities,
no-invocation/linkstamp/action/consumer boundary, compatibility split,
source-order selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation or callable-default inspection, lost identity, evaluator-
borrowed value, linkstamp/action/consumer claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop
after this producer and re-audit `cc_common.bzl` in source order.

## Immediate predecessor

Commit `da0d9a5a5` accepts only complete linking-context-producer defining-
module freezing. `233cdf9ef` accepts only the C++ linking helper and
`cb71a302d` accepts only the universal environment and bounded set subset. None
accepts provider/function invocation, `cc_common.bzl`, configured consumers or
actions.
