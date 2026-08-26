# Current Slug V2 Packet

Packet: `WP-4-7A-imported-frozen-transition-clippy-tail-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit one imported frozen Starlark transition as an attribute `cfg`
value, then prove exact rules_rust `clippy.bzl:463-596` freezes with its
producer-owned lint/provider/common imports. Stop when this module returns.

## Accepted starting point and observed stop

Base is `f4cfaacb3` (`Select clippy test tail loading proof`). The proof-only
packet evaluated the exact tail with exact `lint_test.bzl` exports and stopped
at `clippy.bzl:502`:

```text
error: attr.label cfg must be 'exec' or a transition
```

The value is the frozen `platform_transition` imported from
`lint_test.bzl:37-41`, not an invalid value. Slug's
`attribute_definition` recognizes same-module transient
`TransitionDefinition` values but explicitly discards the frozen half returned
by `TransitionDefinition::from_value`. The rejected 246-line test candidate
was removed; `host_package_load_tests.rs` is restored to its accepted SHA.

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
`StarlarkAttrModule.convertCfg` accepts a Starlark-defined transition value;
`ConfigGlobalLibrary.transition` binds that value to its defining `.bzl`
context before a consumer imports it. Module freeze does not make an otherwise
valid imported transition invalid.

Extend only the existing `attribute_definition` conversion. For a frozen
transition, construct the existing transient generic wrapper from the frozen
implementation value and cloned compact output label, so the normal attribute
and rule freeze path retains the same two fields. Do not add identity,
registry, cache, DICE, configured-transition or invocation behavior. Invalid
values and the special `"exec"` path stay unchanged.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its `TransitionDefinition` keeps implementation, publication owner and
definition-module identity with the producer; detached capture later projects
those fields rather than rebinding the transition to the consumer. This guides
preserving producer-owned contents across import. Copy no Zig code,
representation, identity bytes, ordinal, capture, configured behavior,
algorithm or diagnostic. Bazel 9.2 decides compatibility.

The Buck2 utility review selects existing representations: one cloned
`CompactString` and `FrozenValue::to_value` projection into the already-owned
generic transition wrapper. No new collection, interner, cache, hash owner,
clone-heavy graph, memory ledger or Stage 9 entry.

## Compatibility

- **Exact:** an imported frozen transition remains a valid `cfg` transition;
  its implementation value and canonical output label survive into the frozen
  rule attribute; exact `clippy.bzl:463-596` source-order freeze and the
  authenticated provider/helper/attribute/aspect/build-setting shapes.
- **Slug-native:** reconstruction of the existing Rust generic transient
  wrapper from frozen contents, Arc/value ownership and fail-closed diagnostics.
- **Unsupported/deferred:** observable transition object equality/hash or
  Bazel internal identity bytes; transition evaluation and configuration
  hashing; helper/rule/aspect execution; configured provider matching, aspect
  application, test runner/actions/runfiles; build-setting configured values
  and CLI flags; output-group values; and the parent frontier after clippy
  returns.

## Allowlist, proof and caps

Only these files may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/package.rs` | `1f3d7d6e317154954b9b09e22b5d841ca118ecb9df1f6dbcef049547ebd6e4c8` | 6,187 | 6,207 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `9a72f95b3a2889bd837d5f80a9827a55e7ad3a2c7e5f781ffb02f20363e28774` | 7,135 | 7,395 |

Caps are 20 production, 260 proof and 280 total additions; deletions do not buy
addition budget. Keep each function at or below 120 lines; exact source
constants may exceed that limit.

Required proof:

1. Recursively freeze an exact lint child and use its imported frozen
   `platform_transition` as the `cfg` of the clippy test target label-list.
   After package-schema projection, prove `TransitionDefinition::implementation()`
   is pointer-identical to the lint child's `_platform_transition_impl` export
   and output `//command_line_option:platforms` survives. Preserve and prove the
   already-accepted same-module transition path and invalid-value failure.
2. Evaluate the accepted clippy prefix plus exact unabridged
   `clippy.bzl:463-596`, retaining exact lint/provider/common producer
   identities. Preserve both lazy provider-constructor helpers without
   executing them.
3. Prove all four lint imports and the three provider imports used by the
   prefix/tail are pointer-identical to child exports. Prove exact output-list,
   test provider, required/advertised aspect, merged rule, attached aspect,
   provider alternatives, transition and two true Boolean settings.
4. Preserve every accepted clippy, rustfmt, lint-test and native-global proof.

No new oracle is needed: the pinned Bazel and rules_rust source plus the exact
recursive test discriminate the missing imported-frozen branch.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused imported-transition/clippy-tail proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify the observed frozen-value stop, narrow
conversion, source/producer proof, same-module preservation, compatibility
boundary, Zabel guidance-only role, utility decision, validation and caps.

STOP and `REPLAN` for another production owner; identity/registry/DICE or
configured-transition work; transition execution; helper execution;
constructed native providers; configured provider/aspect/test/build-setting/
action semantics; Java/JVM work; copied Zabel content; dirty authority; skipped
source order; or cap violation.

## Immediate predecessor

`f4cfaacb3` selected a proof-only tail closure. Its exact recursive attempt
exposed this narrower imported-frozen-transition prerequisite and left no code
changes.
