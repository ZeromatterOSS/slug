# Current Slug V2 Packet

Packet: `WP-5-7A-selected-registry-bcr-archive-realization`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private selected-BCR transport/realizer, sole repository materializer,
and immutable-root publication seam
Base: `3bc02039`

Result: the exact accepted rules_rust 0.73.0 BCR plan streams through the
existing verified-capture owner into one bounded Rust-native gzip/GNU-tar
realization, receives its independently verified registry `MODULE.bazel`, and
publishes one complete immutable root through the existing token-revalidated
materializer. The dormant path stays offline and local tar behavior is exact.

## Authority and accepted entry

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority: use `TarGzFunction`, `CompressedTarFunction`,
`StripPrefixedPath`, `CompressedTarFunctionTest`, `_http_archive_impl`,
`utils.bzl` remote-MODULE ordering and
`do_test_local_module_file_patch`. Reuse the accepted producer and artifact
evidence; add an oracle only for a demonstrated discriminating gap.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Inspect it with `git show`: `session_natural_bzl_repository_source.zig` joins a
producer-owned selected view to a completed materialization without host I/O,
and `session_generated_repository_materialization.zig` retains the complete
immutable root in its materialization payload. Preserve that separation; copy
no Zabel behavior, code, representation, scheduler, digest, cache or path.

Commit `3bc02039` already owns ordered direct HTTPS/HTTP1 capture, redirect and
fallback behavior, 128 MiB compressed limit, streaming SHA-256 SRI, connection
completion, cleanup, active-session probes and the sole lock-free callback.
Change its private success value from `()` to an owned verified
`NamedTempFile`; failed URLs still drop their scratch before fallback, and the
first verified URL stops fallback. Extraction failure never tries another
mirror. Generalize only the private capture parameters enough to reuse the
same engine for the single selected MODULE URL with a 1 MiB limit while
preserving archive diagnostics and all transport policy.

## Frozen selected realization

The audited artifact is exactly 67,196,890 compressed and 224,337,920 gzip
bytes, SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
It contains 4,493 logical entries and 4,544 physical headers: 3,544 regular
files, 949 directories and 51 GNU long-name headers,
221,054,883 regular payload bytes, no links/specials/PAX/non-Unicode names,
maximum file 30,793,187 bytes, maximum path 146 UTF-8
bytes and 13 components. Regular modes are 3,517 `0644` and 27 `0755`;
directories are `0755`. There are no absolute/parent paths, normalized
duplicates or namespace collisions. This disposable artifact is evidence,
not a fixture.

Implement `repository_archive_realize.rs` as the only selected gzip/tar/root
owner. It creates one callback-local `TempDir` only after archive SRI succeeds,
uses `flate2::read::MultiGzDecoder` plus a raw sequential 512-byte tar-header
loop, and never buffers the compressed body, decompressed archive or a whole
regular file. Use `tar::Header` only to validate/parse the current raw header;
do not call `tar::Archive::entries()`, which pre-buffers extension payloads.
A counting/cancellation reader, fixed GNU-name buffer and chunked copy enforce:

- 256 MiB total decompressed tar bytes and 256 MiB total regular payload;
- 64 MiB per regular entry, 8,192 physical headers, 8,192 logical entries,
  256 UTF-8 path bytes and 32 path components; and
- only normalized relative nonempty valid-Unicode regular/directory paths,
  exact GNU long-name resolution with at most 256 payload bytes, modes
  `0644`/`0755`, and no PAX, longlink, link,
  sparse, device, FIFO or other special type.

Count every physical header before dispatch. Reject PAX/longlink/sparse before
reading their payload. A GNU long-name payload is read into the fixed bounded
buffer and must name exactly the next regular/directory header; reject an
oversized, orphaned or doubled name. Reject duplicate normalized entries,
file/descendant collisions, absolute, `.`/`..`, malformed/truncated/checksum-
invalid gzip/tar, exceeded ceilings, missing end blocks and nonzero trailing
data as `MaterializationError`. Create parents in entry order;
stream regular bytes, set Bazel's `mode | 0400` result (therefore exact
`0644`/`0755` here), and preserve regular mtimes. Explicit directories are
`0755`; uid/gid and directory timestamps are nonsemantic Slug-native physical
state. Probe active status before each entry and each copy chunk. Non-Unix
executable-mode realization fails closed.

After archive completion, and before publication, capture the one plan-owned
MODULE URL through the same ordered HTTPS/SRI engine with its independent
digest. Keep the extracted `MODULE.bazel` until capture verifies, then replace
it inside the provisional root without following a symlink and force regular
nonexecutable `0644` bytes. The accepted registry file is 4,481 bytes,
SHA-256 `25e3b077128612754c4add1b4c90d20a6be06566b623dee6e32038d0e8f93062`.
No archive path is externally visible, so verified-temp replacement is
Slug-native mechanics with Bazel-exact final bytes/order and nonexecutable
semantics. Its mtime is nonsemantic Slug-native download state. MODULE
transport/SRI failure is `TransportError`; root mutation is
`MaterializationError`.

Return `Materialized::AssociatedImmutable { source_identity, root }`. Its
Slug-native collision-safe association is lowercase SHA-256 over domain
`slug.selected-bcr-root.v1\0` followed by archive digest then MODULE digest;
it is neither a Bazel checksum nor a temp-path identity. Map it to the existing
`GeneratedImmutable`/prepared immutable branch; do not add a materializer or
DICE key. Request equality remains the complete structural plan, while equal
verified content shares this root association.

## Lifetime and publication

