# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-find-toolchain-export-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the earliest parent-needed function definition in exact
rules_rust `utils.bzl` source, prove its pointer-identical binding through a
proof-only `rust.bzl` consumer, and invoke no function.

## Learned facts and closure audit

Base is `d4e264cdc` (`Select post-utils parent import audit`). Commit
`adde01290` already freezes five exact eager-value/dependency slices from the
authenticated 1,032-line rules_rust 0.73.0 `rust/private/utils.bzl`, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
It does not establish the whole child or utility invocation.

The authenticated 1,821-line parent `rust/private/rust.bzl`, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`,
imports fifteen utils functions at exact lines 40-57. Their source-complete
compiler/freeze closures are:

| Exact utils lines | Parent import | Direct nonlocal closure |
|---:|---|---|
| 61-70 | `find_toolchain` | admitted predeclared `Label` only |
| 214-227 | `determine_output_hash` | predeclared `abs`, `hash`, `repr` |
| 262-264 | `deduplicate` | dict comprehension and `.keys()` |
| 315-348 | `expand_dict_value_locations` | helper lines 268-313 |
| 410-445 | `compute_crate_name` | helpers at 374-408, 573-740 and eager substitutions |
| 447-477 | `dedent` | string/list methods and builtins only |
| 536-554 | `transform_deps` | five provider loads plus rules_cc `CcInfo` |
| 556-571 | `transform_link_deps` | `DepVariantInfo` and rules_cc `CcInfo` |
| 742-765 | `can_build_metadata` | provider load plus `can_use_metadata_for_pipelining` |
| 766-786 | `can_use_metadata_for_pipelining` | field reads/builtins only |
| 788-816 | `crate_root_src` | helper lines 818-833 |
| 835-876 | `determine_lib_name` | field reads/builtins only |
| 878-917 | `transform_sources` | paths load plus helper lines 937-965 |
| 919-935 | `get_edition` | predeclared `getattr`/`fail` only |
| 967-991 | `generate_output_diagnostics` | `RustcOutputDiagnosticsInfo` load |

The earliest parent-needed definition is exact lines 61-70. Its ten unabridged
lines hash to
`75fe3e764290fcfcec78cc25d25b4d2486708dafabb112f5d1e44b8e21081be1`.
The function body resolves only admitted predeclared `Label`; no same-module
helper, loaded binding or eager composite is required to compile and freeze it.

## Authorities and decision

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, pinned
`ResolverTest.testBindingScopeAndIndex_functionBlock` and
`testBindingScopeAndIndex_loads`, and the authenticated rules_rust source are
sole exact authority. They distinguish a frozen function/global binding from
executing the body.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only recursively retaining
functions reachable from a loaded defining module after evaluator closure.
Copy no Zig code, representation, owner pointer, traversal/order algorithm,
diagnostic, identity or behavior.

Select only `find_toolchain` because it is the earliest source-defined parent
import and its compiler closure is independently complete. Evaluate the exact
slice under producer `@@rules_rust+//rust/private:utils.bzl`; then use a clearly
proof-only module under producer `@@rules_rust+//rust/private:rust.bzl` with the
actual relative load spelling `:utils.bzl` to retain the import. Invoke neither
the function nor `Label`.

## Compatibility

- **Exact:** exact lines 61-70 bytes/hash, exact child producer, frozen function
  type, actual relative load spelling, exact parent producer, and pointer-
  identical imported binding.
- **Slug-native:** proof-only parent consumer and starlark-rust frozen function
  representation.
- **Unsupported/deferred:** invoking `find_toolchain`; its configured toolchain
  lookup and result/diagnostics; exact complete parent load; all other utils
  exports/bodies; whole-module freeze; and parent line 59 onward.

No production fallback, DICE key, request overlay, retained runtime memory,
async work, fixture, hot path or Buck2-derived utility changes. Those checklist
items are inapplicable to this proof-only packet.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `4698fbdc38ebd78bf6363d43ee167b127669fa10e8d8b8cbcd650981be591970` | 8,362 | 8,482 |

Caps are 0 production, 120 proof and 120 total additions; deletions do not buy
addition budget. Keep the test function at or below 80 lines. The exact source
constant is exempt from the function limit.

Required proof:

1. Embed exact unabridged `utils.bzl` lines 61-70 and verify the SHA-256 above.
2. Freeze only that slice under exact producer
   `@@rules_rust+//rust/private:utils.bzl`; assert `find_toolchain` has type
   `function`. Add no source stub, helper body or function invocation.
3. Evaluate one proof-only parent consumer under exact producer
   `@@rules_rust+//rust/private:rust.bzl` using
   `load(":utils.bzl", "find_toolchain")`; expose the binding without calling
   it and prove frozen pointer identity with the child export.
4. Preserve the accepted eager values and every prior loading proof.

No new oracle fixture is needed. Authenticated source plus pinned Bazel resolver
tests discriminate function/global/load binding; configured toolchain behavior
is deliberately skipped because it remains unsupported here.

The 8,362-line test file exceeds the documentation complexity trigger but is
the cohesive owner of exact external-Bzl loading fixtures. This packet adds one
ten-line source constant and one sub-80-line test; splitting the fixture owner
or adding production orchestration would widen scope.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused exact `find_toolchain` export proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify slice bytes/hash, producer/load
identities, pointer identity, lazy nonexecution, caps, prior-proof preservation,
Zabel guidance-only role and validation.

STOP and `REPLAN` for production change; `find_toolchain`/`Label` invocation;
toolchain/configured behavior; another utils export; full parent load/body;
identity/registry/DICE work; Java/JVM work; copied Zabel content; dirty
authority; skipped compiler dependency; or cap violation.

## Immediate predecessor

`d4e264cdc` authenticated all fifteen parent-imported exports and selected the
earliest independently complete compiler/freeze closure. It authorized no Rust
or invocation work.
