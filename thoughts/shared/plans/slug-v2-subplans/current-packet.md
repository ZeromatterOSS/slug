# Current Slug V2 Packet

Packet: `WP-4-5-7A-builtin-bazel-tools-repo-package-catalog-implementation`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: accepted generated-repository `.bzl` routing and canonical extension-owner
projection `f747507f6`. The proof-only registration and selected-context
candidates remain dirty, parked, and read-only.

## Immediate predecessor

Commit `f747507f6` terminally accepts the generic Root/Canonical child-route
handoff and canonical definition-host owner projection. Full Bzlmod and loading
suites, direct CLI compilation, and independent terminal review passed. Two
fresh-workspace/fresh-output-root rules_rust cqueries then produced the same
result: the selected rules_cc parent resolved
`rules_cc++compatibility_proxy+cc_compatibility_proxy`, authenticated owner
`@@rules_cc+//cc:extensions.bzl % compatibility_proxy`, evaluated the generated
repository, and next stopped at:

```text
UnsupportedCatalog { path: "tools/build_defs/repo/utils.bzl" }
```

The outer context still names `@cc_compatibility_proxy`, but the retained route,
owner, mapping, and source observations prove that the predecessor boundary is
closed. This packet owns only the newly exposed exact built-in content boundary.

## Observable boundary and category

Slug's immutable `@bazel_tools` catalog has no
`tools/build_defs/repo` package. The real rules_cc extension transitively loads
`utils.bzl`; Bazel's embedded-tools archive supplies that file as one member of
one complete direct package, not as an isolated bootstrap artifact.

The packet imports the complete package category at once:

```text
tools/build_defs/repo/BUILD          <- pinned BUILD.repo
tools/build_defs/repo/cache.bzl
tools/build_defs/repo/git.bzl
tools/build_defs/repo/git_worker.bzl
tools/build_defs/repo/http.bzl
tools/build_defs/repo/java.bzl
tools/build_defs/repo/jvm.bzl
tools/build_defs/repo/local.bzl
tools/build_defs/repo/utils.bzl
```

This avoids repeated manifest and package-inventory churn as repository
builtins load sibling helpers. It does not admit the upstream documentation
templates, generated Markdown, development `BUILD`, or any other
`@bazel_tools` package.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
content and behavior authority:

- `tools/build_defs/repo/BUILD` declares `embedded_tools` as every direct
  `*.bzl` plus `BUILD.repo`.
- `src/create_embedded_tools.py` maps `BUILD.repo` to the archive path
  `tools/build_defs/repo/BUILD`; all direct `.bzl` paths are preserved.
- The embedded set is exactly the nine files above, 2,513 text lines and
  96,027 bytes. Every member is nonexecutable.
- The pinned SHA-256 values are:

| Output path | Pinned source | SHA-256 |
|---|---|---|
| `BUILD` | `BUILD.repo` | `58fc51781cf26bfbcbd2c615f4cd0bd64892c3f7332e403eb1a885fea27ff3ca` |
| `cache.bzl` | same | `119c3fb281fcb02ce8aa0cd2f4fa315830ab160b483e4e041986422d2294d15b` |
| `git.bzl` | same | `c4f89658b4465dc4e42f87312b74d549fb434197bf0ade88fc4276550f68811b` |
| `git_worker.bzl` | same | `0bf607d50370d151bba1b541e8023ff040527f50f8fa8884157002ed9c63c339` |
| `http.bzl` | same | `9e908b9d6491cb950a9713d8b758b7b6f83871adbc768eb4997ca12e06ac240a` |
| `java.bzl` | same | `94fa09f776bb93a5ed3de1fccdb3a8f22c8792d01e5d7df6d588817b2cf02d7d` |
| `jvm.bzl` | same | `b3e2ff70d3706171123636248d7175dcb0046bbedea776016d49befc7a810309` |
| `local.bzl` | same | `f41d310ee3fcef8a637ddff5b21eb05724ad377bbb1b679d146327478613e4db` |
| `utils.bzl` | same | `902f228e729bb7ee86f86a3d434ccbddd9350bb5c7c869fa2f5fda90361605db` |

No additional Bazel oracle is required: pinned source bytes, Bazel's own
embedded-tools membership rule, and the existing exact catalog regression are
stronger and more direct evidence for this content-only change. Evaluation may
expose a later language or Host-ABI gap; that observation selects a successor
and does not widen this packet.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
**concept/optimization guidance only**. Its authenticated embedded roots retain
repository-relative paths, whole-source digests, file bytes, and explicit
executable modes. Slug already has the analogous immutable catalog shape, so
reuse the idea, not Zabel code, source selection, scheduler, store, or semantic
claims. Do not use Zabel as authority for membership or bytes.

## Decision and compatibility

Add all nine verbatim archive members to the existing versioned
`BuiltinBazelToolsSnapshot::Bazel9_2` catalog. Extend its derived directory
listing, package-set proof, exact checked-in-assets ledger, and immutable
manifest expectations. Do not add a new repository source, key, evaluator
branch, path fallback, materialization, or special case for rules_cc,
`cc_common`, `cc_internal`, or `utils.bzl`.

- **Exact:** archive-relative paths; `BUILD.repo` to `BUILD` transform; all
  file bytes, SHA-256 values, nonexecutable modes, direct directory entries,
  package-marker visibility, and resulting catalog manifest identity for the
  admitted snapshot.
