# Current Slug V2 Packet

Packet: `WP-4-7A-output-group-info-declaration-global-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: add the fixed `.bzl` `OutputGroupInfo` declaration token required to
compile the exact lazy clippy helper, freeze the following rule, and stop
before output-group construction or the next provider.

## Accepted starting point and source stop

Base is `54ef6cdc` (`Audit OutputGroupInfo declaration global`); implementation
state is clean `fc9473b1`. The complete `rust_clippy_aspect` freezes through
rules_rust 0.73.0 `clippy.bzl` line 404. The exact helper begins at line 406 and
currently fails compilation because `OutputGroupInfo` is absent.

Admit compilation of `_rust_clippy_rule_impl` at lines 406-409 and freeze
`rust_clippy = rule(...)` through line 461. Stop before
`RustClippyTestInfo = provider(...)` at line 463. The source SHA-256 is
`a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

## Fixed behavior and architecture authorities

Bazel 9.2 clean commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority:

- `StarlarkGlobalsImpl.getFixedBzlToplevels` installs
  `OutputGroupInfo.STARLARK_CONSTRUCTOR` directly in fixed `.bzl` globals;
- fixed BUILD-file globals omit that name;
- `OutputGroupInfoApi` names a native callable provider;
- `OutputGroupInfo.OutputGroupInfoProvider` extends `BuiltinProvider`;
- `BuiltinProvider.equals` and `BuiltinProvider.Key.equals` use the concrete
  provider class as process-stable native identity;
- constructor conversion of named group values to artifact nested sets belongs
  to configured semantics and is outside this packet.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is guidance only. Its
`BuiltinProviderId.output_group_info` and separate binding/value paths support
keeping native declaration identity distinct from module/export-owned user
providers and later configured values. Copy no Zig code, numeric discriminant,
enum layout, callable implementation, value model, diagnostic or behavior.

## Decision and compatibility

Add a dedicated zero-state `OutputGroupInfo` native-provider declaration value
beside Slug's loading provider definitions. Its concrete Rust Starlark value
type provides Slug-native internal separation; it is not a `ProviderId` and not
the generic `AnalysisBuiltinCallable`. Observable Bazel class-based equality
and hashability are deferred. It displays exactly as
`<function OutputGroupInfo>`, freezes without an evaluator and implements
invocation only to fail closed with an unsupported loading diagnostic.

Install the value only in complete `.bzl` globals (`bool_config = true`), not
BUILD globals. The exact clippy helper may resolve and capture it, but loading
must not invoke the helper or construct an output-group value.

- **Exact:** fixed `.bzl` name placement; BUILD absence; exact
  `<function OutputGroupInfo>` representation; evaluator-free capture/freeze;
  exact helper and `rust_clippy` declaration loading through line 461.
- **Slug-native:** a zero-sized Rust Starlark declaration token whose distinct
  concrete type prevents conflation with user providers inside this slice and
  whose unsupported invocation fails closed.
- **Unsupported/deferred:** every `OutputGroupInfo(...)` call, including empty;
  observable provider equality and hashability across values/globals;
  named group validation; depset/artifact conversion; provider values, fields,
  indexing, iteration and membership; return from rule/aspect implementations;
  configured target/aspect lookup, attachment, merge and output selection; the
  following clippy-test provider/rule/runner/actions.

The Buck2 utility review selects a zero-state `Allocative` simple value. No
collection, compact string, Arc, interner, cache, clone path, hashing owner,
memory-accounting extension or Stage 9 ledger update is warranted.

## Allowlist, proof and caps

Only these files may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/provider.rs` | `6a452de998de926287f01c172aa8e77b1a7c99a742f6e2f09cfbadac2cc09c93` | 964 | 1,009 |
| `app/slug_loading_v2/src/package.rs` | `8a948df6b7c504b2f1dc468f31f63e7eeadd49c6d0bf77d9538dbd8f6caedbeb` | 6,183 | 6,203 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `4215f59e3d3cbc51f06f19b82c610630541fa79c62a958c8851d5c9838ee9e73` | 6,798 | 6,948 |

Caps are 45 production, 150 proof and 195 total additions; deletions do not buy
addition budget. No new function may exceed 120 lines.

Required proof:

1. Prove the fixed token resolves and freezes in `.bzl`, is absent from BUILD,
   renders exactly as `<function OutputGroupInfo>`, downcasts to its dedicated
   native type rather than a user provider, and rejects invocation before
   producing a value. Do not assert equality or hashability.
2. Extend `CLIPPY_ASPECT_SOURCE` with the exact helper and rule through line
   461. Do not stub `OutputGroupInfo`, abbreviate the helper or reduce the rule.
3. Assert the exported ordinary `rust_clippy` class, no toolchains, and its sole
   `deps` label-list with omitted fields, two ordered provider identities and
   complete attached `rust_clippy_aspect` identity.
4. Preserve clippy mutation proofs and the existing pre-recording rejection for
   provider/aspect-bearing target invocation. Prove neither helper nor native
   provider constructor executes during module freeze.

No new oracle is needed: pinned Bazel source fixes the native global contract,
and the exact rules_rust source extract discriminates the live boundary.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused OutputGroupInfo, clippy and provider/aspect-bearing rule tests;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked` before any rebuilt-binary smoke;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify the native/user identity separation,
`.bzl`-only placement, constructor nonactivation, exact source/stop, complete
attached-aspect identity, compatibility boundary, Zabel guidance-only role,
utility decision, validation and caps.

STOP and `REPLAN` for a constructor value; configured provider/target/aspect or
artifact semantics; reuse of user `ProviderId`; generic native-provider
framework; DICE/analysis/action work; helper execution; the following provider
or rule; Java/JVM work; copied Zabel content; another file; or a cap violation.

## Immediate predecessor

`54ef6cdc` records the compile-time blocker and audited identity boundary after
fully reverting the disproven proof-only candidate.
