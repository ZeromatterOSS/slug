# Current Slug V2 Packet

Packet: WP-4-7A-bazel-provider-initializer-loading
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: provider declaration/constructor loading and one frozen rules_cc child
Base: 4d7a9bbb

Result: admit Bazel's initialized-provider loading shape through the first
rules_cc artifact-category declarations and instances, then stop. Preserve the
existing string-only configured provider consumer and do not enter C++
providers, toolchains, actions or analysis.

## Accepted starting point and next source frontier

Commit `4d7a9bbb` installs `.bzl`-only `cc_common.internal_DO_NOT_USE()`, checks
the selected canonical `rules_cc+` owner, returns one frozen opaque private
token, and keeps BUILD and every C++ method unchanged. Independent terminal
review accepted the public/private separation and exact private diagnostic.

Recursive source order resumes in rules_cc 0.2.17
`cc/common/cc_helper_internal.bzl`. Its third load,
`cc/private/paths.bzl`, has no child loads and only defines the lazy
`is_path_absolute` function, so it freezes on the accepted evaluator. The
first absent evaluated call is then:

```starlark
_ArtifactCategoryInfo, _unused_new_aci = provider(
    """A category of artifacts ...""",
    fields = ["name", "default_prefix", "default_extension", "allowed_extensions"],
    init = _artifact_category_info_init,
)
```

Slug's provider adapter admits the already-selected documented dictionary
schema and has no `init`. The same source immediately invokes the initialized
constructor for the fixed artifact-category rows, reads their fields, and
freezes the resulting private list. Declaration-only acceptance would not
complete a source child, so declaration, initialized construction, raw
construction, field access and freeze are one bounded abstraction here.

## Source provenance and behavior authority

Reuse the accepted repository graph and source materialization. Relevant fixed
inputs are:

- rules_cc 0.2.17 source JSON SHA-256
  `3832f45d145354049137c0090df04629d9c2b5493dc5c2bf46f1834040133a07`;
- rules_cc archive SHA-256
  `283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a`;
- `cc/common/cc_helper_internal.bzl` SHA-256
  `793ab429f8e397df9c486f4c3c7b5c57fae81c8432ba6d08189d65d75676dae1`;
- `cc/private/paths.bzl` SHA-256
  `c982ac685f0bfbd32602d82d1c37f3bf50a2714ca6a13bfd3c08d4e5cc8b8872`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains sole behavior authority.
`StarlarkRuleFunctionsApi.provider` specifies named `fields` and `init`, the
two-value `(provider, raw_constructor)` result when `init` is present, original
argument forwarding, dictionary return, schema checking and raw bypass.
`StarlarkRuleClassFunctions.provider` constructs that pair.
`StarlarkProvider.ArgumentProcessorWithInit` and `RawArgumentProcessor` own the
two call paths. Discriminating pinned tests are
`StarlarkRuleClassFunctionsTest.declaredProvidersWithInit`,
`declaredProvidersWithFailingInit_rawConstructorSucceeds`,
`declaredProvidersWithInitReturningInvalidType_normalConstructorFails`,
`declaredProvidersWithInitReturningUnexpectedFields_normalConstructorFails`,
and `StarlarkProviderTest.rawConstructorBypassesInit`.

No fresh Bazel run is needed: those pinned-source tests and the authenticated
rules_cc source are the accepted regression authority. Add no fixture,
network request, generated repository, mapping, observer or materializer.

## Zabel and Buck2 guidance

Pinned Zabel commit `c7298478e2e56262a2f438e9c065325744c9f0fc` is
architectural guidance only. Its
`starlark_host/engine/build_rule_declaration.zig` keeps one provider definition
as the authoritative owner of schema, initializer and exported identity; the
raw callable points back to that definition, initializer results are validated
before an instance is published, and the rules_cc-shaped regression exercises
the real declaration/constructor sequence. Slug follows that single-owner,
normal-versus-raw split using existing starlark-rust freeze/value mechanisms.
No Zig code, layout, allocator policy, vtable, diagnostic or provider behavior
is copied; Bazel remains the compatibility authority.

