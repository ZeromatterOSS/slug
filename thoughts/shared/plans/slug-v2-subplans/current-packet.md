# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-cc-internal-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 17-line rules_cc
`cc/private/cc_internal.bzl` producer loads and freezes through the already
accepted narrow private bridge. Add no production behavior.

## Learned facts and decision

The predecessor audit starts from commit `eb4110f25`. Pinned rules_cc 0.2.17
provides these remaining generated-proxy roots:

- `cc/private/cc_common.bzl`: 788 lines, SHA-256 `5e6ab737...`;
- `cc/private/cc_info.bzl`: 656 lines, SHA-256 `4424bb87...`;
- `cc/private/toolchain_config/cc_toolchain_config_info.bzl`: 143 lines,
  SHA-256 `8c522773...`.

All three recursively load `cc/private/cc_internal.bzl`: 17 lines, SHA-256
`8241ced58c265334ac3f0e063d492383f1ff7d223736dc2d6a5aa712165de6bb`.
It has no loads and one eager expression at line 17:
`cc_common.internal_DO_NOT_USE() if hasattr(...) else struct()`. Commit
`4d7a9bbb2` accepts the selected `rules_cc+` bridge behavior through a
source-shaped test, but no accepted proof evaluates and freezes the complete
authenticated producer.

The audit classifies exact Skylib `lib/paths.bzl` as accepted complete. The
following are accepted only in source-shaped slices: `cc_helper_internal`,
private CcInfo, launcher/shared-library-hint info, compilation outputs, LTO
context, extra-link-time-library, linker-input/linkstamp/toolchain providers,
and `native_cc_common`. Compile, link, configure-features, legacy-features and
their action-name/toolchain graphs remain broad/deferred. In particular,
toolchain config reaches 1,387-line `legacy_features.bzl`, then 220-line
`action_names.bzl` and 622-line `cc_toolchain_config_lib.bzl`; none is smaller
than the shared 17-line prerequisite.

The complete `cc_common.bzl` direct-load inventory, in source order, is:

| Child | Lines / SHA-256 prefix | Audit class |
|---|---:|---|
| `cc_helper_internal` | 383 / `793ab429` | partial; first reaches `cc_internal` |
| `cc_info` | 656 / `4424bb87` | partial/broad |
| `cc_internal` | 17 / `8241ced5` | missing complete proof; selected |
| launcher / shared-library-hint info | 31 / `41da5476`; 56 / `7d067aad` | partial |
| compilation outputs / compile / compile variables | 226 / `294e3da1`; 2,295 / `bec506ff`; 644 / `463ea66c` | partial; broad; broad |
| linkstamp compile / LTO context | 111 / `6f5ceb39`; 97 / `a17435cd` | broad; partial |
| extra library / library-to-link / linker-input | 192 / `522312ac`; 291 / `5f574233`; 69 / `e4e8a7fc` | partial; broad; partial |
| linking-context-from-outputs / linkstamp / link | 137 / `664a4615`; 44 / `8d5fc394`; 197 / `666e819d` | broad; partial; broad |
| link variables / LTO backends | 392 / `bdf03036`; 540 / `078bfb68` | broad |
| toolchain info / native wrapper | 255 / `f1958957`; 18 / `d8e5feda` | partial |
| toolchain-config info / configure-features | 143 / `8c522773`; 232 / `d950aa9a` | partial/broad; broad |

`cc_helper_internal` first loads complete Skylib paths, then `cc_internal`,
then dependency-free lazy private paths (39 lines, `c982ac68...`). Its later
list extensions, structs, initialized instances and comprehension shapes are
admitted but do not constitute a complete 383-line producer proof. Private
CcInfo also reaches the same incomplete helper/internal producers before its
eager provider/context instances. Lazy function bodies are not invoked by any
of these freezes.

Therefore run only
`WP-4-7A-rules-cc-cc-internal-complete-loading-proof`. Do not widen a partial
child into a complete-producer or full-proxy claim.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Clean `../zabel`
commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides the architectural
decision that the defining module owns the loaded binding and that recursive
freeze must close over the selected producer before reexport. Copy no Zig code,
representation, traversal order, identity, diagnostic or behavior.

- **Exact:** all 17 source lines and SHA-256; canonical `rules_cc+` defining
  owner; `hasattr` selected branch; frozen exported binding and type.
- **Slug-native:** the already accepted narrow public/private bridge and opaque
  token, pending full Bazel builtins injection.
- **Unsupported/deferred:** the fallback `struct()` branch as a parity claim;
  every internal method; complete helper/private CcInfo/toolchain/`cc_common`
  producers; the generated proxy; provider invocation, analysis and actions.

The frozen module heap naturally owns the exported token and no new retained,
request, async or DICE state is introduced. This proof has no fixture or oracle
generation, fallback, memory-growth, hot-path or utility-reuse decision.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

The sole Rust authority is 9,854 lines at base `eb4110f25`, SHA-256
`23a42c43f52b4663549a2f900811ae47fdde862cfbed7bf7798f2524578b059f`.
Its final ceiling is 9,934 lines. The new test function must remain at most 80
physical lines; a file-scope exact-source constant is exempt from that function
ceiling but counts against the packet cap.

Caps are 0 production, 80 proof and 80 total additions; deletions do not buy
budget. Embed all 17 authenticated lines, assert their SHA-256, evaluate them at
exact owner `@@rules_cc+//cc/private:cc_internal.bzl`, freeze the module, and
prove exported `cc_internal` has the accepted opaque type. Also prove the
selected branch exists and remains `.bzl`-only. Invoke no internal member and
add no fixture or fresh Bazel oracle.

Run focused proof, all `slug_loading_v2` library tests, invalidation and BUILD
loading integration, locked core/analysis checks, formatting, hygiene and
`cargo build -p slug_cli_v2`. Measure final additions and obtain independent
review of exact bytes/hash, branch ownership, Zabel's guidance-only role and
compatibility boundaries.

STOP and `REPLAN` for any production change, source/hash mismatch, need to
implement the fallback branch or an internal method, evaluator-owned retained
value, full-child/proxy claim, unpinned source, copied Zabel content, dirty
authority, allowlist escape or cap violation. Stop after this complete producer
and re-audit `cc_helper_internal`; do not continue into private paths or a later
root in the same packet.

## Immediate predecessor

Commit `4b2396f0a` completes exact ObjcInfo proxy aliases after `498e5efc7`
accepts zero-argument `depset()`. Three of six generated-proxy child families
are exact complete; the other three remain deferred behind this audited shared
prerequisite.
