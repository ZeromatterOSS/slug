# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-cc-launcher-info-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 31-line rules_cc
`cc/private/cc_launcher_info.bzl` producer loads the accepted-complete helper,
declares its initialized provider/raw pair, and freezes its lazy constructor.
Add no production behavior and invoke neither callable.

## Learned facts and decision

Base commit is `07077e23d` (`Prove complete private CcInfo freeze`). It adds 892
proof lines and no production, embeds/hash-checks all 656 parent lines, rebuilds
the four exact frozen children, proves every imported identity, all six provider
identities/visibility, all three empty-context shapes, initialized raw type and
every lazy binding type. Focused proof, 243 library tests, 24 invalidation tests,
31 BUILD-loading tests, locked analysis/core checks, CLI build, formatting and
hygiene pass. Independent review accepts caps and compatibility boundaries.

Re-audit of the generated proxy's two incomplete roots finds private
`cc_common.bzl` first in proxy source order. Its helper, private CcInfo and
`cc_internal` children are complete. The next child is rules_cc 0.2.17
`cc/private/cc_launcher_info.bzl`: 31 lines, SHA-256
`41da54762e854191c0217575d385b37cd9729380d7c78d3efbc19049177250dd`.
It loads only complete `cc/common/cc_helper_internal.bzl` (383 lines,
`793ab429...`), defines one lazy constructor, and eagerly declares initialized
`CcLauncherInfo` plus private raw `_`. Existing loading support and accepted
initializer tests cover every evaluator shape. Toolchain-config instead reaches
the 1,387-line legacy-features module plus 220-line action names and 622-line
config library, so it is not the minimum source-ordered successor.

Therefore run only
`WP-4-7A-rules-cc-private-cc-launcher-info-complete-loading-proof`. Do not claim
private/public `cc_common`, the generated proxy, toolchain config, launcher
instances, or configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Bazel's
`StarlarkRuleClassFunctions.provider`,
`StarlarkProvider.ArgumentProcessorWithInit`/`RawArgumentProcessor`, and focused
`declaredProvidersWithInit`/raw-bypass/failure tests supply the already accepted
evaluator contract; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only defining-module
ownership, retained loaded identity and recursive freeze before reexport. Copy
no Zig code, representation or behavior.

- **Exact:** complete source/hash and sole load spelling/canonical owner;
  imported helper pointer identity; initialized-provider source/export identity;
  public provider/private raw/private constructor visibility and frozen types.
- **Slug-native:** composition through the accepted recursive helper builder and
  Slug's frozen heaps.
- **Unsupported/deferred:** constructor, raw, provider or wrapper invocation;
  launcher instances; complete private/public `cc_common`, generated proxy,
  toolchain config, configured C++ semantics or actions.

The helper and launcher frozen heaps own every retained callable; no evaluator
borrow escapes. No production, DICE, request, cache, async, fixture, oracle,
hot-path, fallback or utility-reuse decision is introduced. Lazy constructor
behavior is skipped because invocation is an unsupported later phase.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `07077e23d` the Rust authority is 11,651 lines, SHA-256
`061cfcb5daf5b7b42db794faa1f6f6354dd3ffccaccc6379a7d027845d77cfa3`.
Its final ceiling is 11,771 lines. The new proof function must remain at most
120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 120 proof and 120 total additions; deletions do not buy
budget. Embed/hash all 31 lines; reuse the accepted complete recursive helper
closure and actual rules_cc owner; evaluate at exact owner
`@@rules_cc+//cc/private:cc_launcher_info.bzl`; prove the imported
`wrap_with_check_private_api` pointer, exact `CcLauncherInfo` provider identity,
private raw `_` type/visibility, and private lazy constructor type/visibility.
Invoke no binding and add no fixture or fresh oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceiling and obtain independent review of
bytes, recursive identity, eager/lazy boundary, compatibility split, and
Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, any callable invocation, copied/narrowed source, lost identity,
evaluator-borrowed value, parent/proxy claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
launcher info and re-audit private `cc_common` source order against the
toolchain-config branch.

## Immediate predecessor

Commit `07077e23d` completes private CcInfo. It does not complete private
`cc_common`, the generated compatibility proxy, or any public C++ route.
