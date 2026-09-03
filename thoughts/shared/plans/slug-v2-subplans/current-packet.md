# Current Slug V2 Packet

Packet: WP-5-7A-selected-bcr-regular-0444-mode-implementation-r1

Milestone: M7A bootstrap-critical repository materialization. Admit generic
regular tar entries whose exact mode is `0444` through the existing selected
BCR archive materializer; preserve every other fail-closed archive boundary.

Status: docs-only audit returns `ACCEPT`. Independent architecture review is
required before implementation.

## Accepted predecessor and reproducible replay boundary

Commit `06ddf8252` terminally accepts
`WP-4-7A-proto-common-predeclared-facade-implementation-r1` at 5 production
and 31 proof gross Rust additions, 36 total. Its focused test passes 1/1;
`slug_loading_v2` passes 530 unit tests with one ignored plus integration
targets of 51/29/8/6/2/1/5/1; `slug_query_v2 --lib` passes 55/55; the CLI
build, formatting, diff and daemon-hygiene gates pass. Only the longstanding
three thought-path archive failures remain.

The authenticated rules_rust replay clears the protobuf predeclared facade.
A first run reported `repository_ctx.which invocation exceeds the admitted
Unix limits` in rules_shell 0.6.1. That stop is excluded: the exact
`repository_ctx.which("bash")` call, with `"sh"` only on miss, now
reproducibly succeeds with both `PATH=/bin:/usr/bin:/usr/local/bin` and
`PATH=/usr/bin:/usr/local/bin`. Both are within the accepted byte/component
caps. No `which` cap, symlink or resolver change is authorized.

The exact consumer is rules_shell BCR module version 0.6.1, source-relative
`shell/private/repositories/sh_config.bzl`, SHA-256
`795d028cf310d65265ad3d64cbf896567512dcb31b1d4cafa2f8c92eb65ec1a2`,
4,401 bytes/138 lines. Lines 125-136 contain that two-step lookup.

Both replays reach toolchain-registration row 14, `rules_java+//toolchains`,
and stop while materializing selected rules_java 9.1.0 to probe
`REPO.bazel`:

`selected BCR unsupported entry mode`

This is the reproducible boundary. The archive contains no `REPO.bazel`;
materialization must complete before the normal absent probe can proceed.

## Exact archive and Bazel 9.2 evidence

