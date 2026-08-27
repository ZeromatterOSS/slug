# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-crate-root-export-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact rules_rust `utils.bzl:788-816` plus `:818-833`,
prove the public export retains its private helper and pointer-identical
proof-only parent import, and invoke neither function.

## Accepted base and learned facts

Base is `216b83ac0` (`Prove exact utils expand dict export`). It freezes the
first dependency-bearing exact utils export with its private helper under
producer `@@rules_rust+//rust/private:utils.bzl`, proves hidden helper
visibility, and retains the public function pointer-identically through a
proof-only `@@rules_rust+//rust/private:rust.bzl` parent using actual
`:utils.bzl` spelling. Neither function body was invoked.

The authenticated 1,032-line utils source remains SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
Seven dependency-bearing parent imports remain. The smallest source-complete
closure requiring no loaded provider, accepted eager composite or bazel_skylib
path binding is:

| Exact source | Binding | SHA-256 |
|---|---|---|
| lines 788-816 | `crate_root_src` | `f5a21bb9e1f694a1baec8c238bb52f4eb70f7ec25014f6d0cf71b09e2670ee41` |
| lines 818-833 | `_shortest_src_with_basename` | `7157302d387837bc1d83c2aae3caed49c2cd76a074d58d9d4b6fdc3d6f5f7bdc` |

These two exact slices total 45 source lines. The public function references
only the private helper in addition to predeclared globals, field access and
standard value operations. The private helper references only its parameters,
predeclared `len`, field access and comparisons.

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

Concatenate only the public slice followed by the private-helper slice in exact
source order with a proof-only separator under the exact utils producer. Prove
both bindings freeze as functions and only the public binding is exported. Use
a clearly proof-only module under the exact parent producer with actual
`:utils.bzl` spelling and import only `crate_root_src`. Invoke neither
function.

## Compatibility

- **Exact:** both source-slice bytes/hashes, child/parent producers, actual
  relative load spelling, frozen private/public function types, private
  visibility and helper reachability, and pointer-identical public import.
- **Slug-native:** slice concatenation separator, proof-only parent consumer
  and starlark-rust frozen function representation.
- **Unsupported/deferred:** invoking either selected function; all results,
  diagnostics and configured field behavior; the other six dependency-bearing
  exports; exact complete parent load; whole-utils freeze; parent line 59
  onward.

No production fallback, DICE key, request overlay, retained runtime memory,
async work, fixture, hot path or Buck2-derived utility change is involved.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `9462dc063ed843169f5e0c403f809b347c87d1d036f5df21b9177d4c15f52cdc` | 8,751 | 8,881 |

Caps are 0 production, 130 proof and 130 total additions; deletions do not buy
addition budget. Keep the test function at or below 100 lines. Exact source
constants are exempt from the function limit.

Required proof:

1. Embed both exact unabridged slices separately and verify both SHA-256s. Add
   no other helper, loaded-provider stub or utils source.
2. Concatenate only public then helper slice in exact source order with a
   proof-only separator and freeze under exact producer
   `@@rules_rust+//rust/private:utils.bzl`. Prove both named bindings have type
   `function`, public lookup rejects the private helper, and public lookup
   accepts `crate_root_src`; invoke neither.
3. Evaluate one proof-only parent under exact producer
   `@@rules_rust+//rust/private:rust.bzl`, using actual `:utils.bzl` spelling
   and importing only `crate_root_src`. Prove frozen pointer identity against
   the child export.
4. Preserve the accepted eager values, seven helper-free exports, expand-dict
   closure and every prior loading proof.

No new oracle fixture is needed. Authenticated source plus pinned resolver tests
discriminate closure retention, visibility and frozen export identity.
Invocation is deliberately skipped because source-selection behavior remains
unsupported here.

The large test file remains the cohesive exact external-Bzl fixture owner. The
packet adds two bounded raw constants and one sub-100-line proof; splitting the
owner or adding production orchestration would widen scope.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused crate-root export proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify both slice hashes/lines, defining
closure, producer and load identities, private visibility, public pointer
identity, lazy nonexecution, caps, preserved proofs, Zabel guidance-only role
and validation.

STOP and `REPLAN` for production change; invocation; another helper/provider/
path source; configured source-selection behavior; another export; full parent
load/body; identity/registry/DICE work; Java/JVM work; copied Zabel content;
dirty authority; or cap violation.

## Immediate predecessor

`216b83ac0` accepted the first dependency-bearing exact utils export with 230
unit, 24 invalidation and 31 BUILD-loading tests green. Independent review
verified both source hashes, private visibility, producers, actual load
spelling, public pointer identity and non-invocation.
