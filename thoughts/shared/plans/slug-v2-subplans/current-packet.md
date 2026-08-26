# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-cc-common-compiler-sentinel-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing public `cc_common` loading value
Base: `b0cd7855`

Result: expose Bazel's deprecated `do_not_use_tools_cpp_compiler_present`
struct field as `None`, allowing rules_cc to construct and freeze its exported
`cc_common` wrapper. Stop before any additional native C++ field or configured
semantic claim.

## Accepted starting point and source-order stop

Commit `b0cd7855` accepts only the exact empty-list row of
`cc_internal.freeze`. Ten default empty lists now become evaluator-owned frozen
empty lists while constructing top-level `EMPTY_COMPILATION_OUTPUTS`;
non-empty/general containers remain fail-closed. The source-shaped proof,
configured provider regression, all 206 loading units, locked checks, rebuilt
CLI and hygiene pass within the 15 production, 69 proof and 84 total addition
caps. Independent terminal review returned `ACCEPT`.

Recursive audit of rules_cc 0.2.17 then passes
`cc/private/compile/compile.bzl` and its children at loading time: their C++
operations remain in lazy functions, while their top-level sets, structs and
provider/rule declarations use accepted shapes. The remaining direct children
of `cc/private/cc_common.bzl` likewise expose only accepted declarations and
lazy functions on this path.

At the exported wrapper construction, the first absent evaluated expression
is `cc/private/cc_common.bzl:735`:

```starlark
do_not_use_tools_cpp_compiler_present =
    _cc_common_internal.do_not_use_tools_cpp_compiler_present,
```

`_cc_common_internal` is the already-exposed native/public `cc_common` value.
After this field is captured, the rest of the wrapper entries are imported
providers or lazy Starlark functions, so the `cc_common` struct freezes. Stop
there and separately audit the resumed rules_rust
`rust/private/toolchain.bzl` declarations; do not infer that its toolchain rule
or `config_common.toolchain_type` is admitted.

## Fixed sources and compatibility authority

Relevant fixed inputs:

- rules_cc `cc/private/cc_common.bzl` SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`;
- rules_cc `cc/private/compile/compile.bzl` SHA-256
  `bec506ffc3be08fffc4842b9daac498773534db9916121648a5527fac84cabea`;
- rules_rust `rust/private/toolchain.bzl` SHA-256
  `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`CcModuleApi.compilerFlagExists` is a deprecated zero-result Java method
exported as the struct field `do_not_use_tools_cpp_compiler_present`; its
default `void` result is Starlark `None`. The generated/checked-in Bazel wrapper
captures that value without calling it. This packet reproduces only that exact
observation; no JVM code or artifact enters Slug.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural and test guidance only. Its internal `cc_common` attribute
boundary returns `.none` for this name, and its wrapper-construction regression
captures the field before asserting the public value is `None`. Slug follows
that direct property/phase boundary using its existing public loading value.
No Zig code, layout, dispatch table, diagnostic or behavior is copied; Bazel
remains compatibility authority.

No retained data structure, collection, hash, interner, clone policy or memory
accounting changes. The Buck2 utility skill therefore selects the existing
zero-allocation `None` value and requires no Stage 9 ledger row.

## Compatibility classification

- **Exact:** in complete `.bzl` evaluation, `cc_common` exposes the struct
  field `do_not_use_tools_cpp_compiler_present`; reading it yields `None`;
  rules_cc can capture it into the exported `cc_common` struct and freeze that
  wrapper; the value is not callable.
- **Slug-native:** Rust valid-Unicode diagnostics and the existing
  starlark-rust static `None` representation are native choices.
- **Unsupported/deferred:** every other missing native `cc_common` field;
  invocation of C++ compile/link/toolchain/action functions; configured C++
  semantics; rules_rust toolchain declarations after the wrapper; M8, M7B and
  exact output bytes.

## Ownership, lifetime and implementation boundary

Add one exact attribute arm to the existing stateless `CcCommonModule` value.
It returns `Value::new_none()` and retains no caller, evaluator or source
state. Keep `internal_DO_NOT_USE()` as the existing method and preserve its
rules_cc defining-call authentication. Do not add a second module value,
method table, wrapper struct, stored field or configured decoder.

The BUILD globals remain unchanged and do not expose `cc_common`.
`BzlModuleEvalKey` and recursive source observations remain the sole
invalidation owner. There is no DICE, request, command, async, cache,
publication, cancellation or shutdown change.

## Discriminating proof

- Evaluate a rules_cc-shaped exported wrapper construction that captures the
  field from native `cc_common`, then freeze and read the captured `None`.
- Prove direct `.bzl` access returns `None`, `hasattr` is true and the field is
  not callable.
- Prove an unknown native field remains absent.
- Keep the private bridge, empty HeaderInfo, empty-list freeze, documented
  provider and configured-provider regressions green.
- Preserve BUILD absence of `cc_common`.

## Allowlist and caps

Only these files may change from base `b0cd7855`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/cc_common.rs` | `b303a10c8468275d34413b48e644167a31f9f075fd03d7953c722cb3f9bf82f6` | 180 | 192 | exact deprecated sentinel field |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `e3de0fc48be1e67ee65214abeaa339cca37f2e603625cbb95618e6aa97fdbfd6` | 5,826 | 5,885 | wrapper/property/boundary proof |

Production additions are capped at 8, proof additions at 50 and total
additions at 58. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. The test file exceeds the 2,000-line trigger,
but this proof belongs beside the existing private C++ bridge and freeze tests
sharing `eval_bzl_with_identity`; splitting it would widen `lib.rs` and the
allowlist.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused compiler-sentinel wrapper test;
- existing private bridge and empty-list freeze tests;
- one configured provider analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun absent
integration risk. Recheck base hashes, caps, allowlist, function sizes,
configured-analysis non-widening and the clean Zabel pin.

Independent selection and terminal reviews must verify Bazel authority,
Zabel's guidance-only role, the property-not-method shape, BUILD absence,
compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; another
native field or C++ method; stored state or a second module value; BUILD global
exposure; configured C++ lowering; rules_rust toolchain admission; a
starlark-rust, analysis/build-api or DICE edit; source/mapping/materializer/
network/fixture change; Java/JVM work; copied Zabel code or behavior; cap
violation; or a claim beyond exported rules_cc wrapper construction. Once the
wrapper freezes, audit `rust/private/toolchain.bzl` separately.
