# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-repository-owner-implementation-retry`
Milestone: cross-stage M7 prerequisite implementation retry
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement the accepted file-only source-kind/error boundary for the immutable
`@@bazel_tools` catalog before retrying its route/source owner.

## Active implementation retry

Implement exactly the accepted **Frozen source-kind decision** and
**Reviewed retry contract** below. Their file/asset allowlist, caps, proof, and
stops are the sole authorization. Independent Sol review accepted the file-only
key: its type encodes expected File, a source-known directory is
`WrongKind { actual: Directory }`, and no directory-success, missing, or
generic expected-kind branch is admitted.

Root owns all edits, serial validation, cleanup, integration, and commit.

## Predecessor REPLAN

`WP-5-builtin-bazel-tools-repository-owner-implementation` ends `REPLAN`.
The accepted route/snapshot/catalog design required directory and wrong-kind
as distinct terminals, but the attempted
`BuiltinBazelToolsSourceFileKey(snapshot, path)` carried no expected-kind
identity and its public error enum had no wrong-kind variant. Its focused test
therefore proved directory versus unsupported-catalog only. The packet had
already consumed its sole correction on the required pre-`RepoSpec` Host
guard, so this second material contract miss cannot be corrected in place.

The entire unaccepted Rust, BUILD metadata, tests, and seven copied assets were
discarded. Retained evidence only: all seven pinned Bazel 9.2 archive files
matched their recorded SHA-256 values and archive mode 755; focused 4/4,
bzlmod 470/470, and loading 136/136 passed; core remained at its documented
173/174 external-visibility diagnostic baseline. None of that evidence
authorizes production or source assets until this design is accepted.

## Active decision contract

Audit the existing repository package, Bzl, repo/ignore, and source-file
consumers to determine whether Slug needs:

1. a file-only immutable source key, where a source-known directory is exactly
   `WrongKind { expected: File, actual: Directory }` and no separate
   directory-success surface exists yet; or
2. a general typed immutable entry key whose structural key includes
   `ExpectedKind::{File, Directory}`, with file bytes/mode/digest only on
   File success and explicit kind mismatch.

Choose the smallest observable boundary that can later dispatch existing
package/Bzl file consumers without false exact claims. Freeze key identity,
value/error algebra, equality/validity, normalized-path precedence,
partial-catalog unsupported semantics, integrity ordering, and how the Host
source path fails closed before `RepoSpec`. No lock may cross a DICE compute.

The versioned snapshot, seven-file verbatim catalog, manifest framing, exact
file SHA-256/archive mode, canonical `bazel_tools` route, root-carrier
Need/error ordering, and absence of Host/runtime source selection remain as
accepted in the predecessor design. Do not broaden them while deciding kind.

## Compatibility and stops

Exact: source-known File versus Directory kind, normalized path lookup,
verbatim bytes, file SHA-256/archive executable state, and canonical built-in
repo name. Slug-native: typed key/value/error names, snapshot identity,
manifest framing/digest, diagnostics, and compact storage. Deferred:
out-of-catalog missing claims, directory enumeration, symlinks/special nodes,
complete embedded tools, package/Bzl evaluation, external MODULE mappings,
rules_shell/platforms/coverage, Test semantics/execution/BEP, Windows,
JVM/Java, and exact Bazel identity bytes.

Stop and `REPLAN` on a generic filesystem abstraction, Host observation,
runtime source selection, invented missing-file semantics, package activation,
a second repository graph, or an expected-kind field that does not affect key
identity and tests.

## Scope and proof

This design-only packet may edit canonical/current, Stage 4/5/8 bookkeeping,
and the routing log. Add no Rust, BUILD/Cargo/dependency, source asset, fixture,
DICE key, package/loading/core/query/command code, schema/wire, Test/REAPI/BEP,
JVM/Java, Windows branch, Stage 9/10, or workspace file. Cap bookkeeping at
180 net lines.

Require: a complete live-consumer audit; one selected algebra with rejected
alternative; exact path/kind/error precedence; structural equality/validity
and compact storage decision; an explicit successor file/asset allowlist,
caps, focused owner and direct-dependent tests, and stops; source/structure,
archive active-layout, credential, cap, and diff checks; and independent Sol
review because this corrects a public DICE/source identity contract.

One bounded correction is allowed; a second material miss is `REPLAN`. At
`ACCEPT`, schedule only the reviewed retry implementation, commit, and
continue.

## Frozen source-kind decision