Transport failure drops its capture and creates no root. Decode/extract failure
drops archive capture plus provisional root. MODULE failure drops both captures
and root. Unwind uses the same RAII ownership. On success both captures are
explicitly closed, then the callback returns the complete root. The existing
post-callback token check alone either retains it in `provisional_roots` and
publishes success or drops it as stale. Session discard/replacement releases
the retained root. Same-session duplicates reuse success; changed A/B/A
requests re-realize, with the restored A association equal to the first.

## Dependencies and file authority

From base, add only core direct `flate2 = { workspace = true }` and
`tar = { workspace = true }`. The isolated locked resolution adds exactly
`adler2 2.0.1`, `crc32fast 1.5.1`, `filetime 0.2.29`, `flate2 1.1.9`,
`miniz_oxide 0.8.9`, `simd-adler32 0.3.10`, `tar 0.4.46`, and `xattr 1.6.1`,
with no existing-version drift. Candidate locked check passed; expected final
hashes are Cargo.lock
`f2f76bbcfe089f7464c33cc1ff56719e3e8f9a0a11d9f897ec4a87cb5258858b`
and core Cargo
`339f0c10a4abe53d688660f35c565e3a5a4e4098212d0159ba927198effd334e`.

Allowed existing files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `Cargo.lock` | `72987efb5e59306ad089a738a84c4ff3a2f73fe54e7cdf9f99d93a2b3a001194` | 4,953 |
| `app/slug_core_v2/Cargo.toml` | `847f18b03cb47b6b196d4ce155d6b32698a3c67ae125313b8415ade07148662d` | 51 |
| `app/slug_core_v2/src/runtime/mod.rs` | `241f7141a8846dfda2d9465efe383a10da532ae6395980add584b2b4e7bce15f` | 336 |
| `app/slug_core_v2/src/runtime/repository_io.rs` | `5fa00cc870980cb43b96d30a9205b4ce08c11203780f9b6ceaff4bd1895b1e6f` | 4,700 |
| `app/slug_core_v2/src/runtime/repository_archive.rs` | `f38a0e2489b9a15b21f25fd35c3429b7a936f46fad2c7f399e9c8497bacb8dd5` | 820 |
| `app/slug_core_v2/src/runtime/repository_archive_http.rs` | `764cc681eaf19b1355cff89f3f079b1ed75c2016321be88637a6ecd73b09f655` | 700 |
| `app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs` | `17b54cb423b823a8a879c508de3e5f3d741b30688a7e067b3317253e76b73466` | 1,700 |
| `app/slug_core_v2/src/runtime/tests/repository_archive_http_tests.rs` | `94f5b0a3a0d2f34e39d6bf46244c9252f28984d926c1cb8b61096cc8953d18da` | 800 |

New files are `repository_archive_realize.rs` <=520 and
`tests/repository_archive_realize_tests.rs` <=700. Production additions are
<=700 and proof additions <=1,000. `repository_io.rs` is wiring only; transport
policy stays in HTTP and realization policy in the new module.

## Proof and validation

Add focused in-memory/generated tests for gzip/GNU-longname extraction,
regular bytes/mtime/modes, implicit/explicit directories, every path/type/
duplicate/malformed/truncation/ceiling row, streaming and active cutoff.
Include oversized/orphan/double GNU names, PAX payload rejected before
allocation, GNU longlink, and raw physical-header-count exhaustion.
Prove archive-SRI-before-root, independent MODULE SRI and replacement bytes/
mode, error-stage and cleanup matrix, explicit capture deletion, one complete
root promotion, stale-drop, same-session reuse and direct-session A/B/A source
association. Keep all nine transport and ten accepted archive/session rows.
Use the pinned source regression instead of porting broad link/PAX/strip tests,
because those forms are absent and fail closed.

Run `cargo fmt --check`, focused archive/HTTP/realizer tests, `cargo test -p
slug_core_v2 repository_ -- --nocapture`, `cargo check -p slug_core_v2
--locked`, and rebuild `slug_cli_v2`. With clean `slugd` lifecycle and fresh
roots, the existing disposable registry must move both wildcard-removed
rules_rust `query` and `build` past repository materialization; record the next
honest internal/public terminal. In disposable roots, make pinned Bazel 9.2
materialize this exact archive/MODULE and compare the complete normalized tree
manifest to Slug: all 4,493 relative paths and types, every regular file's
SHA-256/POSIX mode, and every archive-sourced regular file's integer mtime.
Directory metadata and the replaced MODULE mtime are explicitly excluded.
Commit neither artifact nor manifest. Run `git diff --check` and independent
review.

## Compatibility and STOP

- **Exact:** selected plan, archive URL/redirect/fallback/SRI; sequential
  selected archive-entry regular-file bytes/modes/mtimes and logical directory
  presence; registry MODULE transfer/order/SRI/final bytes/nonexecutable
  result; local archive.
- **Slug-native:** Rust streaming mechanics and valid-Unicode paths, ceilings
  and diagnostics, strict duplicate/path rejection, directory physical
  metadata, source association, provisional lifetime and session sequencing.
- **Unsupported/deferred:** generic `http_archive`, PAX/non-Unicode/links/
  specials/sparse, absolute/parent/duplicate paths, nonempty strip/patch/
  overlay, auth/proxy/netrc, repository-rule breadth, toolchains/providers/
  actions/input trees, crate_universe, M8/M7B and exact output bytes.

STOP on dirty overlap, source/hash/dependency drift, artifact exceeding any
ceiling/form, broader tar behavior, whole-file/archive buffering, system tar,
subprocess/Java/JVM, I/O under DICE/lock, second materializer, retained capture
path, temp-path identity, global provider/AWS-LC, local archive change, fixture
copy, or public diagnostic widening. `REPLAN` before crossing a boundary.