- **Slug-native:** Rust static catalog representation, manifest framing,
  DICE key/type names, `Arc` storage, error types, and package-set carrier.
- **Unsupported/deferred:** all other embedded-tools packages; repository-rule
  execution not already admitted; any newly exposed Starlark builtin or Host
  ABI; physical built-in materialization; exact JVM/HotSpot state; C++ rule or
  action semantics; and later bootstrap action/input-tree/REAPI breadth.

Non-decisions: no parser or `set` work; no Rust-defined rule implementation;
no change to the Buck2-derived Starlark engine; no repository function
semantics; no docs/templates; no fallback scan of `../bazel-9.2.0`; no runtime
dependency on the Bazel checkout; and no Java/JVM component.

## Ownership, request, revision, and memory

The existing `CATALOG` in `builtin_repository.rs` remains the sole producer of
file bytes, modes, directory listings, and the versioned manifest. Existing
`BuiltinBazelToolsSourceFileKey` and repository source-observation/load keys
consume that immutable snapshot without new dependencies. Package discovery
continues to derive markers from the same catalog listing.

The snapshot is compile-time static service memory; source values retain
existing shared immutable byte storage. There is no command overlay, mutable
host read, cache, lock, background task, async transfer, cancellation path, or
shutdown obligation. All requests and overlapping DICE transactions observe
the same structural route identity. The changed manifest intentionally
invalidates prior catalog-derived routes once; A/B/A within the built binary
remains stable.

No DICE key, equality implementation, or lock changes. The existing source key
still owns invalid/wrong-kind/unsupported separation and `Need` behavior. No
retained value borrows evaluator or command scratch.

## Proof matrix

Repository-owned tests must prove:

1. all nine checked-in files equal the pinned Bazel sources byte-for-byte,
   have the recorded SHA-256 values, and are nonexecutable;
2. checked-in built-in assets are exactly the reviewed catalog—no unlisted
   payload or omitted member;
3. `tools/build_defs/repo` lists `BUILD` plus all eight `.bzl` files in lexical
   direct-child order and is discovered as a package under both root and
   `tools` subtree queries;
4. ordinary built-in source reads return `utils.bzl` and at least one sibling
   through the existing immutable owner, while invalid/wrong-kind/unsupported
   errors remain distinct;
5. manifest identity is identical across DICE instances/transactions and the
   previously admitted `tools/build_defs/cc` package remains exact; and
6. two fresh rules_rust cqueries advance beyond `UnsupportedCatalog` for
   `tools/build_defs/repo/utils.bzl` and stop identically at the next authentic
   boundary or succeed. Do not repair that successor in this packet.

No new fixture is admitted. Reuse the existing real rules_rust workspace and
fresh temporary output roots. The upstream documentation and repository-rule
tests are skipped because they test evaluation semantics outside this
content/catalog change; existing Slug loading suites cover the admitted read,
listing, marker, and package-set behavior.

## Allowlist, caps, validation, and stops

Frozen existing-file blobs at `f747507f6`:

- `app/slug_bzlmod_v2/src/builtin_repository.rs`
  `df1550cc7f31bd206eb56c1599c706d1c2535193`;
- `app/slug_bzlmod_v2/src/host_module.rs`
  `0546ea5dc3a03823fe65bce175b6cbb1ea1ce518`;
- `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`
  `bb8768af72d3995f3d873d6afb07b29f7b664429`; and
- `app/slug_loading_v2/src/external_subtree_package_set_tests.rs`
  `3f390d3320dfc3945838dfbc9f0b01bb19acf6f4`.

Only those four existing files, the nine new files beneath
`app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/repo/`, and scheduling
documents may differ from the base. Copied exact-source growth is fixed at
2,513 lines/96,027 bytes. Cap net Rust production growth at 75 lines and net
Rust test growth at 100 lines. No new crate, dependency, key, fixture, unsafe
code, task, lock, cache, fallback, public stability shim, or executable asset.

`host_module.rs` exceeds the 2,000-line review trigger, but its only permitted
change is the separately pinned manifest digest assertion; catalog ownership
does not move there. No touched function may gain a new responsibility or
cross 150 lines. This is immutable content breadth, not a changed retained
representation or demonstrated hot path, so no Buck2 utility or performance
experiment applies.

Validation:

1. source-vs-pinned byte/hash/mode inventory check;
2. focused built-in catalog, listing, source-read, and package-set tests;
3. complete `cargo test -p slug_bzlmod_v2` and
   `cargo test -p slug_loading_v2` serially;
4. `cargo build -p slug_cli_v2`;
5. clean stale `slugd`, then two fresh-workspace/fresh-output-root real
   rules_rust cqueries and clean stale `slugd` afterward;
6. `cargo fmt --all -- --check`, `git diff --check`, allowlist/cap and dirty-
   isolation checks; and
7. independent terminal patch review.

`REPLAN` before implementation if pinned membership or the `BUILD.repo`
transform differs; if any new production owner, source kind, key, dependency,
fallback, materializer, parser/builtin, Host ABI, repository execution, or rule
semantics must change; if a file outside the allowlist is required; if copied
bytes cannot remain verbatim; if existing cc catalog behavior regresses; or if
caps are exceeded. After implementation, a new authentic evaluation terminal
is successor evidence, not permission to broaden this packet.
