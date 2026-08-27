# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compilation-helper-complete-loading-proof-r2`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 666-line rules_cc
`cc/private/compile/cc_compilation_helper.bzl` producer retains every imported
identity, its private constant/provider, all 12 lazy functions and the public
one-field captured-function helper struct. Add no production and invoke nothing.

## Learned facts and decision

Base commit is `5c3b4492f` (`Recognize Starlark crate in archive checker`). The
accepted `cb71a302d` predecessor creates the process-stable exact 30-name Bazel
9.2 universe, migrates every active BUILD, `.bzl`, root/nonroot/include MODULE,
REPO and core evaluator, enables the real `SetType`, removes REPO's stale shim,
excludes `chr`/`ord` and preserves context overlays. Commit `5c3b4492f` then
restores the archive app-path gate for the accepted low-level crate. Only the
three longstanding thoughts-path baseline rows remain.

The earlier complete-helper attempt stopped only because lazy
`_module_map_struct_to_module_map_content` resolved absent `set` at line 251.
The complete +855 proof candidate was removed byte-for-byte before the universe
packet. That architectural blocker is now resolved; retry the same complete
source from the new clean base rather than narrowing or substituting it.

Rules_cc 0.2.17 `cc/private/compile/cc_compilation_helper.bzl` is 666 lines,
SHA-256 `2c484cade81f0d70efd203612b5492e5578871b2b3e9be7987de42de9c57863f`.
Its five loaded children—Skylib paths, common helper, semantics, private CcInfo
and cc_internal—have complete accepted freeze proofs. Its eager surface retains
eight public and one private imported values, one private string constant, one
private provider declaration, ten private and two public lazy functions, and a
public one-field struct capturing the private initialization function.

Run only `WP-4-7A-rules-cc-compilation-helper-complete-loading-proof-r2`. Do
not invoke a function/provider, inspect provider constructor behavior, run
action helpers or claim `compile.bzl`, private/public `cc_common`, generated
proxy, configured C++ or action execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Existing
complete child proofs and accepted provider/struct/lazy-function evaluator
regressions cover every eager shape; no fresh oracle is needed. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer
implementation whose defining-module ownership of imported values, captured
functions and recursive freeze informs the proof architecture only. Copy no Zig
code, representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all nine imported pointer
  identities and their visibility; exact private constant; private provider
  callable identity; ten private and two public function types/visibility; exact
  public helper field-name/value mapping and captured-function pointer identity.
- **Slug-native:** realization through one starlark-rust defining-module heap
  retaining imports, provider, functions and struct; no exact schemaless struct
  iteration-order claim.
- **Unsupported/deferred:** every function/provider invocation, provider schema
  and returned values, compilation/helper actions, the parent `compile.bzl`,
  private/public `cc_common`, generated proxy, configured C++ and execution.

The frozen defining-module heap owns the private provider/function/struct and
retains child heaps for imported values; no evaluator borrow or foreign owner
escapes. No production, DICE, request, cache, async, fixture, oracle, hot-path,
fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `5c3b4492f` the Rust test authority is 13,988 lines, SHA-256
`b20e5a907359a846a8b32617556b90810682a1d46a2963c01acebb9b2e388cd8`.
Its final ceiling is 15,038 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 1,050 proof and 1,050 total additions; deletions do not
buy budget. Embed/hash all 666 lines; evaluate at exact owner
`@@rules_cc+//cc/private/compile:cc_compilation_helper.bzl` with
`bazel_skylib -> bazel_skylib+` mapping and the five complete children. Prove
all nine imported bindings pointer-identical to their child exports with exact
visibility. Prove `_VIRTUAL_INCLUDES_DIR`, private `_ModuleMapInfo` identity,
all 12 functions, the exact public/private name sets, and the public
`cc_compilation_helper.init_cc_compilation_context` pointer identity. Invoke
nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/eager inventory, identity/visibility, no-invocation
boundary, compatibility split, branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, any invocation, lost imported/captured identity, evaluator-borrowed
value, consumer/parent claim, unpinned source, copied Zabel content, dirty
authority, allowlist escape or cap/function violation. Stop after this helper
and re-audit `compile.bzl` source order against the proxy/toolchain consumers.

## Immediate predecessor

Commit `5c3b4492f` admits only the accepted Starlark crate to archive checker;
`cb71a302d` accepts only the exact universal environment and bounded set subset.
Neither accepts this helper, additional callable ABI, helper invocation, parent
compilation modules, configured C++ or actions.
