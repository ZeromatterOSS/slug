# Current Slug V2 Work Packet

Packet: WP-4-6-7A-provider-declaration-identity-closure-audit-r1

Status: docs-only audit/design checkpoint complete; implementation is frozen
but awaits independent review. No Rust work is authorized before `ACCEPT`.

## Predecessor checkpoint and selected stop

Commit `2945accbe` terminally accepts the execution-group declaration closure
within its 170/220/390 caps. It preserves nonallocating ordinary-rule state,
the compact retained group/transition representation, exact declaration
validation and the configured fail-closed boundary. Its focused and broad
loading/query/CLI, formatting, diff, archive and daemon-hygiene gates pass.

The authenticated bounded-PATH rules_rust replay clears the selected rules_cc
execution-group declarations and reaches selected rules_java 9.1.0
`java/common/rules/java_runtime.bzl:256-259`:

```text
java_runtime = rule(
    ...
    provides = [
        JavaRuntimeInfo,
        platform_common.TemplateVariableInfo,
    ],
)
error: rule provides must contain exported provider constructors
```

No Java rule implementation, initialized-provider callback or configured
provider operation executes before this declaration error.

## Durable selected rules_java closure

The BCR coordinate is `rules_java` 9.1.0. Its durable source descriptor is
`https://bcr.bazel.build/modules/rules_java/9.1.0/source.json`, SHA-256
`da589573c1dee2c9ac4a568b301269a2e8191110ff0345c1a959fa7ea6c4dfd6`.
It selects
`https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`,
a 114,566-byte/114-entry archive with SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`
and integrity `sha256-Thooolwu+lNQDJKNIs7/vFBd2VszWi0CWDaik7WSIS8=`.
Its 94 regular entries are mode `0444`; its 20 directories are `0755`.

The immediate source is regular `0444`
`java/common/rules/java_runtime.bzl`, SHA-256
`d908d3836e5796195596a1d7b4d36d7ca5c674db76acd5c4555b40204a602c08`,
9,847 bytes/260 lines with trailing LF. Lines 31-54 export initialized
`JavaRuntimeInfo`; lines 256-259 advertise it followed by
`platform_common.TemplateVariableInfo`.

The complete matching selected archive closure is:

| Source-relative path | SHA-256 | Bytes/lines | Declaration role |
|---|---|---:|---|
| `toolchains/java_toolchain_alias.bzl` | `56f84699c33ebd2e871615b30bcab4ae5a824cfab78e0dd80afc7e1fbf92e510` | 3,937/110 | lines 60-65 constrain on JavaRuntimeInfo plus TemplateVariableInfo; lines 69-73 advertise both plus already-supported ToolchainInfo |
| `java/private/java_info.bzl` | `02438c92066a825629a47f6dd01d9ea2200dc90a666b68fb4ee1ebf09e6a3026` | 47,438/1,038 | lines 825-886 and 1014-1038 export initialized JavaInfo and JavaPluginInfo |
| `java/bazel/rules/bazel_java_import.bzl` | `c8a4747c72ec57e64cbf6cda8da0e5a0f3320583db58c57321e126391b0c62a1` | 1,962/66 | advertises imported frozen JavaInfo; already passes |
| `java/bazel/rules/bazel_java_library.bzl` | `97f87b1bc3c6a5faa186e26d325578e2aca274a8131755a7805976b7ee5fb2f7` | 2,117/65 | advertises imported frozen JavaInfo; already passes |
| `java/bazel/rules/bazel_java_plugin.bzl` | `9e03f44ac8d32bb4f20f8dc83c80ed079e6c1dfe28c20ea0d7d133fe361b44bc` | 5,726/154 | advertises imported frozen JavaPluginInfo; already passes |
| `java/bazel/rules/bazel_java_binary.bzl` | `7788ed4c824a57be330f692f4e9ac4dab378bc70fa461b0ded4f046f444b8ab8` | 19,305/476 | advertises imported frozen JavaInfo; already passes |

All are regular `0444` with trailing LF. There are no other selected
`rule(provides=...)` sites. Only same-module JavaRuntimeInfo is a live
initialized callable at its advertiser; the JavaInfo and JavaPluginInfo
advertisers import already-supported frozen initialized callables and are
regression-preserved, not newly closed.

TemplateVariableInfo has exactly three declaration occurrences across two
files: `java_runtime.bzl:258`, alias required-provider `:64`, and alias
advertisement `:71`. Its three nondeclaration uses remain deferred:
`java_runtime.bzl:152` and alias `:26` construct instances, while alias `:48`
performs configured Target indexing.

## Bazel 9.2 authority and learned facts

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is the sole compatibility authority:

- `analysis/starlark/StarlarkRuleClassFunctions.java:1153-1155`, SHA-256
  `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`,
  routes `provides` through one provider-key converter;
- `analysis/starlark/StarlarkAttrModule.java:597-610`, SHA-256
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`,
  accepts Provider values, rejects unexported values and uses exact keys;
- `packages/StarlarkProvider.java:211-218,415-427,470-484`, SHA-256
  `cac43a3a9ab1d8653e05ae8b4304ffa6b573bad66bbfb207641d1a652d10cdc1`,
  gives initialized and ordinary user providers the same export/key contract;
- `packages/BuiltinProvider.java:39-80`, SHA-256
  `4551cf08d71dd305a14546f88f184dcef30f3c5882242899a83f865b531ecd93`,
  makes builtin providers always exported with stable keys;
