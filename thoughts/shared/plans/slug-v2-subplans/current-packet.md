# Current Slug V2 Work Packet

Packet: WP-7-128-m7a-builtin-jdk-source-catalog-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Make the Bazel 9.2 embedded `@@bazel_tools//tools/jdk` runtime subtree a
complete, immutable Slug source-catalog package. Exact to the pinned archive:
the 13 file paths, bytes and executable bits; full Unix permission bits are
not a catalog representation. Slug-native: sorted catalog-derived directory
listings, typed source errors and domain-separated immutable DICE route
identity. Unsupported/
deferred: evaluating every rule in the JDK BUILD, configured JDK toolchains,
the full `@bazel_tools` archive, Java execution, and other package families.
The authentic `//:probe` replay must clear the missing-JDK-BUILD diagnostic;
a later Loading stop cannot widen this packet.

## Source, owner and decision

Pinned Bazel 9.2.0 source commit
`8220c6198837d5c13d53fea211cf3282aa12408a` in
`src/create_embedded_tools.py:27-30,63-87` maps `tools/jdk/BUILD.tools` to
runtime `tools/jdk/BUILD` and drops the distinct source-tree BUILD.
`tools/jdk/BUILD.tools:1-5,119-123` loads rules_java/rules_python and its
local `launcher_flag_alias.bzl`, and glob-selects all six local Bzl files.
The pinned Bazel 9.2.0 binary at the Bazelisk SHA-256 `7668a95d...c99048694`
contains precisely 13 `embedded_tools/tools/jdk/*` entries, all regular
0644. `BUILD` is 5,438 bytes with SHA-256
`0ea0dae7e42fdfb3f2fbe945fae3e84e57e263ac6affd4dc6a74b351868832a6`,
matching `BUILD.tools` upstream. Ignored-RC/no-fetch Bazel query proves the
JDK target family and `:bzl_srcs` inclusion of all six local `.bzl` files.
The 13-entry JDK ZIP subtree is the byte/executable-bit authority; never use the installed tree's
extraction permissions or the legacy 9.0 sync script.

Stage 5's `app/slug_bzlmod_v2/src/builtin_repository.rs::CATALOG` is the
natural immutable source owner. Its catalog-derived source-key value, directory
listing and manifest SHA-256 already provide typed missing/wrong-kind errors,
structural route identity and source observation. Append only the exact sorted
JDK entries and checked-in verbatim bytes under
`app/slug_bzlmod_v2/builtin/bazel_tools/tools/jdk/`; the current snapshot
identity changes deterministically with the expanded catalog. The runtime
`BUILD` creates a Bazel package boundary: a separate `BUILD.bazel` sidecar
uses the existing `slug_builtin_catalog_files` macro, and the parent
`builtin_bazel_tools_files` compile-data group must depend on it. The sidecar
is Slug build metadata, not a runtime catalog file. No DICE key, lock, side
cache, host filesystem fallback, or repository mapping changes.
The static source bytes are service/DICE-retained; no command scratch is
captured. This packet does not synthesize a smaller BUILD or claim that source
availability alone yields package-loading compatibility.

## Evidence and validation

Pre-change, assert the source key rejects `tools/jdk/BUILD` as outside catalog
and listing `tools/jdk` is Missing; the rebuilt authentic smoke already stops
with missing `@@bazel_tools//tools/jdk` BUILD. Then verify all 13 values against
the pinned ZIP paths/SHA-256/executable bits, the exact checked-in-path list, sorted
direct-child listings for `tools`/`tools/jdk`/`tools/jdk/nosystemjdk`, an
unsupported JDK sibling negative, deterministic two-DICE route identity, and
an explicit assertion that runtime BUILD matches BUILD.tools but differs from
source-tree BUILD. Exclude only the sidecar from the exact runtime catalog
path-list test. Reuse existing source-catalog test harness rather than adding
a Stage 1 fixture. Run selected Bzlmod tests with Cargo JSON `--no-run`, exact
nonignored preflight, focused/protected execution, direct dependent compiles
including Bazel's `//app/slug_bzlmod_v2:slug_bzlmod_v2` compile-data closure,
rebuilt CLI, and one normalized-PATH authentic `//:probe` replay.
Refresh the two existing exact route-manifest expectations in
`builtin_repository.rs` and `host_module.rs`, and run their protected tests.
The replay only proves the missing-content stop cleared. Check `slugd`
before/after; run pinned rustfmt, diff, plan and archive checks.

