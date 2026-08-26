# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-provider-schema-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: provider schema normalization and loading-only provider values
Base: `9c51999f`

Result: complete the next recursively loaded rules_cc child by admitting
Bazel's non-initialized schemaless and string-list provider declarations plus
optional arbitrary loading-time field values. Preserve the accepted configured
string-provider projection and stop before `cc_info.bzl` invokes a private C++
native method.

## Accepted starting point and source-order stop

Commit `9c51999f` accepts the initialized-provider shape used by
`cc/common/cc_helper_internal.bzl`: one assignment-bound provider owns the
normal and raw constructors, schemaful instances use shared schema names and
compact field ordinals, arbitrary Starlark values freeze in the module heap,
and initialized values remain rejected by configured analysis. Focused and all
202 loading units, one configured-provider regression, locked core checking,
the rebuilt CLI and hygiene pass. Independent review restored the legacy
unbound-provider diagnostic and then returned `ACCEPT`.

Recursive source order returns through rules_rust 0.73.0
`rust/private/toolchain.bzl` and rules_cc 0.2.17
`cc/common/cc_common.bzl`, then the generated compatibility proxy's
`symbols.bzl`, then `cc/private/cc_common.bzl`. Its first child,
`cc/common/cc_helper_internal.bzl`, now freezes completely. The second child,
`cc/private/cc_info.bzl`, recursively enters
`cc/private/link/create_extra_link_time_library.bzl`. That file's own two
loads are already accepted. Its first absent evaluated expression is line 34:

```starlark
ExtraLinkTimeLibraryInfo = provider("ExtraLinkTimeLibraryInfo")
```

The same child then declares a second schemaless provider, a schemaful
string-list provider, and a documented-map provider. It immediately constructs
the documented provider with a list value:

```starlark
_KeyInfo = provider(
    "_KeyInfo",
    fields = ["build_library_func", "constant_fields", "depset_fields"],
)
ExtraLinkTimeLibrariesInfo = provider(
    "ExtraLinkTimeLibrariesInfo",
    fields = {"libraries": "A list of extra libraries."},
)
_EMPTY = ExtraLinkTimeLibrariesInfo(libraries = [])
```

Slug currently requires `fields` to be a documented dictionary, then requires
every declared constructor field to be present and a string. Therefore the
schemaless declaration is the honest first stop, while declaration-only support
would still leave the same source child incomplete. The bounded abstraction is
non-initialized provider schema normalization plus direct loading construction.

After this child freezes, stop. Do not claim `cc_info.bzl` completes: its later
top-level empty compilation context reaches
`cc_internal.create_header_info()`, which is a separate private C++ semantic
surface requiring a fresh source-order audit.

## Fixed sources and compatibility authority

Reuse the accepted source materialization and repository graph. Relevant fixed
inputs are:

- rules_rust 0.73.0 archive SHA-256
  `2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`;
- `rust/private/toolchain.bzl` SHA-256
  `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
- rules_cc 0.2.17 source JSON SHA-256
  `3832f45d145354049137c0090df04629d9c2b5493dc5c2bf46f1834040133a07`;
- rules_cc archive SHA-256
  `283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a`;
- generated compatibility-proxy `symbols.bzl` SHA-256
  `2adedeeaaad8c0e664dc35e9bf1480b1d6dc3d7840034f9efe3ee78476fc5902`;
- `cc/private/cc_common.bzl` SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`;
- `cc/private/cc_info.bzl` SHA-256
  `4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`;
- `cc/private/link/create_extra_link_time_library.bzl` SHA-256
  `522312ac48567566725f0768a6961fcaa78577fa24ac8007d5b1b8ca19698e82`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
`StarlarkRuleFunctionsApi.provider` makes `fields` optional and accepts a
string sequence, string-to-string dictionary or `None`; all declared fields
are optional. `StarlarkRuleClassFunctions.provider` leaves a missing/`None`
schema schemaless, gives list and dictionary inputs a schema, and returns one
provider callable when `init` is absent. `StarlarkProvider.RawArgumentProcessor`
rejects positional arguments and sends named values to either
`StarlarkInfoNoSchema` or `StarlarkInfoWithSchema`. Schemaless values accept
arbitrary names; schemaful values reject unknown names; both retain arbitrary
valid Starlark values. `declaredProviders`, `providerWithFields`,
`providerWithFieldsOptional`, `providerWithExtraFieldsError`,
`providerWithEmptyFieldsError`, `basicInstantiation`,
`basicInstantiationWithSchemaWithSomeFieldsUnset`,
`schemalessProvider_getSchema`, and the schema validation tests discriminate
the selected rows. No new Bazel run or fixture is required.

## Zabel and Buck2 architectural guidance

Pinned clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test guidance only.
`src/starlark_host/engine/provider_schema.zig` distinguishes schemaless from
schemaful definitions, keeps declaration-order names plus one canonical name
index, and makes schemaless membership accept every name.
`build_rule_declaration.zig` keeps schema, initializer, publication owner and
export identity on one `ProviderDefinition`; `providerFactoryCall` normalizes
missing/`None`, list/tuple and dictionary schemas before publishing the
callable; its normalized-schema regression distinguishes schemaless from an
empty schema and exercises arbitrary direct fields. Slug follows the one-owner,
schema-kind and fail-closed consumer boundaries through existing starlark-rust
values. No Zig code, layout, allocator, digest, runtime or behavior is copied;
Bazel remains compatibility authority.

