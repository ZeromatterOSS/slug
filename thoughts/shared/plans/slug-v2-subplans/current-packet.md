# Current Slug V2 Packet

Packet: `WP-4-7A-post-rust-analyzer-source-order-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: recursive external-Bzl manifest, accepted rules_rust source order and next semantic owner
Base: `129ff448`

Result: identify the first newly evaluated unsupported expression after the
accepted `rust/private/rust_analyzer.bzl` module completes at line 484 and
returns through the recursive rules_rust load graph. Authenticate that exact
call against pinned Bazel 9.2, use pinned `../zabel` only for architectural
guidance, and write one bounded implementation packet or `REPLAN`. This is a
docs-only audit; do not edit Rust.

## Accepted starting point

Commit `129ff448` completes declaration-time loading of
`rust/private/rust_analyzer.bzl`:

- the selected defining module retains its immutable
  `rules_rust -> dep+` mapping while root apparent `dep_alias` remains
  distinct;
- `current_rust_analyzer_toolchain` freezes its one canonical requirement;
- `rust_analyzer_detect_sysroot` freezes, in order,
  `@@dep+//rust:toolchain_type` and
  `@@dep+//rust/rust_analyzer:toolchain_type`;
- missing or conflicting raw apparent rule strings fail closed; and
- neither implementation executes, so `ctx.toolchains`, provider/path
  semantics, actions and returned providers remain deferred.

Focused proof and all 256 `slug_loading_v2` tests pass. Locked core check,
rebuilt CLI, formatting and diff checks pass; archive status contains only its
known three retained thoughts paths. Growth is 7 production and 33 proof
additions, 40 total. Independent terminal review returned `ACCEPT`.

## Audit authority and route

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned Git objects and the accepted archive; the live sibling
checkouts may have different HEADs.

The known static return path is:

1. `rust/private/rust_analyzer.bzl` ends at line 484.
2. Its caller `rust/toolchain.bzl` next names
   `//rust/private:rustfmt.bzl` at lines 11-14.
3. `rust/private/rustfmt.bzl` first loads `common.bzl` and
   `lint_test.bzl` before its own provider/aspect/rule declarations.
4. Some children may already be complete and memoized through the accepted
   recursive closure. Static file order alone therefore does not prove the
   next evaluated expression.

Inspect Slug's recursive selected-route manifest/load order and the accepted
archive to distinguish cached modules from newly evaluated modules. Reuse an
existing deterministic load failure/trace if it proves the frontier; add no
fixture, network request or Bazel run. Record the exact module, line range,
expression and first missing Slug semantic fact.

For the selected expression, inspect the smallest authoritative Bazel 9.2 API,
implementation and focused tests needed to establish:

- constructor/call ABI and evaluation environment;
- defining module/package/repository context;
- retained equality, ordering, export and freeze semantics;
- whether any implementation function remains lazy;
- validation/failure timing; and
- the exact boundary before target invocation, configured dependencies,
  providers, toolchains, analysis or actions.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect only the owner/projection modules relevant to the actual selected
expression. Prefer its explicit-input, immutable-owner and thin-projection
lessons where they fit Slug's Rust architecture. Do not copy Zig code,
representation, mapping behavior, evaluator rules or DICE relations, and do
not treat a similarly named native surface as Bazel behavior.

Apply the Buck2 utility-reuse audit if the selected packet would alter a
retained hot-path representation, mapping, interning, hashing, compact
collection/string or memory accounting. Reuse existing owners unless the audit
proves a bounded split is necessary.

## Required audit result

Update only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

The replacement implementation packet must state:

- one exact result and first absent fact;
- Bazel 9.2 evidence and accepted rules_rust line anchors;
- the Zabel architectural guidance used and explicit non-copy boundary;
- exact, Slug-native and unsupported/deferred classifications;
- input/owner/revision/lifetime and invalidation consequences;
- an allowlisted file table with base SHA-256 and final line caps;
- production, proof and total addition caps;
- focused discriminating proof and serial validation commands;
- independent review requirements; and
- STOP/`REPLAN` boundaries before semantic widening.

Select the smallest source-order declaration or expression that advances the
accepted recursive load. If the first missing behavior requires invocation,
`ctx.toolchains`, configured dependencies, provider execution, actions, Java,
an unbounded API family or a new semantic owner that cannot fit one packet,
record the unsupported boundary or `REPLAN` rather than skipping ahead.

## Validation and STOP

Run `git diff --check`, verify the canonical plan and compact manifest name
the same packet, verify all cited pinned commits/archive identities, and run
`scripts/v2_archive_status.sh`. The archive checker may report only its known
three retained thoughts paths plus the active plan edits. Obtain independent
audit review before committing the selection.

STOP on Rust edits, a guessed frontier, current sibling HEADs used as authority,
an unpinned source, a new fixture/oracle/network request, target invocation,
`ctx.toolchains`, analysis/action changes, Zabel behavior/code adoption,
Java/JVM work, public rules_rust success claims or a packet without bounded
proof and caps.
