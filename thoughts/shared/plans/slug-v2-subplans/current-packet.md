# Current Slug V2 Packet

Packet: WP-4-6-7A-apple-common-declaration-provider-fail-closed-implementation-r1

Milestone: M7A bootstrap-critical loading/ruleset closure. Implement the bounded
`apple_common` declaration facade whose provider keys remain valid in loading
schemas but cannot silently participate in configured-target lookup before
configured Apple providers exist.

Status: design checkpoint `94738a9a2` is accepted. Implementation is active
only under the frozen allowlist, caps, proof, validation and stops below.

## Accepted predecessor, replay boundary and rejected design

Commit `96fe2d6cb` accepts generic selected-BCR regular mode `0444` at 2
production and 12 proof gross Rust additions, 14 total. Authenticated
bounded-PATH replay clears rules_java 9.1.0 materialization, then stops while
recursively loading selected rules_cc 0.2.4 at
`@@rules_cc+//cc/private/rules_impl:objc_common.bzl:22` because
`apple_common` is not predeclared.

The first `WP-4-7A` audit correctly found the three declaration operations but
is rejected. Reusing an ordinary `BuiltinProviderKey` would let
`starlark_provider_identity` erase declaration-only provenance into a normal
`ProviderIdentity`. `AnalysisConfiguredTargetValue::is_in` would then return
`false`, and `at` would report only that the target does not provide the key.
Both outcomes falsely imply configured Apple provider lookup is implemented.

## Exact selected-source closure

The durable BCR descriptor
`https://bcr.bazel.build/modules/rules_cc/0.2.4/source.json`, SHA-256
`2bd87ef9b41d4753eadf65175745737135cba0e70b479bdc204ef0c67404d0c4`,
selects the 276,390-byte release archive at
`https://github.com/bazelbuild/rules_cc/releases/download/0.2.4/rules_cc-0.2.4.tar.gz`,
SHA-256
`8dcd63392f0bb48adf74f413a9f39ba0fedcb8f99bf085a3b450f06d171dbb6d`,
matching integrity
`sha256-jc1jOS8LtIrfdPQTqfOboP7cuPmb8IWjtFDwbRcdu20=`. The descriptor uses
`strip_prefix = "rules_cc-0.2.4"`, its authenticated MODULE-version patch and
`patch_strip = 1`.

A complete scan of the 400-entry release finds five code references across
three ordinary `0644`, trailing-LF sources:

| Source-relative path | SHA-256 | Bytes/lines | Observation |
|---|---|---:|---|
| `cc/private/rules_impl/objc_common.bzl` | `bb508b0e6d973b5953fcdc90df0ac0570de45bb6b07e5d35c7f16e2b3218994e` | 9,107/242 | top-level `Objc` lookup and zero-argument `apple_toolchain()` |
| `cc/private/rules_impl/objc_compilation_support.bzl` | `1f078126197ea03e8201a2e6d4187c042b8da27eb5bca1c81d79f786d360356d` | 37,216/1,016 | top-level `XcodeVersionConfig` lookup; configured `get_apple_config` use |
| `cc/private/toolchain/unix_cc_toolchain_config.bzl` | `6094987775711ee7016ce79781da6a87f6cf07c37296632ddf3e7239736d9fcc` | 70,236/1,981 | configured `XcodeVersionConfig` use only |

The complete declaration category is therefore `Objc`,
`XcodeVersionConfig`, and zero-argument `apple_toolchain()`. Every provider
value, returned-toolchain member, configuration, environment, rule and action
use lies below a configured function boundary.

## Bazel 9.2 and live Slug boundary evidence

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes:

- `src/main/starlark/builtins_bzl/bazel/exports.bzl`, SHA-256
  `7404fc0e7cb8f6c5c4a0bd82bf3e0e87512a594256624f6360f06f80934439e2`,
  exports the `apple_common` top-level;
- `src/main/starlark/builtins_bzl/common/objc/apple_common.bzl`, SHA-256
  `dcbf8f2cbb1c87e711c44800737b2611a42116fdb9f0acbe25b35af668a75c86`,
  aliases `ObjcInfo` and `XcodeVersionInfo` and exposes the toolchain factory;
- `src/main/java/com/google/devtools/build/lib/analysis/configuredtargets/AbstractConfiguredTarget.java`,
  SHA-256
  `6ec77df09263c0e18d3443dd5911180156c43df4a5df2c2fc42aab01307fbbfc`,
  selects an exported provider for both indexing and membership, returning a
  value/error for indexing and a Boolean for membership; and
