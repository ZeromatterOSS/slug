# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-eager-values-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze only authenticated eager-value slices from rules_rust 0.73.0
`rust/private/utils.bzl`, prove their ordered constants and aliases, and stop
without invoking a utility or copying the 1,032-line module.

## Learned facts and source order

Base is `8609b3623` (`Select post-toolchain utils audit`). Exact
`rust/private/utils.bzl`, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`,
has returned from accepted bazel_skylib paths and rules_cc find-toolchain.
Its remaining `cc_common`, `CcInfo` and seven-provider child bindings are also
accepted exact loading slices. Do not duplicate them.

The full 1,032-line source has no unsupported eager expression. Its eager body
is exactly:

- lines 32-42: ordered six-string `UNSUPPORTED_FEATURES`, SHA-256
  `e649bad3018c5c5048307adc8066152ef5bbbecdda89a69f51d1896be3ee3b8b`;
- line 73: private `_FORCE_DISABLE_CC_TOOLCHAIN = False`, SHA-256
  `d5a539c2509332b4891e9cbffee2cd7e3230eeb79dd58ebd14be56661b79dc0d`;
- lines 601-650: 31-pair `_encodings`, nested-comprehension 63-pair
  `_substitutions`, and its public alias, SHA-256
  `e0526a4d2bc5bc9d04544ecdbde305667c5a015b0c7f4597858891ae668f7b85`;
- lines 664-676: lazy `_encode_raw_string` plus its public alias, SHA-256
  `b5ad15479c25ae84b1dba206ffc924d455003aaff98b5371773a3104f08d9027`;
- lines 692-740: lazy `_replace_all`, included only because
  `_encode_raw_string` resolves that global while the selected slices compile
  and freeze, SHA-256
  `e5643897c866136bd788b242be0c983a2ae3aab511a1b7676c2d118be0200cd2`.

All other zero-indentation declarations are lazy functions. Their bodies are
parsed by the admitted Starlark dialect but remain outside this proof.

## Authorities and decision

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior
authority. Pinned `net/starlark/java/eval/testdata/loop.star` and
`comprehension.star` establish tuple destructuring, nested comprehension clause
order, formatting evaluation and list result order. The authenticated
rules_rust source fixes the concrete strings and pairs.

Slug's existing Starlark evaluator already owns tuples, lists, strings, Boolean
constants, nested comprehensions, formatting, aliases and recursive freeze. Add
no production behavior. Concatenate only the five exact slices plus one clearly
proof-only struct projection retaining the three private values. The fifth
slice closes only a lazy compiler/freeze dependency; it does not select
`_replace_all` behavior. Evaluate under exact producer
`@@rules_rust+//rust/private:utils.bzl` and invoke no function.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its defining-module freeze traversal demonstrates that composites, aliases and
function references reachable after evaluator closure must all remain frozen.
This guides proof shape only. Copy no Zig code, representation, owner pointer,
ordering algorithm, capture algorithm, diagnostic, identity or behavior.

The Buck2 utility review selects no action. This is proof-only and changes no
retained structure, hash, compact collection/string, interner, clone path,
graph storage or memory accounting.

## Compatibility

- **Exact:** exact bytes/hash of the five source slices; exact producer;
  ordered unsupported-feature strings; all 63 ordered substitution pairs; and
  the two source alias identities/types after freeze.
- **Slug-native:** proof-only projections exposing private constants and
  starlark-rust frozen list/tuple/function representation.
- **Unsupported/deferred:** exact whole-file freeze; invoking `_replace_all` or
  any other utility; utility results/diagnostics; configured toolchain/allocator
  behavior; and later utils, allocator or parent source.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `eb24c4670afeca68b8e966a0348adeb2daa1edeb553e4db35cae1120fd0f7d1b` | 8,160 | 8,410 |

Caps are 0 production, 250 proof and 250 total additions; deletions do not buy
addition budget. Keep the test function at or below 120 lines. Exact source
constants are exempt from the function limit.

Required proof:

1. Embed the five exact unabridged slices at lines 32-42, 73, 601-650, 664-676
   and 692-740 separately, and verify each SHA-256 above. The last slice is a
   non-invoked compiler/freeze dependency only. Do not embed other utils source
   or a 1,032-line fixture.
2. Concatenate the slices with one named proof-only struct projection exposing
   the private false kill switch and the private targets of both aliases.
   Freeze under exact producer
   `@@rules_rust+//rust/private:utils.bzl`.
3. Prove `UNSUPPORTED_FEATURES` is exactly `thin_lto`, `module_maps`,
   `use_header_modules`, `fdo_instrument`, `fdo_optimize`,
   `rules_rust_unsupported_feature` in order.
4. Prove the kill switch is false. Prove by frozen pointer identity that
   `substitutions_for_testing` aliases the derived list and contains exactly 63
   ordered two-string tuples: the leading `_z -> _zz_`, then clobber-protection
   and encoding rows for each of the 31 authenticated encoding pairs in source
   order.
5. Prove by frozen pointer identity that `encode_raw_string_for_testing` aliases
   `_encode_raw_string` and freezes as type `function`; invoke neither it nor
   any other helper.
6. Preserve every accepted keyword-only, struct, descriptor, rules_cc, clippy,
   lints, rustfmt, paths and find-toolchain proof.

No new oracle fixture is needed: authenticated source plus pinned Bazel
Starlark comprehension regressions discriminate the eager-value evidence gap.
The broader rules-rust oracle exercises configured behavior and is deliberately
skipped because that phase remains unsupported here.

The 8,160-line test file is large but remains the cohesive owner of exact
external-Bzl loading fixtures. The packet adds one isolated set of source
constants and one sub-120-line test; splitting loading fixtures or production
orchestration would widen scope.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused exact utils eager-values proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify slice bytes/hashes, source line anchors,
exact producer, ordered 63-pair derivation, alias identity, lazy nonexecution,
Zabel guidance-only role, validation and caps.

STOP and `REPLAN` for production change; copied full utils source; utility
invocation; configured/toolchain/allocator behavior; another child; identity/
registry/DICE work; Java/JVM work; copied Zabel content; dirty authority;
skipped source order; or cap violation.

## Immediate predecessor

`8609b3623` selected the post-find-toolchain audit. It authenticated all cached
children and found no unsupported eager expression, only this bounded proof gap.
