# Current Slug V2 Packet

Packet: WP-4-7A-proto-common-predeclared-facade-implementation-r1

Milestone: M7A bootstrap-critical loading/ruleset closure. Admit the exact
Bazel 9.2 default declaration-time shape of the private
`proto_common_do_not_use` predeclared `.bzl` symbol without admitting native
proto methods, providers, configured semantics or actions.

Status: ready for bounded implementation after the docs-only audit returned
`ACCEPT`. Independent terminal review is required before acceptance.

## Immediate predecessor and replay boundary

Commit `f7c365234` terminally accepts bounded direct-external-Label
`repository_ctx.read` at 98 production and 265 proof gross Rust additions,
363 total. The focused read gate passes 5/5; `slug_loading_v2` passes 529 unit
tests with one ignored plus integration targets of 51/29/8/6/2/1/5/1 tests;
`slug_query_v2` passes 55/55; the CLI builds; formatting/diff and daemon
hygiene pass. Only the longstanding three thought-path archive failures remain.

The authenticated rules_rust replay clears the read, creates the selected
apple generated repository and reaches toolchain-registration row 12. Loading
`@@protobuf+//bazel/private/toolchains/prebuilt/BUILD.bazel` recursively enters
`:protoc_authenticity.bzl`, `//bazel/common:proto_common.bzl`,
`//bazel/common:proto_lang_toolchain_info.bzl` and finally
`//bazel/private:native.bzl:3`, where it stops with:

`Variable proto_common_do_not_use not found`

This is a generic predeclared-global loading boundary. The protobuf consumer is
the discriminator, never an activation branch.

## Pinned Bazel and protobuf evidence

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes the exact default contract:

- `BazelRuleClassProvider.java`, lines 253-264, registers the private Java
  `BazelProtoCommon` object as a `.bzl` top-level;
- `src/main/starlark/builtins_bzl/common/exports.bzl`, lines 17-26, replaces it
  with a Starlark `struct` having exactly one member,
  `INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION`, whose default value is
  `True` (`FlagConstants.java`, lines 48-50);
- `StarlarkBuiltinsFunction.java`, lines 271-300, installs exported top-levels
  in both BUILD-loaded and MODULE-loaded `.bzl` environments but not directly
  in BUILD files; the unprefixed export cannot be disabled by the ordinary
  injection override;
- `AutoloadSymbols.java`, lines 315-337, preserves the same one-member struct
  when the private symbol is selected for removal because protobuf still reads
  the flag; and
- `incompatible_autoload_externally_test.sh`, lines 223-239, proves that the
  retained reflection inventory contains the uppercase flag and not native
  `compile` breadth.

SHA-256 is
`a7de1ba5a700468ead269865f2563378ea0851d3430844ee6491591e52fd3d91`
for `BazelRuleClassProvider.java`,
`946df798515c772d8b562c5ebb1dcc13a61a5846bcb25addad2bc72e813b1097`
for `exports.bzl`,
`1a7e9423c7087e3528271f2a678b675961697729c7b191454c6521b386508589`
for `StarlarkBuiltinsFunction.java`,
`a24ab0e0c6cb6e306b1d54d4caf642bb0069df3753fc362d3f0f2ee7cbc31ae4`
for `AutoloadSymbols.java`,
`3bfe8a047697b704cdf81f8a46f1d9722b826a04ed6d7ea9d6bcfee2f151d130`
for `FlagConstants.java` and
`470abf02e4f8efbc23024dcd04ccd0f5c6fa5c993f93ae28475375d987201238`
for the shell test.

The selected module is protobuf 33.4. Its BCR `source.json` has SHA-256
`555f8686b4c7d6b5ba731fbea13bf656b4bfd9a7ff629c1d9d3f6e1d6155de79`
and archive integrity
`sha256-aH6YpHGXO1xf1xF1DEC4uCwK3jP2Sdtl4AspDyk0Wis=`. Exact release sources are
ordinary non-executable files with one trailing LF:

| Path | SHA-256 | Bytes/lines | Facade role |
|------|---------|-------------|-------------|
| `bazel/private/toolchains/prebuilt/BUILD.bazel` | `c221fadf2be3cfe23f2290c025a463ff68ba2add2a80f20bffd64e13bb8234b6` | 1,409/32 | loads and calls the authenticity rule |
| `bazel/private/toolchains/prebuilt/protoc_authenticity.bzl` | `0bb4726ccba19f521f584f88a92a016df5db6430901e283c06dedbed333325f7` | 3,228/69 | reads the derived public flag only in its rule implementation; declaration calls the toolchain helpers |
| `bazel/common/proto_common.bzl` | `e5813cf2ccdf81ded47ef001177ecb06b55a841e1aa5adf8f834cf39eae6924b` | 15,523/358 | loads the alias and derives fallback members |
| `bazel/common/proto_lang_toolchain_info.bzl` | `78de1d121cf7aab377acb7a4e1d70b5bab70c6aa57b46511f9a4015350d23ac6` | 1,571/26 | observes absent `ProtoLangToolchainInfo` and selects its Starlark provider fallback |
| `bazel/private/toolchain_helpers.bzl` | `2ad0b58e67563cebc843886e07dce69eceddd90ab162fbde5b8098df63de67dc` | 1,774/49 | reads the one Boolean at declaration time |
| `bazel/private/native.bzl` | `941d6b139f4eb695a24688d565ace2aa4cecd67e8f12dd5cee1e65dad7397db6` | 134/3 | aliases `proto_common_do_not_use` to `native_proto_common` |

