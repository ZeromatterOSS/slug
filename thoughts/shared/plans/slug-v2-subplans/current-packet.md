# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-find-cc-toolchain-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact rules_cc 0.2.17 `cc/find_cc_toolchain.bzl`, prove its eager
label/attribute constants and exported function bindings survive module freeze,
and stop when this child returns.

## Learned facts and source order

Base is `a5574d201` (`Select post-paths parent audit`). Exact
`rust/private/rust.bzl` resumes after paths through already-admitted
`@bazel_skylib//rules:common_settings.bzl`, rules_cc `cc_info.bzl`, and
rules_rust `common.bzl`/`providers.bzl`.

The first new direct child is rules_rust 0.73.0
`rust/private/rust_allocator_libraries.bzl`, 302 lines at SHA-256
`ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.
Its cached `cc_common`/`CcInfo` children return before the previously unseen
`rust/private/utils.bzl`, 1,032 lines at SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.

Utils first loads cached bazel_skylib paths and then rules_cc 0.2.17
`cc/find_cc_toolchain.bzl`, 131 lines at SHA-256
`3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`.
That child's sole `//cc/common:cc_common.bzl` load is already admitted.

The exact child eagerly defines
`CC_TOOLCHAIN_TYPE = Label("@bazel_tools//tools/cpp:toolchain_type")` and
singleton `CC_TOOLCHAIN_ATTRS` containing `_cc_toolchain = attr.label(default =
Label("@rules_cc//cc:current_cc_toolchain"))`. Its three functions
`find_cc_toolchain`, `find_cpp_toolchain` and `use_cc_toolchain` remain lazy.

## Authorities and decision

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior
authority. The authenticated rules_cc source fixes source order, constants and
exports. Previously accepted Bazel Label/attribute/rule evidence fixes canonical
label resolution and declaration projection. Invoke no helper, so no configured
toolchain oracle is needed.

Slug already owns the required load, Label, dictionary, label-attribute/default,
function and recursive freeze shapes. Add no production behavior. Embed exact
source and use the existing loaded-child evaluator. A proof-only consumer may
load the constants into a rule declaration so the retained singleton schema and
canonical default can be observed without calling a toolchain helper.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its module-freeze graph principle says exported closures and a nested
declaration dictionary must retain their reachable values after the evaluator
closes. This guides proof shape only. Copy no Zig code, representation, owner
pointer, field ordering, capture algorithm, diagnostic, identity or behavior.

The Buck2 utility review selects no action. This is proof-only and changes no
retained data structure, hash, compact collection/string, interner, clone path,
graph storage or memory accounting.

## Compatibility

- **Exact:** exact unabridged source freeze; canonical producer and child
  identities; exact source-defined export/type set; canonical
  `CC_TOOLCHAIN_TYPE`; and
  the singleton label declaration/default retained by `CC_TOOLCHAIN_ATTRS`.
- **Slug-native:** starlark-rust frozen value/closure representation and the
  proof-only consuming rule projection.
- **Unsupported/deferred:** invoking any exported helper; configured or legacy
  C++ toolchain lookup; exact Label/attribute display text; and every later
  utils, allocator or parent expression after this child returns.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `a8fccee5660d823d9ab3c67be160e04f8328ffd4b0e1ab572fe3365a3075146b` | 7,935 | 8,235 |

Caps are 0 production, 300 proof and 300 total additions; deletions do not buy
addition budget. Keep the test function at or below 120 lines; the exact 131-
line source constant is exempt from the function limit.

Required proof:

1. Embed exact unabridged rules_cc 0.2.17
   `cc/find_cc_toolchain.bzl:1-131` and verify its SHA-256.
2. Freeze under producer `@@rules_cc+//cc:find_cc_toolchain.bzl` with its sole
   cached child at `@@rules_cc+//cc/common:cc_common.bzl`; preserve the exact
   direct load spelling and invoke no exported helper.
3. Prove the exact source-defined export/type set: `CC_TOOLCHAIN_ATTRS` as
   `dict`, `CC_TOOLCHAIN_TYPE` as `Label`, and the three named functions as `function`.
   Sort only names for comparison; claim no module iteration order.
4. Prove `CC_TOOLCHAIN_TYPE` resolves canonically to
   `@@bazel_tools+//tools/cpp:toolchain_type` under an explicit producer mapping.
5. Through a proof-only consumer, prove `CC_TOOLCHAIN_ATTRS` is a singleton
   `_cc_toolchain` Label declaration whose default resolves canonically to
   `@@rules_cc+//cc:current_cc_toolchain`; do not invoke an implementation.
6. Preserve every accepted keyword-only, struct, rules_cc, descriptor, clippy,
   lints, lint-test, rustfmt and paths proof.

No new oracle is needed: authenticated source and accepted Bazel Label/
attribute evidence discriminate this loading-only gap.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused exact find-toolchain child proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify source bytes/hash, recursive order,
producer/child identities, exact source-defined export/types, canonical labels,
singleton declaration proof, lazy nonexecution, Zabel guidance-only role, validation and
caps.

STOP and `REPLAN` for a production change; helper invocation; configured
toolchain behavior; another new child; identity/registry/DICE work; Java/JVM
work; copied Zabel content; dirty authority; skipped source order; or cap
violation.

## Immediate predecessor

`a5574d201` accepted the post-paths docs rollover. This audit accounted for all
cached direct children and selected the first unseen recursive child only.
