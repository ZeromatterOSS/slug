# Current Slug V2 Packet

Packet: `WP-5-6-7A-selected-registry-bcr-dependency-closure-correction`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: core dependency/lockfile boundary and accepted archive implementation
contract
Base: `ce813cae`

Result: correct the exact dependency and lockfile authority for the accepted BCR
archive implementation, then freeze one implementation packet or `REPLAN`.
This packet is docs-only.

## Accepted predecessor

The semantic/archive shape and HTTP lifecycle are accepted. The implementation
remains bounded to an archive-only direct HTTP/1 connection: synchronous
command-owned resolution, immutable per-attempt resolver, explicit Ring TLS
configuration, pinned connection/sender/body across bounded runtime entries,
capture/hash writes outside Tokio, final driven shutdown, bounded extraction
and existing token-revalidated publication. Registry behavior remains
unchanged.

Pinned Bazel 9.2 commit `8220c619…` remains behavior authority. Pinned
`../zabel` commit `c7298478…` guides only the separation of the
producer-owned selected semantic view from physical materialization and the
natural view/root join. Copy no Zabel code, dependency choice, transport,
scheduler, cache, archive, digest, path, output or behavior.

## Correction trigger

The first implementation contract incorrectly inherited workspace Rustls,
which would enable AWS-LC beside Hyper-Rustls's Ring feature and make unchanged
implicit registry provider selection ambiguous. Independent review corrected
that to a direct non-workspace Rustls 0.23 dependency with
`default-features = false`, only `ring`, explicit archive-local
`builder_with_provider`, and no global provider installation.

The corrected contract then incorrectly marked `Cargo.lock` read-only. Cargo's
lockfile records each workspace package's direct dependency names, and
`flate2`/`tar` bring an unresolved bounded transitive closure. `cargo check
--locked` correctly rejected the mismatch. The worker reverted all partial
edits; the live checkout is clean and no Rust candidate exists.

This is the second material contract correction, so it is a design replan
rather than a lockfile waiver or an implicit scope expansion.

## Decisions and evidence required

1. In an isolated worktree at `ce813cae`, add workspace `base64`, `flate2`,
   `tar` and `tower-service`, plus direct Ring-only Rustls 0.23, to core.
   Resolve the lockfile once without changing production code.
2. Freeze the isolated accepted lock diff: add direct dependency names
   `base64 0.21.7`, `flate2`, `rustls`, `tar` and `tower-service` to
   `slug_core_v2`; add exactly `adler2 2.0.1`, `crc32fast 1.5.1`,
   `filetime 0.2.29`, `flate2 1.1.9`, `miniz_oxide 0.8.9`,
   `simd-adler32 0.3.10`, `tar 0.4.46` and `xattr 1.6.1` with their Cargo-
   resolved sources/checksums/dependencies. No existing package version/source/
   checksum or unrelated workspace package may change.
3. Prove `cargo tree -p slug_core_v2 -e features -i rustls --locked` contains
   Ring and no AWS-LC feature. Prove unchanged `HyperRegistryIo` can still
   construct its implicit native-root client without provider ambiguity.
4. The first unconstrained resolution attempted an unrelated `wasip2` downgrade.
   Preserve the existing `wasip2 1.0.4+wasi-0.2.12` entry exactly. The resulting
   candidate lock is 4,952 lines with SHA-256
   `ecedcf984bf6704dbbb48a62cb56ec56bbc0f221d09db573fea96706bf7bf710`.
5. Freeze `Cargo.lock` in implementation authority from its original 4,875-line
   `ee9acebd…` entry with a 4,960-line ceiling and only the exact 77-line
   accepted addition. Retain the seven source files, hashes/ceilings and every
   production/proof/relocation cap.
6. Preserve the accepted implementation contract verbatim except for the
   dependency/lockfile correction. Do not reopen HTTP lifecycle, BCR semantics,
   extraction, session publication, registry behavior or milestone scheduling.
7. Require the implementation worker to run Cargo with `--locked` after the
   one authorized lockfile update and to stop on any further lock drift.

## Compatibility and ownership

- **Exact:** accepted selected route/load behavior and admitted Bazel 9.2 BCR
  URL/SRI/archive/MODULE behavior; lockfile integrity for the actual Rust graph.
- **Slug-native:** direct Ring HTTP/1 transport and dependency representation,
  bounded lifecycle/resources, diagnostics and session/root ownership.
- **Unsupported/deferred:** every archive/repository/toolchain/action/M8/M7B
  surface already deferred by the accepted implementation contract.

Request equality, DICE identity and repository results contain no dependency,
provider, runtime or physical-path state. The TLS configuration is service
memory; resolver, connection, capture, digest and root remain command/transfer
scratch. Lockfile bytes describe the compiled implementation only and never
enter build semantics.

Write authority is canonical/current, Stage 5, Stage 6 and at most one routing
row. Rust, Cargo manifests/lockfile, tests, fixtures, generated/vendor content,
registry code and `@bazel_tools` are read-only. Documentation caps are <=40
canonical, <=220 current, <=100 per stage plan, one routing row and <=460
aggregate.

Validate the isolated dependency/lock diff, Ring-only feature graph, scheduling
agreement, packet structure and `git diff --check`; obtain independent review
before selecting any implementation packet.

STOP any live Rust/Cargo edit, package/version/checksum drift, AWS-LC/dual
provider, global provider installation, registry change, eighth source file,
HTTP/archive/session reopening, weakened `--locked`, cap waiver, subprocess/
Java/JVM, second successor or milestone closure. `REPLAN` before widening.