- `src/test/java/com/google/devtools/build/lib/rules/objc/ObjcStarlarkTest.java`,
  SHA-256
  `2ed4e579e72fcb3161bd7949ef50acdb2eb4b49382d855da54e844123e495103`,
  proves exact `ObjcInfo` absent/present membership and absent indexing once
  configured ObjC providers exist.

Slug's `provider.rs::BuiltinProviderKey` hashes and compares by its static
name. `starlark_provider_identity` converts it immediately to the shared
`ProviderIdentity::Builtin(CompactString)`. `package.rs` intentionally uses
that conversion for rule/aspect advertised providers and attribute/subrule
provider constraints, normalizing with existing `SmallSet` and immutable
`Arc` slices. `slug_analysis_v2::analysis_value` uses the same conversion in
`AnalysisConfiguredTargetValue::at` and `is_in`, then consults its existing
`SmallMap<ProviderIdentity, FrozenValue>`.
An exhaustive live call-site search finds these are the only configured-target
operations that convert a raw Starlark provider key; all other uses are loading
schema conversion or tests.

The retained build-API `ProviderIdentity`, provider collections and configured
target values must not change in this implementation.

## Corrected design and compatibility classification

Accepted design: add a provider-owned sibling Starlark token backed by
a closed `#[repr(u8)]` two-variant kind: `ObjcInfo` and `XcodeVersionInfo`.
The token is immutable, `Allocative`, structurally hashes/compares by variant,
with a token-domain hash discriminator, and is not equal to an ordinary
`BuiltinProviderKey`. It contains no dynamic name, collection, pointer graph
or interner.

The ordinary `starlark_provider_identity` recognizes the sibling and lowers it
to the existing `ProviderIdentity::builtin(kind.name())`. This preserves its
use in loading-time `provides` and provider-constraint schemas, including
existing order, deduplication, freeze/import and final package identity. It
does not add a second retained schema representation.

Add one configured-target-specific provider-key conversion in `provider.rs`.
It detects the sibling before ordinary identity lowering and returns this
stable Slug-native diagnostic for either variant:

`apple_common.<field> is declaration-only; configured-target membership and indexing are unsupported`

Here `<field>` is replaced only by exact `Objc` or `XcodeVersionConfig`.

`AnalysisConfiguredTargetValue::at` and `is_in` must use that conversion.
Both operations error even if a configured target contains a coincidentally
named `ProviderIdentity`; there is no false `false`, no ordinary missing-
provider error and no lookup. Every existing provider token preserves its
current success, false and error behavior. Attribute/subrule constraint
matching continues over actual retained `ProviderIdentity` values; a missing
Apple provider is an ordinary constraint mismatch, not configured Starlark
membership/indexing.

Classify as **exact** for the selected rules_cc consumer the three declaration
operations, acceptance of both aliases as provider keys in loading schemas,
consistent alias/constraint identity through freeze and import, and omission
of `apple_common` from direct BUILD globals for this selected exposure boundary.

Classify as **Slug-native** the restricted three-member facade, static Slug
`.bzl` availability rather than Bazel's repository/flag guard, closed sibling
token plus its collision-safe `ProviderIdentity::Builtin` loading projection,
canonical constraint retention, opaque Apple-toolchain token, and explicit
configured-target rejection while the configured provider category is absent.

Keep **unsupported/deferred** Bazel's other seven `apple_common` members; full
struct reflection; provider construction, values and fields; configured
`ObjcInfo` and `XcodeVersionInfo`; all returned-toolchain members; Apple/Xcode
fragments, environments, selection, rules, actions and outputs; repository/
flag/autoload exposure parity beyond the selected boundary.

## Owners, lifetime and utility review

- New private `apple_common.rs` owns the facade, exact three-name reflection,
  zero-argument factory and opaque toolchain token.
- `provider.rs` solely owns the closed declaration-only key kind, Starlark
  equality/hash/display, loading identity lowering and configured-use guard.
- `package.rs::complete_loading_globals` remains the sole environment owner;
  it registers the facade only in the existing `.bzl` branch. `lib.rs` only
  declares the private module.
- `slug_analysis_v2::analysis_value` remains the configured target Starlark
  view owner and changes only its two key-entry operations.

