# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-documented-provider-initializer-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: initialized provider schema normalization and existing provider owner
Base: `2ebc6fe1`

Result: accept documented string-to-string schemas on the existing loading-only
initialized provider family, completing rules_cc `CcInfo` and
`CcLauncherInfo` declarations. Stop before the next private C++ method.

## Accepted starting point and source-order stop

Commit `2ebc6fe1` accepts only the no-argument empty HeaderInfo row. The private
capability returns a fresh immutable loading value with four `None` module
fields and four immutable empty header-list observations. Named/non-empty
calls, hashing, dependencies and configured C++ lowering remain unsupported.
Focused proof, all 204 loading units, configured analysis, locked checks,
rebuilt CLI and hygiene pass. Independent selection review corrected the next
stop to `CcInfo`; terminal review returned `ACCEPT`.

The first absent expression is rules_cc 0.2.17
`cc/private/cc_info.bzl:260–269`:

```starlark
CcInfo, _ = provider(
    doc = "Provider for C++ compilation and linking information.",
    fields = {
        "compilation_context": "A `CcCompilationContext`.",
        "linking_context": "A `CcLinkingContext`.",
        "_debug_context": "A `CcDebugInfoContext`.",
        "_legacy_transitive_native_libraries": "A `CcNativeLibraryInfo`.",
    },
    init = _create_cc_info,
)
```

Slug's accepted initialized provider requires `fields` to be a string list.
All callable, raw-constructor, original-argument forwarding, dictionary-result
validation, optional-field, arbitrary-value, identity and freeze behavior is
already present. The honest missing abstraction is documented-dictionary schema
normalization for that same owner.

After this declaration, `cc_info.bzl` freezes. Source order returns through
`cc/private/cc_common.bzl` and reaches `cc/private/cc_launcher_info.bzl`, whose
`CcLauncherInfo, _ = provider(fields = { ... }, init = ...)` uses the same
shape and also freezes. The shared-library hint child uses an accepted direct
documented provider. The LTO child uses accepted direct documented providers
and an accepted dictionary-valued direct instance. Evaluation next enters
`cc/private/compile/cc_compilation_outputs.bzl`; its top-level
`EMPTY_COMPILATION_OUTPUTS = create_compilation_outputs_internal()` first calls
`_cc_internal.freeze(objects)` at line 86. That private method is the stop.

## Fixed sources and compatibility authority

Reuse the accepted rules_rust/rules_cc materialization. Relevant fixed inputs:

- `cc/private/cc_info.bzl` SHA-256
  `4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`;
- `cc/private/cc_launcher_info.bzl` SHA-256
  `41da54762e854191c0217575d385b37cd9729380d7c78d3efbc19049177250dd`;
- `cc/private/compile/lto_compilation_context.bzl` SHA-256
  `a17435cd56fa165c71081e99f9af73407f7b4cc1dc086e53771dcf74df81b3f4`;
