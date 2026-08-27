# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-expand-dict-export-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact rules_rust `utils.bzl:268-313` plus `:315-348`,
prove the public export retains its private helper and pointer-identical
proof-only parent import, and invoke neither function.

## Accepted base and learned facts

Base is `13ebf0a14` (`Prove exact utils leaf exports`). It freezes all seven
helper-free functions imported by exact `rust.bzl` under producer
`@@rules_rust+//rust/private:utils.bzl` and proves their pointer-identical
proof-only parent imports through producer
`@@rules_rust+//rust/private:rust.bzl` with actual `:utils.bzl` spelling.
No function body was invoked.

The authenticated 1,032-line utils source remains SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
The accepted parent-import audit leaves eight dependency-bearing exports.
Source order first reaches this complete closure:

| Exact source | Binding | SHA-256 |
|---|---|---|
| lines 268-313 | `_expand_location_for_build_script_runner` | `73cd67a0bf9e2b370f7d287cefe1fa73efa20552a8f99f7cdb45ecf14c24d64d` |
| lines 315-348 | `expand_dict_value_locations` | `0c8ce89317f00a453998d33aa2236824bff20eb6cdb0092dc5077604033e10bd` |

These two exact slices total 80 source lines. The public function references
only the private helper. The helper references predeclared globals, field
access and standard string/list methods. It requires no loaded provider,
bazel_skylib path binding, eager composite or other same-module definition.

## Authorities and decision

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, pinned
`ResolverTest.testBindingScopeAndIndex_functionBlock` and
`testBindingScopeAndIndex_loads`, and authenticated rules_rust source are sole
exact authority. The packet proves function/global/load binding after freeze,
not either function's result.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only recursively retaining
a private defining-module helper reachable from an exported frozen function.
Copy no Zig code, representation, owner pointer, traversal/order algorithm,
diagnostic, identity or behavior.

Concatenate only the helper slice followed by the public slice with a proof-
only separator under the exact utils producer. Prove both bindings freeze as
functions. Use a clearly proof-only module under the exact parent producer with
actual `:utils.bzl` spelling and import only
`expand_dict_value_locations`. Invoke neither function.

## Compatibility

- **Exact:** both source-slice bytes/hashes, child/parent producers, actual
  relative load spelling, frozen private/public function types, retained
  private-helper reachability, and pointer-identical public imported binding.
- **Slug-native:** slice concatenation separator, proof-only parent consumer
  and starlark-rust frozen function representation.
- **Unsupported/deferred:** invoking either selected function; all results,
  diagnostics and configured field/macro behavior; the other seven dependency-
  bearing exports; exact complete parent load; whole-utils freeze; parent line
  59 onward.

No production fallback, DICE key, request overlay, retained runtime memory,
async work, fixture, hot path or Buck2-derived utility change is involved.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `decf245713bc1884df48e824e4a13db66691caa2f9935b86621f78be200730c7` | 8,606 | 8,786 |

Caps are 0 production, 180 proof and 180 total additions; deletions do not buy
addition budget. Keep the test function at or below 100 lines. Exact source
constants are exempt from the function limit.

Required proof:

1. Embed both exact unabridged slices separately and verify both SHA-256s. Add
   no other helper, loaded-provider stub or utils source.
2. Concatenate only helper then public slice with a proof-only separator and
   freeze under exact producer `@@rules_rust+//rust/private:utils.bzl`. Prove
   both named exports have type `function`; invoke neither.
3. Evaluate one proof-only parent under exact producer
   `@@rules_rust+//rust/private:rust.bzl`, using actual `:utils.bzl` spelling
   and importing only `expand_dict_value_locations`. Prove frozen pointer
   identity against the child export.
4. Preserve the accepted eager values, all seven helper-free exports and every
   prior loading proof.

No new oracle fixture is needed. Authenticated source plus pinned resolver tests
discriminate closure retention and frozen export identity. Invocation is
deliberately skipped because macro-expansion behavior remains unsupported here.

The large test file remains the cohesive exact external-Bzl fixture owner. The
packet adds two bounded raw constants and one sub-100-line proof; splitting the
owner or adding production orchestration would widen scope.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused expand-dict export proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify both slice hashes/lines, defining
closure, producer and load identities, public pointer identity, lazy
nonexecution, caps, preserved proofs, Zabel guidance-only role and validation.

STOP and `REPLAN` for production change; invocation; another helper/provider/
path source; configured macro behavior; another export; full parent load/body;
identity/registry/DICE work; Java/JVM work; copied Zabel content; dirty
authority; or cap violation.

## Immediate predecessor

`13ebf0a14` accepted all six remaining helper-free parent imports with 229
unit, 24 invalidation and 31 BUILD-loading tests green. Independent review
verified all source hashes, producers, real load order, pointer identity and
non-invocation.