- `rules/platform/PlatformCommon.java:43-50` and
  `starlarkbuildapi/platform/PlatformCommonApi.java:29-43`, SHA-256
  `010d4bb681cf44d6ed913ddd05d24eef1c4e214c6543f38f69e780a7e9d64d36`
  and `ef93f7b95a54069ba9fdba9ce978cec938db5b4f4a64471fb78c3b2b2f6c611b`,
  expose TemplateVariableInfo as its builtin provider constructor/key; and
- `analysis/TemplateVariableInfo.java:31-65`, SHA-256
  `21847cc2e32e271ea5a7c44f17f51ffe3baacd163652ad5f99fa270b8f94b8da`,
  owns the singleton provider and materially broader instance constructor.

`StarlarkRuleClassFunctionsTest.java:2498-2535` (SHA-256
`e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`)
proves initialized-provider export identity. `TemplateVariableInfoTest.java`
(SHA-256
`e3762b88d8871e7c77538a6e57e0dc420f8225080e4a124face71c99acc3298e`)
proves configured construction/indexing and is deferred, not copied into this
declaration packet.

Slug's immediate first failure is `JavaRuntimeInfo`: live
`InitializedUserProviderCallable` receives its `ProviderId` in `export_as`, but
`starlark_provider_identity` recognizes only its frozen sibling. After that
fix, TemplateVariableInfo would fail because the existing analysis-builtin
identity allowlist names only DefaultInfo and ToolchainInfo. Existing
`ProviderIdentity`, compact strings, Arc advertised-provider slice,
deduplication, freeze and equality already represent both values.

## Decision and compatibility boundary

Audit result: `ACCEPT` for one generic **provider declaration-identity
closure**, pending independent review.

Classify as **exact**: recognize an exported live initialized user-provider
constructor by the same producer-owned `ProviderId` as its frozen form;
recognize exactly the existing TemplateVariableInfo callable as builtin
`ProviderIdentity::Builtin("TemplateVariableInfo")`; accept both through the
shared rule/aspect `provides` and required-provider declaration paths; preserve
order/deduplication, freeze/import/re-export, and reject raw constructors,
unexported initialized providers, unrelated callables and non-providers.

Classify as **Slug-native**: the existing compact string/user-ID
representation and this configured diagnostic:

`platform_common.TemplateVariableInfo configured-target membership and indexing are unsupported`

Keep **unsupported/deferred**: TemplateVariableInfo invocation, instance
construction, target membership/indexing and make-variable semantics; Java
provider payloads, rule implementations, fragments, toolchain resolution and
actions; other platform_common provider breadth. Direct BUILD exposure is
unchanged.

## Ownership, retained data and fail-closed analysis seam

`app/slug_loading_v2/src/provider.rs` remains the sole production owner. In
`starlark_provider_identity`, project a live `InitializedUserProviderCallable`
only when its existing `OnceCell<ProviderId>` is populated, and add only
TemplateVariableInfo to the existing analysis-builtin identity match. In
`configured_target_provider_identity`, reject TemplateVariableInfo before the
shared projection. Do not add it to `alloc_starlark_provider_callable`.

No `package.rs` edit is needed: `declaration_provider_identity` and
`declaration_required_providers::is_provider` already use the shared classifier,
so both `provides` and `attr.label(providers=...)` receive the exact identities.
No analysis production edit is needed: configured Target indexing and
membership already call the configured classifier before map lookup. The
existing unsupported callable invocation remains the construction boundary.

Add no retained field, owner, representation, map, interner, registry, cache,
DICE key/input/observation, lock, await, retry or fixture. The projection clones
the existing compact ProviderId once into the already-owned Arc slice. Existing
structural equality/invalidation and module/source fingerprints remain
unchanged. Buck2/V1 supplies no implementation; no Stage 9 row is required.

## Frozen successor implementation and proof

After independent `ACCEPT`, activate
`WP-4-6-7A-provider-declaration-identity-closure-implementation-r1` and change
only:

- `app/slug_loading_v2/src/provider.rs`, sole production owner plus adjacent
  classifier/configured-rejection proof; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, proof only.

Caps: 16 production Rust, 140 proof Rust, 156 aggregate gross additions.

Required proof must cover selected-source-shaped JavaRuntimeInfo plus
TemplateVariableInfo advertisement in exact order; the alias-shaped required
provider list; ordinary/Bzlmod rule and aspect declaration; same-module live
export plus freeze/import/re-export; regression-preserved imported frozen
JavaInfo/JavaPluginInfo identities;
raw/unexported/non-provider rejection; exact TemplateVariableInfo builtin
identity; configured pre-rejection before lookup; unchanged unsupported
Template invocation; unchanged DefaultInfo, ToolchainInfo, ordinary/frozen
providers and rules without these identities. No initialized callback or Java
implementation may execute.

Run serial focused provider/declaration tests first, then full loading library
and integration targets, query library, direct pinned-nightly CLI rebuild, the
exact authenticated bounded-PATH replay, stale-slugd, formatting, diff,
archive, allowlist and cap gates. Replay must clear the selected declarations
and stop only at the next independent typed boundary.

Return `REPLAN` if the live initialized identity is unavailable after export;
TemplateVariableInfo cannot be rejected before configured lookup; another
production owner, retained marker or new key/cache/fixture is needed; invocation
or configured Java/Template semantics are required; ordinary provider behavior
changes; or the two-file allowlist/caps fail.
