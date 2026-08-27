# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compile-action-templates-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 266-line rules_cc
`cc/private/compile/compile_action_templates.bzl` producer over its six accepted
children. Prove all imported identities and lazy functions without invocation.

## Learned facts and decision

Base implementation commit is `97faa6e71` (`Prove complete C++ compile variables
freeze`). It byte-verifies the complete 18-line native wrapper and 644-line
compile-variable producer. The proof uses the same globals instance to retain
the exact `cc_common` alias, asserts all three child identities/mappings and four
imported pointers, and proves the 25-field `_VARS` mapping, private
provider/sentinel identity, ordered 22-item source-type set, six public plus seven
private functions, and exact public/all-visibility names. It invokes nothing.
Focused, all 254 loading-library, 24/31 integration, locked analysis/core, CLI,
format/diff and archive gates pass within 0/875/875; independent review returned
`ACCEPT`.

The required source-order audit returns to rules_cc 0.2.17
`cc/private/compile/compile.bzl`. Its first remaining incomplete direct child is
266-line `compile_action_templates.bzl`, SHA-256
`10a43c512a85458f45a0223a7ddc7c1b56f8072872b765b1744d336ff91ec794`.
All six loaded children now have accepted complete proofs: Skylib paths,
`cc_helper_internal.bzl`, semantics, `cc_internal.bzl`, compilation helper and
compile variables. Private `cc_common.bzl` reaches the same module recursively
through `compile.bzl`, so compile, generated-proxy and toolchain consumers share
this frontier.

The eager surface retains nine public imported aliases—`paths`,
`CPP_SOURCE_TYPE_HEADER`, `CPP_SOURCE_TYPE_SOURCE`, `artifact_category`,
`cc_semantics`, `dotd_files_enabled`,
`serialized_diagnostics_file_enabled`, `get_copts` and
`get_specific_compile_build_variables`—plus private `_cc_internal`. It defines
one public and four private lazy functions. There is no top-level constructor,
container, provider invocation or native method call.

Run only
`WP-4-7A-rules-cc-compile-action-templates-complete-loading-proof`. Do not
invoke a function, inspect callable ABI/results, implement a native C++ method,
or claim `compile.bzl`, private/public `cc_common`, proxy, configured C++,
actions or execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Accepted
complete-child and lazy-function regressions cover every eager shape; no fresh
oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership of imported functions/values and recursive publication
informs the proof architecture only. Copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all six child labels/mappings;
  all ten imported pointer identities and visibility; one public and four
  private function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining imported child heaps and lazy functions.
- **Unsupported/deferred:** every function invocation and callable ABI/result;
  native C++ methods; parent `compile.bzl`; private/public `cc_common`, proxy,
  configured C++, actions and execution.

The frozen producer retains child heaps for imported values and owns its
functions. No evaluator borrow or invocation value escapes. No production, DICE,
request, cache, async, fixture, oracle, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `97faa6e71` the Rust test authority is 15,726 lines, SHA-256
`a1b77b812687f806e6b418edf45fb1857064ecc543eb1fd080b9653de084e728`.
Its final ceiling is 16,326 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 600 proof and 600 total additions; deletions do not buy
budget. Embed/hash all 266 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/compile:compile_action_templates.bzl` with
`bazel_skylib -> bazel_skylib+` mapping and the six accepted complete children
in source order. Reuse their actual defining identities and assert every child
label/mapping.

Prove all ten imports pointer-identical to child exports with exact visibility.
Prove `create_compile_action_templates` public and
`_create_compile_action_template`, `_declare_compile_output_tree_artifact`,
`_maybe_declare_dotd_tree_artifact` and
`_maybe_declare_diagnostics_tree_artifact` private, all of type `function`.
Assert exact public and all-visibility name sets. Invoke nothing and add no
fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/function inventory, defining identities, no-invocation
boundary, compatibility split, branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported identity, evaluator-borrowed value,
parent/consumer claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after this producer and
re-audit complete `compile.bzl` against proxy/toolchain consumers.

## Immediate predecessor

Commit `97faa6e71` accepts only native/compile-variable producer freezing.
`3060e4d4d` accepts only the compilation helper, and `cb71a302d` accepts only
the exact universal environment and bounded set subset. None accepts this
producer, callable behavior, parent compilation modules, configured C++ or
actions.
