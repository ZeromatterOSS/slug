# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-compilation-outputs-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 226-line rules_cc
`cc/private/compile/cc_compilation_outputs.bzl` producer loads its three
accepted-complete children, evaluates its provider/sentinel/empty-output rows,
and freezes every lazy binding. Add no production behavior and manually invoke
no binding.

## Learned facts and decision

Base commit is `974b9e981` (`Prove complete LTO context freeze`). It adds 207
proof lines and no production, embeds/hash-checks all 97 LTO-context lines,
rebuilds the helper/internal closure, proves both imported identities, two
provider identities, three lazy types and the exact empty context. Focused
proof, 246 library tests, 24 invalidation tests, 31 BUILD-loading tests, locked
analysis/core checks, CLI build, formatting and hygiene pass. Independent
review accepts caps and compatibility boundaries.

Private `cc_common.bzl` source order now reaches rules_cc 0.2.17
`cc/private/compile/cc_compilation_outputs.bzl`: 226 lines, SHA-256
`294e3da16da4444122e7dee058ec1e06b30cec93d64a32f217cf9e1e3e4bfb44`.
Its helper, `cc_internal`, and LTO-context children are complete. Eager source
rows declare a private unbound-sentinel provider/instance and public
`CcCompilationOutputsInfo`, then source-invoke
`create_compilation_outputs_internal()` once to construct exact empty outputs.
That call uses only accepted empty list freeze, empty depset, imported wrapper,
and empty LTO shapes. Existing source-shaped loading proof covers this exact
eager sequence. Every later function body is lazy; no unsupported eager
expression remains. Toolchain config is still the broader later proxy branch.

Therefore run only
`WP-4-7A-rules-cc-private-compilation-outputs-complete-loading-proof`. Do not
claim manual/lazy function behavior, compile actions, private/public `cc_common`,
generated proxy, toolchain config, or configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing accepted
provider, empty-depset/list-freeze, wrapper and recursive-load regressions cover
the evaluator shapes; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only defining-module
ownership, retained child/captured-function identity and recursive freeze before
reexport. Copy no Zig code, representation or behavior.

- **Exact:** complete source/hash and three-load source order/canonical owners;
  all imported pointer identities; sentinel/output provider identities,
  visibility and types; exact source-owned empty-output declaration sequence,
  provider identity, empty list/None/LTO shapes and wrapped-function type; all
  lazy binding types/visibility.
- **Slug-native:** composition through accepted child builders and Slug frozen
  heaps, including the helper-defined closure retained by the parent output.
- **Unsupported/deferred:** manual invocation beyond the exact source-owned
  empty construction; create/merge/validation semantics; complete compile/action
  modules, private/public `cc_common`, generated proxy, toolchain config,
  configured C++ semantics or actions.

Frozen child/parent heaps own every callable, sentinel, provider and empty
output; the captured wrapper result retains its defining helper and no evaluator
borrow escapes. No production, DICE, request, cache, async, fixture, oracle,
hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `974b9e981` the Rust authority is 12,026 lines, SHA-256
`7bc061a5d18c8fc11dccc602d46a328d8c521ad8af4b39841d9e9b7349dbaf99`.
Its final ceiling is 12,476 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 450 proof and 450 total additions; deletions do not buy
budget. Embed/hash all 226 lines; reuse accepted complete helper/internal/LTO
closures; evaluate at exact owner
`@@rules_cc+//cc/private/compile:cc_compilation_outputs.bzl`; prove all five
imported pointer identities, exact distinct sentinel/output provider identities,
private sentinel provider/instance visibility, all public/private lazy function
types, and exact `EMPTY_COMPILATION_OUTPUTS` provider ID. Prove its ten empty
list fields, two None fields, helper-owned `temps` function type, and pointer-
identical empty LTO context. Manually invoke no binding; add no fixture or oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, recursive/captured identities, eager/lazy boundary, compatibility split,
and Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, manual invocation, copied/narrowed source, lost child/captured identity,
evaluator-borrowed value, parent/proxy claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
compilation outputs and re-audit private `cc_common` source order against the
toolchain-config branch.

## Immediate predecessor

Commit `974b9e981` completes the LTO compilation context. It does not complete
compilation outputs, any compile action, private `cc_common`, or the generated
compatibility proxy.