- `cc/private/compile/cc_compilation_outputs.bzl` SHA-256
  `294e3da16da4444122e7dee058ec1e06b30cec93d64a32f217cf9e1e3e4bfb44`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StarlarkRuleClassFunctions.provider` accepts either a string sequence or a
string-to-string dictionary as `fields`, normalizes the schema, installs a
callable `init`, creates one provider identity and returns the provider/raw pair.
`StarlarkProvider.ArgumentProcessorWithInit` forwards original positional and
named arguments, requires the initializer result to be a string-keyed
dictionary, and creates through the same schema factory as the raw constructor.
The factory permits omitted schema fields and rejects unknown fields. Existing
pinned tests for declared providers with init, invalid return dictionaries,
unexpected fields and raw bypass discriminate the shared behavior; the live
rules_cc declarations discriminate the newly admitted dictionary schema form.
No new Bazel run is required.

## Zabel and Buck2 architectural guidance

Pinned clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test guidance only.
`provider_schema.zig` owns one normalized schema abstraction for documented
dictionaries and name sequences. `build_rule_declaration.zig` keeps schema,
initializer, raw constructor, publication owner and export identity on one
`ProviderDefinition`; construction projects schema ordinals from that owner.
Slug follows the one-owner and normalization boundaries but copies no Zig code,
representation, layout, allocator, errors, runtime or behavior. Bazel remains
compatibility authority.

Reuse the current `Arc<[CompactString]>` schema names, ordinal `SmallMap`
instance slots, `Value`/`FrozenValue`, `Dupe` and `Allocative`. Documentation
values are validated then discarded as already done for direct documented
providers. Add no map, interner, registry, side store or digest. No Stage 9
ledger row is required.

## Compatibility classification

- **Exact:** `.bzl` initialized `provider` accepts a string-to-string
  documented `fields` dictionary; invalid field names/docs fail; it returns
  the provider/raw pair; both share one assignment-bound identity and schema;
  normal calls forward original arguments through `init`, require a string-
  keyed dictionary, accept arbitrary freezeable values, permit omitted fields
  and reject unknown fields; raw calls bypass `init`, reject positional
  arguments and apply the same optional schema; the selected `CcInfo` and
  `CcLauncherInfo` declarations freeze.
- **Slug-native:** initialized instances remain loading-only; Rust storage,
  valid-Unicode strings, discarded documentation and nonrequired diagnostics
  remain native.
- **Unsupported/deferred:** omitted/`None`/schemaless initialized providers;
  tuple schemas; provider concatenation; initialized instances returned by
  configured analysis; CcInfo/CcLauncherInfo configured semantics;
  `_cc_internal.freeze` and every later C++ provider/toolchain/action method;
  later rules_cc/rules_rust source, M8/M7B and exact output bytes.

## Ownership, lifetime and implementation boundary

Keep `user_provider_from_arguments` as the only declaration adapter. When
`init` is present, normalize either the existing unique string list or an
existing documented string dictionary to the same canonical
`Arc<[CompactString]>`. Feed it to the unchanged
`InitializedUserProviderCallable::allocate_pair`. Do not add a schema-kind bit:
list and documented schemas have identical instance membership semantics and
documentation is not retained.

The existing initialized callable remains the sole owner of source label,
export identity, schema, callback and raw constructor. Normal/raw instances
continue using the generalized loading-only schemaful representation and freeze
all values through the module heap. No configured decoder changes.

`BzlModuleEvalKey` and recursive source observations remain the sole
invalidation owner. There is no DICE, request, command, async, cache,
publication, cancellation or shutdown change.

## Discriminating proof

- Evaluate and freeze rules_cc-shaped documented-dictionary initialized
  `CcInfo` and `CcLauncherInfo` declarations.
- Invoke normal construction with positional/named original arguments and
  prove initializer execution, optional arbitrary returned fields and frozen
  field reads.
- Invoke the raw constructor and prove callback bypass, shared ProviderId,
  optional fields, unknown rejection and positional rejection.
- Reject non-string dictionary names/docs, non-callable init, non-dictionary
  initializer results and unexpected returned fields.
- Keep existing list-schema initialized, direct provider-schema, empty
  HeaderInfo and configured provider analysis regressions green.
- Do not implement or claim `_cc_internal.freeze` or full `cc_common.bzl`.

## Allowlist and caps

Only these files may change from base `2ebc6fe1`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/provider.rs` | `c4369ce97f6c0c53d188aebc70b85d116dfd4a5e63ca547454ba01efed67ca5a` | 957 | 1,025 | documented initialized schema normalization |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `f77c9174c201c9256e09b49d1fa2c23b045c67d2642ffb84fbb8e7673aebd8d2` | 5,671 | 5,790 | source-shaped identity/freeze/boundary proof |

Production additions are capped at 60, proof additions at 110 and total
additions at 170. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. The test file exceeds the 2,000-line trigger,
but the focused proof belongs beside the existing initialized-provider and
provider-schema regressions sharing the same evaluator helper; splitting it
would widen `lib.rs` and the allowlist.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused documented initialized-provider test;
- existing list initialized-provider, direct provider-schema and empty
  HeaderInfo tests;
- one configured provider analysis regression;
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

The schema-normalization change requires independent selection and terminal
implementation reviews. Both must verify Bazel authority, Zabel's guidance-
only role, single identity/representation, freeze safety, configured
fail-closed behavior, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; analysis/build-api/DICE edit;
a second identity, callable or instance representation; retained documentation;
schemaless/tuple initialized widening; provider concatenation; configured
admission; `_cc_internal.freeze` or another C++ method; source/mapping/
materializer/network/fixture change; Java/JVM work; copied Zabel code or
behavior; cap violation; or a claim beyond the documented initialized-provider
declarations. After `CcLauncherInfo` freezes, stop at
`cc_compilation_outputs.bzl:86` and audit that method separately.
