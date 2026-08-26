# Current Slug V2 Packet

Packet: `WP-4-7A-lints-child-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: recursively freeze exact rules_rust 0.73.0
`rust/private/lints.bzl`, prove its provider and ordered rule-schema identities,
and stop when this child returns.

## Accepted starting point and source order

Base is `614114027` (`Select post-clippy parent audit`) plus the accepted code
commit `db51996b9`. `rust/defs.bzl` has SHA-256
`5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`
and direct-load order toolchain, clippy, common, lints. The exact clippy child
now returns; common and providers are already complete through its recursive
loads. Therefore the first new child is `rust/private/lints.bzl`.

Authenticated sources:

- `lints.bzl`, 98 lines, SHA-256
  `0c6dcf615bb9f43d57c4056253f89a9f1bed0b16b9e17d8eed64da85d1b05677`;
- `providers.bzl`, SHA-256
  `57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`;
- cached `common.bzl`, SHA-256
  `cee50122624c7fd9c9a6545a647062f350dd25bc8cf6dda873944290463d4db6`.

The lint function body and its `LintsInfo(...)` call remain lazy. The sole eager
declaration is `rust_lint_config = rule(...)` with ordered `rustc`,
`rustc_check_cfg`, `clippy`, `rustdoc` attributes of kinds StringDict,
StringListDict, StringDict and StringDict. Each has documentation and omits its
declaration default (`None`). The already-accepted later invocation projection,
which is deferred here, supplies typed empty dictionaries.

## Authorities and decision

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StarlarkAttrModule.stringDictAttribute` and `stringListDictAttribute` select
the exact dictionary kinds and preserve ordinary named doc/default policy;
pinned rule-class tests discriminate their typed defaults. The existing Slug
constructors, documentation validation, rule projection and provider export
owner already cover this source. The remaining gap is proof only.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Its producer-owned imported provider identity and declaration-owned
`RuleDefinition.attrs` order support using Slug's existing provider and schema
owners. Copy no Zig code, representation, owner pointer, identity bytes,
capture, algorithm, diagnostic or behavior. Bazel 9.2 decides compatibility.

The Buck2 utility review selects no action. This packet adds only test source
and assertions; it changes no retained data structure, hash, compact
collection/string, interner, clone path, graph storage or memory accounting.

## Compatibility

- **Exact:** recursive unabridged lints-child freeze, imported LintsInfo
  producer identity, exact implementation source binding, rule export identity,
  and ordered name/kind/omitted-default schema.
- **Slug-native:** existing frozen Rust/Arc storage and test-only identity
  probes.
- **Unsupported/deferred:** lint rule/helper execution; LintsInfo construction;
  configured dictionary values and validation; configured provider, analysis,
  action and execution behavior; and the parent frontier after lints returns.

## Allowlist, proof and caps

Only this file may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `e9d74d339a754fc00d50361979cdbfed4c8f80659169e332739934acd29510c3` | 7,394 | 7,614 |

Caps are 0 production, 220 proof and 220 total additions; deletions do not buy
addition budget. Keep each function at or below 120 lines; the exact source
constant may exceed that limit.

Required proof:

1. Embed exact unabridged `lints.bzl:1-98` and verify its SHA-256 against the
   authenticated source slice.
2. Freeze it through the existing loaded-child harness with a provider child
   exporting `LintsInfo` under the exact producer identity.
3. Prove the imported `LintsInfo` is pointer-identical to the provider-child
   export and that successful freeze did not invoke `_rust_lint_config` or
   construct a provider value.
4. Prove the exact source binds `implementation = _rust_lint_config`, both
   values freeze successfully, and `rust_lint_config` retains the lints-child
   export identity. Do not claim direct pointer inspection of the private
   frozen implementation field.
5. Prove exact ordered declared names/kinds, nonmandatory/configurable policy,
   and omitted (`None`) declaration defaults for all four attributes. Typed
   empty invocation values remain existing evidence, not a new claim.
6. Preserve every accepted clippy, lint-test, rustfmt and dictionary-schema
   proof.

No new oracle is needed: pinned source, Bazel constructors/tests and the exact
recursive loading proof discriminate this child.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused exact lints-child proof;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked`;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify source bytes/hash, source order,
producer/schema proof, lazy nonexecution, compatibility boundary, Zabel
guidance-only role, utility decision, validation and caps.

STOP and `REPLAN` for a production change; invocation/configured behavior;
another child; provider construction; identity/registry/DICE work; Java/JVM
work; copied Zabel content; dirty authority; skipped source order; or cap
violation.

## Immediate predecessor

`614114027` selected the post-clippy parent audit. It authenticated lints as the
first new child and found only a missing exact recursive proof.
