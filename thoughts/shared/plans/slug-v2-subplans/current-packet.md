# Current Slug V2 Work Packet

Packet: WP-7-111-m7a-apple-common-platform-struct-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the load-time `apple_common.platform` struct and its platform-value
structs in ordinary and Bzlmod-loaded `.bzl` files. Authenticated
rules_swift 3.1.2 `swift/internal/target_triples.bzl:20-30` eagerly reads
nine members into a dict while loading registration-row-56 toolchain Bzl.
The rebuilt authentic `slug build //:probe` must clear the current
`apple_common.platform` absence in that file; a later independently typed
stop is acceptable. Configured Apple/Xcode or Swift behavior, toolchain
selection, repository materialization and M7A close are not claimed.

Exact Bazel 9.2 slice: `apple_common.platform` is a struct containing ten
members, including `catalyst`; each member is a hashable ordinary struct
with `name`, `name_in_plist`, `platform_type` and `is_device` fields matching
the pinned builtins table. Their type, structural equality, hashability
and equivalent dict-key lookup (not numeric hash bytes), and
`dir`/`getattr` surface match the focused oracle. The `macos` and
`catalyst` values remain distinct despite sharing plist name `MacOSX`.
Direct BUILD files do not gain the `apple_common` global. Slug-native:
the deliberately restricted outer Apple facade, diagnostic wording and
the existing Rust Starlark namespace/value struct stringification
(`repr`, `str` and formatting derived from them), including field
order/spacing;
unsupported/deferred: its other absent members including
`apple_common.platform_type`, Apple configuration/Xcode operations,
toolchain methods, configured Swift and cross-evaluator pointer identity.
Platform-text-derived build output or semantic use also remains deferred.
No Java or Bazel runtime enters Slug.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`
owns the observed representation in
`src/main/starlark/builtins_bzl/common/objc/apple_platform.bzl:47-66`
and `apple_common.bzl:18-40`, exported by `bazel/exports.bzl:19,50`.
The parallel `ApplePlatform.java` enum is a migration duplicate, not the
Starlark value seen by the oracle. The authenticated cached rules_swift
`swift/internal/target_triples.bzl` SHA-256 is
`fd276e157ed241d5d704c659b53a5863d00a721f406465a1d539b8028a7edf43`.
Its later `.name_in_plist` use in
`swift/toolchains/xcode_swift_toolchain.bzl:127` is outside this
load-time acceptance.

A fresh offline Bazel 9.2 scratch oracle at
`/tmp/slug-wp811-apple-platform` ran with ignored RCs and disabled
repository downloads. Its positive `query --output=xml //:proof` exits 0:
all nine Swift-consumed values have `type=struct`, the `ios_device` repr is
`struct(is_device = True, name = "ios_device", name_in_plist = "iPhoneOS", platform_type = "ios")`,
ten namespace members include `catalyst`, reconstructed same-field structs
compare equal and retrieve the same dict key, while distinct members do
not compare equal. Separate invalid namespace and member queries exit 7
and list their respective allowed fields. This discriminates a string or
Java-enum-like placeholder from the actual source-defined struct. Scratch
remains outside this checkout and proves no configured Swift behavior.
Slug's existing `AllocStruct` prints that value as
`struct(name="ios_device", name_in_plist="iPhoneOS", platform_type="ios", is_device=True)`;
the generic formatter's insertion order and spacing are a recorded
Slug-native stringification difference, not an exact text claim. Changing
the shared Starlark formatter or inventing a one-off platform type is
outside this packet. A later consumer requiring exact platform text remains
deferred and must be separately evidenced.

`app/slug_loading_v2/src/apple_common.rs` is the sole production owner.
Extend its existing Bzl-only global with one `FrozenValue` for the platform
namespace allocated on the same `GlobalsBuilder` frozen heap. Build the ten
members with the existing Starlark `AllocStruct` representation and source
table, and return the same namespace value for repeated access within a
globals instance. This avoids a new value type, registry, semantic key,
cache, lock or manual hash/equality algorithm. The ordinary Bzl and Bzlmod
global routes share the existing installation; the direct BUILD route
remains unchanged. The fixed eleven-struct allocation is globals-instance
memory; a loaded Bzl may retain a value through its already owned frozen
module. Existing Bzl source manifests and DICE observations own source
identity, errors, invalidation and A/B/A restoration; no new semantic input
or retained projection is introduced. The current facade's provider tokens
and opaque `apple_toolchain()` behavior remain untouched.

