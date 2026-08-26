# Current Slug V2 Packet

Packet: `WP-4-7A-run-environment-info-declaration-global-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: install the fixed `.bzl` `RunEnvironmentInfo` declaration token needed
to compile the exact rules_rust lint-test helper module, prove that module
freezes without invoking its helpers, and stop before the clippy-test tail.

## Accepted starting point and source stop

Base is `37aeab638` (`Audit post-rust_clippy source frontier`); implementation
state is clean `993ba5e4`. The accepted source-shaped rustfmt proof recreated
`LINT_TEST_COMMON_ATTRS`, `platform_transition` and deliberately failing helper
bodies inside `rustfmt.bzl`; it did not compile the real defining
`rust/private/lint_test.bzl` helper bodies.

The real clippy module loads `//rust/private:lint_test.bzl` at
`clippy.bzl:19-25` before evaluating `RustClippyTestInfo` at line 463. The
defining module has no child loads. Its `lint_test_aspect_impl` resolves the now
accepted `OutputGroupInfo` and `depset` names at lines 82-100. Its
`lint_test_rule_impl` then resolves `DefaultInfo` and `depset`, but compilation
first stops at absent `RunEnvironmentInfo` on line 154. The following
`OutputGroupInfo` call at line 158 is already name-resolvable and remains lazy.

The authenticated sources are:

- rules_rust 0.73.0 `rust/private/lint_test.bzl`, lines 1-159, SHA-256
  `4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`;
- `rust/private/clippy.bzl`, lines 19-25 and 463-596, SHA-256
  `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`;
- `rust/private/rustfmt.bzl`, SHA-256
  `71ad2f08f5cf2d7a88ee983339c4371309d2ef2b0c407dcc1266bbe83155fb7a`.

Admit the fixed declaration and exact helper compilation through
`lint_test.bzl:159`. Stop when the child returns to `clippy.bzl`; do not
evaluate or claim the remaining line 463-596 tail in this packet.

## Fixed behavior and architecture authorities

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. The
object is locally available and must be read explicitly with `git show`; do not
change the clean `../bazel` checkout from its current unrelated commit.

- `StarlarkGlobalsImpl.getFixedBzlToplevels:91-107` installs
  `RunEnvironmentInfo.PROVIDER` beside `DefaultInfo.PROVIDER` and
  `OutputGroupInfo.STARLARK_CONSTRUCTOR`; fixed BUILD globals omit it.
- `RunEnvironmentInfo:36-104` defines one singleton native provider whose
  constructor owns fixed and inherited environment values.
- `RunEnvironmentInfoApi:35-109` gives the provider its public name and
  callable constructor schema.
- `BuiltinProvider:39-101` makes the concrete provider class authoritative for
  equality/key identity and renders it as `<function RunEnvironmentInfo>`.
- Starlark's module scope resolver checks names in function bodies during
  compilation, before a helper can be invoked.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture guidance only. Its
`BuiltinProviderId.run_environment_info`, native/starlark provider-identity
split, one provider-definition owner and separate loading binding support a
distinct declaration token rather than a user-provider ID or generic analysis
callable. Copy no Zig code, discriminant, enum layout, constructor, provider
value, configured lowering, test-environment behavior, diagnostic or hash.

## Decision and compatibility

Add a dedicated zero-state `RunEnvironmentInfo` native-provider declaration
value beside `OutputGroupInfo` in Slug's loading provider owner. Its concrete
Rust Starlark type supplies Slug-native internal separation from both the
existing native token and module/export-owned user providers. It displays
exactly as `<function RunEnvironmentInfo>`, freezes without evaluator state and
fails closed on every invocation before producing a value.

Install it only in complete `.bzl` globals (`bool_config = true`), not BUILD
globals. The exact lint-test helpers may resolve and capture the token, but
must not execute during loading.

