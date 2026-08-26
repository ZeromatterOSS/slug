# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-archive-materialization-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `05-bzlmod-and-repository-graph.md`,
`06-analysis-toolchains-and-actions.md`, and core's request-owned repository
materializer/command session
Base: accepted selected-registry source owner and accepted root-package
external-Bzl route/load vertical

Result: design one bounded Rust-native materialization owner for the exact
selected-registry BCR archive slice required by rules_rust 0.73.0, then freeze
one implementation packet or `REPLAN`. This packet is docs-only.

## Accepted predecessor and live terminal

The frozen eight-file root route/load candidate is accepted against its actual
declared surface: root BUILD external loading from an already materialized
selected-registry source. Its direct selected-registry transaction proves the
structural request, route-before-child order, self/mapped recursion, package
result and lifecycle; focused and broad Rust validation pass. Ordinary route
callers remain closed. The corrected disposable command proof advances real
rules_rust from the old Host-loader rejection to the exact `rules_rust+`
materialization request. It no longer claims the downstream, unreachable
`repository_rule(doc=...)` terminal.

Native materialization then returns
`SpecError("repository override has unsupported attributes")`. The accepted
Bzlmod producer must not erase the complete `RepoSpec`, and loading must not
infer or inject a physical root. The next owner is the sole core repository
materializer.

## Learned facts and source authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is behavioral authority:

- `IndexRegistry#createArchiveRepoSpec` invokes the complete BCR archive setter
  chain; `IndexRegistryTest#testGetArchiveRepoSpec` proves ordered URLs, SRI,
  empty patch/overlay maps and registry MODULE replacement facts;
- `tools/build_defs/repo/http.bzl#_http_archive_impl` calls
  `download_and_extract` before `patch`, and `utils.bzl#patch` applies remote
  patches, replaces `MODULE.bazel`, then applies local patches;
- `HttpDownloader#download` tries URLs in order, and `HttpConnector` owns
  HTTP(S) redirect behavior and its finite redirect limit;
- `DecompressorValue#getDecompressor(Path)` and `DecompressorValueTest` select
  gzip/tar from `.tar.gz` or `.tgz` path suffixes when `type` is absent.

The exact rules_rust archive is 67,196,890 compressed bytes and 224,337,920
uncompressed bytes. Its SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`,
matching `sha256-LQyLlnthnVcXvoIQ9SokxapiTjIpo43EBxcS2x3VIvI=`. It has 4,493
entries: 3,544 regular files, 949 directories, no links and 27 executable
regular files. Its GNU `ustar  \0` header is rejected by Slug's private ustar
reader.

The existing `repository_io.rs` is 6,140 lines and mixes retained session state,
local/archive/Git/generated realization and extraction tests. Its archive slice
allows only one local `file://` URL, exact `type = "tar"`, hexadecimal `sha256`,
`strip_prefix` and a private ustar subset. `NativeDemandCommand::progress`
runs synchronously after each async DICE attempt, while core's private
`HyperRegistryIo` already owns an async Hyper HTTPS client but does not follow
redirects. Workspace dependencies contain `base64`, `flate2` and `tar`; core
does not yet enable them.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept/test guidance only.
`session_selected_external_loading_sources.zig`,
`session_bzl_repository_source_contract.zig`,
`session_natural_bzl_repository_source.zig` and
`session_selected_registry_materialization.zig` keep selected semantic views
above physical realization and join an immutable selected view with its
materialized root. Copy no Zig code, scheduler, cache, transport, archive,
digest, path or behavior.

## Decisions required

1. Freeze the exact accepted input shape: ordered nonempty HTTPS archive URLs,
   SHA-256 SRI, absent archive type inferred as gzip/tar from the URL suffix,
   empty `strip_prefix`, empty remote patch/overlay maps,
   `remote_patch_strip = 0`, and one registry MODULE URL plus SHA-256 SRI.
   Preserve the already accepted local `file://`/hex-SHA256/tar slice exactly.
2. Select one private shared HTTP transport owner or justify two clients. The
   selected design must cover ordered fallback, 301/302/303/307 redirects,
   relative/absolute `Location`, bounded redirects, status/body failures,
   streaming to command-owned temporary storage and exact SHA-256 verification.
   Registry 404 semantics must remain registry-owned.
3. Select a lawful async boundary. Network waits must not block a Tokio/DICE
   worker; gzip/tar/filesystem work must not run on one. No materializer mutex
   may span transport, extraction or DICE. If a blocking task is proposed,
   specify cancellation signaling, join/shutdown behavior and bounded cleanup;
   otherwise keep extraction on the synchronous command owner outside the DICE
   attempt.
