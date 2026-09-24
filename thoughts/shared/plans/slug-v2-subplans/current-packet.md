# Current Slug V2 Work Packet

Packet: WP-7-103-m7a-selected-bcr-regular-0555-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit a regular tar entry with mode `0555` in the existing selected-BCR
`tar.gz` materializer on Linux/Unix. The source-authentic rules_kotlin 1.9.6
archive must pass Slug's current `selected BCR unsupported entry mode` stop
when the normalized-PATH authentic build demands its `REPO.bazel` source
observation. A later independently typed stop is acceptable. `REPO.bazel` is
the lookup that triggers materialization, not an archive member or special
mode rule.

Exact Bazel 9.2 slice: for an admitted regular entry with header mode `0555`,
extract its payload bytes and set the output mode to `mode | 0400` (`0555`),
then preserve the header mtime as already implemented. Slug-native: the
existing finite tar entry/mode grammar, Unicode/path/namespace/size limits,
directory `0755` normalization, Rust Host observations and structurally
associated immutable source identity. Unsupported/deferred: other previously
rejected regular/directory modes, symlink/hardlink/special tar entries, local
PAX and broader Bazel decompressor parity, non-Unix mode materialization,
rules_kotlin Loading/analysis/toolchain success and M7A close. Preserve all
accepted mode, integrity, cancellation, error and publication boundaries.

## Source, owner and decision

The pinned Bazel 9.2 `ArchiveRepoSpecBuilder.java:35-58` and
`tools/build_defs/repo/http.bzl:194-218` route a BCR archive to the ordinary
decompressor. `CompressedTarFunction.java:88-147` strips the prefix, writes a
regular file, then calls `chmod(entry.getMode() | 0400)` and preserves mtime;
its `CompressedTarFunctionTest.java:45-66` covers extraction but has no
mode-specific assertion. The authenticated rules_kotlin 1.9.6 BCR
`source.json` names the tar with SHA-256
`3b772976fec7bdcda1d84b9d39b176589424c047eb2175bed09aac630e50af43`.
Verified local archive bytes have 86 regular entries at `0555`, 22 directories
at already-admitted `0755`, no links and no `REPO.bazel`. The first header is
regular `BUILD.bazel` with raw USTAR mode `0000555\0`; Bazel's extracted
`BUILD.bazel` is also a regular `0555` file with matching bytes. This is a
direct discriminator, not a claim that all Bazel tar modes are supported.

`app/slug_core_v2/src/runtime/repository_archive_realize.rs::extract` is the
sole semantic owner. Its regular-mode allowlist rejects `0555` before reading
the header path or applying `strip_prefix`; its existing regular writer already
uses `set_mode(mode | 0o400)` and header mtime. Add only `0o555` to that
regular-mode set. Do not move the gate, special-case `REPO.bazel`, synthesize a
file, change mode normalization, or alter transport/request/DICE owners.
Keep existing capture SHA-256 verification, immutable request association,
generation-scoped errors, activity checks, provisional-root cleanup and final
observation/promotion. The tar digest includes header mode bytes, so changing
the mode changes the existing source association; no new key, retained value,
cache, lock, file discovery or memory owner is needed. The temporary output
root and capture remain phase/session scratch until accepted or discarded.
The DICE ownership and no-lock-across-compute guidance in
`docs/developers/dice.md` remains unchanged.

This is a same-owner one-token allowlist extension in a cohesive archive
extractor; the file's size does not justify a structural split. The Buck2
utility-reuse skill is inapplicable because no retained representation,
hashing utility, collection or clone policy changes.

## Evidence and validation

No new Stage 1 fixture is selected: pinned Bazel source specifies the exact
mode projection; verified source-authentic BCR tar bytes and Bazel's extracted
mode discriminate the observed gap. An independent tiny scratch Bazel 9.2
`http_archive` oracle at `/tmp/slug-wp803-mode-oracle` confirms the same
projection: the local-file tar with regular `BUILD.bazel`, `payload.txt` and
nested source at `0555` and a directory at `0755` queried successfully on a
fresh output base (exit 0); extracted files remained mode `555`, directory
`755`. The successful invocation used `--ignore_all_rc_files` and a `file://`
URL only. Bazel's `--repository_disable_download` also blocks local file
URLs, so that flag's preliminary exit 7 is an invocation control, not a
semantic negative. The scratch tar stays under `/tmp`, not in this checkout.
The upstream decompressor test is not copied
because it does not assert modes; the existing Slug tar-header writer supplies
the stronger focused discriminator. Fixture growth is zero and no archive,
credential or upstream source is committed.

