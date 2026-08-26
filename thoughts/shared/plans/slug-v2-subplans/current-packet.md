# Current Slug V2 Packet

Packet: `WP-4-7A-clippy-test-tail-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: add one proof-only recursive source-shaped loading test for exact
rules_rust `clippy.bzl:463-596`, using the already-exact lint-test child and
producer-owned provider/common projections. Prove the whole tail freezes and
its retained identities are exact; change no production behavior.

## Accepted starting point and source closure

Base is `83cc1b6b1` (`Audit clippy tail after RunEnvironmentInfo`). Commit
`45b479e56` already freezes exact unabridged `lint_test.bzl:1-159` and proves
the four `clippy.bzl:19-25` imports are pointer-identical to their child
exports. Commits through `993ba5e4` accept the complete `rust_clippy_aspect`
and `rust_clippy` declarations through line 461.

The authenticated rules_rust 0.73.0 sources are:

- `clippy.bzl`, SHA-256
  `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`;
- `lint_test.bzl`, SHA-256
  `4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`;
- `providers.bzl`, SHA-256
  `57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`;
- `common.bzl`, SHA-256
  `cee50122624c7fd9c9a6545a647062f350dd25bc8cf6dda873944290463d4db6`.

The audit proves no new production terminal in `clippy.bzl:463-596`:

1. `RustClippyTestInfo` is the accepted documented two-field user-provider
   declaration; `_CLIPPY_OUTPUT_GROUPS` is an ordinary frozen string list.
2. Both helper bodies only capture already-resolved child/provider globals and
   remain lazy.
3. `_rust_clippy_test_aspect` has the same admitted fixed shape as the accepted
   rustfmt test aspect: three ordered `attr_aspects`, one exported required
   aspect, one defining-module advertised provider and string documentation.
4. `rust_clippy_test` performs the admitted `dict(base, **overlay)` merge over
   the child-owned four common attributes and adds one label list with two
   provider alternatives, the identical attached aspect and child transition.
   `test = True` is already retained in rule capability.
5. Both final rules use the admitted `config.bool(flag = True)` descriptor.
   Their provider constructors occur only in lazy implementations and do not
   execute during loading.

## Authorities and architecture

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. Reuse
the accepted provider export-key, aspect requires/provides, rule/test,
attribute/transition, dictionary merge and Boolean build-setting anchors. The
exact selected rules_rust sources discriminate the live declaration order and
producer names; no new oracle is needed.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its producer-module/export-name `ProviderIdentity`, declaration-owned
`RuleDefinition`, detached `BuildSettingDefinition`, and capture-time
retention of advertised providers support keeping imported identities with
their producers and declarations evaluator-free. Copy no Zig code,
representation, provider value, invocation capture, configured behavior,
algorithm or diagnostic. Bazel 9.2 decides compatibility.

No retained representation changes. The existing Arc/compact provider,
aspect, rule and build-setting owners are reused; the Buck2 utility skill and
Stage 9 memory ledger need no update.

## Compatibility

- **Exact:** source-order compilation/freeze of `clippy.bzl:463-596`; the
  producer module/export names of every used provider/helper; output-group
  list order; aspect requires/provides and attached-object identity; common
  attribute order, target provider alternatives and transition; test-rule
  capability; two true Boolean build-setting definitions.
- **Slug-native:** existing Rust frozen values, Arc ownership and admitted
  fail-closed target-invocation diagnostics.
- **Unsupported/deferred:** helper/rule/aspect execution; constructed
  `OutputGroupInfo` or `RunEnvironmentInfo`; configured provider matching,
  aspect application, transition, test runner/actions/runfiles; build-setting
  configured values and CLI flags; output-group values; and declarations
  reached by parents after this module returns.

## Allowlist, proof and caps

Only `app/slug_loading_v2/src/host_package_load_tests.rs` may change. Its base
SHA-256 is
`9a72f95b3a2889bd837d5f80a9827a55e7ad3a2c7e5f781ffb02f20363e28774`,
base length is 7,135 lines and final ceiling is 7,405 lines.

Caps are zero production, 260 proof and 260 total additions; deletions do not
buy addition budget. Keep each function at or below 120 lines; an exact source
constant may exceed that limit.

Required proof:

1. Reuse `LINT_TEST_SOURCE` unchanged and load its four exports through a
   child with the exact defining identity. Create provider/common children
   whose exported values retain the authenticated
   `@@rules_rust+//rust/private:providers.bzl` identities; do not rebind them to
   the clippy parent.
2. Evaluate the already-accepted clippy prefix plus an exact unabridged
   `clippy.bzl:463-596` tail. Preserve all documentation and both lazy helper
   bodies, including the provider-constructor calls.
3. Prove all four lint imports and the three provider imports used by the
   accepted prefix/tail are pointer-identical to child exports. Prove exact
   `RustClippyTestInfo`, output-list, aspect, merged rule, transition, attached
   aspect and two Boolean build-setting identities/shapes.
4. Prove no helper executes. Preserve existing fail-closed Boolean target
   invocation proof, and do not claim configured behavior from a successful
   declaration freeze.
5. Preserve every accepted clippy, rustfmt, lint-test and native-global proof.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused clippy-tail proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify exact tail preservation, recursive
producer identities, helper nonexecution, no production change, compatibility
boundary, Zabel guidance-only role, validation and caps.

STOP and `REPLAN` for a production edit; another test file; a changed existing
fixture; helper execution; constructed native providers; configured
provider/aspect/transition/test/build-setting/action semantics; Java/JVM work;
copied Zabel content; dirty authority; skipped source order; or cap violation.

## Immediate predecessor

`83cc1b6b1` accepted `45b479e56` and selected the source-order tail audit. The
audit proves the tail needs only this bounded loading proof.
