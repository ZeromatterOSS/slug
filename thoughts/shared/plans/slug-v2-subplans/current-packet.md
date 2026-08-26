# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-skylib-paths-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact bazel_skylib 1.8.2 `lib/paths.bzl`, prove its exported
function bindings survive module freeze, and stop when this child returns.

## Learned facts and source order

Base is `f301d89d3` (`Select post-lints parent audit`). Exact `rust/defs.bzl`
next reaches `rust/private/rust.bzl`, 1,821 lines at SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
Its first direct child is previously unseen bazel_skylib 1.8.2
`lib/paths.bzl`, 320 lines at SHA-256
`96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`.
The child has no recursive loads.

The exact paths child declares ten functions. Their bodies contain standard
Starlark string/list/zip operations but remain lazy. Four integer state
constants and the final `paths = struct(...)` are the only other eager values.
That struct retains all ten functions.

## Authorities and decision

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StructProvider` and pinned `StarlarkRuleClassFunctionsTest` establish a struct
holding callable function fields. Bazel's schemaless `StarlarkInfoNoSchema`
sorts field keys for its table; do not infer exact observable order from source
order.

Slug already owns standard function/default/variadic syntax, integer constants,
the `.bzl` `struct` global and recursive frozen Starlark values. Add no
production behavior. The proof embeds the exact child, freezes it under the
exact producer identity and observes the exported composite without invoking
its functions.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its module-freeze traversal principle says closures retained by an exported
composite must remain reachable after the defining evaluator closes. This
guides the proof shape only. Copy no Zig code, representation, field ordering,
owner pointer, identity, capture algorithm, diagnostic or behavior. Bazel 9.2
decides compatibility.

The Buck2 utility review selects no action. This packet adds only an exact test
source and assertions; it changes no retained data structure, hash, compact
collection/string, interner, clone path, graph storage or memory accounting.
The large test file remains the cohesive owner of exact external-Bzl loading
fixtures; the exact 320-line constant is isolated and the test stays below 120
lines.

## Compatibility

- **Exact:** exact unabridged paths source freeze, exact producer identity,
  exact ten-member name set and each source-bound member surviving as a frozen
  function value.
- **Slug-native:** starlark-rust frozen value/closure representation and current
  constructor-order struct iteration.
- **Unsupported/deferred:** exact Bazel struct iteration/order; invoking any
  path function; its path/string result or diagnostic behavior; and the parent
  `rust.bzl` frontier after paths returns.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `e08900d8db938c7c29b8722b8213f2d1a8226e90129730015895ebbe5418a3c6` | 7,574 | 7,994 |

Caps are 0 production, 420 proof and 420 total additions; deletions do not buy
addition budget. Keep the test function at or below 120 lines; the exact source
constant may exceed that limit.

Required proof:

1. Embed exact unabridged bazel_skylib 1.8.2 `lib/paths.bzl:1-320` and verify
   its SHA-256 against the authenticated source.
2. Freeze it with exact producer identity
   `@@bazel_skylib+//lib:paths.bzl`; successful freeze must retain the exported
   composite without invoking a helper.
3. Prove the exact field-name set `basename`, `dirname`, `is_absolute`, `join`,
   `normalize`, `is_normalized`, `relativize`, `replace_extension`,
   `split_extension`, `starts_with`.
4. Prove the source binds each name to a function and every field remains a
   frozen value of type `function`. Compare a sorted name set; do not claim
   Bazel-exact iteration order or inspect unavailable private function exports.
5. Preserve every accepted keyword-only, struct, clippy, lints, lint-test and
   rustfmt proof.

No new oracle is needed: authenticated source, pinned Bazel struct tests and
the exact frozen composite proof discriminate the missing coverage.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused exact paths-child proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify source bytes/hash, source order,
producer/member/function proof, lazy nonexecution, the non-exact ordering
boundary, Zabel guidance-only role, utility decision, validation and caps.

STOP and `REPLAN` for a production change; function invocation; an exact field-
order claim; another child; identity/registry/DICE work; Java/JVM work; copied
Zabel content; dirty authority; skipped source order; or cap violation.

## Immediate predecessor

`f301d89d3` selected the post-lints audit. It authenticated paths as the first
new child of `rust.bzl` and found only a missing exact recursive freeze proof.
