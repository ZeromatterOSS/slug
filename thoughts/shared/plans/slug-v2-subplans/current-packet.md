# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compile-build-variables-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 18-line rules_cc
`cc/private/rules_impl/native_cc_common.bzl` leaf and 644-line
`cc/private/compile/compile_build_variables.bzl` producer. Prove their exact
eager/imported surfaces and defining identities without invoking anything.

## Learned facts and decision

Base commit is `3060e4d4d` (`Prove complete C++ compilation helper freeze`). It
byte-verifies all 666 helper lines and proves all imported pointers, exact
public/all-visibility names, private constant/provider, 12 lazy functions and
the public captured-function struct over five complete children. Independent
review found one mapped helper child whose reused manifest identity was empty;
the accepted correction retains the actual mapping and asserts all five child
labels/mappings. Focused, all 253 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff and archive gates pass; review returned
`ACCEPT`.

The required source-order audit reaches rules_cc 0.2.17
`cc/private/compile/compile.bzl` (2,295 lines, SHA-256 `bec506ff…`). Its first
incomplete direct child is 266-line `compile_action_templates.bzl`
(`10a43c51…`), which itself first loads 644-line
`compile_build_variables.bzl` (SHA-256
`463ea66c2423cab80153ccbb25193516e00e887d24bd4ba6d7b485a19c8d8b54`).
Private `cc_common.bzl` also loads that producer directly, so generated proxy,
rules_rust/toolchain and compile/action-template paths share this frontier.

The producer's only not-yet-completely-proved leaf is 18-line
`cc/private/rules_impl/native_cc_common.bzl` (SHA-256
`d8e5fedab99534bd1a926dd780e2bfdc66e9a8e70cd29561a591969123084e46`),
whose sole statement aliases the existing `.bzl` `cc_common` predeclared value.
The other two children, complete `cc_helper_internal.bzl` and
`cc_internal.bzl`, are accepted. Eager evaluation creates a 25-field `_VARS`
struct, empty-schema private provider plus sentinel instance, ordered
`_SOURCE_TYPES_FOR_CXXOPTS` set and function defaults; all remaining bodies are
lazy. The accepted universal environment owns every referenced common builtin.

Run only `WP-4-7A-rules-cc-compile-build-variables-complete-loading-proof`.
Do not invoke a function/provider, inspect callable ABI or results, implement a
native C++ method, or claim the action-template, compile, cc_common proxy,
configured C++ or action paths.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing complete
child proofs and accepted provider/struct/depset/set/lazy-function regressions
cover every eager shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
separation of defining-module-owned global containers, provider schemas and
function defaults from later invocation values informs the proof architecture
only. Copy no Zig code, representation, algorithm, diagnostic or behavior.

- **Exact:** both complete sources/hashes/owners/mappings; native wrapper alias;
  all four producer imports and visibility; `_VARS` field-name/value mapping;
  provider callable and sentinel instance identity; ordered source-type set;
  all 13 function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through starlark-rust frozen defining-module
  heaps retaining imports/defaults/globals; no schemaless-struct iteration-order
  parity claim.
- **Unsupported/deferred:** every function/provider invocation and callable ABI;
  native C++ methods and compile-variable values; action-template/compile parent;
  private/public `cc_common`, proxy, configured C++, actions and execution.

The frozen producer owns its struct, provider, sentinel, set and functions and
retains child heaps for imports. No evaluator borrow or invocation value escapes.
No production, DICE, request, cache, async, fixture, oracle, hot-path, fallback
or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `3060e4d4d` the Rust test authority is 14,852 lines, SHA-256
`03dc3da427bff072b60134770caf0dcc74a9cd3cdc4fa61c0efaafa9acabd2be`.
Its final ceiling is 15,902 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 1,050 proof and 1,050 total additions; deletions do not
buy budget. Embed/hash all 18 and 644 authenticated lines. Evaluate the native
leaf at `@@rules_cc+//cc/private/rules_impl:native_cc_common.bzl`, then evaluate
the producer at
`@@rules_cc+//cc/private/compile:compile_build_variables.bzl`, both with empty
repository mappings. Reuse complete helper/internal children with their actual
defining identities. Assert all three child labels/mappings and pointer identity
for `extensions`, private allowlist, `_cc_internal` and `_cc_common_internal`.

Prove `_VARS` has exactly its 25 source fields/strings; prove the private
`_UnboundValueProviderDoNotUse` callable export identity and `_UNBOUND` instance
identity; prove the source-type set's exact type, order and membership; prove
six public and seven private function types/visibility and exact public and
all-visibility name sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/eager inventory, defining identities, no-invocation
boundary, compatibility split, branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported/global/default identity, evaluator-borrowed
value, parent/consumer claim, unpinned source, copied Zabel content, dirty
authority, allowlist escape or cap/function violation. Stop after this producer
and re-audit action-template/compile source order against proxy/toolchain
consumers.

## Immediate predecessor

Commit `3060e4d4d` accepts only the complete compilation-helper freeze.
`cb71a302d` accepts only the exact universal environment and bounded set subset.
Neither accepts this producer, callable behavior, parent compilation modules,
configured C++ or actions.
