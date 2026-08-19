# Current Slug V2 Packet

Packet: `WP-1-6-7A-rules-rust-0.73-toolchain-action-owner-evidence`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `86d23ca8`
Accepted owner audit: `86d23ca8`
Result: generate one isolated Bazel 9.2/rules_rust 0.73 discriminator before
designing the external toolchain owner.

## Why evidence is the smallest prerequisite

Live analysis still rejects external topology registrations, external native
toolchain references, and external registered toolchains in
`slug_analysis_v2/src/dice.rs`. The accepted direct-label evidence is complete
for its narrower surfaces: `nonroot-module-consumers` pins nonroot registration
and dev suppression, `toolchain-resolution-first-platform` pins root order and
selected context, and `exec-groups-action-platform` pins immutable action-owner
selection. None expands rules_rust 0.73's module-extension-generated
`@rust_toolchains//:all`, proves its apparent-to-canonical mapping, or exposes
the selected Rust provider/action relationship.

The older `rules-rust-basic` record is Bazel 9.1.1/rules_rust 0.71.1 and checks
only test/run success plus mnemonic counts. Rewriting it would mix M7B run/test
breadth with this M7A owner decision. A new analysis-only fixture is therefore
uniquely smaller than guessing a DICE topology owner or broadening the old
fixture.

## Exact authority and caps

Write only these eleven files:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
4. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/fixture.toml`;
5. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/expected/oracle.json`;
6. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/MODULE.bazel`;
7. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/pkg/BUILD.bazel`;
8. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/pkg/lib.rs`;
9. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/pkg/main.rs`;
10. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/pkg/data.txt`;
11. `tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace/cquery_format.bzl`.

Fixture-authored content is <=350 physical lines. Generated `oracle.json` is
<=3,000 physical lines and <=200 KiB. Docs are <=40 canonical, <=180 current
and <=180 Stage 6 net lines. Aggregate physical growth is <=3,750 lines. Every
other file is read-only; no harness change is authorized.

## Frozen fixture and command contract

Use Bazel 9.2.0 at pinned commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, rules_rust 0.73.0, edition 2024
and the Stage 10 pinned nightly `nightly/2025-09-14`. The fixture uses bzlmod,
`use_repo(rust, "rust_toolchains")`, and
`register_toolchains("@rust_toolchains//:all")`. It contains only one minimal
`rust_library`, one `rust_binary`, and one data file. Use daemon retention and
`startup_argv = ["--ignore_all_rc_files"]`; do not read workspace/home RCs or
claim remote execution/cache evidence.

Pin exact, anchored command output for:

- `query --order_output=full` of the generated registration package filtered
  to toolchain rules, preserving exact canonical-label membership and only the
  formatter's deterministic order; registration precedence is claimed solely
  through the selected provider/action result;
- cquery of the binary and its direct configured/toolchain edges, plus a
  Starlark provider projection that exposes the admitted CrateInfo
  owner/type/edition/root/output/dependency relationship without claiming
  opaque configuration bytes;
- text aquery restricted to the binary's Rustc and runfiles/symlink closure,
  preserving owner/action order, selected execution platform, compiler and
  process-wrapper association, parameter-file use, and declared outputs; and
- cold, unchanged warm, edition 2024 -> 2021 -> 2024 mutation/restoration.

The edition mutation must change the provider projection and opaque Rustc
ActionKey and restoration must recover both. Compare equality/restoration of
the opaque key only; do not claim its bytes. All output patterns are fully
anchored and fail on extra selected registrations, configured edges, provider
fields, or admitted action blocks.

Provenance must cite Bazel 9.2
`RegisteredToolchainsFunction#getBzlmodToolchains`, target-pattern expansion,
single/multi-toolchain resolution, `ResolvedToolchainContext`, and aquery text
formatting, plus rules_rust 0.73 `rust/extensions.bzl`,
`rust/private/repositories.bzl`, `rust/private/toolchain.bzl`,
`rust/private/rust.bzl`, and `rust/private/utils.bzl`.

Generate once, shut down the fixture Bazel server, then replay from no server
without `--update-expected`. Inventory authored/generated lines and bytes,
verify Bazel/rules_rust/nightly pins, and require clean no-update comparison,
archive, diff-check, credential-pattern and scope gates. A nondeterministic or
unanchorable output is REPLAN, not message-shape acceptance.

## Compatibility, STOP and successor

Exact: Bazel 9.2 extension expansion/mapping and the observed rules_rust 0.73
provider, toolchain, configured-edge and action relationships named above.

Slug-native: the future DICE owner, structural configuration/action identity,
compact provider/context representation, and any observed family boundary.

Unsupported/deferred: crate_universe, proc macros, build scripts, full sysroot
input closure, action execution/REAPI/materialization, public named groups,
applied aspects, M7B run/test/BEP breadth, and exact Bazel identity bytes.

STOP on Rust, Cargo/BUILD outside the fixture, Stage 10, harness changes,
`rules-rust-basic`, Java/JVM delegation, broad rules_rust evaluation,
execution/cache claims, M7A closure, M8/M7B/M9 activation, cap excess, or a
second successor. If bounded evidence requires full sysroot expansion, secret
configuration, or runner redesign, record REPLAN.

After independent ACCEPT, schedule exactly one docs-only successor:
`WP-6-7A-external-rules-rust-toolchain-owner-design`.
