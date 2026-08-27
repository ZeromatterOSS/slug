# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-configuration-field-loading-binding-r2`

Milestone: M7A command/ruleset bootstrap closure.

Result: install Bazel 9's `.bzl`-only `configuration_field` predeclared binding
with its exact two required positional-or-named string parameters, preserve lazy
reference/freeze, and fail every invocation closed before constructing a
late-bound value. Do not retain or resolve a configuration field.

## Learned facts and decision

Base commit is `ed4144030` (`Select complete C++ semantics proof`); the Rust
authority remains action-names commit `9e312f958`. That accepted proof adds 328
test lines and no production, byte-verifies all 220 source lines, and proves 33
constants, the 33-field action struct, seven ordered lists, and seven final
aliases. All 248 loading-library, 24 invalidation and 31 BUILD-loading tests,
locked checks, CLI build and hygiene pass. Independent review returned `ACCEPT`.

The attempted exact 234-line `cc/common/semantics.bzl` proof stopped before
invocation: Starlark name resolution rejects `configuration_field` at line 80
inside lazy `_get_coverage_attrs`. The worker changed no production and the
candidate proof was fully removed. The first binding implementation was also
removed after independent review found its `named-only` Rust ABI rejected
Bazel-valid positional calls. Only the r2 scheduling documents and required
routing row are now dirty. A test-local substitute, narrowed source or
unclassified dummy would violate the exact-complete packet.

Pinned Bazel 9.2 exposes `configuration_field(fragment, name)` only in the
`.bzl` top-level API. Both required string parameters have `named = true` and
retain `positional = true` from Bazel's `@Param` default, so two-positional,
two-named and positional-then-named forms are valid. Its implementation requires
Bzl initialization context, validates the registered fragment/field, and creates
a late-bound default. Slug has no retained late-bound attribute value or
configured resolver in this M7A slice. The bounded prerequisite is therefore the
exact `.bzl` binding/type/ABI and lazy name-resolution surface, with one stable
Slug-native fail-closed error for every otherwise valid invocation. BUILD
absence remains exact. After acceptance, retry complete semantics without
invoking any function.

Therefore run only `WP-4-7A-bazel-configuration-field-loading-binding-r2`. Do not
construct a descriptor, admit `attr.label(default = configuration_field(...))`,
validate fragment names/fields, resolve configuration, retry semantics, or claim
configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
`StarlarkBuildApiGlobals` and `BazelBuildApiGlobals` are the exact authority for
global placement and ABI. Pinned `Param.java` supplies the positional-default
fact; existing source-backed Starlark ABI/error tests cover dual positional and
named binding. No oracle is needed for the explicitly unsupported valid call.
Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only installing
`configuration_field` as a `.bzl` predeclared binding while keeping its
declaration-owned late-bound descriptor/resolver separate. Copy no Zig code,
representation, algorithm, diagnostic or behavior.

- **Exact:** `configuration_field` exists as a callable only in `.bzl` loading
  globals; `fragment` and `name` are required positional-or-named strings; all
  lawful positional/named/mixed forms bind; lazy functions may reference the
  binding and freeze; BUILD globals do not expose it.
- **Slug-native:** an otherwise ABI-valid call returns one stable fail-closed
  unsupported diagnostic before allocating or retaining a descriptor.
- **Unsupported/deferred:** every successful invocation; fragment/field
  validation, late-bound descriptor identity, label-attribute defaults,
  configuration resolution/invalidation, complete semantics/compile,
  private/public `cc_common`, toolchain config and configured C++.

The static predeclared function owns no retained semantic value and creates none
on error. No DICE, request, cache, async, hot-path, fallback or utility-reuse
decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` for this
  material REPLAN row only.

At base `ed4144030`, `package.rs` is 6,219 lines/SHA-256
`8818b416e74ab838a65fdc60f148374c51e6ef122bc94addf2cfa9f7ae80e4fe` and the
test authority is 12,804 lines/SHA-256
`72c9d73c961bcbcaac256dc7b9daaafb05797ae418b1c6097bd5162199e0bba9`.
Final ceilings are 6,239 and 12,884 lines. Each new function must remain at most
120 physical lines.

Caps are 20 production, 80 proof and 100 total additions; deletions do not buy
budget. Add one `.bzl`-only predeclared Rust function with required
positional-or-named `fragment: str` and `name: str`; on a valid call, return a
stable unsupported error without allocating a result. Prove callable type and
BUILD absence; identical valid-call failure for two-positional, positional-plus-
named, two-named and reverse-named forms; exact omitted/duplicate/excess/wrong-
type ABI rejection; lazy captured reference freeze; and no new public retained
type. Add no fixture or oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
ABI, placement, no-result failure, compatibility split, the failed-source
evidence and Zabel's guidance-only role.

STOP and `REPLAN` for any retained descriptor/schema change, source-specific
test substitute, BUILD exposure, successful invocation, fragment registry,
configured resolution, DICE/cache change, non-stable failure, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
the binding and reselect the complete semantics proof.

## Immediate predecessor

Commit `9e312f958` completes action names. Scheduling commit `ed4144030` selected
semantics, but exact source revealed this missing global. Commit `82b58818b`
selected its first binding packet, whose rejected named-only candidate was fully
removed after review exposed Bazel-valid positional calls. Neither semantics nor
any `configuration_field` invocation is accepted.