The Buck2 utility review requires shared `Arc<[CompactString]>` schema names
and compact `u32` slots for schemaful loading instances. Only schemaless
instances retain dynamic `CompactString` names beside their values. Reuse
deterministic `SmallMap`, `Value`/`FrozenValue`, `Dupe` and `Allocative`; add no
hash map, tree map, interner, registry, side store or new digest domain. This is
an existing-utility decision, so no Stage 9 ledger row is needed.

## Compatibility classification

- **Exact:** `.bzl` `provider` without `init` accepts omitted/explicit-`None`
  `fields` as schemaless and a unique string list as schemaful; documented
  dictionary declarations remain accepted; the call returns one provider
  callable; direct construction rejects positional arguments, accepts arbitrary
  freezeable named values, permits omitted schema fields, rejects unknown
  schemaful fields, and permits arbitrary names when schemaless; empty schema
  and schemaless remain distinct; field access and module freeze work; duplicate
  or non-string list fields fail closed; the selected rules_cc declarations and
  `_EMPTY` instance freeze.
- **Slug-native:** one provider identity may project a full documented all-
  string instance to the accepted configured `StarlarkUserProvider`, while
  other direct instances use the loading-only arbitrary-value representation;
  both carry the same assignment-bound `ProviderId`. Rust storage/layout,
  valid-Unicode strings and nonrequired diagnostic wording remain native.
- **Unsupported/deferred:** tuple schemas; schemaless/dictionary initialized
  providers; documentation extraction; provider concatenation; loading-only
  instances returned by analysis; schemaless/list providers as configured
  dependency values; `cc_internal.create_header_info()` and every other C++
  provider/toolchain/action method; later rules_cc/rules_rust source, M8/M7B
  and exact output bytes.

## Ownership, lifetime and implementation boundary

The existing provider global remains the declaration factory. Add one compact
schema-kind value to the existing non-initialized callable: schemaless, list
schema, or documented-map schema. The callable remains the sole owner of
source/export identity and shared schema names. Missing and explicit `None`
normalize identically; an empty list remains a schemaful empty declaration.

Generalize the accepted loading-only provider instance family so initialized
and direct schemaful instances share `Arc` schema names plus ordinal field
slots, while a schemaless variant retains dynamic names. Every retained value
must freeze through the module heap. Do not retain evaluator scratch or add a
second provider identity.

For a documented-map/no-init call only, preserve the existing configured
projection when every declared field is present exactly once and is a string.
That path must keep returning the unchanged `StarlarkUserProvider`. Optional or
non-string documented instances, list-schema instances and schemaless
instances use the loading-only type and therefore remain rejected by the
unchanged analysis decoder. Unknown schema fields and positional arguments
fail before either representation is allocated.

`BzlModuleEvalKey` and the existing recursive source observations remain the
sole invalidation owner. There is no DICE, request, command, async, cache,
publication, cancellation or shutdown change.

## Discriminating proof

- Evaluate and freeze the selected rules_cc-shaped declarations and `_EMPTY`
  list-valued instance; read `libraries` after freeze.
- Prove omitted and explicit-`None` schemas accept different arbitrary named
  values, while `fields = []` rejects every name.
- Prove string-list schemas accept arbitrary values and omitted fields, reject
  unknown fields and positional arguments, and reject duplicate/non-string
  declaration fields.
- Prove documented-map providers retain the existing full-string configured
  instance type, while optional/non-string instances use the loading-only type
  with the same exported identity and cannot downcast as `StarlarkUserProvider`.
- Keep the initialized-provider normal/raw identity and failure matrix green.
- Keep the existing provider doc/freeze and one configured string-provider
  analysis regression green.
- Do not add a full rules_cc fixture or claim `cc_info.bzl` succeeds.

## Allowlist and caps

Only these files may change from base `9c51999f`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/provider.rs` | `c11b609ffc52a642d6f06cce509a4adb6eb4c51a7249985d889b1464b01777cb` | 880 | 1,100 | schema kind and loading instance owner |
| `app/slug_loading_v2/src/package.rs` | `53e6231435c0db7692bae79c96585c434c7136af6365dff9688317d21f512683` | 5,833 | 5,845 | optional provider factory argument only |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `2880e3eb640f55184372f6ee410f016d1fc1b68c8d5bfb129f4e19c188294aa3` | 5,501 | 5,660 | source-shaped freeze and boundary proof |

Production additions are capped at 220, proof additions at 150 and total
additions at 370. Deletions do not buy addition budget. No new or touched
function may exceed 150 lines. `package.rs` exceeds the 2,000-line review
trigger, but its sole permitted edit is the existing provider adapter's
`fields` optionality; moving that signature would split one globals owner.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused provider-schema/loading test;
- existing initialized-provider and provider-doc/freeze tests;
- one focused configured string-provider analysis test;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun unless focused
evidence exposes integration risk. Recheck base hashes, caps, allowlist,
function sizes, configured-analysis non-widening and the clean Zabel pin.

The retained schema kind and loading-instance representation require
independent selection and terminal implementation reviews. Both must verify
Bazel authority, Zabel's guidance-only role, single identity, compact storage,
freeze/lifetime safety, configured fail-closed behavior, compatibility classes
and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; analysis/build-api/DICE edit;
a second provider identity; configured admission of schemaless/list or
loading-only instances; tuple or initialized schema widening; provider
concatenation; evaluator borrow in frozen state; a generic Starlark runtime
edit; a new collection/hash/interner/registry; `cc_internal` or C++ semantic
method; source/mapping/materializer/network/fixture change; Java/JVM work;
copied Zabel code or behavior; cap violation; or a claim beyond the selected
rules_cc child. After `create_extra_link_time_library.bzl` freezes, stop and
audit `cc_info.bzl` separately.
