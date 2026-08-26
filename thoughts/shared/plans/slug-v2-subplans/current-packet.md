# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-data-attribute-doc-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute constructors
Base: `75709828`

Result: accept loading-only string/`None` documentation on `attr.int`,
`attr.string_list`, `attr.string_dict`, and `attr.string_list_dict`, advancing
rules_rust's `rust_toolchain` declaration to its first allowed-value
constraint. Do not retain documentation as semantic identity.

## Accepted starting point and source-order stop

Commit `75709828` retains Bazel's Boolean `allow_files` predicate through the
existing transient, frozen and package-owned label-list schemas. Omitted,
explicit `None` and false normalize to no-file; true normalizes to any-file.
The source-shaped `rust_stdlib_filegroup` freezes and projects its mandatory,
non-single-artifact `srcs` schema without admitting a file target or running
its implementation. All 209 loading tests, configured analysis, locked checks,
rebuilt CLI and hygiene pass at 37 production and 84 proof additions.
Independent terminal review returned `ACCEPT`.

Source order next begins `rust_toolchain = rule(...)` at line 664 of
rules_rust 0.73 `rust/private/toolchain.bzl`. Its implementation remains lazy.
The allocator, binary, cargo, channel and clippy descriptors use admitted
label/string/default/cfg/single-file shapes. The first absent evaluated
argument is line 695:

```starlark
"debug_info": attr.string_dict(
    doc = "Rustc debug info levels per opt level",
    default = {"dbg": "2", "fastbuild": "0", "opt": "0"},
),
```

The later `env`, `extra_*_rustc_flags`, and
`extra_rustc_flags_for_crate_types` descriptors use the same documentation ABI
on string-dict, string-list and string-list-dict values. The first distinct
semantic stop is line 727: `attr.int(values = [-1, 0, 1], ...)`. Do not admit
that allowed-value constraint or claim the full `rust_toolchain` rule freezes.

## Fixed sources and compatibility authority

The selected rules_rust file has SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.
Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StarlarkAttrModuleApi` declares `doc` as named, default `None`, and typed
string-or-None on these constructors. `createAttrDescriptor` carries it for
documentation extraction, while rule attribute semantics and target values do
not depend on its text. Existing Slug label, label-list, bool and string
constructors already validate string/`None` with `discard_attribute_doc` and
do not retain it.

This packet reproduces the accepted invocation/type behavior and semantic
non-identity only. Starlark documentation extraction remains unsupported.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural/test guidance only. Its generic attribute dispatch validates
that `doc` is string or `None` and then discards it from `AttrDefinition`,
keeping declaration semantics independent of documentation text. Slug reuses
its existing Rust helper and follows that transient boundary. No Zig code,
layout, dispatch, diagnostic or behavior is copied; Bazel remains compatibility
authority.

No retained representation changes. The Buck2 utility audit therefore selects
the existing borrowed Starlark value validation path and adds no collection,
allocation, clone, interner, hash, cache, memory-accounting owner or Stage 9
ledger row.

## Compatibility classification

- **Exact:** the four selected `.bzl` attribute constructors accept omitted,
  explicit `None`, and string `doc`; non-string/non-`None` values reject; the
  documentation text does not affect frozen rule or package schema equality;
  the rules_rust-shaped declaration prefix reaches the first `values`
  constraint without invoking its implementation.
- **Slug-native:** Rust/Starlark diagnostic rendering and non-retention of doc
  text in the admitted semantic schema.
- **Unsupported/deferred:** documentation extraction; `doc` on unselected
  attribute kinds; `values` constraints and target-value enforcement;
  completion or invocation of `rust_toolchain`; file resolution,
  `config_common.toolchain_type`, M8, M7B and exact output bytes.

## Ownership and implementation boundary

Add only a named `doc: Option<Value>` argument plus
`discard_attribute_doc(doc)?` to the existing int, string-list, string-dict and
string-list-dict methods in `attr_methods`. Do not add fields to
`AttributeDefinitionGen`, `RuleAttributeSchemaGen`, `AttributeSchema` or any
target/package value. Do not introduce a second helper or documentation store.

There is no DICE, request, command, analysis, async, mapping, repository,
publication, cancellation or shutdown change.

## Discriminating proof

- For each selected constructor, freeze otherwise-identical rules with omitted,
  explicit `None`, and distinct string documentation; prove their retained
  schemas are equal and typed defaults remain unchanged.
- Prove integer, list, dictionary and provider-like documentation values reject.
- Evaluate a rules_rust-shaped `rust_toolchain` prefix through `debug_info`,
  `env`, string-list and string-list-dict descriptors while keeping the
  implementation lazy.
- Prove `attr.int(values = [-1, 0, 1])` remains rejected and marks the next
  source-order stop.
- Keep the stdlib filegroup, Rust analyzer, rules_cc wrapper and configured
  analysis regressions green.

## Allowlist and caps

Only these files may change from base `75709828`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `0fa9b13ebba97b6c4b8c6911e529dba608aad8af5377c989f4ffa1ed4fe103e7` | 5,861 | 5,875 | selected doc arguments and validation calls |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `069511a9cb48f17d96787c6670f081c30f66edd5ca47aee5a0c3149f84e03ea5` | 5,944 | 6,030 | ABI matrix, identity and source-prefix proof |

Production additions are capped at 12, proof additions at 80 and total
additions at 92. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. Both files exceed the 2,000-line trigger, but
the ABI belongs in the single existing constructor table and its recursive
loading proof belongs beside the adjacent rules_rust tests; splitting would
create a second owner and widen the allowlist.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused data-attribute documentation/source-prefix proof;
- existing stdlib filegroup, Rust analyzer and rules_cc wrapper proofs;
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
Zabel's guidance-only role, documentation non-identity, wrong-type rejection,
the `values` stop, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; retained doc
text or documentation extraction; `values` support or target-value validation;
another attribute kind; rule/toolchain completion; a retained representation,
collection or registry; DICE/analysis/repository/source changes; Java/JVM work;
copied Zabel code or behavior; cap violation; or a claim beyond reaching the
first allowed-value constraint. Audit `attr.int(values=...)` separately after
this prefix loads.
