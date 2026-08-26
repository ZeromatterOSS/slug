# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-archive-materialization-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private archive HTTP transport/realizer, sole repository materializer,
and native command session
Base: `ce30c8b9`

Result: implement the exact rules_rust 0.73.0 BCR archive slice and advance two
fresh real commands to the same next honest terminal. Do not implement generic
`http_archive`.

## Accepted architecture

`RepositoryMaterializationRequest` and its complete `RepoSpec` remain the
structural semantic input. The producer-owned selected-registry view remains
distinct from physical realization and joins its provisional root only at the
accepted loading boundary. Pinned `../zabel` commit `c7298478…` guides that
ownership shape only; copy no Zig code, scheduler, transport, cache, archive,
digest, path, output or behavior. Pinned Bazel 9.2 commit `8220c619…` remains
the sole behavior authority.

Add one private archive-only `ArchiveHttpTransport`. It retains native-root
Rustls configuration or an initialization failure, but no Hyper legacy client,
pool, executor, socket or task. Construct that configuration with an explicit
Ring provider local to the builder. Do not install a process-global provider or
change `HyperRegistryIo`.

`NativeDemandCommand::progress_inner` remains synchronous and outside each
DICE attempt. It passes the existing current-thread runtime into the sole
`RepositoryMaterializer::materialize_native`; no runtime enters structural
request state. The materializer's existing pre-I/O and post-I/O token checks
remain, and a narrow lock/release active-token probe may run between transfer
steps. No mutex spans DNS, runtime entry, capture write, extraction, DICE or
callback work.

For every HTTPS attempt:

1. Outside Tokio, parse the authority, synchronously resolve it to immutable
   socket addresses, then recheck the active token. Resolution is a completed
   non-preemptible command step, never a Tokio blocking task.
2. Create a per-attempt `HttpConnector` with that immutable resolver and a
   30-second connect ceiling; wrap it with the retained TLS configuration in
   HTTPS-only, HTTP/1-only mode. Preserve the original URI hostname for TLS
   server-name verification and Host, and send origin-form path/query.
3. Use bounded current-runtime entries to connect/handshake, drive request
   headers, and yield at most one body frame while polling the same pinned
   direct `hyper::client::conn::http1::Connection`. The connection, sender and
   body remain command-stack state between entries. There is no executor,
   `spawn`, async filesystem or digest work.
4. Between frames, synchronously hash/write the capture, enforce its ceiling
   and recheck the token. Header and frame-idle ceilings are 30 seconds. After
   terminal body/error, drop sender/body and drive connection shutdown for at
   most 5 seconds. Timeout/drop closes the in-tree socket/futures; nothing is
   detached.
5. Apply redirect/fallback synchronously. Each changed authority is freshly
   resolved and every attempt owns a fresh capture.

## Exact admitted BCR shape and realization

Add the BCR branch only when all conditions hold:

- rule `@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive`;
- nonempty ordered HTTPS `urls`; exactly one SHA-256 `integrity` SRI;
- absent `type`, with every admitted source URL ending `.tar.gz` or `.tgz`;
- empty `strip_prefix`, `remote_patches`, `remote_file_urls` and
  `remote_file_integrity`; zero `remote_patch_strip`;
- exactly one HTTPS `remote_module_file_urls` entry and one SHA-256
  `remote_module_file_integrity` SRI; and
- no other attribute.

Preserve the accepted one-`file://`, hexadecimal-`sha256`, exact-`tar`,
optional-strip local branch and its diagnostics/proof. Do not reinterpret it
through the new gzip/GNU-tar implementation.

Try source URLs in order. Follow only 301/302/303/307 with relative or absolute
`Location`, remain HTTPS and allow at most 40 redirects. URI, DNS, connect,
TLS, status, redirect, header/body/idle/shutdown and SRI failures advance to the
next source URL. After all fail, publish one generation-scoped typed transport
result. Stream each attempt to a fresh capture; retain no archive `Vec`.

Verify the exact SHA-256 before creating an extraction root. On Unix, decode
gzip and GNU tar synchronously outside Tokio. Admit only valid-Unicode regular
files and directories after removal of leading `./`; reject absolute/parent
traversal, links, special types, normalized duplicates, unsafe ancestor/type
collisions and destination escape. Preserve regular-file Unix permission bits,
including the demonstrated 27 executables. Fail BCR extraction closed on
non-Unix. Enforce 128 MiB compressed, 512 MiB uncompressed, 8,192 entries and
4,096 path bytes.

Only after extraction succeeds, fetch the registry MODULE URL with the same
redirect/stream/SRI rules and a 1 MiB body ceiling, then atomically replace root
`MODULE.bazel` with mode `0644`. Failure drops capture/root and publishes no
success. Return the verified archive SHA-256 hex as immutable source identity;
the complete `RepoSpec`, including MODULE SRI, remains structural alongside
it. Rename the internal preidentified-immutable attempt variant if necessary;
do not overload generated-repository naming.

The exact rules_rust artifact remains 67,196,890 compressed bytes,
224,337,920 uncompressed bytes, 4,493 entries and SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Reuse it only for real-command proof; add no copied fixture.

## Exact file authority and ceilings