The Buck2 utility review reuses the retained starlark-rust `Value`/
`FrozenValue` ownership, `CompactString`, deterministic `SmallMap`, `Dupe`
where pointer-sized values require an explicit cheap clone, and `Allocative`.
The Stage 9 configured-analysis/provider row already admits those utilities
behind V2 provider identity. Add no `HashMap`, `BTreeMap`, interner, registry,
cache or graph side store, and add no ledger row because there is no new donor
import or representation family.

## Compatibility classification

- **Exact:** `.bzl` `provider` accepts the selected string-list schema plus a
  callable `init`; the call returns a two-element provider/raw pair; the normal
  constructor forwards the original positional and named arguments to `init`;
  `init` must return a string-keyed dictionary whose keys fit the schema; the
  initializer dictionary may omit declared optional fields; the raw constructor
  bypasses `init` and rejects positional arguments; both constructors create
  instances with the same exported provider identity; fields retain their
  Starlark values and are readable; the selected rules_cc artifact-category
  declarations, instances, comprehensions and structs freeze without executing
  analysis.
- **Slug-native:** separate loading-only initialized callable/raw/instance Rust
  projections instead of widening the configured string-provider value; Rust
  display/type/freeze representation and nonrequired diagnostic wording;
  current frozen-module ownership and equality cutoff.
- **Unsupported/deferred:** initialized-provider instances returned from rule
  analysis; dict/`None` schemas on the initialized route; schemaless initialized
  providers; documentation extraction; provider concatenation; initializer
  closure introspection; C++ provider semantics; every `cc_internal` method;
  later rules_cc/rules_rust source; toolchains, actions, configured analysis,
  M8/M7B and exact output bytes.

## Natural owner, lifetime and invalidation

The existing provider global remains the sole declaration factory. For the
selected initialized route it creates one evaluator-owned provider callable
containing schema, initializer closure and assignment-bound `ProviderId`; a
raw callable retains a Starlark reference to that same callable rather than a
second identity. Normal and raw instances retain a deterministic field map of
Starlark values. Freezing the `.bzl` module freezes the callable, closure, raw
reference, instances and field values into the module's heap. No retained
value borrows evaluator scratch or stores an evaluator pointer.

`BzlModuleEvalKey` and the existing observed source/module fingerprints remain
the semantic owner and invalidation boundary. Identical frozen modules may cut
off downstream recomputation; source edits use the existing observed path and
recursive manifest flow. There is no request overlay, command-local result,
async transfer, cancellation, task join, persistence or shutdown change.

Keep the existing non-initialized `UserProviderCallable` and
`StarlarkUserProvider` configured-analysis representation unchanged. An
initialized instance returned from a rule implementation continues to fail
closed as a non-admitted provider kind. This is a declared unsupported
boundary, not a fallback, and needs no deletion ledger.

## Implementation boundary

1. Extend only the `.bzl` provider adapter to distinguish the accepted existing
   documented-map/no-init path from the selected string-list/callable-init
   path. Accept the selected positional string `doc` with list/callable `init`.
   Preserve all existing argument acceptance and failures, and reject
   list/no-init, dict/init and `None`/init in this packet.
2. Add loading-only initialized provider callable, raw constructor and instance
   values in the existing provider owner. The raw constructor must reference
   the authoritative callable and cannot acquire independent identity.
3. Bind the provider identity through the existing top-level `export_as`
   boundary. Both constructors must report instances with that identity.
4. Forward already-evaluated original positional/named arguments to the
   initializer. Require a dictionary result, validate string keys and selected
   schema membership while allowing omitted declared fields, retain arbitrary
   freezeable Starlark field values, and expose them through normal attribute
   lookup. Raw construction bypasses the callback and rejects positional
   arguments.
