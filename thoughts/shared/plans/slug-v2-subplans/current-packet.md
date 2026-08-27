# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-helper-internal-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 383-line rules_cc
`cc/common/cc_helper_internal.bzl` producer loads its three accepted-complete
children, evaluates its eager rows, and freezes all bindings without invoking a
lazy helper. Add no production behavior.

## Learned facts and decision

Base commit is `c4e49424c` (`Prove complete rules_cc private paths freeze`). It
adds 66 proof lines and no production, embeds/hashes all 39 private-paths lines,
evaluates the exact `rules_cc+` owner, and freezes public lazy
`is_path_absolute` without invocation. Focused proof, 240 library tests, 24
invalidation tests, 31 BUILD-loading tests, locked analysis/core checks, CLI
build, formatting and hygiene pass. Independent review accepts exact bytes,
caps, ownership and compatibility boundaries.

The next recursive producer is rules_cc 0.2.17
`cc/common/cc_helper_internal.bzl`: 383 lines, SHA-256
`793ab429f8e397df9c486f4c3c7b5c57fae81c8432ba6d08189d65d75676dae1`.
Its three loads are now accepted complete in exact source order:

1. Skylib `lib/paths.bzl` (320 lines, `96cce438...`);
2. rules_cc private `cc_internal.bzl` (17, `8241ced5...`);
3. rules_cc private `paths.bzl` (39, `c982ac68...`).

After loading, lines 42-214 eagerly create string/list constants, concatenate
and extend two lists in source order, build `extensions`, declare the
initialized `_ArtifactCategoryInfo` provider/raw pair, construct 22 fixed
instances, and freeze two more structs including a dictionary comprehension.
Existing accepted proofs cover every evaluator shape and the exact initialized
provider/instance semantics, but only in source-shaped slices. Lines 216-383
are lazy function declarations and must freeze without body invocation. No
further load or unsupported eager expression remains.

Therefore run only
`WP-4-7A-rules-cc-helper-internal-complete-loading-proof`. Do not equate the
complete helper producer with private CcInfo, `cc_common`, configured C++ or the
generated proxy.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides the
architecture that each defining child owns its loaded value, the parent retains
those identities, and recursive freeze closes over the complete parent before
reexport. Copy no Zig code, representation, traversal, identity or behavior.

- **Exact:** complete helper bytes/hash, load spellings/order and canonical
  owners; pointer-identical loaded values; eager list/struct/provider-instance
  results; frozen exported/private binding types and visibility.
- **Slug-native:** only proof composition in Slug's existing frozen module heap.
- **Unsupported/deferred:** invoking any lazy helper/internal callable or making
  any manual/post-freeze provider call beyond the exact 22 source-owned eager
  provider/initializer calls; complete private CcInfo, toolchain config,
  `cc_common` or generated proxy; configured C++ semantics, actions and analysis.

The four frozen module heaps naturally own all functions, providers, instances,
lists and structs; no value borrows evaluator scratch. No production, DICE,
request, cache, async, fixture, oracle, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `c4e49424c` the Rust authority is 9,963 lines, SHA-256
`2500077497b02f4d6d27da59ebcf862ab58c9874cd771a0d122f5930edcffeb8`.
Its final ceiling is 10,443 lines. The new test function must remain at most
100 physical lines; a file-scope exact-source constant is exempt from that
function ceiling but counts against the packet cap. The oversized test module
remains cohesive because it owns the private evaluator/load harness and the
adjacent authenticated rules-source constants; this packet adds no production
responsibility or general-purpose source archive.

Caps are 0 production, 480 proof and 480 total additions; deletions do not buy
budget. Embed/hash all 383 lines and evaluate them at exact owner
`@@rules_cc+//cc/common:cc_helper_internal.bzl` with the three exact frozen
children and the actual Skylib repository mapping. Prove loaded binding pointer
identity, initialized-provider/raw/instance types, fixed instance count, public
struct/list/function types and private visibility. Permit only the exact 22
source-owned eager provider/initializer calls; invoke no lazy helper, internal
member or callable manually, and add no fixture or fresh Bazel oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure additions and function/file ceilings. Obtain
independent review of bytes/hash, child identities, eager/lazy boundary,
ownership, Zabel's guidance-only role and compatibility classes.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, lazy/manual/internal invocation beyond the 22 source-owned eager calls,
copied/narrowed source, lost child identity, evaluator-borrowed frozen value,
parent/proxy claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap violation. Stop after the complete helper and re-audit
private CcInfo plus `cc_common` source order.

## Immediate predecessor

Commit `c4e49424c` completes the last loaded child of helper-internal. It does
not itself complete the 383-line parent or any remaining generated-proxy root.
