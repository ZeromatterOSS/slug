# Current Slug V2 Packet

Packet: `WP-5-7A-selected-registry-bcr-verified-capture-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: private selected-BCR archive transport below the sole repository
materializer
Base: `9b390370`

Result: the exact accepted `SelectedBcrTarGz` plan reaches ordered HTTPS
transport and SHA-256-SRI-verified command scratch. The capture is deleted and
the native session publishes a generation-scoped `MaterializationError` saying
selected-registry extraction is deferred. This is a transport-entry packet,
not archive realization: no captured byte or path survives the callback and no
repository root, MODULE replacement or success result is created.

## Accepted predecessor and learned facts

Commit `1807b1d4` owns mutually exclusive private `LocalTar` and
`SelectedBcrTarGz` plans. The local one-file/hex-SHA256/tar implementation and
proof remain exact. The selected plan admits only the produced Bazel 9.2
`type = "tar.gz"`, ordered nonempty HTTPS URLs, 32-byte SHA-256 SRI, empty
strip/patch/overlay fields, zero patch strip and one HTTPS registry MODULE
fact. Malformed shapes are stable `SpecError`; exact shapes currently stop at
a generation-scoped deferred `TransportError`.

The live path is `WorkspaceRuntime::drive_command`'s current-thread Tokio
runtime -> completed DICE `Need` attempt -> synchronous
`NativeDemandCommand::progress_inner` ->
`RepositoryMaterializer::materialize_native` -> lock-free `materialize_with`
callback -> `materialize_native_attempt` -> private archive plan. DICE roots,
transactions and the materializer lock are absent while the callback runs;
`materialize_with` rechecks the session token before publishing its result.
The dormant `LocalRepositoryIo` path runs its callback under `spawn_blocking`
and has no native runtime. It must keep returning the accepted deferred BCR
transport result with zero network I/O.

Pinned Hyper 1.11 exposes a directly polled HTTP/1 connection future. The
legacy client spawns its driver and its default resolver may `spawn_blocking`,
so neither is admitted. A disposable Cargo resolution proves the three exact
direct edges below add no package stanza and keep Rustls Ring-only; inheriting
workspace `tokio-rustls` is rejected because its default enables AWS-LC.

## Behavior authority and guidance

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. `HttpDownloader#download` and
`HttpDownloaderTest` methods `downloadFrom2UrlsFirstOk`,
`downloadFrom2UrlsFirstSocketTimeoutOnBodyReadSecondOk`,
`downloadTwoUrls_firstNotFoundAndSecondOk`, and
`downloadFrom2UrlsFirstTlsErrorSecondOk` own the admitted mirror-fallback
cases; `downloadAndReadOneUrl_checksumMismatch` owns checksum behavior.
`HttpConnector`, its `MAX_REDIRECTS = 40`, and
`HttpConnectorTest#pathRedirect_301/#pathRedirect_303` own accepted response
and redirect behavior. `_http_archive_impl`, `IndexRegistry#createArchiveRepoSpec`
and the accepted producer proof own the selected source shape. Reuse these
pinned-source regressions; add no oracle fixture.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept/test-only architectural
guidance. Its `src/load/session_natural_bzl_repository_source.zig` joins a
producer-owned selected view to a typed materialization fact without host I/O,
and `src/bzlmod/session_generated_repository_materialization.zig` retains an
immutable root only with its complete durable manifest. Apply only that
view/realization ownership split. Copy no Zabel code, scheduler, transport,
cache, archive, digest, path, output or behavior. Bazel 9.2 owns every
compatibility claim.

## Decision, compatibility and non-decisions

- **Exact:** the accepted plan and local branch; source URLs tried in declared
  order with no later mirror after success; mirror fallback for the pinned
  404, TLS-certificate, body-read-timeout and SRI-mismatch cases; SHA-256 over
  streamed response bytes; 200/206 success; only 301/302/303/307 redirects;
  relative and absolute `Location`; follow at most 39 redirects and reject the
  40th. Exact claims are restricted to the selected BCR plan.
- **Slug-native:** synchronous OS DNS, resolved-address order and 64-address
  ceiling; immediate mirror fallback for other per-URL failures; HTTPS-only
  redirects; raw Tokio/Rustls/Hyper HTTP/1 mechanics; native trust roots;
  Ring-local provider; timeout/error text; 128 MiB compressed-capture ceiling;
  command/session sequencing; and the deliberate post-verification
  `MaterializationError` terminal.
- **Unsupported/deferred:** gzip/tar and executable modes; extraction and
  provisional root; registry MODULE fetch/replacement; generic
  `http_archive`; HTTP downgrade, auth/proxy/netrc; patches/overlays/links/
  specials; Bazel recoverable same-URL retry decisions/backoff and any source
  selection that depends on them; and all later M7A/M8 semantics.

Capture-only is the smallest lawful owner because it publishes no physical
state. A private continuation lives only inside one native materialization
callback and owns resolver results, TLS/HTTP state, capture and hash. On exact
SRI success the archive wrapper closes and deletes the capture before returning
`MaterializationError("selected-registry BCR archive extraction is deferred")`.
Real transfer failures remain `TransportError`; parser failures remain
`SpecError`. The changed stage is the packet's direct-session observable; the
existing public command error collapse remains unchanged.

This discard is a temporary bridge: the remaining invariant gap is that a
successfully verified source is not consumed into the requested immutable
repository root. Delete it when the immediate successor consumes the same
verified capture into a bounded gzip/tar/MODULE realization before the existing
final token check. That successor owns the replacement. A direct test requiring
deletion before the deferred-extraction terminal prevents retention or
accidental success from becoming permanent.

## Transport, request and memory contract

