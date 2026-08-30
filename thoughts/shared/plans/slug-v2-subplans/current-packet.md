# Current Slug V2 Packet

Packet: `WP-4-5-7A-builtin-bazel-tools-cc-direct-package-catalog-implementation`

Milestone: M7A registered-toolchain closure prerequisite.

Base: accepted external-.bzl source-observation cutover `1b997c5ef`,
accepted TestingBootstrap loading ABI `ecee4aca5`, and accepted selected-BCR
realization `1599d730c`. The proof-only registration and selected-context
candidates remain dirty, parked, and read-only.

## Observable boundary

The preceding packet preserves exact Root-request source children and routes
Root built-ins plus Canonical external `.bzl` reads through the shared
immutable observation owner. Its focused and full suites pass, independent
terminal review returns `ACCEPT`, and two fresh-workspace/fresh-output-root
rules_rust cqueries clear the old immutable-owner guard and stop identically at:

```text
HostRepositorySourceObservationError ... Builtin(
    UnsupportedCatalog {
        path: "tools/build_defs/cc/action_names.bzl",
    },
)
```

This is the catalog's typed fail-closed result. It is not a parser, `set`,
`cc_common`, `cc_internal`, rules_rust, TestingBootstrap, or C++ semantic
failure.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior and content authority:

- `src/create_embedded_tools.py`, `src/BUILD`, `tools/BUILD`, and
  `tools/build_defs/BUILD` select `//tools/build_defs/cc:srcs` into the
  immutable `@bazel_tools` repository;
- `tools/build_defs/cc/BUILD` owns one direct package. Bazel package
  boundaries exclude the `tests` and two `whitelists` subpackages, so the
  complete direct file set is exactly `BUILD`, `action_names.bzl`, and
  `cc_import.bzl`;
- all three upstream tree entries are ordinary non-executable files;
- the exact pinned files are:

  | Path | Bytes | Lines | SHA-256 |
  |------|------:|------:|---------|
  | `tools/build_defs/cc/BUILD` | 838 | 43 | `a24f1afcd5bfaaf9fc88ae3455213c83d61988bac5a80e58dd9f954281f6009d` |
  | `tools/build_defs/cc/action_names.bzl` | 5,400 | 135 | `ede4d3bd51a2a772180a0f3a47cf083e898d4104ec8de27f30ca36a5b8c13951` |
  | `tools/build_defs/cc/cc_import.bzl` | 889 | 24 | `a11736b1cf82a1216b62b6c8af280d739721c6dde470ff83cd939112a0a84093` |

- `action_names.bzl` is ordinary Starlark data. Buck2-derived starlark-rust
  already owns parsing and `set`; this packet adds no language feature; and
- `cc_import.bzl` is admitted because it shares the same exact upstream
  package/category, not because the current dependent happens to request it.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer design and optimization
guidance only. Its scoped embedded read and immutable digest/manifest ownership
support using Slug's existing authenticated catalog and avoiding physical
materialization. Zabel's bytes, repository layout, scheduler, and semantic
claims are not authority.

## Compatibility classification

- **Exact:** the three pinned Bazel 9.2 source byte sequences, archive modes,
  paths, direct directory listing, per-file SHA-256 values, catalog integrity
  failures, and the resulting versioned catalog manifest identity.
- **Slug-native:** the existing manifest framing/domain, Rust catalog table,
  DICE key names, and in-memory `Arc<[u8]>` representation. These already
  preserve collision-safe structural identity and immutable sharing.
- **Unsupported/deferred:** the `tools/build_defs/cc/tests` and `whitelists`
  subpackages, every other uncataloged `@bazel_tools` category, physical
  materialization, C++ action semantics, configured testing/coverage
  invocation, Windows-only discovery, and later action families.

No parity claim widens beyond the three admitted files. BCR Starlark remains
the complete rule/control-flow owner, including `cc_internal`; `cc_common`
remains only a consumer of the generic Host/provider ABI.

## Frozen architecture

Use only the accepted `BuiltinBazelToolsSnapshot::Bazel9_2` catalog:

1. check in the three upstream files verbatim below
   `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/cc/`;
2. add three sorted `CATALOG` entries with exact path, `include_bytes!`,
   SHA-256, and `executable: false`;
3. add the same three sorted rows to the integration test's reviewed `FILES`
   ledger;
4. extend the existing direct-directory proof to require exactly
   `BUILD`, `action_names.bzl`, and `cc_import.bzl`, all files; and
5. update the exact route-manifest assertion to the identity derived by the
   unchanged manifest algorithm.

Do not add a source key, cache, parser path, evaluator special case, repository
name/path branch, fallback, copied-byte adapter, or physical materialization.
The existing catalog key remains the sole source owner, validates each file
before publication, and returns its static bytes through the existing zero-copy
observation path.

The manifest continues to frame the snapshot tag, ordered entry count, each
path, file digest, mode, and byte length. Adding this complete package must
therefore change route identity. Existing manifest byte/mode discrimination,
cross-DICE invariance, wrong-kind, unsupported-catalog, and exact checked-in
asset tests remain authoritative.

## Frozen implementation successor

Implement only this packet with exactly:

- `app/slug_bzlmod_v2/src/builtin_repository.rs` blob
  `5b18c39f037ca59d372d9dc31848d390c3e9c7ce`;
- `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs` blob
  `6215a27fe725e0df8e4d2fe9ee1ce3ada28cb5e3`;
- new exact asset
  `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/cc/BUILD`;
- new exact asset
  `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/cc/action_names.bzl`;
  and
- new exact asset
  `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/cc/cc_import.bzl`.

Cap additions at 40 production Rust lines, 60 proof Rust lines, 230 exact asset
lines/7,200 asset bytes, and 330 aggregate Rust-plus-asset lines. Existing file
size is not permission to refactor the catalog owner or test ledger.

No loading, package, registration, analysis, command, core, REAPI, Cargo,
parser, starlark-rust, `set`, `cc_common`, or `cc_internal` file may
change. The thirteen pre-existing dirty files and all parked proof/selected-
context hunks remain byte-for-byte unstaged.

## Required proof and validation

1. Verify the three checked-in files byte-for-byte against the pinned Bazel
   commit, including SHA-256, byte count, line count, and non-executable mode.
2. Run the exact catalog bytes/digests/modes, checked-in-assets, direct
   directory-listing, manifest identity/discrimination, wrong-kind, and
   unsupported-catalog proofs.
3. Run complete serial `slug_bzlmod_v2` and `slug_loading_v2` suites.
4. Rebuild `slug_cli_v2`, clean daemon state, and run two real rules_rust
   cqueries from independent fresh workspace/output roots. Both must clear
   `action_names.bzl`'s catalog error and stop identically at the next
   independent boundary.
5. Run formatting, `git diff --check`, allowlist/blob/cap, dirty-isolation,
   archive, and no-parser/no-cc-special-case audits.
6. Obtain independent terminal implementation review before acceptance.

## Stops and ordered successor

`REPLAN` for any upstream byte/mode/hash mismatch; adding only
`action_names.bzl`; importing a child subpackage; changing manifest framing;
new source/evaluator/parser semantics; a C++/rules_rust/repository-name special
case; physical materialization; work outside the allowlist/caps; or inability
to isolate parked dirty state.

At terminal `ACCEPT`, rerun the unchanged four-row registered-toolchain proof
and classify the next real dependent boundary. Continue through generic
loading/provider/action owners only. Do not infer a Rust C++ rule engine from
the fact that `action_names.bzl` was the first missing consumer.
