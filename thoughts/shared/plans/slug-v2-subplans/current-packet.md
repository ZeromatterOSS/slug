# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-archive-materialization-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private archive HTTP transport/realizer, sole repository materializer,
native command session and exact Cargo dependency closure
Base: `c7365840`

Result: implement the exact rules_rust 0.73.0 BCR archive slice and advance two
fresh real commands to the same next honest terminal. Do not implement generic
`http_archive`.

## Accepted architecture

`RepositoryMaterializationRequest` and its complete `RepoSpec` remain
structural. The producer-owned selected-registry view remains distinct from
physical realization and joins its provisional root only at the accepted
loading boundary. Pinned `../zabel` commit `c7298478…` guides that ownership
shape only; copy no Zig code, scheduler, transport, cache, archive, digest,
path, output or behavior. Pinned Bazel 9.2 commit `8220c619…` is behavior
authority.

Add one private archive-only `ArchiveHttpTransport`. It retains an explicit
Ring/native-root Rustls configuration or initialization failure, but no legacy
client, pool, executor, socket or task. Never install a process-global crypto
provider and do not change `HyperRegistryIo`.

`NativeDemandCommand::progress_inner` remains synchronous outside DICE. It
passes the existing current-thread runtime into the sole materializer; runtime
state never enters request identity. Existing pre/post-I/O token checks remain,
with brief released active-token probes between transfer steps. No mutex spans
DNS, runtime entry, capture write, extraction, DICE or callback work.

For each HTTPS attempt:

1. Outside Tokio, resolve the authority synchronously to immutable socket
   addresses, then recheck the token. Resolution is a completed command step,
   never a Tokio blocking task.
2. Create a per-attempt `HttpConnector` over that resolver with a 30-second
   connect ceiling; wrap it in HTTPS-only, HTTP/1-only TLS. Preserve the original
   hostname for TLS/Host and send origin-form path/query.
3. Bounded runtime entries connect/handshake, drive headers, and yield at most
   one body frame while polling the same pinned direct HTTP/1 `Connection`.
   Connection/sender/body remain command-stack state. No executor, spawn, async
   filesystem or digest work runs in an entry.
4. Between frames synchronously hash/write the capture, enforce its ceiling and
   recheck the token. Header/frame-idle ceilings are 30 seconds. Drop sender/body
   at terminal and drive shutdown for at most 5 seconds. Timeout/drop closes the
   in-tree socket/futures; nothing detaches.
5. Apply redirects/fallback synchronously. Each changed authority is freshly
   resolved and each attempt owns a fresh capture.

## Exact BCR shape and realization

Admit only:

- rule `@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive`;
- nonempty ordered HTTPS `urls`; one SHA-256 `integrity` SRI;
- absent `type`, every source URL ending `.tar.gz` or `.tgz`;
- empty `strip_prefix`, `remote_patches`, `remote_file_urls` and
  `remote_file_integrity`; zero `remote_patch_strip`;
- one HTTPS `remote_module_file_urls` entry and one SHA-256
  `remote_module_file_integrity` SRI; and
- no other attribute.

Preserve the accepted one-`file://`, hex-`sha256`, exact-`tar`, optional-
strip local branch and diagnostics/proof; relocate it without reinterpretation.

Try source URLs in order. Follow only 301/302/303/307 with relative/absolute
`Location`, remain HTTPS and allow at most 40 redirects. URI/DNS/connect/TLS/
status/redirect/header/body/idle/shutdown/SRI failure advances to the next URL;
after all fail publish one generation-scoped transport result. Stream each
attempt to a fresh capture with no archive `Vec`.

Verify SHA-256 before root creation. On Unix, decode gzip/GNU tar synchronously.
Admit valid-Unicode regular files/directories after leading-`./` removal;
reject absolute/parent traversal, links, specials, normalized duplicates,
ancestor/type collisions and escape. Preserve regular-file Unix permission
bits. Fail BCR extraction closed on non-Unix. Enforce 128 MiB compressed,
512 MiB uncompressed, 8,192 entries and 4,096 path bytes.

After extraction, fetch the registry MODULE with identical redirect/stream/SRI
rules and a 1 MiB body ceiling, then atomically replace root `MODULE.bazel`
mode `0644`. Failure drops scratch and publishes nothing. Return verified
archive SHA-256 hex as source identity; complete `RepoSpec`, including MODULE
SRI, remains structural. Use a correctly named preidentified-immutable internal
attempt, not generated-repository naming.

The exact real artifact remains 67,196,890 compressed bytes, 224,337,920
uncompressed bytes, 4,493 entries, 27 executables and SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use it only for command proof; add no fixture.

## Exact file/dependency authority

Modify exactly:

