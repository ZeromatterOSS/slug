# Current Slug V2 Work Packet

Packet: WP-7-81-m7a-builtin-cpp-source-catalog-r1
Status: accepted (independent design and final ACCEPT)

## Outcome and compatibility

Install the exact Bazel 9.2 embedded source entries for
`@@bazel_tools//tools/cpp` and `//tools/cpp/modules_tools` into Slug's existing
built-in repository catalog. This removes the current `NoBuildFile` source-route
failure and makes the real BUILD and sibling `.bzl` bytes available through the
same tracked route as accepted built-in content. It is an M7A source-ingress
prerequisite, not authentic complete package Loading or configured C++ admission.

Exact: upstream source bytes, installed path mapping and file modes for the
selected entries, directory-listing kind/order, per-file SHA-256 and source-file
lookup. Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
owns the source. `src/create_embedded_tools.py:31-70` maps
`tools/cpp/BUILD.tools` to installed `tools/cpp/BUILD` and prefers it over
the developer-tree `BUILD`; its general `BUILD.tools` mapping applies to
`tools/cpp/modules_tools/BUILD.tools` too. Slug-native: the existing
domain-separated catalog-manifest/DICE route identity, not Bazel's checksum.
Unsupported/deferred: recursive `@rules_cc//cc:cc_library.bzl` and generated
compatibility-proxy Loading, ordinary repository-package Starlark `deps`,
configured `cc_library`/full `cc_toolchain`, module-tool binary acquisition,
config_setting analysis, actions/REAPI, bootstrap and M8. Do not claim a
successful complete tools/cpp package load from a copied BUILD or a stub rule.

Predecessor WP780 at `fa23e0707` accepted native `label_flag` with independent
design/final ACCEPT and 20 focused tests. WP779 accepted native filegroup.

## Source set and owners

Port these seven source files verbatim, with upstream bytes and installed names:

- `tools/cpp/BUILD.tools` -> `builtin/bazel_tools/tools/cpp/BUILD`;
- `tools/cpp/cc_flags_supplier.bzl`, `cc_toolchain_config_lib.bzl`,
  `compiler_flag.bzl`, `toolchain_utils.bzl`, `unix_cc_toolchain_config.bzl`;
- `tools/cpp/modules_tools/BUILD.tools` ->
  `builtin/bazel_tools/tools/cpp/modules_tools/BUILD`.

Retain the three already accepted C++ configure `.bzl` files unchanged.
`tools/cpp/BUILD.tools:14-128` owns the genuine cc_library load, native
declarations, `label_flag`, filegroups and `glob(["*.bzl"])`; all eight top-level
`.bzl` files must be present before any later complete-load claim.
`modules_tools/BUILD.tools:1-22` declares OS-selected binary filegroups;
its `aggregate-ddi{,.exe}` and `generate-modmap{,.exe}` leaves are generated
installed artifacts with separate release-platform provenance, not files to
invent or copy from a source-tree developer BUILD.

`app/slug_bzlmod_v2/src/builtin_repository.rs` remains the sole catalog,
source-key, listing and manifest-identity owner. Add constant `CatalogEntry`
rows in stable path order and existing SHA-256 validation, with no new key,
lock, interner, cache, hash algorithm or mutable state. Its `Arc`-shared
source values remain DICE-retained; include_bytes! are process/static storage.
The manifest identity must change structurally with every selected byte/mode
and remain distinct from per-file content hashes. Existing
`HostRoutePackageLookup` and built-in source observations consume the catalog
through DICE; no direct filesystem fallback or injected semantic snapshot.
`app/slug_bzlmod_v2/BUILD.bazel` and package-local `BUILD.bazel` files expose
only compile_data, not Bazel semantic aliases. Consult
`docs/developers/dice.md` and the Stage 9 utility decision; retain existing
`Sha256`, compact source handles and Allocative ownership, with no donor import
or performance claim.

## Scope, routing and stops

Base `fa23e0707`; branch `codex/wp781-builtin-cpp-catalog-review`. Worker1
owns only the seven upstream file ports, catalog entries and Bazel compile_data
metadata. Worker2 owns only `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`
once Worker1 publishes entries; root owns `builtin_repository.rs`'s internal
listing/manifest tests and `host_package.rs`'s package-marker test,
integration, docs and serial validation. Reviewer checks the design before
implementation and the final actual diff/evidence. Estimate under 20 KiB
upstream source, under 150 Rust production lines and under 220 proof lines;
these are review triggers, not caps. Do not change Loading/Analysis behavior,
MODULE/lock, `rules_cc` fixtures, generated binaries or unrelated catalog files.

No new fixture is required: reuse built-in catalog tests and exact pinned
source hashes. A `NoBuildFile` -> `Package(Build)` marker discriminator must
run through the existing observed built-in route, without evaluating the
unavailable recursive `rules_cc` closure. A later packet must authenticate
the public wrapper, generated proxy and its eager private children, then
separately admit ordinary repository-package Starlark deps before claiming
complete package Loading. Missing proxy/module binaries must remain errors,
not hidden placeholders. REPLAN only if the pinned installed mapping differs
from the selected bytes, a source entry cannot be represented by the existing
catalog, or the bounded source-route test needs a new semantic owner.

## Acceptance

Compare every selected file byte-for-byte to pinned upstream and record
SHA-256/mode; do not use developer-tree `tools/cpp/BUILD`. Verify sorted
unique direct listings for `tools/cpp` and `tools/cpp/modules_tools`, typed
missing-source errors for binary leaves, exact DICE source values and manifest
identity byte/mode discrimination. Prove the built-in route now selects
`tools/cpp/BUILD` as a package marker and retains observed source/mapping
identity, without promoting that marker to a loaded package or target graph.
Preserve existing catalog invariants and accepted content; old tests remain
passing. Focused Bzlmod unit/integration tests and one direct Loading compile
dependent, with pinned nightly toolchain freshly resolved, serial offline/locked
Cargo JSON `--no-run` preparations capped at 60s, exact nonignored preflight
and executed pass counts. Run Rust formatting on changed files, diff/archive/
plan-status checks and independent final review. No Slug binary, Bazel,
daemon, NativeLink or BuildBuddy run is selected; a fresh oracle is unnecessary
because upstream source bytes and existing catalog behavior discriminate this
source-only change.

Independent design review ACCEPT confirms the seven-file installed mapping,
0644 modes, existing DICE/catalog owners and explicit source-only boundary.

## Acceptance receipt

Seven installed files compare byte-for-byte with pinned Bazel 9.2 source; all
seven SHA-256 values match their catalog rows and each installed mode is 0644.
The new manifest identity is
`d614eabb9f52bb03a0c51a96cc942c8dadb669929b18a9338ae7286d23d6fa40`.
The observed built-in route returns `Package(Build)` for both C++ directories;
missing generated module-tool binaries remain typed unsupported-catalog leaves.
This advances only source/catalog ingress, not complete package Loading.

Pinned nightly, offline/locked Cargo JSON `--no-run` succeeded; exact preflight
selected the focused unit and integration tests. Full `slug_bzlmod_v2` library
suite passed 626/626, and the catalog integration suite passed 4/4. The direct
`slug_loading_v2` Cargo check passed. Changed-file rustfmt, `git diff --check`,
plan-status and archive-status checks passed. Independent final review ACCEPT
audited exact upstream bytes/modes and the bounded compatibility claim. The
packet took about eight minutes elapsed; Rust preparation/check took about one
minute total, test execution under one second, with the remainder in source
audit, integration and independent review. No replan was needed.
