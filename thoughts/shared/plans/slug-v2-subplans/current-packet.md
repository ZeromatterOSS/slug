# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-http-lifecycle-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: core archive transport/realizer, request-owned repository materializer,
and native command session
Base: `207b225b` plus the accepted BCR semantic/archive-shape design

Result: correct the rejected Hyper task/DNS lifecycle, then freeze one bounded
implementation packet or `REPLAN`. This packet is docs-only.

## Accepted predecessor and correction trigger

The selected-registry source and root external-Bzl load vertical is accepted.
It advances the real rules_rust command to a complete BCR `http_archive`
materialization request, which the sole native materializer rejects. The
accepted archive-shape design keeps that request structural and places physical
realization below the producer-owned selected view.

The first implementation draft is rejected. A
`hyper_util::client::legacy::Client<TokioExecutor>` necessarily spawns HTTP
connection drivers and may spawn blocking DNS work. It cannot satisfy the
draft's simultaneous claims of a shared legacy client, no spawned task and no
shutdown/join obligation. No Rust was changed under that invalid contract.

## Source authority and architectural guidance

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains behavior authority through `IndexRegistry#createArchiveRepoSpec`,
`IndexRegistryTest#testGetArchiveRepoSpec`, `_http_archive_impl`, `patch`,
`HttpDownloader#download`, `HttpConnector`, `DecompressorValue` and their named
tests. Preserve the already accepted rules_rust artifact facts and exact BCR
shape; this correction changes transport lifecycle only.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its selected external loading/source contracts keep the producer-owned
semantic view distinct from physical materialization and join the immutable
view with its root at the natural loading boundary. Retain that ownership
shape. Copy no Zig code, scheduler, transport, cache, archive, digest, path,
output or behavior; never use Zabel output as acceptance evidence.

The pinned local Hyper sources prove:

- legacy client HTTP/1 and HTTP/2 handshakes execute connection futures through
  the supplied executor;
- even with pooling disabled, the connection driver is spawned;
- default `HttpConnector` DNS uses `tokio::task::spawn_blocking`; dropping its
  `GaiFuture` aborts the join handle but cannot guarantee a running blocking
  resolver has stopped; and
- direct `hyper::client::conn::http1` exposes the connection future, while
  `HttpsConnector` can wrap an `HttpConnector` with a caller-supplied resolver.

## Selected correction

Do not share or alter `HyperRegistryIo`. Add one private archive-only transport
owner with cached native-root Rustls configuration, but no legacy client,
connection pool, executor or retained socket. Registry 404/status/body behavior
and its existing lifecycle remain unchanged.

For each archive or registry-MODULE URL attempt:

1. On the synchronous native command owner, parse the HTTPS authority and call
   the host resolver to completion before entering Tokio. Build a per-attempt
   immutable resolver result. This blocking DNS call is never inside DICE,
   Tokio, a Tokio worker or a spawned task. Native materialization is already a
   synchronous, non-preemptible progress step; cancellation is observed before
   resolution and immediately after it.
2. Build a per-attempt `HttpConnector` over that immutable resolver, with a
   finite TCP connect timeout, and wrap it in the retained native-root TLS
   configuration. Offer HTTP/1 only; negotiated protocol is not an admitted
   semantic surface.
3. Use bounded existing-runtime `block_on` entries to connect, perform the
   direct Hyper HTTP/1 handshake, drive the request headers, and yield at most
   one response-body frame at a time while also polling the same pinned
   connection future. The connection, sender and body remain command-stack
   transfer state between entries. Spawn no task and perform no filesystem or
   digest work inside an entry.
4. After each frame returns, synchronously hash and write it to the command-
   owned capture, enforce the body ceiling and observe cancellation before the
   next entry. Apply finite connect/header/idle waits. After terminal body/error,
   drop the sender and use one bounded final entry to drive the connection to
   shutdown. A timeout drops only command-stack futures and their socket; no
   client, pool, driver, resolver or join handle survives.
5. Apply redirect/fallback policy synchronously between attempts. Each changed
   authority gets a fresh completed resolver result and each attempt gets a
   fresh capture. Gzip/tar/filesystem work begins only after `block_on` exits.

