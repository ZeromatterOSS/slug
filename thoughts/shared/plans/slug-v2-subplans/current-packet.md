# Current Slug V2 Packet

Packet: WP-4-7A-bazel-cc-common-private-bridge-loading
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: complete `.bzl` globals, defining-call module provenance, and one
stateless private C++ loading token
Base: 919ecfa5

Result: admit only the first rules_cc private bridge after the completed
bazel_skylib child. Expose `.bzl` `cc_common.internal_DO_NOT_USE()` with
mandatory rules_cc owner checking, return a frozen opaque token, and stop
before any internal C++ method or later rules_cc expression.

## Accepted starting point and source frontier

Commit 919ecfa5 completes selected bazel_skylib@1.8.2
`rules/common_settings.bzl`. All String build-setting descriptor identities
load, but only the existing true/single configured slice may record. Source
order returns to rules_rust 0.73.0 `rust/private/toolchain.bzl`.

That file's second load is `@rules_cc//cc/common:cc_common.bzl`. In Bazel 9.2,
rules_cc 0.2.17 routes that load through its generated compatibility proxy
`@cc_compatibility_proxy//:symbols.bzl`, then
`@rules_cc//cc/private:cc_common.bzl`. The latter first loads
`cc/common/cc_helper_internal.bzl`; its first Skylib `paths` child consists of
lazy functions, scalar constants and an already-admitted keyword-only struct.
The next child is `cc/private/cc_internal.bzl`, whose only evaluated expression
is:

```starlark
cc_internal = cc_common.internal_DO_NOT_USE() if hasattr(cc_common, "internal_DO_NOT_USE") else struct()
```

Slug's complete `.bzl` globals have no `cc_common`, so name resolution is the
first absent surface. Do not jump to the remainder of cc_helper, private
cc_common, the generated proxy, or rules_rust's rule body.

## Source provenance

Reuse the selected repository graph and materialized-source owners:

- rules_rust 0.73.0 source JSON SHA-256:
  `8eeb3d9ba7c57916b63887a651e8f84c2f68b7243af9e712d728c2a0b7882255`;
- `rust/private/toolchain.bzl` SHA-256:
  `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
- rules_cc 0.2.17 source JSON SHA-256:
  `3832f45d145354049137c0090df04629d9c2b5493dc5c2bf46f1834040133a07`;
- rules_cc 0.2.17 archive SHA-256:
  `283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a`;
- `cc/extensions.bzl` SHA-256:
  `a190a467ac48329a76e1a9ccab1fea53519af4bb2202e22346b23fc24dcf9872`;
- Bazel-9 generated compatibility `symbols.bzl` SHA-256:
  `2adedeeaaad8c0e664dc35e9bf1480b1d6dc3d7840034f9efe3ee78476fc5902`;
- `cc/common/cc_common.bzl` SHA-256:
  `65e91cf0fa7ebb1c8efc84bbf6b1c4ec4db46f5e5ed4606759aa4a45a23b4063`;
- `cc/private/cc_common.bzl` SHA-256:
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`;
- `cc/common/cc_helper_internal.bzl` SHA-256:
  `793ab429f8e397df9c486f4c3c7b5c57fae81c8432ba6d08189d65d75676dae1`;
- bazel_skylib `lib/paths.bzl` SHA-256:
  `96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`;
- `cc/private/cc_internal.bzl` SHA-256:
  `8241ced58c265334ac3f0e063d492383f1ff7d223736dc2d6a5aa712165de6bb`.

Add no source route, generated repository, mapping, observer, fixture archive,
network oracle or materializer.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`bazel/exports.bzl` exports `cc_common` as a `.bzl` toplevel.
`cc_common_bazel.bzl` constructs that public wrapper from private
`_builtins.internal.cc_common` and `_builtins.internal.cc_internal` values.
Its zero-argument `internal_DO_NOT_USE` function calls
`cc_internal.check_private_api` with rules_cc in the allowlist, then returns
the private value. `CcStarlarkInternal.checkPrivateApi` selects the innermost
calling Starlark module. `BuiltinRestriction` accepts canonical repositories
whose name begins `<allowed module>+`; a foreign label fails as
`file '<canonical label>' cannot use private API`.

`BazelStarlarkEnvironment` injects exported toplevels into BUILD- and
MODULE-loaded `.bzl` environments. BUILD-file injection accepts exported
rules, not exported `.bzl` toplevels, so BUILD must not gain `cc_common`.

Pinned Zabel commit `c7298478e2e56262a2f438e9c065325744c9f0fc` remains
architectural guidance only. `builtins_cc_primitives.zig` deliberately does
not install a public `cc_common` from its private leaf. It exposes private
native state through a bootstrap capability, requires an owner callback for
private-API checks, and lets the builtins layer form the public wrapper. Slug
follows the public/private phase split and mandatory fail-closed owner rule.
Because full Bazel builtins injection is not admitted, Slug's complete `.bzl`
globals owner supplies a narrow public projection over one opaque private
token. No Zig code, layout, vtable, method set, analysis object, diagnostic or
algorithm may be copied.

## Compatibility classification

- **Exact:** `cc_common` placement in `.bzl` and absence from BUILD; presence
  and zero-argument binding of `internal_DO_NOT_USE`; success for the selected
  canonical `rules_cc+` owner; rejection for a foreign owner with the pinned
  private-API diagnostic; no implementation or analysis execution while this
  child loads.