For each declared URL, resolve `host:port` synchronously on the command owner,
outside Tokio, DICE and locks; retain at most 64 immutable socket addresses for
that attempt. OS resolver latency cannot be canceled or time-bounded without a
forbidden task and is an explicit residual risk. Use the original hostname for
TLS SNI and `Host` and origin-form path/query for the request.

Each address uses `TcpStream -> tokio_rustls::TlsConnector -> TokioIo ->
hyper::client::conn::http1::handshake`. Build an explicit Ring provider and
native-root client config per callback; install no process-global provider.
No client, connector service, pool, executor or task is retained or spawned.
Connect, TLS/headers, each body-frame entry and final connection disposal are
bounded at 15/30/30/5 seconds. Directly poll the pinned connection concurrently
with request/body progress. Yield at most one body frame per runtime entry;
write and hash that frame synchronously after leaving Tokio. After dropping
the sender/body, poll the pinned client connection future to completion; a
timeout drops the sole connection/socket and returns a transport failure.

Create a fresh `NamedTempFile` per top-level URL. Reject `Content-Length` or
streamed bytes above 128 MiB, never allocate the complete body, and delete
partial captures on every redirect/fallback/error/stale/cancellation path.
The immutable selected plan remains request structural state. Resolver values,
TLS config, buffers (one Hyper frame), hasher and capture are transfer-owned
command scratch; none enters DICE, equality, a cache, retained materializer
state or a semantic path.

The callback briefly probes the active session before a new address, redirect
or frame and stops more work when inactive; no lock spans work. The existing
post-callback token check then propagates the typed stale-session error and
remains the sole publication authority. Overlapping commands own disjoint
transport scratch. A duplicate in the same active session reuses its published
terminal. Error terminals are not cross-session acceptance: every later
command recaptures, while changed A/B/A requests retain existing structural
request equality and generation behavior without a URL/path side table.

## Exact dependency and file authority

Entry hashes/lines from `1807b1d4` are: `Cargo.lock` 4,876/
`29c633ff…`, core `Cargo.toml` 46/`27dfec84…`, `runtime/mod.rs`
333/`fcd2f5b4…`, `dice.rs` 11,636/`8c791759…`, `repository_io.rs`
4,539/`d1ceab49…`, `repository_archive.rs` 723/`1e6236d7…`, and
`tests/repository_archive_tests.rs` 1,333/`1a1cacfb…`.

Add exactly:

```toml
rustls = { version = "0.23.42", default-features = false, features = ["ring"] }
rustls-native-certs = { workspace = true }
tokio-rustls = { version = "0.26.4", default-features = false }
```

The isolated result is core `Cargo.toml` 49 lines/SHA-256 `847f18b0…`
and `Cargo.lock` 4,879/`72987efb…`: only those three direct names enter the
existing `slug_core_v2` lock dependency list, no package stanza changes, and
the Rustls tree contains Ring/std/TLS12/logging with no AWS-LC. Existing
Hyper/Hyper-Rustls packages and versions remain exact; the new path does not
use Hyper-Rustls's legacy client.

Write only these nine files: `Cargo.lock`, core `Cargo.toml`, `runtime/mod.rs`,
`runtime/dice.rs`, `runtime/repository_io.rs`, `runtime/repository_archive.rs`,
new `runtime/repository_archive_http.rs`, existing
`runtime/tests/repository_archive_tests.rs`, and new
`runtime/tests/repository_archive_http_tests.rs`. Ceilings are respectively
4,879, 49, 335, 11,730, 4,700, 850, 620, 1,500 and 850 lines. Keep production
additions <=760 and proof additions <=1,000. The >2,000-line DICE/materializer
files may receive wiring only; all transport policy/state belongs in the new
private module. `repository_io.rs` stays below 5,000.

## Proof, validation and STOP

Loopback proof uses a private test injection below production's HTTPS parser/
TLS boundary; no fixture is added. Prove Host/origin-form request, address
order, 200/206, 301/302/303/307, relative/absolute redirects, rejection of 308
and HTTP downgrade, redirect 40 boundary, the four admitted mirror-fallback
cases, immediate Slug-native fallback without a same-URL retry claim, fresh
resolution and capture, multi-frame streaming/SHA-256, mismatch, declared/
streamed size caps, connect/header/frame/disposal timeout, peer-held-open
disposal, connection completion/drop and cleanup. Static proof rejects `spawn`,
`spawn_blocking`, legacy `Client`, `GaiResolver`, executor, async filesystem,
global provider, full-body `Vec`, extraction and root creation in the new
transport owner.

Extend direct parser/native-session proof for exact stage mapping, capture
deletion, stale-token interruption, overlap, same-session duplicate reuse and
cross-command A/B/A recapture. Re-run the nine accepted archive tests. Validate
`cargo fmt --all -- --check`, `git diff
--check`, exact lock/Cargo hashes, Ring-only `cargo tree`, focused transport,
archive and native-session tests, `cargo check -p slug_core_v2 --locked`, the
core lib suite with the known baseline query failure recorded separately,
`cargo build -p slug_cli_v2 --locked`, exact `slugd` cleanup, and a fresh
wildcard-removed rules_rust query/build replay. The public replay may claim
only the unchanged collapsed repository-session terminal; direct proof owns
the new inner materialization stage.

STOP on dependency/version drift, AWS-LC/global provider, inability to poll and
dispose the sole connection without a task, network under dormant repository
I/O, DICE/lock I/O, retained capture/path/socket, full buffering, extraction or
root effects, changed local archive bytes/diagnostics, wider repository or CLI
behavior, fixture/Zabel/Bazel-tools mutation, subprocess/Java/JVM, cap breach,
second materializer, or a second material correction. `REPLAN` before widening.