Add a focused `repository_archive_realize_tests.rs` test using its current
`append`/`header` writer: a regular `BUILD.bazel` and a nested regular source
at `0555` with known bytes and mtimes pass full `realize_selected_bcr`, retain
`0555` on Unix, and preserve the existing verified source association. Keep
the current reject-mode test's `0600`, `0750` and special-bit controls; add a
neighboring rejected mode only if it materially distinguishes the new set.
Reuse existing tests for `0640`, directory `0750`, malformed/special entries,
active cutoff, source association, transform and file-session generation
behavior. No weakened assertion or broader archive mode claim is acceptable.

Resolve pinned cargo/rustc from `rust-toolchain`; compile Core library tests
with Cargo JSON `--no-run`, batch exact selectors through preflight, then run
the new/protected tests serially. Rebuild `slug_cli_v2` before the authentic
normalized-PATH build; check/clean `slugd` before and after. Run formatting,
diff, archive and plan-status checks, and obtain independent DESIGN and FINAL
reviews. No broad suite, remote-transport, NativeLink or BuildBuddy gate is
selected by this source-materialization packet.

## Scope and stops

Base clean main `64a07e9c0`. Allowlist: this manifest and canonical Current
paragraph; Stage 5 owner contract only for accepted invariant; production
`app/slug_core_v2/src/runtime/repository_archive_realize.rs`; focused tests
`app/slug_core_v2/src/runtime/tests/repository_archive_realize_tests.rs`.
Production growth is approximately one token; proof growth under 90 lines.
These are review triggers, not permission ceilings. REPLAN if the authenticated
tar does not use ordinary regular `0555`, Bazel's `0555` output does not match
the existing writer, mode-only changes escape existing source identity, or
passing the present stop requires a distinct entry type/transport/DICE owner.
A later rules_kotlin source or Loading failure is a new packet.

## Immediate predecessor

WP802 at `64a07e9c0` accepted package-relative Label conversion and typed
`select()` keys in a generated BUILD. Its five-row Bazel 9.2 fixture and seven
focused/protected Rust tests passed, and the authentic build advanced from
rules_python/bazel_skylib to this selected-BCR entry-mode stop at toolchain
registration row 55. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The verified
rules_kotlin 1.9.6 BCR archive's first regular `BUILD.bazel` header and its
other 85 regular entries are `0555`; its 22 directory entries are `0755`.
Bazel's extracted `BUILD.bazel` retains `0555`. A separate, offline pinned
Bazel 9.2 scratch `repository_ctx.extract` query passed on a fresh output
base (3.46 s), preserving three regular `0555` files and one `0755`
directory. The sole production change adds `0o555` to the regular tar mode
set; the writer's byte, mode and mtime projection and all existing safety
gates are unchanged. No fixture was added because pinned source plus the
verified authentic archive/extracted result directly discriminate the one
mode, with the new source-shaped Rust tar-header regression preserving it.

The final pinned-toolchain Core JSON library test compile passed (2.66 s).
Exact preflight selected seven nonignored tests; the new full-realization
test and six protected mode/rejection/source-association/file-session tests
each passed. The CLI rebuild passed (2.49 s). The authentic normalized-PATH
smoke passed the old selected-BCR entry-mode stop, then failed separately at
row 55 in `@@protobuf+//bazel/private:java_lite_proto_library.bzl:76` on
`configuration_field(fragment = "proto", ...)` with `invalid configuration
fragment name 'proto'`. No `slugd` remained before or after. Format, diff,
archive and plan-status checks passed. The scratch oracle and reviews were
independent; packet wall-clock/review durations were not separately timed.
No remote transport, NativeLink or BuildBuddy authentication path was used.
Other tar modes/types and full Kotlin/protobuf Loading remain deferred; M7A
and M8 remain open.