The child is a `FrozenValue`, not a live `Value<'v>` requiring GC tracing.
Allocate the child on `GlobalsBuilder::frozen_heap` before installing the
`AppleCommon` wrapper on that same heap; `Globals::build` retains the heap
and module evaluation keeps it reachable after the temporary globals object
drops. The existing `TestingModule` and `CoverageCommonModule` in
`testing_bootstrap.rs:122-173` use the same frozen-child
`starlark_simple_value!` pattern. Do not store a live evaluator-heap value
in the wrapper. Test repeated `platform` access and use after freezing and
dropping the globals source; if that pattern cannot safely satisfy the
Starlark lifetime contract, choose a bounded traced/frozen complex carrier
within this owner rather than a cross-heap reference.

## Evidence and validation

Update the colocated `apple_common.rs` protected test that currently
requires `platform` to be absent. Add a source-shaped loading test that
constructs the nine-entry dict, checks `dir`/`getattr`, all ten four-field
values and exact type/Slug-native repr, structural equality and reconstructed-key
lookup, including the macOS/Catalyst discriminator. Freeze the module and
check the values survive. Keep negative tests for unknown namespace/member,
calling the namespace/value, direct BUILD global absence, and still-absent
outer members (`platform_type`, configured methods). Reuse the existing
provider-identity test; add a Stage 1 fixture only for a demonstrated
oracle-reproduction gap. Compile the Loading library tests with pinned
Cargo JSON `--no-run`, preflight exact nonignored selectors, run them
serially, rebuild `slug_cli_v2` before authentic smoke, and check/clean
`slugd` before and after. Run format, archive, diff and plan-status checks
and independent DESIGN/FINAL reviews. No broad suite or remote-service gate
is selected, and no `~/.bazelrc` contents, BuildBuddy credential,
NativeLink or remote transport enters this Loading packet.

## Scope and stops

Base clean main `4272bd781`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 Apple facade owner contract after acceptance; production
and focused tests in `app/slug_loading_v2/src/apple_common.rs` only.
Approximate production/proof growth of 60/100 lines is a review trigger,
not a permission ceiling. The existing globals and Apple facade stay
cohesive; do not split them for this fixed constant table. REPLAN if the
actual Bazel value is not a Starlark struct, `AllocStruct` cannot preserve
the admitted hash/equality surface, the cached namespace needs a new
retained owner, or clearing the authentic stop requires configured Apple
semantics. Do not paper over a new independent stop.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT after the structural
versus text-class correction. The Bazel 9.2 offline scratch oracle at
`/tmp/slug-wp811-apple-platform` distinguishes ordinary struct values,
four-field contents, structural equality/hashable key lookup and ten-member
attribute discovery from a Java-enum or string placeholder. The final
pinned-nightly Cargo JSON Loading library `--no-run` succeeds with zero
compiler errors (about 4.4 seconds); preflight verifies three exact
nonignored tests and all three pass serially in under one second. The
source-shaped test uses nine Swift table keys, checks all ten platform
values, same-instance namespace reuse, macOS/Catalyst distinction and
post-globals-drop frozen access. Protected facade/provider negatives pass.

The CLI rebuild succeeds before the normalized-PATH authentic one-shot
smoke. That smoke clears `apple_common.platform` at row 56 and stops at
`@@rules_swift+//swift/toolchains:xcode_swift_toolchain.bzl:970` on
`configuration_field(name = "xcode_config_label", fragment = "apple")`.
This is a separate Loading surface, not configured Apple or Swift parity.
Format, diff, archive, plan-status and pre/post-smoke `slugd` hygiene checks
pass. Packet research/design, implementation and review took roughly
twenty minutes elapsed. No remote transport or credentials were used.

## Immediate predecessor

WP810 accepted at `4272bd781`: extension-invoked loaded `.bzl` functions
now resolve `Label()` against their own manifest-authenticated source,
including imported helpers. Six focused/protected tests and independent
reviews pass; the rebuilt authentic smoke clears the row-56 `Label()` error
and reaches this separate `apple_common.platform` absence. M7A remains
partial.