- **Exact:** fixed `.bzl` placement and BUILD absence; exact printable
  representation; evaluator-free name resolution/capture; source-order
  compilation and recursive export of exact `lint_test.bzl` through line 159.
- **Slug-native:** the zero-sized Rust declaration token and its fail-closed
  invocation boundary; distinct concrete Rust types provide internal native
  declaration separation within the admitted loading slice.
- **Unsupported/deferred:** every `RunEnvironmentInfo(...)` construction,
  including empty/default construction; observable provider equality and
  hashability; environment/inherited-environment validation or values;
  `testing.TestEnvironment`; configured provider return/lowering; executable
  and test environment behavior; all helper execution, actions, runfiles,
  depsets and output-group values; configured aspect/provider/transition
  semantics; and all `clippy.bzl:463-596` declarations.

The Buck2 utility review selects the existing zero-state `Allocative` simple
value pattern. No collection, string, slice, interner, cache, clone path, hash
owner, graph storage or memory ledger changes; no Stage 9 ledger update.

## Allowlist, proof and caps

Only these files may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/provider.rs` | `319fb513cd6450d6f26e2471dd5429b2f22feebf12e6b083075cf7f5a6673ec4` | 990 | 1,025 |
| `app/slug_loading_v2/src/package.rs` | `9a09888463baffa893514ce54b0f0f675085839c573fafbebd512ed61a63329d` | 6,185 | 6,205 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `650589be17c52a6bced33271e495c48d699880bf71e5c4cc7e21b9b3876e837d` | 6,918 | 7,148 |

Caps are 35 production, 220 proof and 255 total additions; deletions do not buy
addition budget. No new function may exceed 120 lines; keep the 159-line
source fixture in a module constant.

Required proof:

1. Prove the fixed token resolves and freezes in `.bzl`, is absent from BUILD,
   renders exactly as `<function RunEnvironmentInfo>`, downcasts to its
   dedicated native type rather than `OutputGroupInfo` or a user provider, and
   rejects invocation before producing a value. Assert no equality/hash parity.
2. Recursively load an exact, unabridged `lint_test.bzl` child through a parent
   using the exact `clippy.bzl:19-25` four-symbol load. Preserve all lines
   1-159, including `rlocationpath`, both helper bodies, common attributes,
   transition and provider constructions; do not abbreviate or stub any
   body/global.
3. Prove the parent imports the child-owned `LINT_TEST_COMMON_ATTRS`,
   `platform_transition`, `lint_test_aspect_impl` and `lint_test_rule_impl`
   values, and that both helpers freeze without invocation. The proof must
   retain the exact `RunEnvironmentInfo(environment = {...})` expression, the
   preceding `DefaultInfo` and following `OutputGroupInfo` expressions, so
   source order and global resolution remain discriminating. Do not reconstruct
   common attributes or the transition in the parent.
4. Preserve all accepted lint common-attribute, transition, rustfmt and clippy
   proofs. Do not append `RustClippyTestInfo` or another clippy-tail declaration.

No new oracle is needed: pinned Bazel source fixes the native global contract,
and the exact rules_rust source extract discriminates the live boundary.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused `RunEnvironmentInfo` and exact lint-test helper-freeze proofs;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked` before any rebuilt-binary smoke;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify the recursive source stop, fixed-global
authority, native/user/OutputGroupInfo separation, constructor nonactivation,
compatibility boundary, exact excluded clippy tail, Zabel guidance-only role,
utility decision, validation and caps.

STOP and `REPLAN` for a constructed provider value; observable provider
equality/hash; a shared generic native-provider framework; configured
provider/test/environment/aspect/transition/action work; helper execution;
evaluation of the clippy tail; DICE or analysis changes; Java/JVM work; copied
Zabel content; another file; dirty authority; or a cap violation.

## Immediate predecessor

`37aeab638` selected the source-order audit after `993ba5e4` accepted the fixed
`OutputGroupInfo` declaration and `rust_clippy` closure.