The two key values are immutable globals retained by the loading environment;
the factory result is evaluation scratch until normal module freeze. Existing
`ProviderIdentity` values in frozen schemas and configured graphs retain their
current compact representation, equality and invalidation. No request input,
DICE key, observation, cache, lock, fallback, mutable state or asynchronous
owner is added. Failure/cancellation publishes no module or configured result;
overlapping requests share only immutable globals and frozen values.

Buck2/V1 review selects existing V2/Starlark `Allocative`, closed enums,
`SmallSet`, `SmallMap` and immutable `Arc` storage as retained dependencies,
not copied code. Add no map, set, string allocation, interner or cache for the
two static token variants. A compile-time size assertion must keep the kind one
byte. No benchmark is required for two static globals and two constant-time
type checks. `package.rs` is over 11,000 lines, so all substantive behavior
stays in the dedicated/provider/analysis owners. `provider.rs` and
`analysis_value.rs` remain below 2,000 lines under the caps.

No fixture is added. The pinned sources and authenticated replay are the
provenance evidence; the configured Bazel ObjC test is not ported because its
Apple provider/rule semantics remain deferred.

The sibling is an explicit temporary boundary, not a configured-provider
fallback. Its invariant is that declaration-only tokens never reach configured
membership/index lookup. A future separately reviewed Stage 6 configured-Apple
provider packet may delete it only after admitting complete `ObjcInfo` and
`XcodeVersionInfo` construction, retained values, materialization and target
lookup. That packet must replace the rejection proofs with Bazel's absent/
present membership and indexing cases while preserving loading-schema identity.

## Required discriminating proof

Implementation must prove:

- both sibling variants have distinct structural equality/hash, exact display
  names, one-byte kind size and `Allocative`; neither equals an ordinary
  built-in key;
- ordinary and Bzlmod `.bzl` evaluation supports exactly the three selected
  operations, freezes/re-exports both keys, and direct BUILD globals omit the
  facade;
- rule/aspect `provides` and attribute/subrule provider constraints accept the
  keys and retain ordinary `ObjcInfo`/`XcodeVersionInfo` identities with
  existing ordering/deduplication;
- for each key, both `key in target` and `target[key]` produce the exact
  declaration-only diagnostic, including when the target map contains the
  same normal identity;
- normal built-in and user providers preserve present/absent membership,
  indexing, invalid-key diagnostics and hash/equality behavior;
- every deferred facade name and every nested toolchain access fails closed;
  failed evaluation freezes/publishes no module; and
- authenticated replay clears all three declarations and stops at the next
  independent boundary before configured Apple behavior executes.

## Implementation allowlist, caps and validation

Only these files may change:

- `app/slug_loading_v2/src/apple_common.rs` (new, with adjacent tests);
- `app/slug_loading_v2/src/provider.rs` (sibling token/conversions and tests);
- `app/slug_loading_v2/src/lib.rs` (one private module declaration);
- `app/slug_loading_v2/src/package.rs` (import/registration only); and
- `app/slug_analysis_v2/src/analysis_value.rs` (two configured key operations
  and adjacent tests).

Caps: 180 production Rust, 180 proof Rust and 360 aggregate gross additions.
`package.rs` is capped at four production lines; `provider.rs` at 80 aggregate
lines; `analysis_value.rs` at 75 aggregate lines. No build-API, DICE, fixture,
asset, Cargo or documentation file may change during implementation.

Run serially:

- focused `slug_loading_v2` apple/provider tests;
- focused `slug_analysis_v2` declaration-only target-key tests;
- `cargo test -p slug_loading_v2 --lib --quiet` plus its integration targets;
- `cargo test -p slug_analysis_v2 --lib --quiet`;
- `cargo test -p slug_query_v2 --lib --quiet`;
- `cargo build -p slug_cli_v2 --quiet`, stale-`slugd` cleanup and one
  authenticated bounded-PATH replay; and
- `cargo fmt --check`, `git diff --check`, archive hygiene and exact
  allowlist/cap checks.

Return `REPLAN` if configured-target provenance cannot be rejected before
ordinary identity lowering; provider constraints require retaining a second
tagged identity; `ProviderIdentity` or `ProviderCollection` must change; an
Apple provider value/member executes during declaration; another facade member
is required; a map/interner/cache/key/fixture is proposed; either large file
crosses 2,000 lines; or the allowlist/caps fail.
