# Current Slug V2 Packet

Packet: `WP-5-7A-selected-registry-bcr-transport-entry-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private selected-BCR archive plan, sole repository materializer, and
native command/runtime boundary
Base: `1807b1d4`

Result: audit the now-accepted semantic-plan/physical-realization seam and
select exactly one smallest transport-entry implementation packet or `REPLAN`.
This packet is docs-only. Do not reactivate the earlier full archive contract
without re-deriving its authority, hashes, dependencies, lifetimes and proof
from the live split.

## Accepted entry and live observation

Commit `1807b1d4` owns mutually exclusive private `LocalTar` and
`SelectedBcrTarGz` plans. The local one-file/hex-SHA256/tar implementation and
proof moved mechanically and remain exact. The selected BCR plan accepts only
the produced required `type = "tar.gz"`, ordered nonempty HTTPS URLs, 32-byte
SHA-256 SRIs, empty strip/patch/overlay fields, zero patch strip and one HTTPS
registry MODULE fact. It owns no runtime, transport, capture or root.

Malformed selected BCR shapes publish stable `SpecError`; the exact valid plan
publishes the generation-scoped Slug-native `TransportError` saying transport
is deferred. Focused archive/parser/session proof passes 9/9. The full core
suite is 288 pass/one independently reproduced baseline-only query diagnostic
failure. Locked compile, formatter, feature and diff checks pass.

A fresh disposable `rules-rust-073-toolchain-owner` root with only the parked
wildcard registration removed reaches the repository-session non-success
terminal. The public command layer intentionally collapses its inner result to
`repository session failed`; the direct native-session proof retains the exact
deferred transport result. Treat this as producer-to-plan wiring evidence, not
archive materialization or a public-diagnostic claim.

## Source authority and guidance

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains behavior authority through `IndexRegistry#createArchiveRepoSpec`,
`IndexRegistryTest#testGetArchiveRepoSpec`, `_http_archive_impl`, `patch`,
`HttpDownloader#download`, `HttpConnector`, `DecompressorValue` and their named
tests. Reuse the accepted rules_rust 0.73.0 artifact facts and producer proof
at `app/slug_bzlmod_v2/src/selected_repo_spec.rs:857-889` and its direct
`type = "tar.gz"` assertion. Do not add an oracle unless this audit identifies
a discriminating evidence gap.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept-only architectural
guidance. Its selected-source contracts keep the producer-owned semantic view
above nonsemantic physical realization and join an immutable root only after
realization succeeds. Copy no Zabel code, scheduler, transport, cache, archive,
digest, path, output or behavior. Bazel 9.2 alone owns compatibility claims.

Recheck the pinned local Hyper sources: legacy clients spawn connection drivers
and default DNS may use `spawn_blocking`; direct HTTP/1 exposes the connection
future and permits a caller-supplied immutable resolver. Record any source/API
drift before relying on the earlier lifecycle design.

## Audit decisions required

1. Trace the exact live call from `NativeDemandCommand::progress_inner` through
   the current-thread runtime, `RepositoryMaterializer::materialize_native`,
   the lock-free callback and post-attempt token check to the private archive
   plan. Name where an async transfer can be driven without entering DICE or
   structural request state.
2. Decide whether the next bounded owner is transport-to-verified-capture only
   or must include gzip/tar/MODULE realization to preserve atomic publication.
   A capture-only packet needs a private command-owned continuation and exact
   deletion condition; it may not become retained semantic state or a second
   materializer.
3. Revalidate the archive-private direct HTTP/1 lifecycle: synchronous
   command-owned resolution, immutable address results, Ring-local TLS config,
   original hostname/Host, bounded connect/header/frame/shutdown entries,
   directly driven and joined connection future, ordered fallback/redirects,
   fresh capture per attempt, and no executor, task, async filesystem or global
   provider.
4. Recompute exact Cargo authority from `1807b1d4`. Only `base64 0.21.7` is now
   direct. Any `hyper`/`hyper-rustls`/`rustls`/`tower-service` or later
   `flate2`/`tar` edge and every lock byte must be separately justified. Keep
   workspace AWS-LC out of the archive path and never install a global provider.
5. Freeze command scratch, transfer-owned memory, cancellation, timeout,
   capture cleanup, provisional-root promotion, session generation, stale-token
   and warm/A/B/A behavior. No lock may span DNS, runtime entry, I/O, DICE or
   callback work; no physical path enters request equality.
6. Select one exact file allowlist with entry hashes, complexity ceilings and
   production/proof caps from the split modules. Preserve
   `repository_io.rs <= 5,000`; do not merge archive ownership back into it.
7. Specify focused loopback/lifecycle proof, direct dependent compiles and the
   fresh rules_rust replay. If the existing public error collapse prevents a
   claimed observable, state the internal/session proof honestly; do not widen
   CLI/session diagnostics inside a transport packet.

The prior full-contract artifact remains 67,196,890 compressed bytes,
224,337,920 uncompressed bytes, 4,493 entries and SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
It is real-command evidence only, not a fixture to copy.

## Compatibility and STOP

- **Exact:** accepted local archive behavior; produced Bazel 9.2 selected-BCR
  field/type/order/SRI shape; any separately evidenced URL/redirect/archive/
  MODULE behavior selected by the successor.
- **Slug-native:** private plan representation, Rust transport/lifecycle and
  ceilings, diagnostics, session sequencing and physical-root lifetime.
- **Unsupported/deferred:** all BCR physical work until a successor is
  accepted; generic `http_archive`; HTTP/auth/proxy/netrc; nonempty patches or
  overlays; links/specials; repository-rule semantics; wildcard registration;
  toolchains/providers/actions/input trees; crate_universe; M8/M7B; exact
  configuration/output bytes.

Write authority is this manifest, canonical Live Status, Stage 5, Stage 6 and
at most one routing row only if a reusable `REPLAN` lesson is found. Rust,
Cargo, tests, fixtures, generated/vendor content and `@bazel_tools` are
read-only. Documentation caps are <=40 canonical, <=220 current, <=100 per
stage plan, one routing row and <=460 aggregate.

Validate source pins, Zabel's guidance-only role, Cargo/runtime/session trace,
scheduling agreement, packet structure and `git diff --check`; obtain
independent review before selecting implementation.

STOP any Rust edit, behavior claim from Zabel, legacy/shared client,
executor/spawn/Tokio DNS, unjoined connection future, network/extraction in
DICE, lock across I/O/DICE, retained transfer scratch, unbounded/full-buffer
work, second materializer, semantic path, registry/global-provider change,
subprocess/Java/JVM, fixture mutation, broader repository behavior, second
successor or milestone closure. `REPLAN` before widening.