This is the complete relevant consumer closure, not an asset-admission list.
All source bytes, recursive load routing, mapping, manifests, provider
declarations, `rule`, `config_common.toolchain_type`, `Label` and attribute
schemas are already owned. No protobuf source is copied by this packet.

## Audit decision and compatibility boundary

Audit result: `ACCEPT`. The smallest complete category is one immutable
predeclared `.bzl` value equivalent to:

`proto_common_do_not_use = struct(INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION = True)`

Implement it in both ordinary loading and Bzlmod `.bzl` globals and keep it
absent from direct BUILD-file globals. The struct must have exactly that one
member. Consequently `ProtoLangToolchainInfo` and
`INCOMPATIBLE_PASS_TOOLCHAIN_TYPE` remain absent, so protobuf's pinned
`getattr`/`hasattr` fallbacks take their Bazel 9.2 default paths.

Classify as **exact** the symbol name, top-level `.bzl` availability, value
type `struct`, single-member `dir`, Boolean `True`, missing-member behavior for
the two pinned fallback probes, exact `native.bzl` alias, and absence from the
direct BUILD environment under Bazel 9.2 defaults.

Classify as **Slug-native** the Rust/starlark-rust allocation and diagnostics,
and pinning the default Boolean in the binary's loading-global definition
rather than reproducing Bazel's Java option/builtins-injection machinery. The
fixed value has no mutable request or host input.

Keep **unsupported/deferred**:

- `BazelProtoCommon` methods, `external_proto_infos`, native `ProtoInfo` keys,
  `ProtoLangToolchainInfo` or `INCOMPATIBLE_PASS_TOOLCHAIN_TYPE` members;
- false/nondefault `--incompatible_enable_proto_toolchain_resolution`,
  `--incompatible_autoload_externally`, custom builtins paths, injection
  overrides, `native.legacy_globals` and arbitrary builtins replacement;
- proto configuration fragments, providers, rule implementations, toolchain
  selection, authenticity validation, actions, output groups and generated
  BUILD/configured semantics;
- any protobuf/rules_rust consumer branch, copied source, new catalog member,
  repository mapping/source change or anticipation of the next replay stop.

## Existing owner, lifecycle and implementation seam

`app/slug_loading_v2/src/package.rs::complete_loading_globals` is already the
sole constructor for ordinary, Bzlmod and direct BUILD loading environments.
Use starlark-rust's standard frozen `AllocStruct` there, inside the existing
`.bzl`-only branch. Do not add a custom Starlark type: its type/reflection would
not be the exact Bazel struct contract.

The value is immutable loading-environment data fixed by the Slug binary's
named Bazel 9 compatibility target. It adds no request projection, DICE key,
observed input, equality policy, retained side cache, lock, task or async
ownership. Each evaluation receives the same semantic constant through the
existing globals path; frozen modules and recursive manifests remain the only
retained results. Concurrent requests cannot vary this admitted default. A
future Bazel target or supported command override must re-audit and structurally
own the changed semantics rather than mutate this value.

No fallback or donor exists. The normal missing-global error remains for every
unadmitted symbol/member and direct BUILD access.

## Required proof

Add an adjacent focused test in `package.rs` that:

- evaluates the exact 134-byte/3-line protobuf `native.bzl` source under both
  ordinary and Bzlmod loading globals and proves the exported alias is a
  `struct` with exactly the uppercase member set to `True`;
- proves `dir`, `hasattr` and `getattr(default)` for the two intentionally
  absent native members, and proves the value is not callable and exposes no
  native proto methods;
- proves direct BUILD globals do not predeclare the top-level; and
- freezes the pinned source hash while adding no fixture.

The authenticated replay is the full recursive consumer/provenance proof. It
must clear only the missing global and the declaration-time chain, then stop at
the next independently owned boundary. The broad Bazel proto tests are skipped
because they exercise explicitly deferred configured providers, toolchains and
actions. No benchmark is required: this is one immutable field with no new key,
collection, graph edge or demonstrated hot path.

## Allowlist, caps and validation

Only `app/slug_loading_v2/src/package.rs`, including its adjacent `#[cfg(test)]`
module, may change. No other Rust, protobuf source, fixture, catalog, Cargo or
documentation file may change during implementation.

Gross additions are capped at 8 production Rust, 45 proof Rust and 53 total.
`package.rs` exceeds the 2,000-line complexity trigger but remains cohesive for
this packet because it already exclusively constructs all three loading-global
variants; a new module for one standard struct would split that ownership. The
existing constructor may grow by at most five lines and no new helper is
authorized.

Run serially:

- the focused `proto_common_do_not_use`/loading-globals unit test;
- `cargo test -p slug_loading_v2 --lib --quiet` and every loading integration
  target;
- `cargo test -p slug_query_v2 --lib --quiet`;
- `cargo build -p slug_cli_v2 --quiet`, followed by stale-`slugd` cleanup and
  one authenticated replay;
- `cargo fmt --check`, `git diff --check`, archive checker and exact
  allowlist/cap verification.

Return `REPLAN` if exact reflection requires a custom value, Java method or
dynamic flag owner; a second production file, DICE key, source/catalog change,
consumer branch or proto provider/analysis behavior is required; the symbol
must enter BUILD globals; the pinned consumer observes another native member;
the replay needs a false flag; speculative broader semantics are needed; or the
allowlist/caps fail.
