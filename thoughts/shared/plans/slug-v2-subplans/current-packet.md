# Current Slug V2 Packet

Packet: `WP-5-7A-selected-registry-bcr-archive-realization-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private selected-BCR capture/realization boundary, sole repository
materializer, and immutable-root publication seam
Base: `3bc02039`

Result: audit the accepted verified-capture bridge and select exactly one
bounded archive-realization design packet or `REPLAN`. This packet is docs-only.
Do not retain the current capture, add extraction dependencies or reactivate the
earlier full archive contract without re-deriving its behavior, resource,
ownership, dependency and proof boundaries from the accepted code.

## Accepted entry and live evidence

Commit `3bc02039` preserves the immutable `SelectedBcrTarGz` plan and adds a
native-runtime-only callback that resolves at most 64 addresses synchronously,
drives raw Ring Rustls/Hyper HTTP/1 without a task/client/global provider,
streams at most one frame per runtime entry into a 128 MiB capture, verifies
SHA-256 SRI and explicitly deletes the verified file. It then publishes the
generation-scoped `MaterializationError` text `selected-registry BCR archive
extraction is deferred`. The dormant materializer path remains offline at its
accepted deferred `TransportError`.

No DICE transaction or materializer lock spans DNS/runtime/I/O. Transfer state
enters neither request equality nor retained state. The existing final token
check remains publication authority; stale overlap stops before later-mirror
scratch and drops the current capture. Same-session duplicates reuse their
terminal while later A/B/A commands recapture.

Nine transport proofs cover ordered success/fallback, address/redirect/request
shape, streaming/SRI, all ceilings/timeouts, connection completion/disposal,
peer-held-open shutdown, cleanup and stale cutoff. Ten archive/session proofs
pass. Locked check/build and formatting pass; the full core suite is 298 pass
plus its declared unrelated query assertion failure. Fresh wildcard-removed
rules_rust query/build roots still expose only the public collapsed
`repository session failed` terminal.

## Behavior authority and architectural guidance

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Re-read `TarGzFunction`,
`CompressedTarFunction`, `CompressedTarFunctionTest`, `_http_archive_impl`,
`utils.bzl`'s remote-MODULE replacement order and the
`do_test_local_module_file_patch` shell regression. Reuse the accepted
`IndexRegistry#createArchiveRepoSpec` and rules_rust 0.73.0 producer/artifact
evidence. Add an oracle only for a demonstrated discriminating gap; port no
`@bazel_tools` content in this docs packet.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `src/load/session_natural_bzl_repository_source.zig` joins a selected view
to a completed materialization without host I/O, while
`src/bzlmod/session_generated_repository_materialization.zig` retains an
immutable root with its complete durable manifest. The live Zabel checkout may
have a different HEAD; inspect the pinned objects with `git show`. Copy no
Zabel code, archive behavior, scheduler, transport, cache, digest, path,
manifest or output representation. Bazel 9.2 alone owns compatibility claims.

## Audit decisions required

1. Trace how the verified capture can be consumed inside the same lock-free
   callback before deletion, without exposing a path/continuation in DICE or
   adding a second materializer. Name the exact cleanup and promotion points
   for transport failure, decode failure, MODULE failure, stale token, success
   and unwind.
2. Inventory the accepted rules_rust 0.73.0 tar.gz from the existing disposable
   evidence: compression/uncompressed size, entry count, GNU/USTAR/PAX forms,
   path/name bounds, regular/directory modes, executable bits and any link or
   special entries. Admit only demonstrated forms; set compressed,
   uncompressed, per-entry, entry-count, path-depth/name and expansion ceilings.
3. Derive gzip/tar behavior from the pinned Bazel sources: entry order,
   directory creation, duplicate paths, mode floor/preservation, timestamps,
   path traversal/absolute paths, malformed/truncated streams and unsupported
   links/specials. Classify every selected behavior exact, Slug-native or
   unsupported/deferred, including Rust valid-Unicode divergence.
4. Derive registry MODULE replacement as a second independently ordered HTTPS
   URL/SRI transfer after archive realization and before publication. Decide
   whether the private capture engine may be reused without widening generic
   `http_archive`; freeze delete/replace permissions and prove the final
   `MODULE.bazel` bytes and mode.
5. Define one provisional `TempDir` root owned wholly by the callback. Only a
   complete archive plus verified registry MODULE may become the existing
   immutable `Materialized` result; the post-callback token check either retains
   that root through the sole materializer or drops it. No semantic request
   identity may depend on the temp path.
6. Recompute dependencies and exact lock authority from `3bc02039`. Justify
   every direct `flate2`/`tar` or alternative edge, preserve Ring-only Rustls
   and existing versions, and reject subprocess/system tar, Java/JVM and full
   compressed/uncompressed buffering.
7. Select one file allowlist with exact entry hashes, line ceilings and
   production/proof caps. Keep transport policy in its private module,
   `repository_io.rs < 5,000`, DICE/materializer changes to wiring, and local
   archive behavior byte/diagnostic exact.
8. Specify proof for gzip/tar/mode/path/limit/error rows, MODULE independent
   SRI/replacement, cleanup/promotion/stale/A/B/A, dormant zero-I/O behavior and
   the real fresh rules_rust query/build transition. State the internal/session
   observable separately from any public collapsed diagnostic.

The known real artifact is 67,196,890 compressed bytes, 224,337,920
uncompressed bytes, 4,493 entries and SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
It is disposable command evidence, not a fixture to copy or commit.

## Compatibility and STOP

- **Exact:** accepted selected-plan/URL/fallback/redirect/SRI behavior; accepted
  local archive behavior; only separately pinned/evidenced gzip/tar entry,
  executable-mode and remote-MODULE semantics selected by the successor.
- **Slug-native:** Rust transport/archive mechanics, valid-Unicode path domain,
  resource ceilings and diagnostics, provisional-root lifetime and session
  sequencing.
- **Unsupported/deferred:** all BCR extraction/root/MODULE work until a
  successor is accepted; generic `http_archive`; links/specials unless the
  audit proves the artifact requires a bounded form; nonempty patch/overlay,
  auth/proxy/netrc, repository-rule breadth, toolchains/providers/actions/input
  trees, crate_universe, M8/M7B and exact configuration/output bytes.

Write authority is this manifest, canonical Live Status, Stage 5 and at most
one routing row only for a reusable `REPLAN` lesson. Rust, Cargo, lockfile,
tests, fixtures, generated/vendor content, `@bazel_tools`, Bazel and Zabel are
read-only. Documentation caps are <=45 canonical, <=210 current, <=90 Stage 5,
one routing row and <=420 aggregate.

Validate pinned source objects, artifact provenance/inventory, live
capture/materializer ownership, dependency candidates, scheduling agreement,
packet structure and `git diff --check`; obtain independent design review
before selecting implementation.

STOP any Rust/Cargo edit, behavior claim from Zabel, retained capture/path,
network or extraction in DICE/locks, unbounded allocation/expansion, full-body
or full-archive buffering, system archive/subprocess/Java/JVM, second
materializer, semantic temp path, global provider/AWS-LC, local archive change,
fixture/Zabel/Bazel-tools mutation, generic repository widening, second
successor or milestone closure. `REPLAN` before widening.
