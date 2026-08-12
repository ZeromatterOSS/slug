# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-package-preflight-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze route-independent package policy and BUILD-marker preflight for
one materialized nonregistry module.

## Accepted predecessor boundary

Commit `054844d4` accepts the route-independent nonregistry MODULE-closure
design. Its live-owner audit ends `REPLAN` before Rust because exact include
package preflight is still route-bound.

`RepositoryMaterializationKey` and `RepositorySourceFileKey` already own
route-independent local/immutable source bytes. However,
`ExternalRepositoryPackageLookupKey`, `HostRouteRepositoryIgnoreKey`,
`HostRouteRepoFileKey`, and `HostRepositoryPathKey` all require a
`RootRepositoryRoute` carrying apparent/canonical repository names.
`HostRootPackageBoundaryKey` is main-repository-only. Reusing either path
would fabricate routing for a transitive override; bypassing them would lose
REPO.bazel `ignore_directories()`, .bazelignore, deleted-package policy,
BUILD.bazel-before-BUILD selection, symlink/kind handling, and first-error
ordering.

A further identity constraint is explicit: root package policy stores
`--deleted_packages` as `PackageIdentifier` values, while a discovered
module's final canonical repository name is a post-MVS product. The
preselection owner cannot guess `name+` or a multiple-version suffix.

## Accepted design contract

Design the smallest crate-private package-preflight owner identified by
normalized workspace, exact `NonrootModuleKey`, and `PackagePath`. It must
compute root files first, require the exact `NonRegistry(RepoSpec)`, and use
only `RepositorySourceFileKey` for `REPO.bazel`, `.bazelignore`,
`<package>/BUILD.bazel`, and `<package>/BUILD`.

Freeze a route-independent REPO.bazel semantic value over the existing
`evaluate_repo_file` semantics and event contract, plus a route-independent
repository-ignore matcher reusing the existing parser. Preserve validation and
terminal order: invalid package, package-policy input, deleted-package
classification where structurally knowable, repository-ignore evaluation,
ignored package, BUILD.bazel, BUILD, then no marker. All source values retain
existing Missing/wrong-kind/symlink/Need/error semantics and local versus
immutable invalidation.

Resolve the deleted-package identity boundary explicitly. Either identify an
accepted preselection representation that Bazel 9.2 uses before final canonical
mapping, or admit only a structurally proven empty/unambiguous external-deleted
policy and fail closed on every ambiguous entry until selected-graph identity
exists. Do not compare by guessed module name or fabricate a canonical repo.

The output is only a typed package classification and selected marker identity;
it does not return BUILD bytes, evaluate BUILD/Bzl, prepare MODULE fragments,
or activate discovery/MVS/consumers. Freeze exact Rust allowlist, formatted
caps, focused local/immutable A/B/A and REPO/.bazelignore/marker/Need/error/
event/reuse proofs, downstream validation, and independent review gate. Return
`REPLAN` if the existing source owner cannot express REPO/.bazelignore
semantics without duplicate IO or if deleted-package policy requires
post-selection mapping.

## Compatibility

Exact: Bazel 9.2 package-policy and BUILD-marker preflight for explicitly
admitted nonregistry source and deleted-policy shapes. Slug-native: DICE type
and diagnostic names, Host observation framing, compact storage, and non-Bazel
identity bytes. Unsupported/deferred: ambiguous external deleted-package
mapping, MODULE closure/evaluation consumption, command overrides, recursive
selection, post-selection mappings, BUILD/Bzl evaluation, toolchains, Test,
execution/results/BEP/coverage, unadmitted RepoSpecs, Windows, JVM/Java, and
exact Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 230 lines. Add no Rust, Cargo/BUILD
metadata, fixture, asset, dependency, public surface, command behavior, or
production representation. Validate diff/scope/cap, active archive layout,
packet consistency, live owner and pinned-source citations, and independent
latest-diff review.

Stop with `REPLAN` on duplicate source/IO ownership, route/apparent/fabricated
canonical identity, lost REPO/.bazelignore/marker semantics, premature BUILD
bytes/evaluation, graph consumer, lock-held compute, JVM/Java, fifth file, or
cap excess.
Independent review accepts the route-independent package-preflight design.
The concrete seam needs no new file or public API. It composes the existing
source owner and pure evaluators through crate-private DICE keys, while
preserving the current route-bound owners unchanged for their accepted
consumers.

