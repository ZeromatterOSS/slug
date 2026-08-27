# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-skylib-common-settings-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete Bazel Skylib
`rules/common_settings.bzl` build-setting declaration family without
invocation, supplying the final complete direct child of rules_rust toolchain.

## Learned facts and decision

Commit `ee15a98c5` freezes all 27 authenticated lines of dependency-free
`rust/settings/incompatible.bzl`. The remaining incomplete direct child of
`rust/private/toolchain.bzl` is Bazel Skylib `rules/common_settings.bzl`; prior
synthetic toolchain proofs declared `BuildSettingInfo` locally and cannot count
as a complete recursive child.

Authenticated Bazel Skylib `rules/common_settings.bzl` is dependency-free, 181
lines, and has SHA-256
`f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20`.
It declares one documented provider, two private reusable string attributes,
six private functions and the complete nine-rule int/bool/string-list/string
build-setting family. All eager provider, attribute, typed config descriptor
and rule shapes are already admitted.

Run only this packet. Freeze the exact source at its canonical owner and prove
the provider identity, both private attribute bindings, six function bindings,
all nine rule classes/build-setting kinds and their shared attribute schemas,
visibility, and exact ten-public/eighteen-all inventories. Invoke nothing. Do
not continue into the toolchain parent.

## Generic architecture, authorities and compatibility

This is one coherent full build-setting declaration category from BCR
Starlark, not nine Rust host implementations. Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and authenticated Bazel Skylib
bytes are sole exact authority. Reuse accepted evidence; add no fixture/oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its declaration ownership may guide identity
assertions, but no Zig code, representation, algorithm, cache, setting behavior
or diagnostic is copied and Zabel is not compatibility authority.

- **Exact:** complete 181-line source/hash; canonical owner/path/mapping;
  provider owner/name; two private attributes; six private functions; nine
  rule/build-setting/schema declarations; exact ten-public/eighteen-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and declaration/test
  representations.
- **Unsupported/deferred:** every provider/rule/function invocation, setting
  defaults/values, CLI/configuration/transition behavior, configured providers
  and TemplateVariableInfo consumers.

No retained semantic collection, evaluator borrow or invocation result is
added. DICE, request/revision, filesystem, cache, async, memory-ledger and
fallback concerns are inapplicable to this test-only proof. There is no
fallback and no Buck2 utility change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `ee15a98c5`, the Rust test authority is 32,972 lines, SHA-256
`8f173708ff2cfca58bd35a152a7ee83948f4e98c593e656581a3a695d26adc43`.
Its final ceiling is 33,422 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 450 proof and 450 total additions; deletions do not buy
budget. Embed/hash all 181 authenticated lines. Evaluate at
`@@bazel_skylib+//rules:common_settings.bzl`, path
`/bazel_skylib/rules/common_settings.bzl`, with empty mapping and no children.
Prove every eager declaration, visibility and exact inventory. Invoke nothing.

Run the focused proof and its direct compile dependent. Because this follows a
green full loading/integration/dependent checkpoint and two green proof-only
source freezes, do not repeat broad suites unless focused evidence is suspect.
Run formatting, diff, caps/function-size and archive hygiene, then root review
of source authority, full category coverage, declaration/inventory fidelity,
no-invocation scope, generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, unexpected
dependency/eager behavior, any provider/rule/function invocation, configured
setting semantic claim, evaluator-borrowed value, incomplete family, unpinned
source, copied Zabel content, dirty authority, allowlist escape, or cap/function
violation. Stop after this module and re-audit complete
`rust/private/toolchain.bzl`.

## Immediate predecessor

Commit `ee15a98c5` accepts only complete incompatible-settings declaration
loading without invocation.
