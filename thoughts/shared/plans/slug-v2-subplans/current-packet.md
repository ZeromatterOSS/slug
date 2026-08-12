# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-typed-source-kind-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the typed source-kind/error boundary for the immutable
`@@bazel_tools` catalog before retrying its route/source owner.

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
