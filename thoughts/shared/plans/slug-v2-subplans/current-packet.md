# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-label-provider-predicate-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute declaration and package-schema owners
Base: `b1edbe0e`

Result: admit the source-required singleton scalar-label provider predicate,
reuse the existing normalized provider-identity schema, and advance
rules_rust's complete `rust_toolchain` attribute map to its first
`config_common.toolchain_type` call. Keep the implementation lazy.

## Accepted starting point and source-order stop

Commit `b1edbe0e` admits Boolean/`None` scalar-label `allow_files`, checks the
non-`None` conflict with `allow_single_file` before normalization, and projects
the existing Boolean without single-artifact identity. Both rules_rust LLVM
file rows freeze. All 214 loading tests and downstream gates pass, with
independent terminal `ACCEPT` at 10 production, 91 proof and 101 total
additions.

Source order now reaches:

```starlark
"lto": attr.label(
    providers = [RustLtoInfo],
    default = Label("//rust/settings:lto"),
    doc = "...",
),
```

The same scalar-label shape occurs later for
`_experimental_use_allocator_libraries_with_mangled_symbols_setting` with
`providers=[BuildSettingInfo]`. Every intervening and remaining attribute-map
row uses admitted constructors. The next absent evaluated expression is the
rule-level toolchain entry:

```starlark
config_common.toolchain_type(
    "@bazel_tools//tools/cpp:toolchain_type",
    mandatory = False,
)
```

Do not admit this `config_common` method or claim the full rule freezes.

## Fixed sources and compatibility authority

Selected rules_rust source:
`/tmp/slug-rules-rust-registry.MZNsRA/source/rust/private/toolchain.bzl`,
SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.
Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.

`StarlarkAttrModuleApi.labelAttribute` exposes named `providers` with an empty
sequence default. `StarlarkAttrModule.buildProviderPredicate` treats a flat
provider list as one conjunctive set and a nested list as alternatives,
requires exported provider constructors, and installs a nonempty predicate in
the shared label/label-list builder. `StarlarkRuleClassFunctionsTest` proves a
scalar label retains `[ParentInfo]`; `testAttrWithProviders` pins conjunction
and `testAttrWithProvidersList` pins alternatives.

This packet reproduces only omitted/empty and one exported provider in a flat
list. Multiple conjunction members, alternative lists, empty alternatives,
built-in providers and configured provider checking remain deferred.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural/test guidance only. Its one dependency-attribute declaration
owner retains the same optional provider-predicate fact for scalar and list
labels, beside other schema metadata, before later package lowering. Slug
follows that shared ownership boundary but immediately detaches the selected
exported identity into its existing Rust provider schema. It copies no Zig
evaluator value, code, layout, diagnostics or behavior. Bazel remains
compatibility authority.

The Buck2 utility audit selects the existing
`Arc<[Arc<[ProviderId]>]>` normalized provider-predicate representation and
`Allocative` owners already used by label-list attributes. The singleton adds
one immutable outer alternative and one immutable inner conjunction; clones
remain reference bumps. No new collection, hash, interner, utility import or
Stage 9 ledger row is needed.

## Compatibility classification

- **Exact:** the admitted scalar-label subset accepts omitted and empty
  provider sequences as unconstrained; accepts one exported user-provider
  constructor in a flat list; retains its canonical defining-module/export
  identity as one conjunctive predicate; rejects unexported/non-provider and
  unsupported shapes; carries the predicate through freeze; and crosses both
  rules_rust rows without invoking the implementation.
- **Slug-native:** the existing compact nested-`Arc` schema representation and
  Rust diagnostic wording/order.
- **Unsupported/deferred:** multiple-provider conjunctions, nested alternatives,
  empty alternatives, built-in provider identifiers, exact diagnostics,
  configured prerequisite/provider validation, invocation of any constrained
  rule, `config_common.toolchain_type`, completion of `rust_toolchain`, M8,
  M7B and exact output bytes.

## Ownership and implementation boundary

Add a small scalar-label provider parser beside the existing label-list
normalizer. It accepts no argument or `[]` as empty, otherwise requires exactly
one exported user-provider constructor and returns one existing nested-`Arc`
alternative. Add named `providers` only to scalar `attr.label` and assign the
normalized predicate to existing `AttributeDefinitionGen::required_providers`.

Existing freeze and rule schemas already retain the field; the target-call
preflight already rejects all nonempty provider-constrained attrs. Extend the
repository-rule and tag-class projection guards so the new scalar form cannot
be silently dropped. Do not change invocation, provider identity or analysis.

There is no new representation and no DICE, request, command, analysis,
async, mapping, repository, publication, cancellation or shutdown change.

## Discriminating proof

- Accept omitted and empty scalar-label provider sequences plus one imported,
  exported provider; reject `None`, scalars, unexported constructors,
  non-providers, multiple flat members and nested alternatives.
- Prove constrained and unconstrained frozen schemas differ, the retained
  predicate contains the imported provider's canonical defining-module/export
  identity, and order-independent empty forms remain equal.
- Freeze the source-shaped `lto` and hidden build-setting rows with distinct
  imported providers; prove the following `config_common.toolchain_type` call
  remains rejected.
- Prove package invocation of a scalar provider-constrained rule rejects before
  target recording and repository-rule/tag-class projections fail closed.
- Keep the label-list provider/aspect, file allowance, allowed-values, docs,
  stdlib, Rust analyzer and rules_cc proofs plus one configured-analysis
  regression green.

## Allowlist and caps

Only these files may change from base `b1edbe0e`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `a0dc5f05069f3d75b30dda310e0df355bf0ee42cb358cd4183117a78e4452e7c` | 5,983 | 6,030 | scalar provider normalization, adapter and fail-closed guards |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `952c6b10453fd00364dbb59f790aa71a6740d1688ad5ccab2189ae3c4a093411` | 6,402 | 6,540 | ABI, identity, rejection and source-prefix proof |

Production additions are capped at 45, proof additions at 135 and total
additions at 180. Deletions do not buy addition budget. No new function may
exceed 120 lines. `FrozenRuleDefinition::invoke` may not change. The files
exceed the 2,000-line trigger, but normalization belongs beside the existing
provider predicate owner and recursive proof beside adjacent rules_rust tests;
splitting would duplicate owners and widen the packet.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused scalar-label provider ABI/identity/rejection/source-prefix proof;
- existing label-list provider/aspect, file-allowance, allowed-values, docs,
  stdlib, analyzer and rules_cc proofs;
- one configured rule/provider analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun absent
integration risk. Recheck caps, hashes, source stop and clean Zabel pin.

Independent selection and terminal reviews must verify Bazel authority,
Zabel's guidance-only role, reuse of normalized provider identity, singleton
scope, invocation/projection failure, the `config_common` stop, compatibility
classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; a broader
provider-predicate shape; a new retained representation; silent provider loss;
configured provider enforcement; invocation of a constrained rule;
`config_common.toolchain_type`; completion of `rust_toolchain`;
DICE/analysis/repository/source changes; Java/JVM work; copied Zabel code or
behavior; cap violation; or a claim beyond reaching the first rule-level
toolchain-type expression. Audit that `config_common` method separately after
the complete attribute map loads.
