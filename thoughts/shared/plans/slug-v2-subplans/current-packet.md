# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-leaf-exports-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the six remaining helper-free functions imported by exact
`rust.bzl`, prove pointer-identical bindings through a proof-only parent, and
invoke no function.

## Accepted base and learned facts

Base is `d3cb959f6` (`Prove exact utils find toolchain export`). It freezes
exact rules_rust 0.73.0 `utils.bzl:61-70` under producer
`@@rules_rust+//rust/private:utils.bzl` and proves pointer-identical
`find_toolchain` import through exact parent producer
`@@rules_rust+//rust/private:rust.bzl` with actual `:utils.bzl` spelling. No
function or `Label` was invoked.

The authenticated 1,032-line utils source remains SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
The accepted export audit identified six other parent-needed functions whose
bodies resolve only predeclared builtins, comprehensions, field access or
string/list/dict methods and require no same-module helper or loaded binding:

| Exact source | Export | SHA-256 |
|---|---|---|
| lines 214-227 | `determine_output_hash` | `0e4c8febdc878e77987b4a525f8737723a6e7c4c00d409f58df46581edc54d77` |
| lines 262-264 | `deduplicate` | `1647d85c5e861c2a388e9700c9b9182ca3f1ea9cb388d350fb063b8124674e6f` |
| lines 447-477 | `dedent` | `2b851cbad7d7131e011da7f85b50b12ff3fb9e9c698654bb219eb1edb07dc839` |
| lines 766-786 | `can_use_metadata_for_pipelining` | `00078da9862fec4e91d5e0e4453a5395dca29f12e4bc6dd44f280a58643b0b5a` |
| lines 835-876 | `determine_lib_name` | `e42edb4f6802acc91363c06db74ff2322f11dcdba3d2d2d8adbd9091faa660b0` |
| lines 919-935 | `get_edition` | `51f501c5b091031305732a81b909a2abcc3be419bbbfb8577cf8e23ff45c7db8` |

These six exact slices total 128 source lines. The remaining eight parent
imports require same-module helper definitions, loaded providers, accepted
eager composites or bazel_skylib paths and remain deferred.

## Authorities and decision

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, pinned
`ResolverTest.testBindingScopeAndIndex_functionBlock` and
`testBindingScopeAndIndex_loads`, and authenticated rules_rust source are sole
exact authority. The packet proves only function/global/load binding after
freeze, not any function result.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only recursively retaining
exported defining-module functions after evaluator closure. Copy no Zig code,
representation, owner pointer, traversal/order algorithm, diagnostic, identity
or behavior.

The six functions form one coherent helper-free leaf-export family. Concatenate
only the six exact slices with separators under the exact utils producer. Use a
clearly proof-only module under the exact parent producer with actual
`:utils.bzl` spelling and the six names in their real parent-relative order:
`can_use_metadata_for_pipelining`, `dedent`, `deduplicate`,
`determine_lib_name`, `determine_output_hash`, `get_edition`. Invoke none.

## Compatibility

- **Exact:** six source-slice bytes/hashes, child/parent producers, actual
  relative load spelling and name order, frozen function types, and pointer-
  identical imported bindings.
- **Slug-native:** slice concatenation separators, proof-only parent consumer
  and starlark-rust frozen function representation.
- **Unsupported/deferred:** invoking any selected function; all results,
  diagnostics and configured field behavior; the eight dependency-bearing
  exports; exact complete parent load; whole-utils freeze; parent line 59 onward.

No production fallback, DICE key, request overlay, retained runtime memory,
async work, fixture, hot path or Buck2-derived utility change is involved.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `cef40fe68768e9822c03312382d42512a4d0b04004c64e04dbe512ae7cd3fb18` | 8,415 | 8,665 |

Caps are 0 production, 250 proof and 250 total additions; deletions do not buy
addition budget. Keep the test function at or below 100 lines. Exact source
constants are exempt from the function limit.

Required proof:

1. Embed all six exact unabridged slices separately and verify every SHA-256.
   Add no helper, loaded-provider stub or other utils source.
2. Concatenate only those slices with proof-only separators and freeze under
   exact producer `@@rules_rust+//rust/private:utils.bzl`. Prove every named
   export has type `function`; invoke none.
3. Evaluate one proof-only parent under exact producer
   `@@rules_rust+//rust/private:rust.bzl`, using actual `:utils.bzl` spelling and
   the six names in their parent-relative order. Retain them in an ordered list
   and prove frozen pointer identity against child exports in the same order.
4. Preserve the accepted eager values, `find_toolchain` export and every prior
   loading proof.

No new oracle fixture is needed. Authenticated source plus pinned resolver tests
discriminate frozen export identity. Function invocation is deliberately
skipped because its behavior remains unsupported in this packet.

The large test file remains the cohesive exact external-Bzl fixture owner. The
packet adds six bounded raw constants and one sub-100-line proof; splitting the
owner or adding production orchestration would widen scope.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused leaf-export proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify all six slice hashes/lines, producer
and load identities/order, pointer identity, lazy nonexecution, caps, preserved
proofs, Zabel guidance-only role and validation.

STOP and `REPLAN` for production change; invocation; helper/provider/path
source; configured behavior; another export; full parent load/body; identity/
registry/DICE work; Java/JVM work; copied Zabel content; dirty authority; or cap
violation.

## Immediate predecessor

`d3cb959f6` accepted the first helper-free parent import with 228 unit, 24
invalidation and 31 BUILD-loading tests green. Independent review verified its
exact hash, producers, load spelling, pointer identity and non-invocation.