| File | Entry lines | Entry SHA-256 | Ceiling |
|------|------------:|---------------|--------:|
| `Cargo.lock` | 4,875 | `ee9acebd876bedaf474e28c5f14894aa7dec7afb257e2de4b2da903dd8c39800` | 4,960 |
| `app/slug_core_v2/Cargo.toml` | 45 | `6e91459a3b014d5c43a0be92c184448563cd4d71c34aaf92b05479d1f2bd6169` | 58 |
| `app/slug_core_v2/src/runtime/mod.rs` | 332 | `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651` | 338 |
| `app/slug_core_v2/src/runtime/dice.rs` | 11,636 | `8c791759a07abd6eb6f43e34264437570258257faf23270c3055de3ca5f2a626` | 11,670 |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 6,140 | `76f03638d41d5f901b762a0e627cd05290f350fcfcd04e28caaa2e708e94ec9c` | 5,450 |
| `app/slug_core_v2/src/runtime/archive_http_transport.rs` | absent | absent | 650 |
| `app/slug_core_v2/src/runtime/repository_archive.rs` | absent | absent | 1,450 |
| `app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs` | absent | absent | 2,250 |

Enable workspace `base64`, `flate2`, `tar`, `tower-service`, plus direct
Rustls 0.23 with default features off and only `ring`. Use archive-local
`builder_with_provider(ring::default_provider())`; never `install_default`.

The lock diff is exactly 77 additions: five core names (`base64 0.21.7`,
`flate2`, `rustls`, `tar`, `tower-service`) and new packages
`adler2 2.0.1`, `crc32fast 1.5.1`, `filetime 0.2.29`, `flate2 1.1.9`,
`miniz_oxide 0.8.9`, `simd-adler32 0.3.10`, `tar 0.4.46`, `xattr 1.6.1`
with Cargo-resolved metadata. Preserve every existing entry, especially
`wasip2 1.0.4+wasi-0.2.12`. Expected candidate is 4,952 lines/SHA-256
`ecedcf984bf6704dbbb48a62cb56ec56bbc0f221d09db573fea96706bf7bf710`
before source edits, and must not drift afterward.

Relocate the existing archive owner/proof rather than duplicate it. Caps:
<=1,300 new semantic production lines, <=1,500 proof lines, <=1,900 relocated
unchanged lines, <=4,700 aggregate additions; production helpers <=150 lines,
tests <=180. No Git/generated/dormant-`RepositoryIo` cleanup.

## Required proof

Focused proof discriminates:

- exact BCR attributes/SRI and every deferred/extra rejection; local archive
  byte-equivalence;
- ordered failure fallback; relative/absolute redirects, HTTPS downgrade,
  invalid Location, 40 bound and stop-after-success;
- resolver address/port, TLS hostname/Host, origin-form, timeouts and fresh
  authority resolution;
- multi-frame capture writes outside runtime, no archive `Vec`, body limits,
  held-open peer and completed sender/connection shutdown;
- GNU header/leading-`./`, files/dirs/modes, MODULE replacement and identity;
- traversal/duplicate/link/special/collision/entry/path/size cleanup cases;
- zero executor/spawn/async-fs/global-provider, Ring-only/no AWS-LC, no registry
  drift, no lock across I/O, stale-token rejection, warm reuse and A/B/A; and
- static absence of subprocess/direct path/DICE-time I/O/second materializer.

Use loopback HTTP only below production HTTPS parsing or injected seams. Run
formatter/diff hygiene, feature-tree proof, focused transport/archive/session
tests, full `slug_core_v2 --lib`, direct server/CLI compile checks, then rebuild
`slug_cli_v2`. Clean exact `slugd` and replay two fresh wildcard-removed
`rules-rust-073-toolchain-owner` roots. Both must verify identical archive and
MODULE bytes and reach the same next honest terminal. Cargo runs serially with
`--locked` after the one exact lock update.

## Compatibility and STOP

- **Exact:** local archive behavior and admitted Bazel 9.2 BCR URL/redirect/SRI/
  gzip-GNU-tar bytes/Unix modes/MODULE replacement.
- **Slug-native:** Ring HTTP/1 transport, synchronous resolution, bounds,
  diagnostics, session sequencing and root lifetime.
- **Unsupported/deferred:** generic archives, HTTP/downgrade, auth/proxy/netrc,
  patches/overlays, links/specials, non-Unix BCR, generic repository rules,
  wildcard registration, repository-rule effects, toolchains/actions, M8/M7B.

STOP hash/file/dependency/cap mismatch, lock drift beyond exact 77 additions,
AWS-LC/dual/global provider, registry change, legacy client/task/Tokio DNS/
async-fs, unjoined connection, network/extraction in DICE, lock across I/O,
unbounded/full buffer, second materializer, semantic path, subprocess/Java/JVM,
fixture mutation, broader behavior, second successor or milestone closure.
`REPLAN` before widening.