The archive transport may add direct workspace `rustls` and `tower-service`
dependencies solely to retain the TLS configuration and supply the immutable
resolver. The previously selected `base64`, `flate2` and `tar` dependencies
remain. All are already workspace-pinned; no new lockfile package is admitted.

This is intentionally a second client path. Sharing a legacy registry client
would force service-task shutdown into DICE/runtime ownership and widen an
accepted registry surface. The archive path instead owns exactly the lifecycle
needed by a synchronous command materializer. It must not become a public or
generic repository transport.

## Decisions and proof required

Freeze an implementation packet that answers all of these from a complete
call/lifetime trace:

1. Name the exact synchronous progress call that owns DNS/capture writes, each
   bounded runtime entry, the pinned cross-entry transfer state and the exact
   point at which connection shutdown is complete.
2. Specify the immutable resolver type, address/port handling, TLS server-name
   preservation, request target/Host construction, and HTTP/1-only behavior.
3. Specify connect/header/idle/shutdown timeout ownership. A timeout must drop
   the socket and command-stack futures without leaving a task; DNS remains a
   completed synchronous boundary rather than a falsely cancellable Tokio task.
4. Preserve ordered fallback and 301/302/303/307 relative/absolute redirects,
   HTTPS-only redirect admission, 40 redirects, streaming SRI/body ceilings,
   typed generation-scoped failures and fresh captures.
5. Preserve the accepted bounded GNU tar extraction, Unix modes, path/link/
   duplicate safety, registry MODULE replacement, provisional-root publication
   and post-I/O active-token revalidation.
6. Prove no mutex spans DNS, transport, extraction, DICE or callback work; no
   physical path enters request equality; failure/cancellation publishes no
   epoch and drops every capture/root.
7. Freeze exact file authority, entry hashes, line ceilings and focused/broad/
   real-command proof. Prefer private `repository_archive.rs` and
   `archive_http_transport.rs`; do not edit `registry_io.rs` unless the completed
   trace demonstrates an unavoidable compile-only seam.

Focused lifecycle proof must include multi-frame streaming with capture writes
outside runtime entries and a loopback peer that remains open after its response
body: the transfer may return only after sender shutdown and connection-driver
completion. Also prove connect/header/idle/shutdown timeout cleanup, fresh
resolution after redirect authority change, zero executor/spawn/async-filesystem
calls in the archive transport, no registry drift and post-I/O stale-token
rejection.

## Compatibility and STOP

- **Exact:** accepted selected route/load behavior and admitted Bazel 9.2 BCR
  URL/fallback/redirect/SRI/archive-byte/MODULE behavior.
- **Slug-native:** Rust HTTP/1 transport, synchronous host resolution, finite
  resource/time ceilings, diagnostics, request/session sequencing and root
  lifetime.
- **Unsupported/deferred:** generic `http_archive`; HTTP origins/downgrades;
  auth/proxy/netrc; nonempty patches/overlays; archive links/specials; non-Unix
  BCR extraction; repository-rule declarations/effects; wildcard registration;
  toolchains/providers/actions/input trees; crate_universe; M8/M7B; exact
  configuration/output bytes.

Write authority is canonical/current, Stage 5, Stage 6 and at most one routing-
log row. Rust, Cargo, tests, fixtures, generated/vendor content and
`@bazel_tools` are read-only. Documentation caps are <=40 canonical, <=220
current, <=100 per stage plan, one routing row and <=460 aggregate.

Validate the Bazel/Zabel pins, Hyper source claims, full call/lifetime trace,
scheduling agreement, packet structure and `git diff --check`; obtain
independent review before selecting any implementation packet.

STOP any Rust edit, legacy/shared archive client, executor or spawned task,
Tokio/blocking-pool DNS, unjoined connection future, network/extraction inside
DICE, lock across I/O/DICE, unbounded body/archive work, full archive buffer,
second materializer, semantic path, registry behavior change, subprocess/Java/
JVM, fixture mutation, broader repository behavior, second successor or
milestone closure. `REPLAN` before widening.