- **Slug-native:** a Rust zero-sized public projection and zero-sized opaque
  internal token instead of executing Bazel's bundled builtins; Rust freeze,
  display and type identity; the selected canonical-owner predicate; current
  complete-globals construction and invalidation.
- **Unsupported/deferred:** `_builtins`, bundled-builtins loading/injection and
  overrides; main-repository rules_cc override matching; every public
  `cc_common` method except this bridge; every `cc_internal` field or method;
  C++ providers, feature configuration, toolchains, actions and analysis;
  remaining rules_cc/rules_rust source; M8/M7B and exact output bytes.

## Natural owner, lifetime and utility reuse

Add one small `cc_common` loading module. It owns the public Starlark wrapper,
the private opaque token and the private-call check together. The complete
`.bzl` globals owner installs only the public wrapper; BUILD uses its existing
sibling globals and remains unchanged. The call check uses the existing
`BzlEvaluationContext::source_identity_for_call`, so imported-function
provenance and recursive manifest ownership stay in their current owner.

Both values are zero-sized, stateless, `Allocative`, no-serialize simple
values. The private token freezes by value and carries no evaluator pointer,
source identity or semantic method surface. Add no Arc, collection, interner,
hash, registry, cache, DICE key or source input. The Buck2-utility review
selects no import and no Stage 9 ledger update.

No request overlay, source observation, async transfer or command-result
change applies. Existing source/module fingerprints own invalidation. There is
no fallback and no silent owner permit.

## Implementation boundary

1. Add a private loading module containing `CcCommonModule` and opaque
   `CcInternalModule` simple values.
2. Expose exactly one public method, `internal_DO_NOT_USE()`, with no positional
   or named arguments beyond its receiver.
3. Resolve the innermost defining-call module through the existing evaluation
   context. Admit only the selected canonical repository-name prefix
   `rules_cc+`; otherwise emit the pinned `file '<label>' cannot use private
   API` diagnostic.
4. Return the opaque token without exposing attributes or methods. It may be
   bound, exported and frozen only.
5. Install the public wrapper only in `loading_globals()`, beside the other
   `.bzl`-only globals. Do not place it in BUILD globals.
6. Add no generic builtins/private-API framework and no C++ semantic method.

## Discriminating proof

- Evaluate and freeze `cc_common.internal_DO_NOT_USE()` with a canonical
  `@@rules_cc+//cc/private:cc_internal.bzl` context; require the opaque token
  type and absence of any internal method.
- Prove the method is zero-argument: positional, named and unknown forms
  reject through the typed Starlark ABI.
- Evaluate the same call from root and foreign external identities; require
  the exact canonical-label private-API diagnostic.
- Prove BUILD has no `cc_common`, while ordinary `.bzl` globals do.
- Keep Config, Label, aspect, provider and rule loading proof green. Add no
  repository fixture, Bazel run or network request.

## Allowlist and caps

Only these files may change from base 919ecfa5:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| app/slug_loading_v2/src/cc_common.rs | absent | 0 | 100 | public/private bridge and owner check |
| app/slug_loading_v2/src/lib.rs | 90d28c337121ee004302b3f38b104fd1c3a8c07ba6ab3d7574a256e30620c849 | 120 | 123 | private module declaration |
| app/slug_loading_v2/src/package.rs | 42fc1411b2a66a5879443e258441e5845a54eaf47a7309a686265ce904525127 | 5,832 | 5,840 | `.bzl`-only complete-globals installation |
| app/slug_loading_v2/src/host_package_load_tests.rs | d8b1a21bf348557d4a01011d2675dbacbfb3517d8c77a75f6d68b6cc79b26783 | 5,340 | 5,450 | placement, binding, owner, freeze proof |

Additions are capped at 110 production, 110 proof and 220 total. Deletions do
not buy addition budget. No new or touched function may exceed 150 lines.
package.rs exceeds 2,000 lines, but the only allowed edit is installation in
its existing complete-globals owner. The new semantic owner remains below 100
lines. Do not edit rule invocation or configured code.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- `cargo test -p slug_loading_v2 --lib cc_common`
- `cargo test -p slug_loading_v2 --lib bazel_config_typed_descriptors`
- `cargo test -p slug_loading_v2 --lib`
- `cargo test -p slug_loading_v2 --test build_file_loading`
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`
- `cargo build -p slug_cli_v2`
- `cargo fmt --check`
- `git diff --check`
- `scripts/v2_archive_status.sh`

Clean stale `slugd` before and after the broad loading integration. It may
retain only the recorded stale `@external` diagnostic-order failure. Archive
hygiene may report only the known three thoughts paths plus active files.
Recheck hashes, caps, allowlist and function sizes.

The private owner boundary and new globals projection require independent
selection and terminal implementation reviews. Verify source order, exact
placement and diagnostic, defining-call provenance, mandatory owner failure,
opaque-token freeze, Zabel's guidance-only role, lack of C++ semantics, utility
reuse and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; exposing `cc_common` in
BUILD; a public method besides `internal_DO_NOT_USE`; any attribute or method
on the returned token; a silent/optional owner check; a generic allowlist or
builtins framework; bundled builtins loading; main-repository override
matching; C++ provider/toolchain/feature/action/analysis behavior; source,
mapping, observation, DICE, cache, I/O or async changes; Java/JVM work; Zabel
code or behavior adoption; unpinned source; a fixture/oracle/network request;
cap violation; or a broad rules_cc/rules_rust success claim. After this child
freezes, stop and audit the next recursive rules_cc expression separately.
