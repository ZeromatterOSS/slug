# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-aspect-toolchain-requirements-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit mixed String/Label/typed aspect toolchain requirements, preserve
mandatory identity structurally, and freeze rules_rust's `rust_clippy_aspect`.

## Accepted starting point and authenticated source stop

Implementation base is `4aed2438` (`Load config common toolchain
requirements`). The completed private toolchain returns through alias-only
`rust/rust_toolchain.bzl`. The external Bzl driver resolves loads first and
computes them serially in AST order; structural route-plus-label DICE keys and
the manifest/warm-reuse regression prove the remaining analyzer, rustfmt and
toolchain children are reused. `rust/defs.bzl` therefore evaluates
`rust/private/clippy.bzl` next.

The fixed clippy source newly evaluates bazel_skylib's import-free
`lib/structs.bzl`; its function is lazy and its top-level `struct` uses the
accepted surface. The remaining six imports reuse completed children. Clippy's
provider and two string-list build-setting declarations then freeze. Function
bodies and documentation examples are lazy. The first unsupported expression
is lines 370-373:

```starlark
toolchains = [
    str(Label("//rust:toolchain_type")),
    config_common.toolchain_type(
        "@bazel_tools//tools/cpp:toolchain_type",
        mandatory = False,
    ),
]
```

The values evaluate, but the aspect binding accepts at most one String and
cannot unpack the typed second entry. This packet freezes the complete
`rust_clippy_aspect` through line 404 and stops before the lazy helper and
`rust_clippy = rule(...)` beginning at lines 406 and 411.

## Fixed sources and authorities

Selected rules_rust 0.73.0 sources:

- `rust/defs.bzl`, SHA-256 `5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`;
- `rust/toolchain.bzl`, SHA-256 `b94731396dc90e4ef8bbdc753252aac80208aba9cd857a7e7ca74d23f6aabbce`;
- `rust/rust_toolchain.bzl`, SHA-256 `0de5c3ba5c8a71176f881df065810a33eb2355a7007c16e47759653dbacdbd49`;
- `rust/rustfmt_toolchain.bzl`, SHA-256 `e57f8129f8b2dfac8b820ed057ca65d8a5e6945d614d53923ac65b27aaefb6f5`;
- `rust/private/toolchain.bzl`, SHA-256 `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
- `rust/private/clippy.bzl`, SHA-256 `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

Selected bazel_skylib 1.8.2 `lib/structs.bzl` is SHA-256
`c3fa79b9246582cb57c1bd9cbed999afbee822915d5888009bc0a197c43e9749`.

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Fixed anchors are
`StarlarkRuleFunctionsApi.aspect`,
`StarlarkRuleClassFunctions.parseToolchainTypes`/`parseToolchainType`,
`ToolchainTypeRequirement`, and
`StarlarkRuleClassFunctionsTest.testAspectAddToolchain`. They establish mixed
String/Label/typed input, defining-thread label conversion, mandatory true by
default, retained explicit false, stable distinct order and strictest-wins
duplicate normalization.

Architectural guidance is clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a`. Its
`build_rule_declaration.zig` uses one declaration-only
`ToolchainTypeRequirement` slice for both rules and aspects and detaches label
identity plus mandatory state from the evaluator. Slug follows that ownership
shape with its own Rust types. No Zig code, layout, diagnostic, evaluator
algorithm or compatibility behavior may be copied; Bazel alone defines
behavior.

The Buck2 utility audit selects the existing `CanonicalLabel`, inline Boolean,
immutable `Arc<[T]>` and `Allocative`. No new collection, hash, interner, cache,
clone mechanism, utility import or Stage 9 ledger row is warranted.

## Compatibility classification

- **Exact:** aspect `toolchains` accepts distinct String, Label and existing
  typed requirements in one list, resolves String identity in the defining
  module, preserves input order and retains mandatory state through freeze.
  Existing single mandatory String aspects remain exact.
- **Slug-native:** evaluator values detach into valid-Unicode Rust canonical
  labels and immutable shared storage.
- **Unsupported/deferred:** duplicate canonical labels still reject instead of
  strictest-wins merging; configured aspect propagation, toolchain resolution,
  optional absence, invocation and actions; the following clippy declarations;
  other aspect/config-common breadth.

## Implementation boundary

Rename the existing rule-named Rust record to the shared
`ToolchainTypeRequirement`; keep canonical label plus mandatory Boolean and the
existing accessors. Use `Arc<[ToolchainTypeRequirement]>` on transient and
frozen rule and aspect definitions. Replace the aspect's singular optional
label with the shared immutable slice.

Change the aspect binding from string-only unpacking to a value list and reuse
one evaluator-aware parser for rules and aspects. Existing Strings remain
mandatory, Labels retain their canonical identity, typed values retain their
mandatory bit, distinct entries retain order, and duplicate canonical labels
fail closed. Keep current rule parsing and configured consumers semantically
unchanged. Do not add aspect application or configured consumers.

## Discriminating proof

- Prove a frozen aspect retains mixed String, Label and typed requirements in
  order, including default true and explicit false, across a non-root defining
  module and apparent repository mapping.
- Prove true and false remain distinct in frozen aspect state; empty and
  existing singleton String aspects remain accepted.
- Prove wrong entry types, non-list input and duplicate canonical labels fail
  closed without producing an aspect.
- Freeze the source-shaped `rust_clippy_aspect` with canonical mandatory
  `@@rules_rust+//rust:toolchain_type` and optional
  `@@bazel_tools//tools/cpp:toolchain_type`; prove its attributes, providers,
  fragments and documentation remain accepted and its implementation stays
  lazy.
- Keep rule requirement, config-common, rustfmt/analyzer aspect, provider,
  attribute and completed private-toolchain proofs green.

## Allowlist and caps

Only these implementation files may change from base `4aed2438`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `974990551b1d717106c24e37237ef2e1910cf5a64207e659cbec910ac478ee8f` | 6,142 | 6,255 | shared record/parser and retained aspect slice |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `9bc0a07c319b34e8f6b9089415978700d1831e86b3a996948e015e96f05c8ce0` | 6,575 | 6,735 | ABI, identity, rejection and source-aspect proof |

Production additions are capped at 110, proof additions at 160 and total
additions at 270. Deletions do not buy addition budget. No new function may
exceed 120 lines. Plan-only selection edits are excluded from implementation
caps. No analysis, DICE, source, repository, oracle or action file may change.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused aspect mixed-toolchain, identity, rejection and clippy-source proofs;
- existing rule/config-common requirement and aspect-definition proofs;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation`;
- `cargo test -p slug_loading_v2 --test build_file_loading`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after the Rust change;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh`.

Clean stale `slugd` before and after daemon-sensitive checks. Recheck the
allowlist, caps, base/source hashes, clean Bazel/Zabel pins and archive-only
known failures. Independent selection and terminal reviews must verify the
source stop, Bazel authority, Zabel's guidance-only role, shared structural
identity, compatibility classification, validation and caps.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; a source or
authority hash mismatch; silent loss of mandatory state; duplicate
approximation; configured aspect/toolchain work; rule consumer behavior
change; DICE/repository/source/oracle/action work; the following clippy rule;
Java/JVM work; copied Zabel code or behavior; cap violation; or a claim beyond
freezing `rust_clippy_aspect` through line 404. Audit the next expression after
this packet completes.
