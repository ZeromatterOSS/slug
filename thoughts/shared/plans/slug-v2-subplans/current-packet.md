# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-string-allowed-values-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute declaration and package-schema owners
Base: `563699ab`

Result: generalize the retained integer constraint into one typed allowed-value
owner and admit Bazel string allowed values, advancing rules_rust's
`rust_toolchain` declaration through `linker_preference` and `linker_type` to
its first label `allow_files` argument. Keep the implementation lazy.

## Accepted starting point and source-order stop

Commit `563699ab` normalizes the selected signed-32-bit integer allowed-value
subset into one immutable `Arc<[i32]>`, retains it through transient, frozen
and package schemas, and rejects disallowed explicit/plain-select candidates
before target recording. Omitted and empty sequences are unconstrained;
ordinary omitted defaults remain unchecked. Repository-rule and tag-class
projections reject nonempty constraints instead of dropping them. All 212
loading tests and downstream gates pass; independent terminal review returned
`ACCEPT` within 73 production, 160 proof and 233 total additions.

Source order now reaches these first absent evaluated rows in rules_rust 0.73
`rust/private/toolchain.bzl`:

```starlark
"linker_preference": attr.string(
    doc = "The preferred linker to use. ...",
    values = ["cc", "rust"],
),
"linker_type": attr.string(
    doc = "The type of linker invocation: ...",
    values = ["direct", "indirect"],
),
```

After these rows, `llvm_cov` uses accepted label shapes. The next distinct
stop is `llvm_lib` at lines 779-782, whose
`attr.label(allow_files = True, cfg = "exec")` is unadmitted. Do not admit
label file allowance or claim the full `rust_toolchain` rule freezes.

## Fixed sources and compatibility authority

The selected rules_rust file has SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.
Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.

`StarlarkAttrModuleApi.stringAttribute` defines named-only `values`, defaults
it to an empty sequence, and restricts elements to strings.
`StarlarkAttrModule.stringAttribute` forwards that sequence and the shared
`buildAttribute` path installs `Attribute.AllowedValueSet` only when nonempty.
`AttributeProvider.checkAllowedValues` checks every possible explicitly
supplied/configurable candidate. `ConfigurableAttributesTest` proves allowed
and disallowed direct selectors, multiple branch failures, and concatenated
string candidates such as `"on" + select(...)`; ordinary rule defaults remain
unchecked by this path.

This packet reproduces the typed constructor ABI, empty/no-constraint
normalization, retained set identity, and explicit selectable/concatenated
candidate enforcement. Exact Bazel diagnostic wording and set iteration order
are not claimed.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural/test guidance only. Its single `allowed_values` optional
field is shared by string and integer descriptors and stays beside the
declaration-owned default before package capture. Slug follows that unified
ownership boundary with evaluator-free Rust values; it does not copy Zabel's
evaluator value, optional-mask layout, Zig code, diagnostics or behavior.
Bazel remains compatibility authority.

The Buck2 utility audit selects one small typed enum containing the existing
immutable `Arc<[i32]>` or a new `Arc<[CompactString]>`, all covered by
`Allocative`. Each set is sorted and deduplicated once at declaration capture;
schema clones remain reference bumps. Existing request-local configurable
candidate expansion is reused for string concatenation and selector
correlation. No map, hasher, interner, cache, new utility import or Stage 9
ledger row is needed.

## Compatibility classification

- **Exact:** `attr.string(values=...)` accepts list/tuple string sequences,
  rejects non-sequences and non-string elements, treats omitted and empty
  sequences as no constraint, retains every nonempty set, rejects disallowed
  explicit plain-selector and concatenated-selector candidates, and does not
  reject an ordinary rule's unselected default. The rules_rust-shaped prefix
  crosses both selected string constraints without invoking the implementation.
- **Slug-native:** the evaluator-free integer/string enum, sorted/deduplicated
  `Arc` slice semantic identity, valid-Unicode string boundary, and Rust
  diagnostic wording/rendering.
