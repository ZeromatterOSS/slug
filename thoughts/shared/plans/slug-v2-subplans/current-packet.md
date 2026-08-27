# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-link-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 197-line rules_cc
`cc/private/link/link.bzl` module over its four accepted children. Prove its
complete imported/dictionary/function surface without invocation.

## Learned facts and decision

Commit `6959f0370` accepts the complete 44-line `create_linkstamp.bzl` source
over its actual helper child. It proves the import, private provider identity,
public function and exact two-public/three-all inventories without invocation.
Focused, all 271 loading-library, 24/31 integration, locked analysis/core, CLI,
format/diff/source and archive-baseline gates pass within 0/119/119;
independent review returned `ACCEPT`.

Rules_cc 0.2.17 `cc/private/cc_common.bzl` next loads complete
`cc/private/link/link.bzl`, 197 lines, SHA-256
`666e819dee4777d0c3d8624e18588a905046532a6668d89d5744419cbee4a0e2`.
All four source-order children are accepted: `cc_internal.bzl`, compilation
outputs, linking helper and target types. The module retains five imported
identities, eagerly constructs one private five-entry `_TARGET_TYPE`
dictionary, and defines one public lazy function. It has no other top-level
binding shape and invokes no function at top level.

Run only `WP-4-7A-rules-cc-link-complete-loading-proof`. Do not invoke `link`,
inspect callable defaults, create linking outputs, register an action, or claim
the `cc_common.bzl`, `cc_binary.bzl` or `cc_shared_library.bzl` consumers.

## Generic architecture, authorities and compatibility

This is a generic Starlark loader/evaluator proof with authenticated BCR
rules_cc as a demanding integration corpus; it is not a C++-specific parser or
an alternate implementation of rules_cc. Bazel 9 C++ rule/module logic remains
in rules_cc. The BCR `cc_internal.bzl` wrapper remains ordinary loaded
Starlark, while its low-level object is a distinct Rust host capability behind
`cc_common.internal_DO_NOT_USE()`.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Complete source,
accepted-child, frozen tuple/dictionary and lazy-function regressions cover
every top-level binding shape. Rules_cc `tests/simple_binary/BUILD` and the
configured `cc_binary.bzl`/`cc_shared_library.bzl` consumers are skipped because
they require function invocation, configured linking and action behavior from
an unsupported later phase. Add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept-only peer guidance: its generic-evaluator/Bazel-host split and
producer-owned frozen module lifetime support this architecture, but no Zig
code, representation, algorithm, diagnostic or behavior may be copied.

- **Exact:** complete source/hash/owner/mapping; four child labels/mappings;
  five imported pointer identities/visibility; five `_TARGET_TYPE` key/value
  rows independent of dictionary iteration order; one public function
  type/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all four child heaps and owning its dictionary and function;
  unclaimed dictionary iteration order.
- **Unsupported/deferred:** callable-default inspection; function invocation
  and result; link inputs/outputs, validation and diagnostics; toolchain,
  configuration, path and linking behavior; action registration, ActionKey and
  execution; `cc_common.bzl` and configured rule consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this
is test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `6959f0370` the Rust test authority is 25,109 lines, SHA-256
`1c81aa8cc33f12fbca8d9309e03d65401797a723c1050d7f47a6c141b50feee0`.
Its final ceiling is 25,559 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 450 proof and 450 total additions; deletions do not buy
budget. Embed/hash all 197 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:link.bzl`, path
`/rules_cc/cc/private/link/link.bzl`, with empty owner mapping and all children
at their actual defining identities.

Prove exact child mappings: `cc_internal.bzl`,
`cc_compilation_outputs.bzl` and `target_types.bzl` have empty mappings;
`cc_linking_helper.bzl` carries `bazel_skylib -> bazel_skylib+`. Prove private
`_cc_internal` pointer-identical to `cc_internal` and absent publicly. Prove
public `EMPTY_COMPILATION_OUTPUTS`, `create_cc_link_actions`, `LINKING_MODE` and
`LINK_TARGET_TYPE` pointer-identical to their actual child exports.

Prove private `_TARGET_TYPE` is a five-entry dictionary and absent publicly.
For keys `("cpp", "executable")`, `("cpp", "dynamic_library")`,
`("objc", "executable")`, `("objcpp", "executable")`, and
`("objc", "archive")`, prove the exact static/dynamic pair is `None` or
pointer-identical to the corresponding accepted `LINK_TARGET_TYPE` member.
Do not claim dictionary iteration order. Prove public `link` is type `function`.
Assert exact five-public/seven-all name sets. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/dictionary/function inventory, defining
identities, no-invocation/link/action/consumer boundary, compatibility split,
source-order selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding or
dictionary coverage, invocation or callable-default inspection, lost identity,
evaluator-borrowed value, link/action/consumer claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape or cap/function violation.
Stop after this producer and re-audit `cc_common.bzl` in source order.

## Immediate predecessor

Commit `6959f0370` accepts only complete linkstamp defining-module freezing.
`da0d9a5a5` accepts only the linking-context producer and `cb71a302d` accepts
only the universal environment and bounded set subset. None accepts link
function behavior, `cc_common.bzl`, configured consumers or actions.
