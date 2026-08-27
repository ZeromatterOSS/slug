# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-create-linker-input-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 69-line rules_cc
`cc/private/link/create_linker_input.bzl` producer over its one accepted child.
Prove its complete imported/provider/function surface without invocation or
callable-default inspection.

## Learned facts and decision

Base implementation commit is `ace75573b` (`Prove complete C++ create library
freeze`). It byte-verifies all 291 create-library lines over five actual
complete children and proves exact child mappings, six imported identities,
the warning, provider, four functions and exact seven-public/twelve-all
inventories without invocation. Focused, all 260 loading-library, 24/31
integration, locked analysis/core, CLI, format/diff/source and archive-baseline
gates pass within 0/463/463; independent correction review returned `ACCEPT`.

The private `cc_common.bzl` source-order audit next reaches rules_cc 0.2.17
`cc/private/link/create_linker_input.bzl`, 69 lines, SHA-256
`e4e8a7fc9d7be8edd40a2b95e72a96710c05d5bbd610b2c1cc2f274e3672cbd1`.
Its sole child is the accepted complete `cc_internal.bzl` producer with empty
repository mapping.

The module eagerly retains only private `_cc_internal`, private
`_LinkerInputInfo` and public lazy `create_linker_input`. Its three empty
`depset()` and two list defaults are owned inside the frozen function object;
this packet neither exposes nor inspects that callable ABI.

Run only `WP-4-7A-rules-cc-create-linker-input-complete-loading-proof`. Do not
invoke the function/provider, inspect function parameters/default values or
create a linker input.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Accepted complete
child, provider and lazy-function regressions cover every exposed eager shape;
no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and separation from invocation values inform proof
architecture only. Copy no Zig code, representation, algorithm, diagnostic or
behavior.

- **Exact:** complete source/hash/owner/mapping; child label/mapping; imported
  pointer identity/visibility; provider source/export identity; public function
  type/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining the imported child heap, private provider and lazy function.
- **Unsupported/deferred:** every function/provider invocation; callable ABI,
  parameters and default values; linker-input construction; native C++ methods;
  private/public `cc_common`, proxy, configured C++, actions and execution.

The frozen producer retains its child heap and owns its provider/function. No
evaluator borrow or invocation value escapes. No production, DICE, request,
cache, async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `ace75573b` the Rust test authority is 20,245 lines, SHA-256
`d1f1a638ff2a46acbdf5b0e0395207bc315d27eb47a83fd382eefdae1e8300a4`.
Its final ceiling is 20,545 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 300 proof and 300 total additions; deletions do not buy
budget. Embed/hash all 69 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:create_linker_input.bzl`, path
`/rules_cc/cc/private/link/create_linker_input.bzl`, with empty mapping and the
accepted complete child at source load key `//cc/private:cc_internal.bzl`.
Reuse its actual defining identity and assert exact child label/mapping.

Prove private `_cc_internal` pointer-identical to child `cc_internal`. Prove
private `_LinkerInputInfo` has exact provider source/export identity. Prove
public `create_linker_input` is type `function`. Assert exact one-public/
three-all name sets. Invoke nothing, inspect no function default and add no
fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, child/import/provider/function inventory, defining identities,
no-invocation/no-callable-inspection boundary, compatibility split, branch
selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation or callable-default inspection, lost imported/provider
identity, evaluator-borrowed value, consumer claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape or cap/function violation.
Stop after this producer and re-audit private `cc_common` at
`create_linking_context_from_compilation_outputs.bzl`.

## Immediate predecessor

Commit `ace75573b` accepts only complete create-library defining-module
freezing. `ccab93d4c` accepts only LTO backends and `cb71a302d` accepts only the
universal environment and bounded set subset. None accepts linker-input
callable behavior, configured C++ or actions.