The durable BCR descriptor coordinate is
`https://bcr.bazel.build/modules/rules_java/9.1.0/source.json`, SHA-256
`da589573c1dee2c9ac4a568b301269a2e8191110ff0345c1a959fa7ea6c4dfd6`.
It selects the literal release URL
`https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`
with empty `strip_prefix` and integrity
`sha256-Thooolwu+lNQDJKNIs7/vFBd2VszWi0CWDaik7WSIS8=`. The exact release
archive is 114,566 bytes with SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`,
which matches that integrity.

A complete tar listing has exactly 114 logical entries: 94 regular files at
mode `0444` and 20 directories at mode `0755`; there are no other types or
modes. The first entry is `./AUTHORS`, a 305-byte regular `0444` file, so
the current failure is deterministic before extraction can progress.
`MODULE.bazel` is a 4,218-byte regular `0444` file. No selected archive
asset is copied into this repository.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
owns the compatibility behavior.
The repo-relative
`src/main/java/com/google/devtools/build/lib/bazel/repository/decompressor/CompressedTarFunction.java`
has SHA-256
`28dd9b8ace7d64b432b4bf566b1d1325cffea81df338ace428dfff7c756ae333`;
lines 141-151 stream every regular entry and apply
`entry.getMode() | 0400`. Thus Bazel accepts `0444` and retains it exactly.
The repo-relative
`src/test/java/com/google/devtools/build/lib/bazel/repository/decompressor/CompressedTarFunctionTest.java`
has SHA-256
`3a2865acca41f7ebe484886a978aeef2eeb9aba2aa9d3337f0b81a6576c925c2`
and confirms the normal compressed-tar owner.

Slug's `repository_archive_realize.rs::extract` already applies the same
owner-readable projection, but its fail-closed allowlist admits regular
`0644`, `0664`, `0755` and `0775` only. Generic regular `0444` is
therefore the exact missing mode; directory `0755` is already admitted.

## Audit decision and compatibility boundary

Audit result: `ACCEPT`. The smallest complete category is a generic regular
selected-BCR tar entry whose header mode is exactly `0444`. Add `0444` to
the existing regular-mode allowlist. Do not key behavior to rules_java, a path,
a module name or the BCR probe.

Classify as **exact** under Bazel 9.2 the acceptance of a regular `0444`
entry, its bytes and mtime, and its retained `0444` Unix permission after the
existing owner-readable projection. Existing exact archive integrity, path,
namespace, payload, transform order and source association remain unchanged.

Classify as **Slug-native** the deliberately bounded mode/type allowlist, Rust
temporary-directory extraction, cancellation checks and typed diagnostics.
This packet does not claim Bazel's broader arbitrary tar-mode behavior.

Keep **unsupported/deferred** every other newly unadmitted regular or directory
mode; symlinks, hardlinks and other tar entry types; broader PAX/GNU metadata;
Windows permission equivalence; other archive formats; and changes to
strip-prefix, overlays, patches, MODULE replacement, locator/probe semantics,
repository selection or `repository_ctx.which`.

## Owner, lifecycle, invalidation and memory

`SelectedBcrArchive` remains the verified URL/integrity/transform plan.
`realize_selected_bcr` and its single bounded `extract` pass remain the sole
archive materialization and mode owners. The completed root remains an
`AssociatedImmutable` materialization with the existing domain-separated
source association.

Archive header modes are bytes covered by the selected archive integrity and
existing request/source identity. Add no DICE key, request projection,
observation, equality rule, cache, interner, retained collection, lock, task or
fallback. The verified capture and temporary root remain scratch owned by the
existing active repository session; cancellation and every failure drop them,
while success transfers only the immutable root. Concurrent requests retain
the existing content-addressed fetch and session ownership; overlapping
attempts never share a temporary root, and an inactive attempt cannot publish
its root or contaminate a later A/B/A request.

No benchmark is required: one membership alternative is added to the existing
streaming branch, with no retained representation or asymptotic change.
`extract` exceeds the 150-line function trigger but stays cohesive because it
is the one stateful bounded tar pass owning header order, byte limits,
namespace, payload, mode, mtime and cancellation. Splitting one admitted mode
would duplicate that state and error ordering. Both allowlisted files remain
below 2,000 lines.

## Required proof

Extend only adjacent selected-BCR archive tests to prove:

- a synthetic regular `0444` entry materializes with exact bytes, mtime and,
  on Unix, retained mode `0444`;
- existing admitted executable and writable modes remain unchanged;
- existing regular `0600`, directory `0700`, malformed, namespace, limit,
  cancellation, transform and source-association rejections remain green;
- inactive/failed overlap remains unpublished and existing source-association
  A/B distinctions remain exact; and
- the pinned archive hash/listing plus the authenticated replay remain the
  full consumer/provenance evidence. Do not add the archive or a fixture.

The replay must use an explicitly bounded PATH, clear rules_shell and this
mode error, finish selected rules_java materialization, and stop only at the
next independently owned boundary. If it instead exposes another archive mode
or type, return `REPLAN`; do not broaden the matcher in implementation.

## Allowlist, caps and validation

Only these files may change:

- `app/slug_core_v2/src/runtime/repository_archive_realize.rs`;
- `app/slug_core_v2/src/runtime/tests/repository_archive_realize_tests.rs`.

Gross additions are capped at 6 production Rust, 40 proof Rust and 46 total.
No docs, fixture, Cargo metadata or other source may change during
implementation.

Run serially:

- `cargo test -p slug_core_v2
  selected_bcr_realizes_streamed_files_gnu_name_modes_mtime_and_module
  --quiet`;
- `cargo test -p slug_core_v2 selected_bcr --quiet`;
- `cargo test -p slug_core_v2 --lib --quiet`;
- `cargo build -p slug_cli_v2 --quiet`, stale-`slugd` cleanup and one
  authenticated bounded-PATH replay;
- `cargo fmt --check`, `git diff --check`,
  `bash scripts/v2_archive_status.sh` and exact allowlist/cap verification.

Return `REPLAN` if exact `0444` retention requires a second production
owner, a new key/input/cache/lock, platform-wide permission policy, another
mode/type, archive fixture, probe or consumer branch, altered source identity,
or the allowlist/caps fail.
