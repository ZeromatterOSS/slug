# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-int-allowed-values-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute declaration and package-schema owners
Base: `8d3f9b6e`

Result: retain and enforce the first nonempty Bazel integer allowed-value set,
advancing rules_rust's `rust_toolchain` declaration through
`experimental_use_allocator_libraries_with_mangled_symbols` to its first
string allowed-value constraint. Keep the rule implementation lazy.

## Accepted starting point and source-order stop

Commit `8d3f9b6e` accepts omitted, explicit `None`, and string documentation on
`attr.int`, `attr.string_list`, `attr.string_dict`, and
`attr.string_list_dict`. Documentation is type-checked and discarded from the
semantic schema. The source-shaped `rust_toolchain` prefix now reaches line
727 of rules_rust 0.73 `rust/private/toolchain.bzl`, where its first absent
evaluated argument is:

```starlark
"experimental_use_allocator_libraries_with_mangled_symbols": attr.int(
    doc = "... Possible values: [-1, 0, 1]. ...",
    values = [-1, 0, 1],
    default = -1,
),
```

After this row, accepted label/string/list shapes advance source order to
`linker_preference` at lines 766-769. Its
`attr.string(values = ["cc", "rust"])` is a distinct typed family and remains
the stop. Do not admit string constraints, label `allow_files`, or claim the
full `rust_toolchain` rule freezes.

## Fixed sources and compatibility authority

The selected rules_rust file has SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.
Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.

`StarlarkAttrModuleApi.intAttribute` defines named-only `values`, defaults it
to an empty sequence, and restricts elements to Starlark integers.
`StarlarkAttrModule.intAttribute` forwards the typed sequence, while
`buildAttribute` installs `Attribute.AllowedValueSet` only when it is nonempty.
`AttributeProvider.checkAllowedValues` visits every possible explicitly
supplied or configurable value and reports values outside the set. Ordinary
rule defaults are retained without this check. `StarlarkRuleClassFunctionsTest`'s
`testAttrIntValues` is the pinned discriminating source regression.

This packet reproduces the typed constructor ABI, empty/no-constraint
normalization, retained constraint identity, and rule-instance enforcement.
Exact Bazel diagnostic wording and iteration order are not claimed.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural/test guidance only. Its attribute declaration owner retains
`allowed_values` beside the default and other schema facts, validates only int
or string constraint families, and proves evaluator-owned integer values are
available to the loaded declaration. Slug follows the ownership boundary, but
detaches the selected integer set into its existing Rust schema rather than
copying Zabel's evaluator value, optional-mask layout, Zig code, diagnostics,
or behavior. Bazel remains compatibility authority.

The Buck2 utility audit selects the existing immutable `Arc<[T]>` and
`Allocative` pattern already adopted for retained V2 schemas. Integer values
are sorted and deduplicated once at declaration capture so semantic equality
is set-shaped, clone cost is one reference bump, and target checks allocate
nothing. No map, hasher, interner, cache, new utility import, or Stage 9 ledger
row is needed.

## Compatibility classification

- **Exact:** the admitted `attr.int(values=...)` subset accepts list/tuple
  sequences of signed 32-bit integers and rejects non-sequences and non-integer
  elements,
  treats omitted and empty sequences as no constraint, retains every nonempty
  allowed set, rejects disallowed explicit and `select()` branch candidates,
  and does not reject an ordinary rule's unselected default. The
  rules_rust-shaped declaration prefix
  crosses its `[-1, 0, 1]` row without invoking the implementation.
- **Slug-native:** sorted/deduplicated `Arc<[i32]>` semantic identity and Rust
  diagnostic wording/rendering.
- **Unsupported/deferred:** `values` on strings or any other attribute kind;
  integer constraint literals outside the signed 32-bit range; exact
  allowed-value error text/order; configured-value revalidation;
  repository-rule or module-extension-tag use of constrained attrs; completion
  or invocation of `rust_toolchain`; label `allow_files`, file resolution,
  `config_common.toolchain_type`, M8, M7B and exact output bytes.

## Ownership and implementation boundary

Add one immutable allowed-integer slice to the existing transient/frozen
`AttributeDefinitionGen`, `RuleAttributeSchemaGen`, and package-owned
`AttributeSchema`. Normalize it at the `attr.int` adapter, carry it through
freeze and schema projection, expose only a borrowed getter, and validate the
coerced explicitly supplied/select candidates before recording a target. An
omitted attribute keeps its existing default without constraint validation,
matching Bazel's ordinary rule path.

Repository-rule and module-extension-tag projections do not own this fact and
must reject a nonempty constraint instead of dropping it. Built-in schemas and
all non-integer descriptors retain an empty slice. Do not add another registry,
side table, evaluator-owned value, analysis check, or configured-state owner.

There is no DICE, request, command, analysis, async, mapping, repository,
publication, cancellation or shutdown change.

## Discriminating proof

- Accept omitted, empty list, empty tuple, signed-32-bit integer list and
  integer tuple `values`; reject `None`, a scalar, mixed/wrong-type elements and
  dictionaries. Prove an out-of-range constraint literal hits the declared
  unsupported boundary.
- Prove order/duplicates normalize to the same retained set, while distinct
  sets make frozen rules and package schemas unequal.
- Prove allowed explicit values record and disallowed explicit values reject
  before target recording; prove an omitted disallowed default still records.
- Prove every candidate of a configurable `select()` is checked, including an
  invalid non-default branch and invalid default branch.
- Freeze a rules_rust-shaped `rust_toolchain` prefix through the
  `[-1, 0, 1]` descriptor and prove `attr.string(values=["cc", "rust"])`
  remains rejected as the next source-order stop.
- Keep the documentation, stdlib filegroup, Rust analyzer, rules_cc wrapper and
  configured-analysis regressions green.

## Allowlist and caps

Only these files may change from base `8d3f9b6e`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/attrs.rs` | `eb03e38deb17d8b4a607050777f7a17f416f026e027da0b0c0ac007c16b6e78e` | 1,484 | 1,510 | immutable package-schema constraint and borrowed projection |
| `app/slug_loading_v2/src/package.rs` | `c6ff7f749d6f26e852bfcf22d35bf17d099d03641f33dae0b39140d21fa491e7` | 5,869 | 5,950 | constructor normalization, freeze/projection and invocation enforcement |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `35506442b4bec593be917eec16e44182005e5c57dd9e40cef838b8795649be5f` | 6,005 | 6,175 | ABI, identity, enforcement and source-prefix proof |

Production additions are capped at 95, proof additions at 160 and total
additions at 255. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. `package.rs` and the proof file exceed the
2,000-line trigger, but the schema plumbing belongs in the existing declaration
and target-call owners and the recursive loading proof belongs beside adjacent
rules_rust tests; splitting would create duplicate owners and widen the packet.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused integer allowed-value ABI/identity/enforcement/source-prefix proof;
- existing data-doc, stdlib filegroup, Rust analyzer and rules_cc proofs;
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
Zabel's guidance-only role, set normalization, semantic identity, all candidate
enforcement, fail-closed projections, the string-values stop, compatibility
classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; a constraint
other than integer values; a retained evaluator value or new registry; silent
constraint loss in another projection; configured-analysis enforcement;
completion of `rust_toolchain`; DICE/analysis/repository/source changes;
Java/JVM work; copied Zabel code or behavior; cap violation; or a claim beyond
reaching the first string allowed-value constraint. Audit
`attr.string(values=...)` separately after this prefix loads.
