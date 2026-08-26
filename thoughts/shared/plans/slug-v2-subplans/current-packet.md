# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-label-allow-files-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute declaration and package-schema owners
Base: `80425ce9`

Result: admit the Boolean/`None` subset of Bazel scalar-label
`allow_files`, reuse the existing file-allowance schema fact, and advance
rules_rust's `rust_toolchain` declaration through `llvm_lib` and `llvm_tools`
to its first scalar-label provider predicate. Keep the implementation lazy.

## Accepted starting point and source-order stop

Commit `80425ce9` unifies integer and string allowed values in one immutable,
evaluator-free schema enum. String sets normalize at declaration capture and
explicit direct, selectable and concatenated final candidates are checked
before target recording; ordinary defaults remain unchecked. All 213 loading
tests and downstream gates pass, with independent terminal `ACCEPT` at 77
production, 165 proof and 242 total additions.

Source order now reaches these absent rows in rules_rust 0.73
`rust/private/toolchain.bzl`:

```starlark
"llvm_lib": attr.label(
    doc = "The location of the `libLLVM` shared object files. ...",
    allow_files = True,
    cfg = "exec",
),
...
"llvm_tools": attr.label(
    doc = "LLVM tools that are shipped with the Rust toolchain.",
    allow_files = True,
),
```

Between them, `llvm_profdata` uses accepted scalar-label
`allow_single_file=True`. The next distinct stop is `lto`, whose
`attr.label(providers = [RustLtoInfo], ...)` predicate is unadmitted. Do not
admit scalar-label providers or claim the full `rust_toolchain` rule freezes.

## Fixed sources and compatibility authority

Selected rules_rust source:
`/tmp/slug-rules-rust-registry.MZNsRA/source/rust/private/toolchain.bzl`,
SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.
Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.

`StarlarkAttrModuleApi.labelAttribute` types `allow_files` as Boolean,
string sequence or `None`, with `None` as its default. The shared
`buildAttribute` path uses non-`None` presence, rejects simultaneous non-None
`allow_files` and `allow_single_file`, and maps Boolean true to
`FileTypeSet.ANY_FILE` and false to `NO_FILE`. With neither argument, a
dependency label also receives `NO_FILE`. Only `allow_single_file` sets
`SINGLE_ARTIFACT`. `StarlarkRuleClassFunctionsTest.testAttrWithList` and
`testAttrSingleFileWithList` pin the distinction between the allowed-file
predicate and single-artifact identity.

This packet reproduces only the Boolean/`None` constructor subset and its
retained schema projection. Extension-sequence predicates and actual file
target resolution remain deferred.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural/test guidance only. Its declaration owner keeps
`allows_files` separate from `allows_single_file`, checks simultaneous
non-`None` arguments before normalization, and projects the same fact into
the captured schema. Slug follows that ownership and presence-check boundary
using its existing Rust Boolean; it copies no Zig code, layout, diagnostics or
behavior. Bazel remains compatibility authority.

The Buck2 utility audit selects the existing inline `allow_files: bool` in
already `Allocative` schemas. There is no new collection, allocation, hash,
interner, cache, utility import or Stage 9 ledger row.

## Compatibility classification

- **Exact:** the admitted scalar-label subset accepts omitted, `None`, false
  and true; normalizes omitted/`None`/false to no allowed files; maps true to
  any-file allowance without single-artifact identity; rejects simultaneous
  non-`None` `allow_files` and `allow_single_file`; retains the Boolean through
  freeze and package schema; and crosses both rules_rust rows without invoking
  the rule implementation.
- **Slug-native:** one inline Rust Boolean as the retained semantic projection,
  plus Rust diagnostic wording/order.
- **Unsupported/deferred:** string-sequence extension predicates for
  `allow_files`; exact diagnostics; actual source/filegroup validation and
  configured file resolution; scalar-label `providers`, `allow_rules`,
  aspects and materializers; repository/tag use of file-allowing attrs;
  completion/invocation of `rust_toolchain`; M8, M7B and exact output bytes.

## Ownership and implementation boundary

Add named `allow_files` only to the existing scalar `attr.label` adapter.
Before normalization, reject when it and `allow_single_file` are both
explicitly non-`None`. Parse the Boolean/`None` subset with the existing
`unpack_boolean_allow_files`, then store it in the existing
`AttributeDefinitionGen::allow_files` field. Existing freeze, target-schema,
repository-rule and tag-class owners already retain or reject that fact and
must not be duplicated.

There is no new representation and no DICE, request, command, analysis,
async, mapping, repository, publication, cancellation or shutdown change.

## Discriminating proof

- Accept omitted, explicit `None`, false and true scalar-label `allow_files`;
  reject scalar and mapping wrong types and retain extension sequences as an
  explicit unsupported boundary.
- Prove omitted/`None`/false normalize equally, true is structurally distinct,
  true remains separate from `allow_single_file=True`, and every simultaneous
  non-`None` pair rejects even when either Boolean is false.
- Freeze the source-shaped `llvm_lib`, `llvm_profdata` and `llvm_tools` rows;
  prove both selected rows retain file allowance and the following
  `providers=[RustLtoInfo]` row remains rejected.
- Project true into a loaded package target schema without single-artifact
  identity and prove repository-rule/tag-class projections fail closed.
- Keep the prior label-list file allowance, string/integer values, docs,
  stdlib, Rust analyzer and rules_cc proofs plus one configured-analysis
  regression green.

## Allowlist and caps

Only these files may change from base `80425ce9`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `76a67c738567596ac4e673793a02a56d487fd4bf4eb2b9c0b3db6513f61bfc9f` | 5,975 | 5,995 | scalar-label ABI, conflict check and existing Boolean projection |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `e84b03b4c43b27785d96fa7155760e368aefae2d7665ee6708a39b480c28c2c8` | 6,314 | 6,435 | ABI, identity, fail-closed and source-prefix proof |

Production additions are capped at 20, proof additions at 120 and total
additions at 140. Deletions do not buy addition budget. No new function may
exceed 120 lines. `FrozenRuleDefinition::invoke` may not change. The files
exceed the 2,000-line trigger, but the adapter belongs with adjacent `attr.*`
constructors and the recursive proof belongs beside adjacent rules_rust tests;
splitting would duplicate owners and widen the packet.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused scalar-label file-allowance ABI/identity/source-prefix proof;
- existing label-list, allowed-values, docs, stdlib, analyzer and rules_cc
  proofs;
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
Zabel's guidance-only role, the existing Boolean representation, presence
conflict, file/single-file distinction, fail-closed projections, the provider
stop, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; extension
predicates; another scalar-label argument; a new retained representation;
silent file-allowance loss; configured file enforcement; actual file target
resolution; completion of `rust_toolchain`; DICE/analysis/repository/source
changes; Java/JVM work; copied Zabel code or behavior; cap violation; or a
claim beyond reaching the first scalar-label provider predicate. Audit
`providers=[RustLtoInfo]` separately after this prefix loads.
