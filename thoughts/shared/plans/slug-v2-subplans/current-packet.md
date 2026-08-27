# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compile-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 2,295-line rules_cc
`cc/private/compile/compile.bzl` producer over its eleven accepted children.
Prove its complete imported/eager/function surface without invoking anything.

## Learned facts and decision

Base implementation commit is `bb11a1f73` (`Prove C++ compile action templates
freeze`). It byte-verifies all 266 action-template lines, reconstructs six
complete children with their actual labels/mappings, and proves all ten imported
pointers/visibility, one public plus four private lazy functions and exact
public/all-visibility names. It invokes nothing. Focused, all 255 loading-library,
24/31 integration, locked analysis/core, CLI, format/diff and archive gates pass
within 0/482/482; independent review returned `ACCEPT`.

The required audit now reaches rules_cc 0.2.17
`cc/private/compile/compile.bzl`, 2,295 lines, SHA-256
`bec506ffc3be08fffc4842b9daac498773534db9916121648a5527fac84cabea`.
All eleven loaded children have accepted complete proofs: Skylib paths, action
names, helper internal, semantics, CcInfo, cc_internal, compilation helper,
compilation outputs, compile action templates, compile build variables and LTO
compilation context. Private `cc_common.bzl` reaches this parent before its
remaining children, so compile, generated-proxy and toolchain consumers share
this frontier.

The eager surface retains 25 imports: 21 public and four private aliases. It
creates private `_VALID_CPP_SOURCE_TYPES`, private initialized
`_CppSourceInfo` plus `__new_cpp_source_info`, and public
`SOURCE_CATEGORY_CC`, `SOURCE_CATEGORY_CC_AND_OBJC` and
`LTO_SOURCE_EXTENSIONS`. It defines one public `compile` function and 27
private functions. All sets, provider-with-init/raw-constructor and lazy-function
shapes are accepted; no body or constructor is invoked.

Run only `WP-4-7A-rules-cc-compile-complete-loading-proof`. Do not invoke a
function/provider, inspect callable ABI/results, implement a native C++ method,
or claim private/public `cc_common`, proxy, configured C++, actions or
execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Accepted
complete-child and set/provider/lazy-function regressions cover every eager
shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
separation of defining-module-owned imports, provider initializer, global sets
and function defaults from later invocation values informs proof architecture
only. Copy no Zig code, representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all eleven child
  labels/mappings; all 25 imported pointer identities/visibility; four ordered
  sets; private provider callable/raw constructor types and export identity; one
  public and 27 private function types/visibility; exact public and
  all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining imported child heaps, global sets, provider and functions.
- **Unsupported/deferred:** every function/provider invocation and callable
  ABI/result; native C++ methods; private/public `cc_common`, proxy, configured
  C++, actions and execution.

The frozen producer retains child heaps and owns its sets, provider and
functions. No evaluator borrow or invocation value escapes. No production, DICE,
request, cache, async, fixture, oracle, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `bb11a1f73` the Rust test authority is 16,208 lines, SHA-256
`0aee8d95c7f0734ca265f1a48d80de9e96078fc9df2f88106cbcd4935167a6c8`.
Its final ceiling is 19,208 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 3,000 proof and 3,000 total additions; deletions do not
buy budget. Embed/hash all 2,295 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/compile:compile.bzl` with
`bazel_skylib -> bazel_skylib+` mapping and all eleven complete children in
source order. Reuse their actual defining identities and assert every child
label/mapping.

Prove all 25 imports pointer-identical to child exports with exact visibility.
Prove exact type, order and membership of the three-item private valid-source set
and public 24-item CC, 25-item CC-and-ObjC and eight-item LTO sets. Prove private
`_CppSourceInfo` provider callable source/export identity,
`__new_cpp_source_info` function type, all 28 source functions and exact public
and all-visibility name sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/eager/function inventory, defining identities,
no-invocation boundary, compatibility split, branch selection and Zabel's
peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported/eager identity, evaluator-borrowed value,
consumer claim, unpinned source, copied Zabel content, dirty authority, allowlist
escape or cap/function violation. Stop after this producer and re-audit private
`cc_common` and proxy/toolchain consumers.

## Immediate predecessor

Commit `bb11a1f73` accepts only complete action-template freezing.
`97faa6e71` and `3060e4d4d` accept only its compile-variable and compilation
helper children; `cb71a302d` accepts only the universal environment and bounded
set subset. None accepts this producer, callable behavior, configured C++ or
actions.