## Scope and stops

Base clean main `ff9e3ee45`. Allowlist: this manifest, canonical Current,
compact Stage 5 owner note after acceptance; 13 verbatim files under the JDK
builtin subtree; its noncatalog `BUILD.bazel` sidecar;
`app/slug_bzlmod_v2/BUILD.bazel` compile-data group;
`app/slug_bzlmod_v2/src/builtin_repository.rs` catalog/listing test;
`app/slug_bzlmod_v2/src/host_module.rs` one route-identity test expectation;
`app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs` source/manifest tests.
Catalog file is already large but remains the cohesive immutable embedded
source owner; no representation or key redesign. Expected production growth
about 78 catalog lines plus verbatim upstream bytes; test growth about 80
lines is a review trigger. Independent DESIGN and FINAL review required for
shared route identity. REPLAN if the binary archive differs from the pinned
source mapping, the 13-file subtree is incomplete for the declared runtime
package, or exposing it requires a new source/identity owner. Do not implement
the anticipated `java_plugins_flag_alias` Loading boundary here.

## Acceptance receipt

Independent DESIGN review first found the Bazel package-boundary sidecar gap;
the bounded contract correction was rereviewed and ACCEPTED. Independent FINAL
review ACCEPTED the actual bytes, catalog, route identity and build-input
closure. Before the catalog change, Cargo JSON `--no-run` compiled the selected
source-catalog test; exact nonignored preflight selected one test and it failed
at `UnsupportedCatalog { path: "tools/jdk/BUILD" }`. After the change, all 13
checked-in files matched the pinned Bazel archive SHA-256 and non-executable
bits; the runtime BUILD matches upstream `BUILD.tools`, not source-tree BUILD.
The integration test binary selected/passed 5/5, and the library unit binary
selected/passed 6/6 protected module/listing/route tests. The domain-separated
manifest is `a68101afc04f6c48b65f432975245b69b3873792ca2a339b0375a59705e8b655`
on Linux x86-64 and `de2a4424c8a43911b76ba2875446a30a40560af64cb4782cc582d1d3bbdaad7e`
otherwise, independently recomputed from the catalog.

Direct Cargo checks for Bzlmod/Loading/CLI and rebuilt CLI passed. Bazel query
resolved the JDK sidecar filegroup and all 13 inputs; with the required nightly
Rust toolchain setting, Bazel aquery showed all 13 as `:slug_bzlmod_v2` Rustc
action inputs. The parent `builtin_bazel_tools_files` Bazel build passed. A
full BuildBuddy RBE Bazel Rust-library build made 649 processes (318 remote)
but hit the packet's 180-second validation cap; it is unproved, not a pass or
a source-content failure. Normalized-PATH authentic `//:probe` exited 2 after
clearing the missing JDK BUILD stop; its next distinct Loading error is
`launcher_flag_alias.bzl:36` rejecting `configuration_field(fragment =
"java", name = "launcher")`. No configured JDK or full package evaluation is
admitted. No stale `slugd` existed before or after. Pinned rustfmt, diff,
plan and archive checks passed. The packet window was at least 15:00-15:19
UTC; the full Bazel build cap was 180 seconds, while other individual operation
times were not separately instrumented. The 137-line catalog/listing delta
and 96-line external test delta exceeded rough growth estimates, and FINAL
review found the full 13-file and package-boundary coverage proportionate.
M7A remains partial.

## Immediate predecessor

WP827 accepted at `ff9e3ee45`: native kwargs direct None omission and
Boolean 0/1 acceptance cleared protobuf's filegroup Loading stop. The next
authentic failure is the missing JDK built-in BUILD; M7A remains partial.