4. Split a bounded private archive owner from `repository_io.rs`. It must parse
   the complete admitted `RepoSpec`, stream/verify the artifact, decompress and
   safely extract the demonstrated GNU tar regular/directory subset, preserve
   executable modes, reject traversal/link/special/duplicate ambiguity, and
   replace `MODULE.bazel` only after its independent download/SRI succeeds.
   Partial roots and captures remain unpublished scratch.
5. Preserve the existing session protocol: the complete request remains
   structural; transport/extraction failures remain generation-scoped typed
   results; the exact artifact digest supplies immutable source identity; the
   materializer revalidates the active token before publishing a provisional
   root; final command validation alone promotes it. Cancellation/discard drops
   every capture/root and publishes no result.
6. Produce one implementation packet with exact file authority, entry hashes,
   physical ceilings, production/proof caps and focused/broad proof. A likely
   boundary is private `http_transport.rs` plus `repository_archive.rs`, with
   narrow edits to core Cargo/module registration, `registry_io.rs`,
   `repository_io.rs` and `dice.rs`; accept or reject that boundary from the
   completed call/lifetime trace rather than naming a generic repository API.

## Request, revision and memory boundary

`RepositoryMaterializationRequest` and its complete `RepoSpec` are DICE-owned
semantic inputs. Registry URLs/mirrors, environment and lockfile policy, request
generation and selected mapping retain their existing projections. A shared
HTTP client is service/container memory only. Response buffers, digest state,
temporary captures, decompressor/tar state and extraction roots are transfer or
command scratch. Only a completed immutable root is retained nonsemantically by
the existing session after token validation; physical paths never enter request
equality.

Warm reuse and A/B/A must follow exact request equality, accepted observation
validation and repository generation. Overlapping commands remain isolated by
the existing workspace lease. Cancellation at every transport/extraction/
prepublication boundary must leave no published epoch, retained root, detached
task or live capture.

No fallback is selected. Subprocess `curl`/`tar`, synchronous network on a
runtime worker, direct filesystem injection, RepoSpec erasure, local-path
disguise, path-as-identity and a second materializer are rejected rather than
temporary bridges.

## Evidence, compatibility and successor proof

Reuse the accepted selected-registry source oracle, the exact downloaded
rules_rust artifact and the `rules-rust-073-toolchain-owner` command fixture.
Add only loopback transport/extraction unit cases needed to discriminate
fallback, redirect, SRI, cancellation, mode/path safety and MODULE replacement;
no copied registry subtree or new oracle fixture is justified. The future
implementation must rebuild `slug_cli_v2` and replay two fresh disposable roots
with only the parked wildcard registration removed. Both must materialize
identical source bytes and advance to the next honest command terminal.

- **Exact:** accepted root selected route/load behavior; admitted Bazel 9.2 URL
  fallback, redirects, SHA-256 SRI, gzip/GNU-tar regular/directory extraction,
  executable modes and registry MODULE replacement for this BCR source.
- **Slug-native:** Rust transport/extraction representation, bounded resource
  limits, typed diagnostics, command/session lifetime, sequential retry and
  observation/publication epochs.
- **Unsupported/deferred:** nonempty patches/overlays, symlink/hardlink/special
  archive entries, generic `http_archive`, authentication/proxy/netrc breadth,
  repository-rule declarations/effects, wildcard registration, toolchains/
  providers/actions/input trees, crate_universe, M8/M7B and exact
  configuration/output bytes.

## Authority, validation and STOP

Write authority is canonical/current, Stage 5, Stage 6 and at most one routing-
log row. Rust, Cargo, tests, fixtures, oracles, generated/vendor content and
`@bazel_tools` are read-only. Documentation caps are <=40 canonical, <=220
current, <=100 per stage plan, one routing row and <=460 aggregate.

Validate Bazel/Zabel pins and source anchors, exact artifact facts, complete
call/lifetime trace, scheduling agreement, packet structure and
`git diff --check`; obtain independent design review before selecting the
implementation packet.

STOP any Rust edit, new fixture, archive download in DICE compute, blocking
network on a runtime worker, unjoined/detached task, lock across I/O/DICE,
unbounded body/archive extraction, semantic physical path, second materializer,
subprocess/Java/JVM, patch/overlay/link/general repository breadth, command-
proof waiver, second successor, milestone closure, M8/M7B or exact identity
work. `REPLAN` before widening.
