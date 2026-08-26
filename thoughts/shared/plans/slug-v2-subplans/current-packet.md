# Current Slug V2 Packet

Packet: `WP-4-7A-imported-frozen-attribute-transition-clippy-tail-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: project plain imported frozen `attr.*` definitions and an imported
frozen Starlark transition into Slug's existing transient rule-schema types,
then prove exact rules_rust `clippy.bzl:463-596` freezes. Stop when this module
returns.

## Accepted starting point and second observed stop

Base is `8873ca136` (`Correct clippy tail imported transition`). The selected
implementation first admitted the exact frozen `platform_transition` and then
advanced to the next value in `dict(LINT_TEST_COMMON_ATTRS, **overlay)`:

```text
error: rule attribute `platform` must use attr.*()
```

`platform` is the first child-owned frozen `AttributeDefinition` in exact
`lint_test.bzl:45-62`. Slug's `rule` adapter recognizes only the transient half
of `AttributeDefinition::from_value`, just as its attribute `cfg` conversion
recognized only transient transitions. The complete 9-production/248-proof
candidate was removed; both Rust files are restored to their accepted hashes.

The authenticated rules_rust 0.73.0 sources remain:

- `clippy.bzl`, SHA-256
  `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`;
- `lint_test.bzl`, SHA-256
  `4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`;
- `providers.bzl`, SHA-256
  `57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`;
- `common.bzl`, SHA-256
  `cee50122624c7fd9c9a6545a647062f350dd25bc8cf6dda873944290463d4db6`.

## Authorities and decision

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StarlarkAttrModule` produces immutable descriptor values, `dict(base,
**overlay)` retains child values in base order, and `rule(attrs=...)` accepts
those imported descriptors. `ConfigGlobalLibrary.transition` binds a
transition to its defining `.bzl` context; module freeze does not invalidate
either declaration kind.

Add two bounded conversion branches in the existing loading owner:

1. Reconstruct an imported frozen transition from its frozen implementation
   value and compact output label before storing it in a transient attribute.
2. Reconstruct an imported frozen plain attribute definition from its existing
   scalar/compact/default fields only when required-provider, attached-aspect
   and nested-transition fields are empty. The exact four lint common attrs
   meet this boundary. Rich imported attribute declarations still fail closed;
   the transient target overlay continues through its already-admitted path.

Do not alter label coercion, provider identity, attribute ordering, freeze
layout, invocation, DICE, or configured behavior. Add a `#[cfg(test)]`
implementation getter beside the existing frozen-transition output getter only
to discriminate retained pointer identity; it changes no production surface.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its `AttrDefinition.publication_owner` and `TransitionDefinition` publication
owner/definition-module identity keep declarations with their producer through
module freeze, while detached consumers project them later. This guides
preserving child-owned contents rather than importer rebinding. Copy no Zig
code, representation, owner pointer, identity bytes, ordinal, capture,
configured behavior, algorithm or diagnostic. Bazel 9.2 decides compatibility.

The Buck2 utility review selects existing representations. Frozen plain
attributes reuse scalar fields and cloned Arc/compact/default values once per
loading declaration; transition conversion reuses `FrozenValue::to_value` and
one `CompactString` clone. No new collection, interner, cache, hash owner,
graph storage, clone-heavy request path, memory ledger or Stage 9 entry.

## Compatibility

- **Exact:** imported frozen plain attribute definitions remain valid rule
  descriptors with their complete admitted fields; imported frozen transition
  implementation/output survive into the frozen rule schema; exact
  `clippy.bzl:463-596` source-order freeze and its authenticated producer,
  provider, aspect, attribute and build-setting shapes.
- **Slug-native:** reconstruction into existing Rust generic transient wrappers,
  Arc/value ownership, the fail-closed rich-frozen-attribute boundary and
  diagnostics.
- **Unsupported/deferred:** imported frozen attrs with providers, aspects or a
  nested transition; observable transition/attribute equality or Bazel
  identity bytes; transition evaluation/configuration hashing; helper/rule/
  aspect execution; configured provider/aspect/test/build-setting/action
  behavior; output-group values; and the parent frontier after clippy returns.

## Allowlist, proof and caps

Only these files may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/package.rs` | `1f3d7d6e317154954b9b09e22b5d841ca118ecb9df1f6dbcef049547ebd6e4c8` | 6,187 | 6,242 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `9a72f95b3a2889bd837d5f80a9827a55e7ad3a2c7e5f781ffb02f20363e28774` | 7,135 | 7,395 |

Caps are 55 production, 260 proof and 315 total additions; deletions do not buy
addition budget. Keep each function at or below 120 lines; exact source
constants may exceed that limit.

Required proof:

1. Recursively freeze the exact lint child. Prove its four imported plain
   attributes retain names/order, kinds, defaults, executable/exec policy and
   defining-repository label identities in `rust_clippy_test`.
2. Use its imported frozen `platform_transition` as `targets.cfg`. Through the
   frozen rule schema, prove the retained implementation is pointer-identical
   to the child `_platform_transition_impl` export and the output remains
   `//command_line_option:platforms`.
3. Evaluate the accepted clippy prefix plus exact unabridged
   `clippy.bzl:463-596`, retaining exact lint/provider/common producer
   identities. Preserve both lazy provider-constructor helpers without
   executing them.
4. Prove all four lint imports and the three provider imports used by the
   prefix/tail are pointer-identical to child exports. Prove exact output list,
   test provider, required/advertised aspect, merged rule, attached aspect,
   provider alternatives and two true Boolean settings.
5. Preserve same-module attr/transition behavior. Reject a frozen imported
   attribute containing a provider, attached aspect or nested transition, and
   preserve invalid transition failure. Preserve every accepted clippy,
   rustfmt, lint-test and native-global proof.

No new oracle is needed: pinned Bazel/rules_rust source and the exact recursive
test discriminate both missing frozen-value branches.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused imported-attribute/transition clippy-tail proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify both observed frozen-value stops,
narrow plain-attribute/transition conversion, source/producer proof,
same-module and rich-frozen failure preservation, compatibility boundary,
Zabel guidance-only role, utility decision, validation and caps.

STOP and `REPLAN` for rich imported attrs; another production owner; identity/
registry/DICE or configured-transition work; transition/helper execution;
constructed native providers; configured provider/aspect/test/build-setting/
action semantics; Java/JVM work; copied Zabel content; dirty authority; skipped
source order; or cap violation.

## Immediate predecessor

`8873ca136` selected imported frozen-transition loading. Its exact recursive
attempt exposed the immediately following frozen common-attribute prerequisite
and left no code changes.