Modify exactly these files from `ce30c8b9`:

| File | Entry lines | Entry SHA-256 | Ceiling |
|------|------------:|---------------|--------:|
| `app/slug_core_v2/Cargo.toml` | 45 | `6e91459a3b014d5c43a0be92c184448563cd4d71c34aaf92b05479d1f2bd6169` | 58 |
| `app/slug_core_v2/src/runtime/mod.rs` | 332 | `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651` | 338 |
| `app/slug_core_v2/src/runtime/dice.rs` | 11,636 | `8c791759a07abd6eb6f43e34264437570258257faf23270c3055de3ca5f2a626` | 11,670 |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 6,140 | `76f03638d41d5f901b762a0e627cd05290f350fcfcd04e28caaa2e708e94ec9c` | 5,450 |
| `app/slug_core_v2/src/runtime/archive_http_transport.rs` | absent | absent | 650 |
| `app/slug_core_v2/src/runtime/repository_archive.rs` | absent | absent | 1,450 |
| `app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs` | absent | absent | 2,250 |

Enable workspace `base64`, `flate2`, `tar` and `tower-service`. Add one direct
non-workspace Rustls 0.23 dependency with `default-features = false` and only
`ring`; do not inherit the workspace Rustls entry because it enables
`aws-lc-rs`. Hyper-Rustls already selects Ring, so this must leave the resolved
graph Ring-only and `Cargo.lock` unchanged. Build the archive TLS configuration
with `ClientConfig::builder_with_provider(ring::default_provider())`; never call
`install_default`. Prove `cargo tree -e features -i rustls` contains Ring and no
AWS-LC feature, so unchanged implicit `HyperRegistryIo` construction remains
unambiguous.
Relocate the existing local archive owner and its contiguous proof from
`repository_io.rs`; do not duplicate them. Production additions are <=1,300
semantic lines, proof additions <=1,500, relocated unchanged lines <=1,900 and
aggregate textual additions <=4,700. New production helpers are <=150 lines
and tests <=180 lines. Do not mix Git/generated/dormant-`RepositoryIo` cleanup
into the split.

## Required proof

Focused proof must discriminate:

- exact BCR attributes/SRI and rejection of every deferred/extra form while the
  accepted local archive branch remains byte-equivalent;
- ordered fallback after URI/DNS/connect/TLS/status/body/shutdown/SRI failure;
  relative/absolute redirects, HTTPS downgrade rejection, invalid/missing
  Location, 40-redirect bound and no retry after success;
- immutable resolver address/port handling, original TLS hostname and Host,
  HTTP/1 origin-form request, connect/header/idle/shutdown timeouts and fresh
  resolution after authority change;
- multi-frame streaming with hash/capture writes outside runtime entries, body
  ceilings, no archive `Vec`, and a peer held open after its body proving
  sender shutdown plus direct-connection completion;
- real GNU header/leading-`./`, regular/directory extraction, executable modes,
  MODULE replacement order/content/mode and exact archive identity;
- traversal, duplicate, link/special, ancestor/type, entry/path/uncompressed
  limits and partial capture/root cleanup;
- zero archive executor/spawn/async-filesystem/global-provider calls, Ring-only
  Rustls features, no registry behavior drift, no lock across I/O, stale-token
  mid/post-I/O rejection, warm
  nontransfer and request A/B/A rematerialization; and
- static absence of subprocess `curl`/`tar`, direct path injection, semantic
  generation roots, DICE-time I/O and a second materializer.

Use loopback HTTP only below the admitted HTTPS parser or with injected
transport seams; do not weaken production HTTPS admission. Run formatter/diff
hygiene, focused archive/transport/repository-session tests, full
`slug_core_v2 --lib`, and direct `slug_server_v2`/`slug_cli_v2` compile
checks serially. Rebuild `slug_cli_v2`, clean exact `slugd`, then replay two
fresh disposable `rules-rust-073-toolchain-owner` roots with only the parked
wildcard registration removed. Both must verify identical archive and registry
MODULE bytes and advance to the same next honest terminal.

## Compatibility and STOP

- **Exact:** accepted local archive behavior and admitted Bazel 9.2 BCR URL
  order/redirect/SRI/gzip-GNU-tar bytes/Unix modes/MODULE replacement.
- **Slug-native:** Rust HTTP/1 transport, synchronous resolution, time/resource
  ceilings, diagnostics, request/session sequencing and root lifetime.
- **Unsupported/deferred:** generic archives; HTTP origins/downgrades; auth/
  proxy/netrc; nonempty patches/overlays; links/specials; non-Unix BCR; generic
  repository rules; wildcard registration; repository-rule effects; toolchains/
  providers/actions/input trees; crate_universe; M8/M7B; exact output bytes.

STOP any entry-hash mismatch, file/dependency/cap widening, AWS-LC or dual
Rustls-provider activation, global provider installation, registry change,
legacy/shared client, executor/spawn/Tokio DNS/async filesystem, unjoined direct
connection, network/extraction in DICE, lock across I/O/DICE, unbounded/full-
buffer work, second materializer, semantic path, subprocess/Java/JVM, fixture
mutation, broader repository behavior, second successor or milestone closure.
`REPLAN` before widening.