Choose option 1, the file-only key. Every live consumer is file-specific:
repository package loading requests a selected BUILD file; repo and ignore
owners request `REPO.bazel` and `.bazelignore`; repository Bzl loading
requests the label's source file; core requests an exported source file.
Existing Host behavior already returns
`RepositorySourceFileError::WrongKind { actual: Directory }` to these callers,
and no live consumer asks this boundary to return a directory value.

Therefore `BuiltinBazelToolsSourceFileKey(snapshot, path)` structurally means
read this regular file; expected File is encoded by the key type and need not
be a redundant field. Its public result algebra is:

- `Ok(BuiltinBazelToolsSourceFileValue { path, bytes, sha256, executable })`
  for a catalog file whose integrity metadata matches;
- `InvalidPath` for invalid lexical paths;
- `WrongKind { path, actual: Directory }` when the normalized path is a
  source-known strict directory prefix;
- `UnsupportedCatalog { path }` for every other normalized unlisted path; and
- `Integrity { path, expected_sha256, actual_sha256 }` for a listed file
  whose checked-in bytes disagree with the frozen metadata.

There is no separate `Directory` terminal and no exact missing-file terminal.
Precedence is invalid lexical path, exact catalog file plus integrity, known
strict directory prefix, then unsupported catalog. Directory knowledge derives
only from sorted catalog prefixes; there is no enumeration, directory value,
Host observation, or filesystem lookup.

Reject option 2. An `ExpectedKind` field and directory-success value would add
a public identity branch with no consumer or observable result, invite false
completeness claims for the partial catalog, and duplicate the file-specific
type's expected kind. A future admitted directory consumer must design its own
typed boundary rather than widen this key implicitly.

The source key derives structural equality/hash from snapshot and normalized
path. Complete successes and every typed terminal are valid and compare by
their complete structural value; no Need is possible because the catalog is
compiled immutable input. The returned Arc bytes, exact SHA-256, executable
bit, and path all participate in equality. Manifest/snapshot identity remains
on the structurally distinct `RootRepositorySource::BuiltinBazelTools` route.
The route key still computes the root carrier first. Existing Host
materialization/source code must return an explicit unsupported-owner error
before any `repo_spec()`, Host observation, or materialization request.

Use existing `CompactString`, immutable `Arc<[u8]>`, `Allocative`, and
`Dupe` only where every field is cheap-clone. Do not derive `Dupe` across
`CompactString`; ordinary Clone is correct for those values. No global cache,
interner, weak identity hash, lock, or Stage 9 import is authorized.

## Reviewed retry contract

On `ACCEPT`, schedule only
`WP-5-builtin-bazel-tools-repository-owner-implementation-retry`. It may edit:

- `app/slug_bzlmod_v2/{BUILD.bazel,src/lib.rs,src/host_module.rs,
  src/source_preparation.rs}`;
- one new `app/slug_bzlmod_v2/src/builtin_repository.rs`;
- one new `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`;
- exactly the seven reviewed assets under
  `app/slug_bzlmod_v2/builtin/bazel_tools/`; and
- canonical/current and Stage 4/5/8 bookkeeping.

Cap production Rust at 420 net lines, tests at 360, assets at seven files/64
KiB, BUILD metadata at 20, and bookkeeping at 180. Add no generic directory
key/value, built-in consumer dispatch, package/loading/core production edit,
registry/materializer route, runtime source input, generated source, fixture,
Cargo/dependency, schema/wire, command/Test/REAPI/BEP behavior, JVM/Java,
Windows branch, second snapshot, Stage 9/10, or workspace file.

Required proof is: exact SHA-256 and executable-state goldens for all seven
files; golden manifest framing/digest; snapshot/route structural
discrimination; reserved apparent-to-canonical routing after root success
while root Need/error, unknown repo, and local override behavior remain
unchanged; valid file, invalid path, source-known directory WrongKind,
unsupported catalog, and exercised integrity failure; Host rejection before
`repo_spec()` with no observation/materialization; two-workspace and root
A/B/A byte invariance; focused owner tests; full `slug_bzlmod_v2`; direct
`slug_loading_v2` and `slug_core_v2` tests with documented baseline
classification; formatting, pinned-archive byte/mode comparison, archive
active-layout, source/structure, credential, cap, and diff checks; cleanup
review; and independent Sol final review.

Stop and `REPLAN` rather than adding expected-kind identity, directory
success, a distinct Directory error, missing semantics, consumer dispatch,
Host reads, or package behavior. One bounded implementation correction is
allowed; a second material miss is `REPLAN`.
