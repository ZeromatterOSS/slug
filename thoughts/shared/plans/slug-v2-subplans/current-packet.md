# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-common-toolchain-type-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit the final source-required typed toolchain requirement, preserve
its optionality structurally, and complete top-level evaluation of
`rust/private/toolchain.bzl` without invoking `rust_toolchain`.

## Accepted starting point and source-order stop

Implementation base is `ef910068` (`Load scalar label provider predicates`).
It freezes the complete `rust_toolchain` attribute map, including both
singleton scalar provider predicates. The next unadmitted expression is:

```starlark
toolchains = [
    config_common.toolchain_type(
        "@bazel_tools//tools/cpp:toolchain_type",
        mandatory = False,
    ),
]
```

Only a documentation string follows before the rule and the 1,002-line child
end. This packet completes that child's top-level evaluation, then stops for a
fresh caller/source audit. The implementation function remains lazy.

## Fixed sources and compatibility authority

Selected rules_rust source:
`/tmp/slug-rules-rust-registry.MZNsRA/source/rust/private/toolchain.bzl`,
SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Fixed anchors are
`ConfigStarlarkCommonApi.toolchain_type`,
`ConfigStarlarkCommon.toolchainType`,
`ToolchainTypeRequirement`, `StarlarkRuleClassFunctions.parseToolchainTypes`
and `StarlarkRuleClassFunctionsTest.testRuleAddToolchain` plus its duplicate
test. They establish String/Label input, defining-thread label conversion,
default/explicit mandatory state, typed rule-list input, stable distinct order
and strictest-wins duplicate normalization.

Architectural guidance is clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a`. Its
`build_rule_declaration.zig` owns a typed label-plus-mandatory requirement on
the rule declaration, while `build_invocation_capture.zig` detaches canonical
label identity and mandatory state for later consumers. Slug follows that
ownership boundary with its own Rust types. No Zig code, layout, evaluator
value, diagnostic or configured algorithm is copied; Bazel alone defines
behavior.

The Buck2 utility audit selects the existing `CanonicalLabel`, inline Boolean,
`Arc<[T]>` and `Allocative` patterns. No collection, interner, hash, cache,
memory ledger or imported utility is warranted.

## Compatibility classification

- **Exact:** `.bzl` `config_common.toolchain_type` accepts String or existing
  Label input and named Boolean `mandatory` defaulting true; the returned typed
  loading value exposes canonical label and mandatory state; `rule(toolchains)`
  accepts existing String inputs as mandatory plus Label and typed requirement
  inputs, retains distinct entries in order, and freezes the source singleton
  optional C++ requirement.
- **Slug-native:** evaluator-backed labels detach into Slug's valid-Unicode
  canonical Rust identity and compact immutable storage.
- **Unsupported/deferred:** duplicate toolchain labels reject instead of
  strictest merging; typed aspect toolchains, other `config_common` members,
  optional configured resolution, optional target invocation, full
  `rust_toolchain` analysis/actions and the caller beyond the completed child.
  Existing accepted unique mandatory string rule requirements stay exact.

## Implementation boundary

Add one small Starlark-visible requirement value and one evaluator-free public
Rust requirement record containing `CanonicalLabel` and `mandatory`. Use the
record through transient/frozen rule definitions and
`StarlarkRuleImplementation`; it participates in equality and allocation
accounting. Parse rule toolchain lists as values: existing strings and Labels
become mandatory requirements, typed values retain their bit, and duplicate
canonical labels fail closed.

Expose only `config_common.toolchain_type` in `.bzl` loading globals. Resolve
String input with the existing defining-call label owner; preserve an existing
Label's identity. Do not alter aspect parsing. Reject any optional rule
requirement in `FrozenRuleDefinition::invoke` before target publication; the
configured analysis path therefore continues to receive mandatory
requirements only and must read their label explicitly. Do not change DICE,
source loading, repository mapping, toolchain selection or action behavior.

## Discriminating proof

- Prove String and Label inputs, relative/apparent mapping, default/explicit
  true and false mandatory state, field visibility, wrong types and BUILD
  non-callability for the selected declaration surface.
- Prove rule lists retain distinct String, Label and typed entries in source
  order, preserve mandatory identity through freeze, distinguish false from
  true, and reject duplicate canonical labels and unrelated values.
- Freeze the complete source-shaped `rust_toolchain` with the canonical
  `@@bazel_tools//tools/cpp:toolchain_type` optional requirement and prove no
  later top-level expression remains.
- Prove optional rule invocation rejects before target recording while the
  existing mandatory string rules and configured single-toolchain regression
  remain green.
- Keep scalar/list provider/aspect, file-allowance, allowed-values, docs,
  stdlib, analyzer and rules_cc proofs green.

## Allowlist and caps

Only these files may change from base `ef910068`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `59ea0ebeff912192aefe8207c6801f58053e4ae95326aa88398815a7018d14f2` | 6,004 | 6,185 | typed namespace, retained record, parsing and preflight |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `dbad32c210da6762a28bd1030d54058ffeec2b16771822eb1b9a832d449b77b4` | 6,485 | 6,675 | ABI, identity, rejection and complete-child proof |
| `app/slug_analysis_v2/src/dice.rs` | `a62a6c3b6541641905fb4cdf4a8a708528c2d690e760873d8e5d23d6d0936c5e` | 2,959 | 2,990 | explicit label access on mandatory-only consumer |
| `app/slug_loading_v2/tests/bzl_invalidation.rs` | `eff0715413b5f543d81d7a1692cb4c64fc21be744b5b283109f05de5ed83328f` | 2,296 | 2,320 | retained-requirement API adaptation |
| `app/slug_loading_v2/tests/build_file_loading.rs` | `7ac52bf85dca12409209008d85b3713c7847e0fd23697789ce13947928bcc995` | 3,144 | 3,180 | retained-requirement API adaptation |

Production additions are capped at 180, proof additions at 220 and total
additions at 400. Deletions do not buy addition budget. No new function may
exceed 120 lines. `FrozenRuleDefinition::invoke` may gain only the optional
preflight; package publication order may not otherwise change. The large files
keep the declaration and consumer beside their existing owners.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused config-common ABI/identity/rejection/complete-child proofs;
- existing rule toolchain, provider/aspect, file-allowance, allowed-values,
  docs, stdlib, analyzer and rules_cc proofs;
- one configured mandatory-toolchain analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation`;
- `cargo test -p slug_loading_v2 --test build_file_loading`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check` and `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive integration remains 30/31 only for its known stale
`@external` diagnostic-order row and need not rerun absent new integration
risk. Recheck allowlist, caps, base/source hashes, both clean authority pins and
stale `slugd` cleanup.

Independent selection and terminal reviews must verify Bazel authority,
Zabel's guidance-only role, retained mandatory identity, optional preflight,
consumer adaptation, completed-child stop, classifications and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; silent loss
of mandatory state; duplicate approximation; typed aspect requirements; an
additional `config_common` member; configured optional resolution; optional
target publication; toolchain selection changes; DICE/repository/source/action
changes; Java/JVM work; copied Zabel code or behavior; cap violation; or a
claim beyond completing `rust/private/toolchain.bzl`. Audit its caller next.