5. Freeze every retained closure/reference/value through starlark-rust's heap
   ownership. Keep deterministic `SmallMap` field order and `Allocative`.
6. Do not edit analysis decoding, provider API identity, DICE, source loading,
   globals placement, rule invocation or any C++ semantic module.

## Discriminating proof

- Evaluate the pinned rules_cc-shaped initializer declaration with its four
  string fields, construct at least two artifact categories positionally, form
  the name struct/comprehension, read scalar and tuple fields, and freeze the
  module.
- Prove the returned value is exactly a two-element pair and that normal/raw
  instances share the exported provider identity.
- Prove a normal constructor runs and forwards to its initializer while a raw
  constructor bypasses an initializer that would fail, rejects positional
  arguments, and accepts omission of an admitted schema field.
- Prove non-callable `init`, non-dictionary initializer return, non-string
  result key and unexpected schema field fail closed.
- Prove factory dispatch accepts positional string `doc` plus list/callable
  `init`, while list/no-init, dict/init and `None`/init reject. These rows guard
  the packet boundary after replacing the current statically typed map-only
  adapter.
- Downcast normal and raw initialized instances in Rust proof to the new
  loading-only type, and assert neither downcasts as existing
  `StarlarkUserProvider`; this composes the declared analysis fail-closed
  boundary without an analysis edit.
- Keep documented dictionary/no-init provider freeze and one configured
  string-provider analysis regression green.
- Do not add a full rules_cc fixture or claim its parent load succeeds beyond
  this child.

## Allowlist and caps

Only these files may change from base `4d7a9bbb`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| app/slug_loading_v2/src/provider.rs | d4a9face383a696f38ebad82f250fe71585fef24f5975ab6658ac1dc0624bd8c | 593 | 880 | initialized callable/raw/instance owner |
| app/slug_loading_v2/src/package.rs | f39c2f3655bbee8398164289fdc2f67e32e224722c354d8a0b19e4edd0dd2fad | 5,834 | 5,850 | provider factory argument dispatch only |
| app/slug_loading_v2/src/host_package_load_tests.rs | 4043659025eae2d4bd312466734f0374d75aee9d525222e2469ba26b7f42a576 | 5,404 | 5,585 | pinned-source loading/freeze/failure proof |

Additions are capped at 300 production, 180 proof and 480 total. Deletions do
not buy addition budget. No new or touched function may exceed 150 lines.
`package.rs` exceeds 2,000 lines, but its only allowed change is the existing
provider adapter signature/dispatch; provider ownership stays cohesive in the
sub-900-line `provider.rs`. STOP if a general Starlark runtime edit or analysis
representation change is required.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused initialized-provider loading test;
- existing provider doc/freeze test;
- one focused configured string-provider analysis test;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after the Rust change;
- `cargo fmt --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration was run at the immediately
preceding checkpoint and remains 30/31 only for its known stale `@external`
diagnostic-order row; do not rerun it unless focused evidence reveals package
integration risk. Recheck hashes, caps, allowlist, function sizes and the
absence of analysis changes.

The new retained representation and provider identity sharing require
independent selection and terminal implementation reviews. Both reviews must
verify pinned Bazel authority, Zabel's guidance-only role, one authoritative
identity, freeze/lifetime safety, configured-provider non-regression, utility
reuse, exact/deferred boundaries and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; a starlark-rust runtime edit;
an analysis/build-api/provider-identity edit; a second provider identity for
the raw constructor; an evaluator pointer or scratch borrow in frozen state;
dictionary/schemaless initialized-provider breadth; provider concatenation;
public C++ provider/toolchain/action/analysis behavior; source, mapping,
observation, DICE, cache, I/O or async changes; Java/JVM work; Zabel code or
behavior adoption; unpinned source; a fixture/oracle/network request; cap
violation; or a broad rules_cc/rules_rust success claim. After
`cc_helper_internal.bzl`'s artifact-category values freeze, stop and audit the
next recursive rules_cc load separately.
