# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-paths-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 39-line rules_cc
`cc/private/paths.bzl` producer loads and freezes its lazy exported function.
Add no production behavior and invoke no path helper.

## Learned facts and decision

Base commit is `888a305a3` (`Prove complete cc_internal producer freeze`). It
embeds and hash-verifies exact complete `cc/private/cc_internal.bzl`, evaluates
it at the canonical `rules_cc+` defining owner, freezes the selected opaque
bridge token, and rechecks BUILD absence. Growth is 0 production and 43 proof;
focused proof, 239 library tests, 24 invalidation tests, 31 BUILD-loading tests,
locked analysis/core checks, CLI build, formatting and hygiene pass. Independent
review accepts exact bytes, scope, caps and compatibility boundaries.

The remaining `cc_common.bzl` recursive front is
`cc/common/cc_helper_internal.bzl` (383 lines, SHA-256 `793ab429...`). In source
order it loads:

1. accepted exact complete Skylib `lib/paths.bzl` (320, `96cce438...`);
2. now accepted exact complete rules_cc `cc_internal.bzl` (17, `8241ced5...`);
3. rules_cc `cc/private/paths.bzl` (39,
   `c982ac685f0bfbd32602d82d1c37f3bf50a2714ca6a13bfd3c08d4e5cc8b8872`).

The third child has no loads, eager calls, defaults, constants or
comprehensions. Lines 16-39 declare the lazy exported `is_path_absolute`
function; its body is not executed during module evaluation or freeze. Slug's
accepted evaluator can parse and freeze this shape, but no accepted proof owns
the authenticated complete producer. It is the next source-order incomplete
closure unit and is smaller than the 383-line parent or any later root.

Therefore run only
`WP-4-7A-rules-cc-private-paths-complete-loading-proof`. A complete helper or
proxy claim remains premature even though its next syntax is already admitted.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
the architecture: the defining module owns the exported function and recursive
freeze must close over that producer before the parent imports it. Copy no Zig
code, representation, function behavior, traversal, identity or diagnostic.

- **Exact:** all 39 source lines and SHA-256; canonical `rules_cc+` defining
  owner; public function binding, type, visibility and frozen lifetime.
- **Slug-native:** only the proof composition through Slug's existing frozen
  module heap.
- **Unsupported/deferred:** invoking `is_path_absolute`; complete
  `cc_helper_internal`, private CcInfo, toolchain config, `cc_common` or proxy;
  provider/helper invocation, configured C++ behavior, actions and analysis.

The frozen module heap naturally owns the function and its source environment.
No production, retained request, DICE, cache, async, fixture, oracle, hot-path,
memory-growth, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `888a305a3` the Rust authority is 9,897 lines, SHA-256
`e5c05e839146f4387224352fac6c5dcdfd4dfe965119ce2d6028fa4c039c4be0`.
Its final ceiling is 9,977 lines. The new test function must remain at most 80
physical lines; a file-scope exact-source constant is exempt from that function
ceiling but counts against the packet cap.

Caps are 0 production, 80 proof and 80 total additions; deletions do not buy
budget. Embed all 39 authenticated lines, assert line count and SHA-256,
evaluate at exact owner `@@rules_cc+//cc/private:paths.bzl`, freeze the module,
and prove public `is_path_absolute` is a function while absent private names
remain absent. Invoke no helper and add no fixture or Bazel oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure final additions and obtain independent review
of exact bytes/hash, noninvocation, ownership, Zabel's guidance-only role and
compatibility boundaries.

STOP and `REPLAN` for production change, source/hash mismatch, helper invocation,
captured evaluator lifetime, complete-parent/proxy claim, unpinned source,
copied Zabel content, dirty authority, allowlist escape or cap violation. Stop
after this producer and re-audit `cc_helper_internal`'s own eager expressions.

## Immediate predecessor

Commit `888a305a3` completes the smallest shared producer found by the remaining
compatibility-proxy closure audit. It does not complete any of the three
remaining generated-proxy child families.
