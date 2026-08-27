# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compatibility-symbols-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the complete Bazel-9 branch of the BCR-authenticated rules_cc
compatibility `symbols.bzl` producer over its six accepted children. Prove all
imports and seven exports, including the intentional ObjcInfo alias, without
invocation.

## Learned facts and decision

Commit `873f07e2d` byte-verifies and freezes all 788 authenticated private
`cc_common.bzl` lines over 22 accepted direct children. It proves every child
owner/mapping/path, all 35 imported identities, private eager values, 38
private functions, exact 56-field façade order/identity and exact
32-public/78-all inventories without invoking any C++ façade method. Focused,
all 288 loading-library, 25 invalidation and 32 BUILD-loading tests, locked
analysis/core checks and CLI build, format/diff/source and archive-baseline
gates pass within 0/1,236/1,236; root review returned `ACCEPT`.

Rules_cc 0.2.17 `cc/extensions.bzl` generates `symbols.bzl` from its Bazel-9
branch. The authenticated normalized payload is 14 lines, SHA-256
`31c58bfb31755ad1546cc295885704b69f0365a797d77c26481b4863a62c519c`.
It loads complete private cc_common, CcInfo, CcSharedLibraryInfo,
DebugPackageInfo, ObjcInfo and CcToolchainConfigInfo producers, then exports
seven bindings; `new_objc_provider` intentionally aliases `ObjcInfo`. All six
defining modules are now accepted complete. No function or provider is called.

Run only `WP-4-7A-rules-cc-compatibility-symbols-complete-loading-proof`.
Do not execute the module extension/repository rule, evaluate the public
wrapper, invoke an export, or configure a C++ rule, toolchain or action.

## Generic architecture, authorities and compatibility

This remains generic BCR Starlark loading/evaluation and generated-repository
module composition, not a C++ parser or Rust rules implementation. Slug's
Buck2-derived Rust Starlark parser and frozen-module infrastructure evaluate
the BCR-authored Bazel-9 payload at its canonical generated-repository owner
and retain actual child exports.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 generator bytes are sole exact authority.
The pinned version selects the new branch. Existing complete child proofs cover
the semantic closure; add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned re-export and frozen identity
patterns guide the boundary, but no Zig code, representation, algorithm or C++
behavior is copied and Zabel is not compatibility authority.

- **Exact:** authenticated 14-line generator payload/hash; canonical generated
  owner and `rules_cc -> rules_cc+` mapping; six exact child owners/mappings;
  six private imported identities; seven pointer-identical public exports in
  exact inventory, including `ObjcInfo == new_objc_provider`; complete freeze
  without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation;
  one generated module retaining all six accepted child heaps.
- **Unsupported/deferred:** extension/repository-rule execution and physical
  generation; export invocation/provider construction; public
  `cc/common/cc_common.bzl`; C++ rule, toolchain, configuration, action,
  ActionKey and execution behavior.

The generated module is the re-export owner while values remain
pointer-identical to their defining rules_cc modules. No evaluator borrow or
invocation result escapes. DICE, request/revision, filesystem generation,
cache, async and fallback concerns are not exercised by this test-only proof.
There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `873f07e2d`, the Rust test authority is 29,434 lines, SHA-256
`e143102b25e4accfec30780719c183f674acf4f5a34e0029bea7035bdd760669`.
Its final ceiling is 29,934 lines. Each new proof/helper function must remain at
most 120 physical lines. Keep the existing private loading harness as the sole
source-proof ledger; add no production responsibility or generic archive.

Caps are 0 production, 500 proof and 500 total additions; deletions do not buy
budget. Embed/hash the exact 14 normalized payload lines from authenticated
`extensions.bzl:95-108`. Evaluate at canonical owner
`@@rules_cc++compatibility_proxy+cc_compatibility_proxy//:symbols.bzl`, path
`/rules_cc_compatibility_proxy/symbols.bzl`, mapping
`rules_cc -> rules_cc+`, with all six real children. Reuse the complete private
cc_common and provider constructors; do not copy, truncate or mock children.

Prove each child owner/path/mapping and each private imported binding
pointer-identical to its child export. Prove exact public seven/all thirteen
inventories, all public exports pointer-identical to imports, and
`new_objc_provider` pointer-identical to `ObjcInfo`. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
generator authentication, complete children, identities/inventories,
no-invocation scope, generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, generator/hash mismatch, wrong Bazel
branch, missing parser/global/evaluator shape, incomplete/copied child, missing
identity/inventory coverage, invocation/output inspection, evaluator-borrowed
value, C++ semantic claim, unpinned source, copied Zabel content, dirty
authority, allowlist escape, or cap/function violation. Stop after symbols and
re-audit the 18-line public `cc/common/cc_common.bzl` wrapper.

## Immediate predecessor

Commit `873f07e2d` accepts only the complete private cc_common producer and its
children. Prior proxy proofs cover only direct-provider and ObjcInfo slices;
none accepts the complete generated symbols module, public cc_common wrapper,
configured rules or actions.
