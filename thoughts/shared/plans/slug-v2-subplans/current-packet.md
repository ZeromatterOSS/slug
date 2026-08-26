# Current Slug V2 Packet

Packet: `WP-4-7A-clippy-rule-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove that the already-admitted loading model freezes the exact
`rust_clippy` rule declaration, with no production change, and stop before the
following provider declaration.

## Accepted starting point and source stop

Base is `fc9473b1` (`Load clippy aspect toolchain requirements`). The complete
`rust_clippy_aspect` freezes through rules_rust 0.73.0 `clippy.bzl` line 404,
including its ordered private attributes, provider requirements, fragments,
documentation and mixed mandatory/optional toolchain requirements.

Continue through the lazy `_rust_clippy_rule_impl` at lines 406-409 and
`rust_clippy = rule(...)` at lines 411-461. Stop before `RustClippyTestInfo =
provider(...)` at line 463. The pinned source SHA-256 is
`a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

## Authority and admitted reuse

Bazel 9.2 clean commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains sole behavior authority.
The accepted rustfmt target-attribute packet already pinned rule attribute
construction, nested provider alternatives, exported attached-aspect identity,
documentation validation and lazy implementation ownership. The clippy
declaration uses the same contract with less breadth:

- one ordinary non-test, non-executable rule;
- one `deps` `attr.label_list` with documentation;
- two ordered singleton provider alternatives, `CrateInfo` then
  `TestCrateInfo`;
- one exported attached `rust_clippy_aspect`;
- no transition, default, file policy, toolchain requirement or build setting.

The helper body is parsed and retained but never invoked. Its `ctx`, depset,
provider indexing and `DefaultInfo` expressions therefore add no evaluated
surface.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its declaration-owned `RuleDefinition`/`NamedAttribute`/`AttrDefinition`
boundary supports reusing Slug's existing frozen rule schema and retained
producer identities. Copy no Zig code, layout, diagnostic, parser, configured
capture or analysis behavior. No representation changes, hot-path utility,
interner, cache or memory-accounting decision occurs, so the Buck2 utility
skill and Stage 9 ledger require no update.

## Compatibility decision

- **Exact:** loading and freezing this source rule declaration; ordinary rule
  identity; `deps` label-list kind; ordered provider alternatives; attached
  exported aspect identity; omitted descriptor fields; lazy helper ownership.
- **Slug-native:** Rust frozen-value and Arc-backed schema ownership already
  accepted by predecessor packets.
- **Unsupported/deferred:** invoking this provider/aspect-bearing rule;
  configured provider matching; aspect propagation/application; executing the
  helper; the following clippy-test provider/rule/runner and action semantics.

This is a proof-only closure packet. It may not widen a constructor, retained
type, configured consumer or diagnostic.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `4215f59e3d3cbc51f06f19b82c610630541fa79c62a958c8851d5c9838ee9e73` | 6,798 | 6,908 |

Cap is 110 proof additions; deletions do not buy addition budget. No new
function may exceed 120 lines.

Required proof:

1. Extend the existing source-shaped clippy declaration with the exact lazy
   helper and rule through line 461; do not replace it with a reduced rule.
2. Assert exported class name `rust_clippy`, ordinary non-test/non-executable
   capability, no rule toolchains and exactly one declared `deps` label-list.
3. Assert the dependency descriptor's omitted default/file/executable/exec/
   transition state, its two provider identities in source order and its
   attached aspect's complete already-accepted identity.
4. Preserve the complete aspect and mutation proofs, prove the helper remains
   lazy, and keep the existing invocation rejection for provider/aspect-bearing
   attributes green.

No new oracle is needed: the exact pinned source plus the already-accepted
Bazel 9.2 rule/attribute evidence discriminate this declaration.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused clippy and provider/aspect-bearing rule tests;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked` before any rebuilt-binary smoke;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify exact source shape and stop, complete
attached-aspect identity, no production delta, compatibility boundaries,
Zabel guidance-only use, validation and caps.

STOP and `REPLAN` for any production file; source or authority drift; helper
execution; configured invocation/analysis/aspect work; the following provider
or rule; Java/JVM work; copied Zabel behavior; a retained-representation or
utility need; another file; or a cap violation.

## Immediate predecessor

`fc9473b1` accepted the complete clippy aspect using one shared typed rule/
aspect requirement slice. All local gates and independent review passed.
