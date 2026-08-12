# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-repository-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement one immutable built-in `@@bazel_tools` repository/source DICE
owner and canonical routing boundary before package or Test closure activation.

## Active implementation contract

Implement exactly the accepted **Frozen owner design** and **Reviewed successor
contract** below. Their file/asset allowlist, caps, proof, and stops are the
sole authorization. The design packet received independent Sol `ACCEPT` on
2026-08-11: the partial catalog uses unsupported-catalog rather than false
missing, root Need/error ordering is preserved, no `RepoSpec` is fabricated,
and snapshot/path/bytes/digest/mode/manifest identity are structurally owned.

Root owns all edits, serial validation, integration, and commit.

## Predecessor REPLAN

`WP-4-6-8-bazel-tools-test-closure-design` reached its explicit stop. The
pinned Bazel 9.2 binary SHA-256 is
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`;
its installed embedded `MODULE.bazel` SHA-256 is
`a51e647c77be3c7dcb861131e339f2b65301bb572d2a9ac3d7eef30ca5b8a523`.

The verbatim `tools/test/BUILD` SHA-256
`81db88f41f7a9a07af246a42cfa7a8b6e118012b4f41830aaee9ffe4a4a9ee17`
loads `@rules_shell//shell:sh_binary.bzl` 0.6.1 and local
`default_test_toolchain.bzl`; defines toolchain/config-setting/filegroup/
select/sh_binary/coverage-alias targets; and selects wrapper/XML inputs through
`@bazel_tools//src/conditions:windows`, whose BUILD reaches
`@platforms` 1.0.0. The embedded module registers `//tools/test:all` and
owns the remote-coverage extension. Loading the whole package is observable
even when only POSIX setup/XML filegroups are later analyzed.

Slug inserts `bazel_tools -> bazel_tools` into repository mappings, but
`RootRepositoryRouteKey` routes only root direct `local_path_override`
modules and structurally owns that `RepoSpec`. No built-in source root exists.
Exact package loading also requires rules_shell loading, builtin filegroup/
config/toolchain analysis, and sparse noncoverage edge selection. A pruned
BUILD, synthetic package, host Bazel installation scan, or content-free label
would violate verbatim-content and semantic identity rules. The closure packet
ends `REPLAN` with no content, fixture, or production change.

## Accepted design record

Audit Bazel 9.2 built-in repository creation/source ownership and Slug's
`ResolvedGraph`, root mapping, `RootRepositoryRouteKey`,
`HostRepositorySourceFileKey`, repository package/Bzl source keys, package
loader, and request-demand owner. Freeze exactly one V2-owned immutable
`BuiltinBazelTools` source/route variant for canonical repository
`bazel_tools`; do not design package evaluation or Test semantics.

The route is structurally distinct from local/registry repositories and does
not fabricate a `RepoSpec`. It carries a versioned snapshot identity and
strong manifest/content digest. The source key owns normalized repository-
relative path lookup against checked-in verbatim bytes; directory, wrong-kind,
invalid-path, integrity, and unsupported-catalog terminals remain distinct.
The bounded catalog makes no exact missing-file claim outside its admitted
paths. Immutable bytes need no workspace observation epoch, but snapshot
identity and every returned byte participate in DICE equality/invalidation.
No environment, Bazel install path, network, filesystem scan, or workspace
override selects it.

Root and built-in mappings remain distinct. From main, `@bazel_tools` maps
exactly to `@@bazel_tools`. Inside the built-in repo, only source-proven
apparent mappings may be admitted later; this packet must not inject the whole
embedded MODULE dependency graph. No lock may cross a DICE compute.

Storage is compact and V2-owned: `CompactString`, immutable Arc slices,
`Dupe`, `Allocative`, and deterministic small maps where needed. Use
SHA-256 for file/manifest content and structural enum equality for DICE; no
weak hash becomes identity, and Bazel checksum, ActionKey, configured path,
and REAPI digest domains remain separate.

## Compatibility and stops

Exact: pinned 9.2 built-in canonical repo name, checked-in verbatim bytes,
normalized relative lookup, and file SHA-256. Slug-native: snapshot enum,
manifest framing/digest, diagnostics, and storage. Deferred: package
evaluation, embedded MODULE dependency graph, rules_shell/platforms/coverage,
filegroup/toolchain/TestProvider/TestRunner, execution/BEP, other embedded
packages, Windows, JVM/Java, and exact Bazel identity bytes.

Stop and `REPLAN` on runtime-selected sources, a generic registry route
disguised as built-in, full embedded-tools generation, package semantics,
network/JVM requirements, a second repository/package graph, or filesystem
bypass.

## Scope and proof

This design-only packet may edit canonical/current, Stage 4/5/8 bookkeeping,
and Stage 9 only if reuse status changes. Add no source asset, fixture, Rust/
test code, Cargo/dependency, DICE key, schema/wire, JVM artifact, package rule,
executor/materializer, BEP, Test behavior, Stage 10/CI, or workspace file. Cap
bookkeeping at 260 net lines.