- **Unsupported/deferred:** other constraint kinds; integer constraint literals
  outside signed 32-bit range; exact allowed-value error text/order; configured
  analysis revalidation; repository-rule or module-extension-tag constrained
  attrs; label `allow_files`; completion/invocation of `rust_toolchain`; file
  resolution, `config_common.toolchain_type`, M8, M7B and exact output bytes.

## Ownership and implementation boundary

Replace the integer-only field in `AttributeDefinitionGen`,
`RuleAttributeSchemaGen`, and package-owned `AttributeSchema` with one
`AllowedAttributeValues` enum: `None`, `Integer(Arc<[i32]>)`, or
`String(Arc<[CompactString]>)`. Normalize each family at its existing attr
adapter, carry the enum through freeze and schema projection, expose only a
borrowed crate-local getter, and dispatch package-time explicit-value checking
by the enum variant.

Reuse `CoercedAttributeValue::attr_visible_candidates` for constrained strings
so selector correlation and concatenation stay in their existing loading/query
owner. Omitted values keep their existing defaults without validation.
Repository-rule and tag-class projections must reject every non-`None`
constraint. Built-ins and all other descriptors remain `None`.

There is no DICE, request, command, analysis, async, mapping, repository,
publication, cancellation or shutdown change.

## Discriminating proof

- Accept omitted, empty list/tuple and string list/tuple `values`; reject
  `None`, scalars, mixed/wrong-type elements and dictionaries.
- Prove order/duplicates normalize to the same retained set, distinct string
  sets remain unequal, and the earlier integer schema preserves its identity.
- Prove allowed direct, plain-select and concatenated-select string values
  record, while disallowed direct, nondefault/default select branches and
  concatenated results reject before target recording.
- Prove an omitted disallowed string default still records.
- Freeze both rules_rust linker constraints and prove
  `attr.label(allow_files=True)` remains rejected as the next source stop.
- Prove repository-rule and tag-class projections fail closed for string
  constraints; keep all prior integer/docs/stdlib/analyzer/rules_cc and one
  configured-analysis regression green.

## Allowlist and caps

Only these files may change from base `563699ab`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/attrs.rs` | `84901f806043770cb89422c194596706c3cd76f8a4df1a42e9d5ce994c16d2e6` | 1,493 | 1,535 | unified typed constraint and package-schema projection |
| `app/slug_loading_v2/src/package.rs` | `5842ea79f737888db259bf1b998f8631186ef6bdb645743ac44eebbde5095bc7` | 5,931 | 6,035 | typed normalization, freeze/projection and candidate enforcement |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `13abce1313a83bc3ab70a4fc8a10918c041b2b2351fc4b52afc8c899f994855e` | 6,164 | 6,355 | ABI, identity, enforcement and source-prefix proof |

Production additions are capped at 120, proof additions at 180 and total
additions at 300. Deletions do not buy addition budget. No new function may
exceed 120 lines. The pre-existing `FrozenRuleDefinition::invoke` remains the
sole grandfathered larger owner and may not grow beyond its accepted integer
packet size; this packet may only replace its projection/check calls with the
typed enum equivalents. `package.rs` and the proof file exceed the 2,000-line
trigger, but the schema plumbing belongs in the existing declaration and
target-call owners and the recursive loading proof belongs beside adjacent
rules_rust tests; splitting would duplicate owners and widen the packet.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused string allowed-value ABI/identity/enforcement/source-prefix proof;
- existing integer-values, docs, stdlib, Rust analyzer and rules_cc proofs;
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
Zabel's guidance-only role, the unified compact representation, set identity,
default/select/concatenation behavior, fail-closed projections, the label-file
stop, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; another
constraint family; a retained evaluator value or parallel fields/registry;
silent constraint loss; configured-analysis enforcement; label `allow_files`;
completion of `rust_toolchain`; DICE/analysis/repository/source changes;
Java/JVM work; copied Zabel code or behavior; cap violation; growth of the
grandfathered invocation owner; or a claim beyond reaching the first label
file-allowance argument. Audit label `allow_files` separately after this prefix
loads.