## Active implementation contract

Edit only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/repo_file.rs`;
- `app/slug_bzlmod_v2/src/repository_ignore.rs`;
- `app/slug_bzlmod_v2/src/package_policy.rs`; and
- `app/slug_bzlmod_v2/src/host_package.rs`.

In `repo_file.rs`, add a crate-private nonregistry REPO key keyed by
normalized workspace and exact `NonrootModuleKey`. It computes root files
first, requires the exact `NonRegistry(RepoSpec)`, reads only
`RepositorySourceFileKey(..., "REPO.bazel")`, reuses
`RootRepoFileSemanticsProjectionKey` and `evaluate_repo_file`, and preserves
capture/no-capture events, Need, missing-as-empty, source kinds, typed errors,
complete equality, and local/immutable invalidation.

In `repository_ignore.rs`, add the analogous crate-private ignore key. It
computes the new REPO key and `RepositorySourceFileKey(..., ".bazelignore")`,
then reuses `parse_ignore_file` and `RepositoryIgnoreMatcher`. Preserve
REPO ignored-directory plus .bazelignore union, Directory-as-absent, exact
parse/path Need/errors, events only from the owned REPO child, and complete
equality.

Add only a crate-private emptiness accessor to
`CanonicalDeletedPackages`, and make the existing pure
`host_package::invalid_package_name` validator crate-visible without changing
semantics.

In `source_preparation.rs`, add a crate-private
`HostNonregistryPackagePreflightKey` keyed by normalized workspace,
`NonrootModuleKey`, and `PackagePath`. Compute root files/override
classification and package-name validation first, then canonical-deleted policy.
Admit only an empty deleted-package set; any nonempty set is a typed
unsupported terminal before repository-ignore/source work. Compute the new
ignore key, return ignored when matched, then read
`<package>/BUILD.bazel` and `<package>/BUILD` serially through
`RepositorySourceFileKey`. Return only `BuildDotBazel`, `Build`,
`Ignored`, `InvalidPackageName`, or `NoBuildFile`, with typed policy,
source, REPO, ignore, and unsupported-deleted errors. Directory markers are
absent; all other source terminals remain typed. Need is invalid and complete
results/errors compare structurally. Add no caller.

Compatibility remains exact for admitted local/immutable RepoSpecs with an
empty canonical deleted-package set; nonempty deleted policy is explicitly
unsupported/deferred until post-MVS mapping exists. DICE names, diagnostics,
and identity bytes remain Slug-native. All MODULE closure/evaluation,
discovery/MVS, mapping, package loading, toolchain/Test/command/execution
consumers remain deferred.

Cap formatted net growth at 360 production lines, 520 test lines, and 880
total. Add no file, public export, Cargo/BUILD metadata, dependency, fixture,
asset, cache, lock, interner, process-global state, or direct filesystem/IO
path.

Focused tests must exercise the real keys and prove local and immutable A/B/A;
RepoSpec/category change; REPO/.bazelignore union and edit/error recovery;
invalid/unsupported-deleted/ignored/BUILD.bazel/BUILD/no-marker order;
Directory/missing/source failures; Need invalidity; capture/no-capture REPO
events; cold evaluated/warm reused; local versus immutable equality; and
structural absence of `RootRepositoryRoute`, apparent/canonical repository
identity, Host path/file keys, BUILD bytes, evaluation, graph, and consumers.

Run serially focused `host_nonregistry_package` tests, full
`slug_bzlmod_v2`, downstream `slug_loading_v2` and `slug_core_v2` checks,
`cargo fmt --all -- --check`, and `git diff --check`. Also run exact
scope/cap, no-public-surface, credential-pattern, active-layout archive, and
forbidden-edge scans. Obtain independent latest-diff implementation review.
Stop with `REPLAN` on evaluator/parser semantic change, nonempty deleted
policy admission, duplicate IO/source ownership, route/canonical identity,
BUILD bytes/evaluation, consumer, sixth file, or cap excess.
