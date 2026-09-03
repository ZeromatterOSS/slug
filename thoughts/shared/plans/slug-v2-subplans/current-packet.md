# Current Slug V2 Packet

Packet: WP-5-7A-bazel-tools-xcode-configure-catalog-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Add
the exact pinned Bazel 9.2 `tools/osx/xcode_configure.bzl` source as the sole
new built-in `@bazel_tools` catalog member and advance the authentic replay to
its next independently owned boundary.

Status: ready for one bounded implementation. The predecessor, docs-only
audit, exact bytes, existing owner, three-file allowlist, caps, proof and stops
are accepted and frozen with independent reviewer agreement.

## Accepted predecessor and replay evidence

`WP-4-5-7A-builtin-external-bzl-load-routing-implementation-r1` is accepted in
commit `5d5991634` at 80 production and 360 proof gross Rust additions, 440
total. It preserves the original external-module DICE key and current-node
cycle identity while importing the already-owned canonical built-in route only
as invocation scratch after source decode, parse and the exact one-load gate.

The complete `slug_loading_v2` gate passes 526 unit tests plus every
integration target. `slug_query_v2` passes its 55-test focused gate and retains
the established 55/56 diagnostic baseline; `slug_cli_v2` builds. The
authenticated replay clears `lib_cc_configure.bzl`, maps `rules_cc` to
`rules_cc+`, freezes its recursive utility and re-export, then stops while
loading:

`@@bazel_tools//tools/osx:xcode_configure.bzl`

for generated repository
`apple_support++apple_cc_configure_extension+local_config_apple_cc_toolchains`.
This is an exact built-in source catalog miss, not another mapping or routing
failure.

## Accepted catalog audit

`WP-5-7A-bazel-tools-xcode-configure-catalog-audit-r1` returns `ACCEPT`.
Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
provides
`/tmp/bazel-9.2-source-audit-fhUrtf/tools/osx/xcode_configure.bzl` with:

- SHA-256
  `26d758318e481f8971dabd43e24d0b4e85c30eb074da39d3b63c778f39ebd942`;
- exactly 12,993 bytes and 329 lines with one trailing LF; and
- source/archive mode `0644`, represented as catalog `executable: false`.

The installed Bazel 9.2 copy at
`/tmp/slug-bazel92-install-audit/install/3e6f3b7d6fdac67aed908160850e082b/embedded_tools/tools/osx/xcode_configure.bzl`
is byte-identical but mode `0755`. That is the same systematic extraction
artifact already recorded for other built-in sources and is not source or
catalog authority.

The complete smallest source closure is this one file. It has zero syntactic
Starlark loads. Text beginning with `load("@apple_support...")` occurs only
inside strings used to generate a later BUILD file, and `xcode_locator.m` is
referenced only from deferred runtime code. Its public bindings are
`OSX_EXECUTE_TIMEOUT`, `VERSION_CONFIG_STUB`, `run_xcode_locator`,
`xcode_autoconf`, `xcode_configure` and `xcode_configure_extension`.

Existing Bzlmod loading globals already own its declaration-time needs:
`repository_rule` with `configure`, `environ` and `attr.string` schemas, and
`module_extension`. Existing module-extension execution owns repository-call
recording and `module_ctx.extension_metadata(reproducible=True)`. This packet
does not claim or add the Host/Darwin operations appearing in deferred
function bodies.

`tools/osx/BUILD` is not part of this source-only closure. It would load
`xcode_version_flag.bzl`; the authenticated loader has already crossed label
and package routing and failed directly on the exact source lookup. Likewise
`xcode_locator.m`, `xcode_locator_stub.sh` and every other `tools/osx` asset are
not admitted. Bazel's `src/test/shell/bazel/apple/bazel_apple_test.sh` exercises
Darwin repository runtime and is deliberately skipped as an unsupported phase;
the pinned-source regression and live catalog miss discriminate this packet.

## Compatibility classification

- Exact: the pinned source bytes, trailing LF, SHA-256, source/archive
  non-executable mode, path, public bindings, zero syntactic-load closure,
  direct catalog lookup and sorted directory membership match Bazel 9.2.
- Slug-native: the existing immutable DICE source key/value and
  domain-separated manifest digest represent source and route identity.
  Adding the file invalidates users through the complete manifest without a
  Bazel install-tree identity claim.
- Unsupported/deferred: installed extraction mode; `tools/osx/BUILD`, locator,
  stub, `xcode_version_flag.bzl` and all sibling assets; repository-rule body
  execution introduced by this file; Darwin Xcode discovery and subprocesses;
  Host OS/environment/path/file behavior not already admitted; generated BUILD
  and apple_support loads; consumer-specific Apple/C++ toolchain semantics;
  broader built-in catalog growth; and the next replay failure.