Require pinned 9.2 source/archive provenance; exact route/source-key/equality/
invalidation/error ownership; an explicit successor file and source-asset
allowlist/caps; focused identity/path/bytes/error and snapshot invariance tests;
direct loading/core dependents; archive active-layout, source/structure,
credential, and diff checks; and independent Sol review because route identity
and DICE ownership change.

## Frozen owner design

Pinned source is Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` plus the installed archive
identified above. `RootRepositoryRouteKey` must first compute the existing root
module carrier so root Need/error ordering is unchanged. On a successful root,
the reserved apparent name `bazel_tools` selects
`RootRepositorySource::BuiltinBazelTools`; other names keep the current direct
local-override path and errors. The built-in source owns canonical repo and
module name `bazel_tools`, `BuiltinBazelToolsSnapshot::Bazel9_2`, and its
manifest SHA-256. It never owns a `RepoSpec` or materialized Host root.

`BuiltinBazelToolsSourceFileKey` is the only admitted byte owner. Its key is
the immutable snapshot identity plus a validated slash-separated relative
path. It rejects empty, absolute, repeated-separator, dot, parent, backslash,
NUL, and platform-prefix forms before lookup. Its value structurally owns the
file bytes, SHA-256, and executable bit. Known catalog prefixes return a
directory terminal; a file requested with the wrong expected kind returns
wrong-kind; bytes/digest/mode disagreement returns integrity failure; every
other normalized path is unsupported-catalog. A later complete snapshot may
add an exact missing terminal, but this packet must not infer absence from a
partial catalog.

The manifest is sorted by normalized path and hashes the snapshot tag and, for
each entry, length-framed path, exact file SHA-256, executable bit, and byte
length using a Slug-native domain-separated framing. File SHA-256 remains exact
content identity; manifest SHA-256 is not a Bazel checksum, ActionKey,
configuration token, or REAPI digest. Route/source keys derive structural
equality/hash. DICE caches immutable results directly; there is no global
cache, interner, observation epoch, lock, or compute-spanning guard.

The initial catalog is exactly seven verbatim files:

- `MODULE.bazel`;
- `src/conditions/BUILD`;
- `tools/test/BUILD`;
- `tools/test/default_test_toolchain.bzl`;
- `tools/test/dummy.sh`;
- `tools/test/generate-xml.sh`; and
- `tools/test/test-setup.sh`.

This is source ownership evidence, not package activation. The complete
`tools/test` directory, MODULE dependency mappings, Bzl evaluation, BUILD
loading, and every unlisted file remain unsupported/deferred. Existing
`CompactString`, immutable `Arc` slices, `Dupe`, and `Allocative` patterns are
adopted; V1/Buck cells, repository graphs, global interners, caches, and weak
precomputed hashes are rejected. No Stage 9 ledger change is required because
this uses already-adopted utility shapes without importing a representation.

## Reviewed successor contract

On `ACCEPT`, schedule only
`WP-5-builtin-bazel-tools-repository-owner-implementation`. It may edit:

- `app/slug_bzlmod_v2/{BUILD.bazel,src/lib.rs,src/host_module.rs}`;
- one new `app/slug_bzlmod_v2/src/builtin_repository.rs`;
- one new `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`;
- only the seven source assets listed above under
  `app/slug_bzlmod_v2/builtin/bazel_tools/`; and
- canonical/current, Stage 4/5/8 bookkeeping, the routing log, and Stage 9
  only if the implementation changes the accepted reuse decision.

Cap production Rust at 420 net lines, tests at 360, assets at seven files/64
KiB, build metadata at 20, and bookkeeping at 180. Add no package/source
consumer migration, loading/core production edit, generic registry/materializer
route, workspace input, generated source, fixture, Cargo/dependency, schema/
wire, command/Test/REAPI/BEP behavior, JVM/Java, Windows branch, or second
snapshot.

Required proof is: exact SHA-256 and executable-state goldens for all seven
files; manifest framing/digest and snapshot/route structural discrimination;
reserved apparent-to-canonical routing after root success while root Need/error,
unknown repo, and local override behavior remain unchanged; valid file,
directory, wrong-kind, invalid-path, unsupported-catalog, and integrity cases;
two-workspace and root A/B/A byte invariance; focused bzlmod tests; full
`slug_bzlmod_v2` plus compile/test of direct `slug_loading_v2` and
`slug_core_v2` dependents; formatting, archive active-layout, source/structure,
credential, cap, and diff checks; and independent Sol final review.

Stop and `REPLAN` rather than adding a generic consumer dispatcher, widening
the asset catalog, inferring missing paths, reading Host state, or activating
package semantics. One bounded implementation correction is allowed; a second
material miss is `REPLAN`.

One bounded implementation correction is allowed; a second material miss is `REPLAN`. At
`ACCEPT`, commit and continue by designing only the built-in
source-consumer dispatch/package boundary; do not activate package semantics
implicitly.
