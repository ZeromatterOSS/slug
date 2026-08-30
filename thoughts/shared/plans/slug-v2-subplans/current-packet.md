# Current Slug V2 Packet

Packet: `WP-4-5-7A-registered-toolchain-generated-repository-proof`

Milestone: M7A category 6 generated-repository closure.

Base: terminally accepted canonical repository Host-capability implementation
commit `26a68d61c`. The retained selected-context R2 candidate is the explicit
live baseline in three proof files and must remain isolated from this packet.

## Observable result

Proof only: the exact Bazel 9 `@bazel_tools` MODULE registration row expands in
source declaration order through the generic loading path after the generated
`local_config_winsdk` repository is realized. On a non-Windows Host, its
`@local_config_winsdk//:all` row contributes no selected toolchain while the
three catalog-backed rows retain their existing exact selections.

The proof must also demonstrate that this generic repository realization does
not change the already-retained custom selected implementation,
`ctx.toolchains` projection, configured action ownership, or REAPI output.
There is no production Rust, parser, builtin, ruleset special case, selected-
context implementation, action, or REAPI behavior change.

## Authority and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and the authenticated verbatim BCR `@bazel_tools` catalog are sole semantic
authority. Its MODULE source declares, in order:

1. `//tools/launcher:all`;
2. `//tools/test:all`;
3. `@local_config_winsdk//:all`; and
4. `//tools/res:empty_rc_toolchain`.

The accepted Host-capability owner executes the actual pinned
`winsdk_configure.bzl`; its non-Windows branch writes an empty `BUILD` and the
exact `toolchains.bzl`, so row 3 expands successfully to an empty target set.
No `UnsupportedCatalog` may be demanded while resolving these four rows.

- **Exact:** source registration ordering, canonical labels, empty row 3 on an
  admitted non-Windows Host, the three existing catalog-backed selections,
  and unchanged selected custom implementation/`ctx.toolchains`/REAPI values.
- **Slug-native:** retained DICE activation identities and structural action or
  configuration ownership already admitted by predecessor packets.
- **Unsupported/deferred:** Windows winsdk realization, unimported catalog
  breadth, selected-context expansion beyond the retained R2 candidate, and
  new toolchain/action/REAPI semantics.

BCR Starlark owns all rule and registration control flow, including
`cc_internal`; `cc_common` is only a generic Host/provider ABI consumer. This
is not a `set` or C++ parser packet. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` remains architectural and
optimization guidance only, never behavior authority.

## Exact proof-only allowlist and baselines

No production Rust may change. Only these proof files may change, from their
exact live blobs:

- `app/slug_loading_v2/src/registration_expansion_tests.rs`
  `ce333ab6c6f4e79210ec216d710429e3cd9a575d`;
- `app/slug_loading_v2/tests/build_file_loading.rs`
  `7b1e2a98a54b8fa49ce4bda3c32c6d819f0771c4`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`
  `c37f74b275baaed29e10e7fc717ecca8f1ff675c`; and
- `app/slug_reapi_v2/tests/reapi.rs`
  `dd4f59cdf2bb4a8e00c5493aa09d17663f0d92ff`.

The latter three hashes deliberately include the retained dirty selected-
context candidate. Preserve those bytes exactly except for bounded new proof
that composes with them. Stage and commit only this packet's hunks.

Maximum additions: 900 proof Rust lines. No asset, fixture, catalog, manifest,
lockfile, production source, command, server, loader, repository implementation,
analysis implementation, action implementation, or REAPI implementation may
change. Tests must use exact authenticated sources/assets already present; no
semantic stub or ruleset-specific shortcut is allowed.

## Discriminating proof

Add one composed retained-DICE/loading proof that:

- consumes the actual built-in MODULE registration declarations rather than a
  copied four-string substitute;
- preserves all four source rows in declaration order and their canonical
  apparent-to-canonical routing;
- generically realizes the authenticated non-Windows winsdk repository before
  row 3 package expansion;
- obtains an empty row-3 expansion without requesting `UnsupportedCatalog`;
- retains the expected selections from launcher, test, and empty RC rows;
- proves warm reuse and a relevant generated-repository dependency transition
  without changing source order or inventing a fallback; and
- distinguishes the generated empty repository from a missing/unsupported
  repository terminal.

Extend the existing loading, command, and REAPI proof at their natural
assertion seams to show the newly admitted built-in closure leaves the retained
custom selected implementation, `ctx.toolchains` fields, configured action
context, and REAPI projection unchanged. Do not assert only counts or source
shape where a value/label/activation discriminator is available.

Run focused registration/loading tests, full serial `slug_loading_v2`, direct
`slug_core_v2` and `slug_reapi_v2` affected tests, and `slug_bzlmod_v2` if the
composed proof activates its owners. Then run `cargo fmt --all`,
`git diff --check`, exact scope/blob/cap/dirty-isolation audits, and
`scripts/v2_archive_status.sh`. Do not run Cargo commands in parallel on the
shared target directory; clean stale `slugd` before and after daemon-sensitive
proof.

## Stops and successor

`REPLAN` for any production edit; copied or synthetic replacement for the real
four registration declarations; ruleset/winsdk special case; semantic stub;
catalog/fixture mutation; demanded `UnsupportedCatalog`; nonempty row 3 on an
admitted non-Windows Host; order loss; parser or builtin work; selected-context,
action, or REAPI semantic change; change outside the four files/900-line cap;
or inability to isolate the retained dirty candidate.

After terminal `ACCEPT`, return the retained selected-context R2 candidate to
independent latest-diff review before accepting or correcting it.
