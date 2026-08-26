# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-producer-shape-runtime-correction`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: selected BCR RepoSpec producer, private archive transport/realizer and
sole repository materializer
Base: `07359828`

Result: reconcile the exact produced BCR shape and reject the nonconforming
implementation candidate, then freeze one corrected implementation packet or
`REPLAN`. This packet is docs-only.

## Accepted predecessor

The root selected-registry route/load vertical, semantic/physical separation,
direct-connection lifecycle design and exact Cargo dependency closure remain
accepted. Pinned Bazel 9.2 commit `8220c619…` owns behavior. Pinned
`../zabel` commit `c7298478…` guides only producer-owned selected semantic
view versus physical realization and the natural immutable-view/root join.
Copy no Zabel code, transport, scheduler, cache, archive, digest, path, output
or behavior.

## Correction trigger

The implementation packet incorrectly admitted only absent archive `type`.
Slug's accepted selected RepoSpec producer in
`app/slug_bzlmod_v2/src/selected_repo_spec.rs` emits every structural archive
field from selected registry source facts. Its direct archive proof asserts
`type = "tar.gz"`; the real rules_rust source uses that canonical form. It
also emits empty-string `strip_prefix`, empty patch/overlay maps,
`remote_patch_strip = Int(0)`, one registry MODULE URL/SRI, ordered URLs and
archive SRI. Erasing or pretending those fields are absent is forbidden.

The retained candidate compiled and two extraction tests passed, but is
rejected before integration:

- it replaced all native `http_archive` dispatch with the BCR parser, breaking
  the accepted local file/tar branch rather than selecting between two plans;
- it used blocking raw `TcpStream`/Rustls HTTP parsing, not the accepted direct
  Hyper connection pinned across existing-runtime entries, and had no
  mid-transfer active-token probes;
- it followed all 3xx with a five-hop limit rather than exact
  301/302/303/307 and 40 redirects;
- it created the root before archive SRI verification and did not advance to
  the next source URL after every admitted transport/SRI failure;
- it reused generated-immutable naming, did not relocate the 838-line archive
  owner/proof, left `repository_io.rs` above its ceiling and supplied only two
  focused tests.

The candidate and its dependency prerequisite were removed entirely with
`apply_patch`; the live checkout is clean at the exact `07359828` source,
manifest and lock hashes.

## Decisions required

1. Freeze the exact BCR parser shape as the producer emits it: required
   `type = "tar.gz"`, required empty-string `strip_prefix`, required empty
   maps, required zero integer patch strip, ordered nonempty HTTPS URLs, archive
   SHA-256 SRI and one HTTPS MODULE URL/SRI. Reject absent/wrongly typed/extra
   fields for this demonstrated slice.
2. Preserve the accepted local parser as a separate first-class plan. Dispatch
   by exact mutually exclusive shape and retain its diagnostics/tests
   byte-for-byte; neither parser may partially accept then reinterpret the
   other.
3. Retain the accepted HTTP owner: archive-only explicit Ring/native-roots TLS,
   synchronous command-owned immutable resolution, direct
   `hyper::client::conn::http1` state pinned across bounded entries on the
   existing runtime, one frame returned per entry, synchronous capture/hash
   writes between entries, active-token probes and driven shutdown. Raw manual
   HTTP, a new runtime, legacy client, task or async filesystem is forbidden.
4. Freeze exact fallback/redirect semantics: only 301/302/303/307, relative or
   absolute HTTPS Location, 40 redirects, fresh authority resolution/capture,
   and next source URL after URI/DNS/connect/TLS/status/redirect/header/body/
   idle/shutdown/SRI failure. Create no extraction root before archive SRI.
5. Retain bounded GNU tar extraction, mode/path/link/duplicate/collision safety,
   verified MODULE replacement, exact archive identity, existing materializer
   publication and final validation. Use a preidentified-immutable name.
6. Require relocation of the contiguous existing archive owner/proof so
   `repository_io.rs` meets its ceiling. Restore the full discriminating proof
   matrix; two happy-path extraction tests are insufficient.
7. Retain the accepted exact 77-line Cargo lock delta, Ring-only graph and
   eight-file authority, but update the implementation packet's parser evidence
   and any necessary line ceilings from the complete call trace.

## Evidence and proof

Reuse the selected producer proof, exact rules_rust source/artifact, accepted
Bazel setter/downloader/decompressor anchors and existing local archive tests.
Add no oracle fixture. A corrected packet must prove:

- the exact produced `type`/empty/zero fields and local/BCR mutual exclusion;
- existing-runtime direct-Hyper frame lifecycle, token probes and shutdown;
- ordered fallback plus exact redirect set/count and SRI-before-root ordering;
- local byte-equivalence, real GNU tar/modes/MODULE bytes, safety/limit cleanup;
- Ring-only lock graph, no registry/global-provider drift, warm/A/B/A reuse;
- two fresh wildcard-removed rules_rust commands reaching the same next honest
  terminal.

## Compatibility and STOP

- **Exact:** accepted local archive and produced Bazel 9.2 BCR RepoSpec/URL/SRI/
  gzip-GNU-tar/MODULE behavior.
- **Slug-native:** direct Ring HTTP/1 representation, synchronous DNS, bounds,
  diagnostics, session sequencing and root lifetime.
- **Unsupported/deferred:** all generic archive/repository/auth/patch/link/
  non-Unix/toolchain/action/M8/M7B breadth already deferred.

Write authority is canonical/current, Stage 5, Stage 6 and at most one routing
row. Rust, Cargo, tests, fixtures, generated/vendor content, registry code and
`@bazel_tools` are read-only. Documentation caps are <=40 canonical, <=220
current, <=100 per stage, one routing row and <=460 aggregate.

Validate exact producer/Bazel facts, clean restored hashes, full call/lifetime
trace, scheduling, structure and `git diff --check`; obtain independent review
before selecting implementation.

STOP any live Rust/Cargo edit, producer-field erasure, local-branch regression,
raw/manual or legacy HTTP, new runtime/task/Tokio DNS/async fs, unjoined
connection, root-before-SRI, wrong redirect/fallback, registry/global-provider
change, lock/cap waiver, subprocess/Java/JVM, broader behavior, second successor
or milestone closure. `REPLAN` before widening.