## Required implementation and ownership

Add the exact upstream file verbatim at
`app/slug_bzlmod_v2/builtin/bazel_tools/tools/osx/xcode_configure.bzl`.
Do not normalize bytes, rewrite Starlark, copy from an installed tree or change
its executable mode.

In `builtin_repository.rs`, add one lexically ordered static `CATALOG` entry
after `tools/launcher/empty.sh` and before `tools/res/BUILD`, with the exact
path, hash and `executable: false`. Extend the existing sorted direct-listing
proof so `tools` contains directory `osx` and `tools/osx` contains only
`xcode_configure.bzl`.

In the public catalog test, add the matching lexically ordered `FILES` row.
Update the two catalog-owned complete-manifest expectations from
`c313fad68f4e475d744dc6de7b658515b33c634905222e934a9d09129371f56f`
to
`3927ae2a3d8a6ec40f9dac0ef9f3833424ae4cbd6c56dcc9ab1d7d8ecee8abfc`.
Also update the downstream test-only host-route/capability expectation in
`host_module.rs` from its already-stale `de4c723127e85a58d4fc5331e16135cdc1448afc0edb3792a1515ee2266f198f`
to that same new digest. Do not change production host-module code, the
manifest domain or its version.

`BuiltinBazelToolsSnapshot`, the static `CATALOG`, `validated_file`,
`BuiltinBazelToolsSourceFileKey` and the derived directory-listing key remain
the sole owners. No request/session input, filesystem observation,
materialization, mutable cache or fallback participates. Concurrent and warm
DICE reads retain existing deduplication, equality and permanent validity.

The exact 12,993 bytes are static program/catalog memory and are exposed
through the existing `Arc<[u8]>` value; no new retained type or duplicate
runtime copy is authorized. `builtin_repository.rs` remains below the 2,000
line complexity trigger, and the change is one cohesive catalog row plus its
existing proofs. No hot-path measurement, Buck2 utility review, donor code,
new oracle fixture or fallback ledger applies.

## Implementation allowlist and caps

Only these files may change:

- `app/slug_bzlmod_v2/builtin/bazel_tools/tools/osx/xcode_configure.bzl`
- `app/slug_bzlmod_v2/src/builtin_repository.rs`
- `app/slug_bzlmod_v2/src/host_module.rs` (test-only digest expectation)
- `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`

Gross Rust additions are capped at 12 production, 40 proof and 52 Rust total.
The asset must be exactly 12,993 bytes/329 lines. Aggregate gross additions,
including that exact asset, are capped at 400. Formatting or cleanup does not
create headroom. Expected accounting is 6 production, 16 proof and 22 total
Rust additions plus the 329-line asset, or 351 aggregate additions.

## Required proof and validation

- Recompute and compare the source and checked-in SHA-256; prove exactly one
  trailing LF, 12,993 bytes, 329 lines and non-executable source mode. Compare
  pinned source and checked-in bytes directly; installed bytes are
  corroboration only.
- Prove source lookup returns the exact path/hash/bytes/mode and that unknown
  neighboring paths remain `UnsupportedCatalog`.
- Prove sorted direct listings add only `osx` under `tools` and only
  `xcode_configure.bzl` under `tools/osx`.
- Prove the public `FILES` manifest remains exactly the physical asset set and
  all three catalog and downstream host-route/capability expectations equal
  `3927ae2a3d8a6ec40f9dac0ef9f3833424ae4cbd6c56dcc9ab1d7d8ecee8abfc`.
- Run rustfmt and diff checks, serial focused built-in catalog unit/integration
  tests, the named downstream `host_module` route test, the complete
  600-test `slug_bzlmod_v2` suite and a direct
  `slug_loading_v2` consumer check. Rebuild `slug_cli_v2` before replay.
- Clean `slugd` before and after the authenticated replay. The replay must
  clear this `UnsupportedCatalog`, load and freeze the exact source through
  the existing route, and record the next typed boundary without implementing
  it. Run archive and artifact hygiene gates.

## Terminal stops

Return `ACCEPT` only if exact bytes/mode/hash, one-file closure, listings,
physical manifest, all three digest proofs, focused/full tests, direct
dependent, build and replay pass within all four files and caps.

Return `REPLAN` before adding `tools/osx/BUILD`, `xcode_locator.m`,
`xcode_locator_stub.sh`, `xcode_version_flag.bzl` or any other asset; changing
an evaluator, repository API, route, key, manifest version or materialization
owner; adding install-tree/filesystem fallback; implementing Host/Darwin,
generated BUILD, apple_support or toolchain behavior; or crossing an
independent next replay boundary. Any production change in `host_module.rs`
also requires `REPLAN`.
