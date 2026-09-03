# Current Slug V2 Packet

Packet: WP-5-7A-bazel-tools-lib-cc-configure-catalog-audit

Milestone: M7A bootstrap-critical loading/repository execution closure. Audit
the exact built-in Bazel 9.2 source slice beginning at
`@bazel_tools//tools/cpp:lib_cc_configure.bzl`, the first honest boundary after
the accepted generic Unix repository executable lookup.

Status: ready for one bounded docs-only audit. No Rust or catalog-content edit
is authorized by this packet.

## Accepted predecessor

`WP-4-5-7A-repository-context-which-implementation-r1` returns `ACCEPT` after
one correction rereview at 240 production, 498 proof and 738 total gross Rust
additions. It adds no DICE key, direct filesystem or process-environment read,
retained candidate cache, materialization owner, rules_shell branch, shell-name
branch or toolchain special case.

The full loading suite passes and `slug_cli_v2` rebuilds. The authentic
rules_rust 0.73 replay, with `/bin:/usr/bin:/usr/local/bin` as its bounded
declared PATH, clears rules_shell 0.6.1's generic Unix `which` probes. The next
failure is the built-in catalog's typed `UnsupportedCatalog` result for
`tools/cpp/lib_cc_configure.bzl`, reached while apple_support configures its
generated local C++ repositories.

## Audit question

At pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, determine the complete smallest
verbatim `@bazel_tools` source/package/listing closure required to admit
`//tools/cpp:lib_cc_configure.bzl` through Slug's existing in-memory built-in
repository. The audit must distinguish:

1. exact upstream file bytes, executable modes, package membership and direct
   or recursive load dependencies that belong to this slice;
2. existing catalog entries and routing/listing/integrity owners that can be
   reused without changing their semantic identity;
3. any generated, configured, native-rule, platform or repository behavior
   referenced by the source but not required merely to load the admitted
   facade; and
4. the next independent replay boundary after an exact source-only addition,
   or `REPLAN` if the closure cannot be bounded without crossing another owner.

The authentic apple_support consumer is a discriminator only. It cannot select
alternate bytes, a reduced facade, a Rust implementation of Starlark behavior,
or an apple_support/C++/toolchain branch.

## Required evidence

- Inspect the pinned Bazel 9.2 source tree and record exact paths, SHA-256
  digests, modes, byte counts, loads and exported symbols for every proposed
  catalog member.
- Compare the installed Bazel 9.2 `@bazel_tools` repository when source-tree
  packaging or generated repository layout could differ.
- Trace the existing `BuiltinBazelToolsSnapshot`, manifest, source-file and
  directory-listing owners, including their exact integrity tests and update
  workflow.
- Reuse accepted catalog evidence where it already proves a byte-identical
  member. Add no new oracle unless a demonstrated source-versus-installed-tree
  ambiguity remains.
- Classify every result as exact, Slug-native or unsupported/deferred. Catalog
  content itself may be admitted only as exact verbatim Bazel 9.2 bytes.

## Audit allowlist and bounds

Documentation changes are limited to the canonical plan, Stages 4 and 5, and
this manifest. The audit is capped at 80 canonical-plan lines, 260 Stage 4
lines, 320 Stage 5 lines and 360 manifest lines. Prefer a much smaller record
when the exact closure is a single facade plus already-admitted dependencies.

Do not edit Rust, Cargo metadata, tests, fixtures, generated repositories,
catalog bytes or hashes during this audit. Do not run or embed a JVM helper.
Pinned Bazel may run externally only as an oracle when installed-tree evidence
is genuinely needed.

## Terminal result

Return `ACCEPT` only with a bounded exact-content implementation packet naming
the full file allowlist, integrity/update workflow, proof, replay gate and hard
size caps. Return `REPLAN` if the installed source differs from the pinned tree,
the direct load expands into an unbounded catalog subtree, existing manifest or
listing identity cannot safely own the addition, or loading the file inherently
requires a new semantic owner. Any later implementation must port upstream
content verbatim and must not synthesize, abbreviate or reinterpret it.
