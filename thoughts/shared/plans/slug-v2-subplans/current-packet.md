# Current Slug V2 Packet

Packet: `WP-5-m1-external-query-package-identity`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted external source identity commit `980373f9`, accepted
dependency-free external Starlark-rule query evidence and REPLAN, and the
independently accepted package-identity design in the owner plan. The 17-row,
598-line `module-local-override` fixture is frozen.

Implement only the private request-local repository-qualified query
package/candidate owner accepted in the owner appendix. Read `AGENTS.md`, the
orchestration skill and implementation-worker reference,
`docs/developers/dice.md`, and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing. Start from the
live checkout and inspect dirty diffs. Do not edit the accepted design while
implementing it.

The exact production allowlist is:

- `app/slug_query_v2/src/graph.rs`
- `app/slug_query_v2/src/provenance.rs`
- `app/slug_query_v2/src/loading_environment.rs`

The exact focused-test allowlist is unit tests inside `graph.rs`,
`provenance.rs`, and `loading_environment.rs` plus:

- `app/slug_query_v2/tests/loading_query.rs`
- `app/slug_cli_v2/tests/cli.rs`, only by extending
  `direct_external_query_matches_one_shot_and_retained_daemon_output_and_events`

No loading, bzlmod, CLI production, server, fixture, oracle, Cargo metadata,
protocol, canonical scheduling, external Bzl-owner, or other path may change.

Add exactly one private
`QueryPackageIdentity(Arc<QueryPackageIdentityData>)` whose data is either
`Root { package: PackagePath }` or `External { canonical_repo:
CanonicalRepoName, apparent_repo: ApparentRepoName, package: PackagePath }`.
Use private validated root/external constructors reached through
`QueryLabel::owner_identity()`. Reject root-with-apparent,
external-without-apparent, and root/nonroot mismatches as complete query
errors. Manual equality, hashing, and ordering use only full canonical package
identity; the first apparent route survives canonical alias deduplication.
Construction may clone owned typed-string buffers, but completed identity
clone is one pointer-sized `Dupe`. Add no interner, cache, map, lock, raw-string
owner, second shared route, DICE key/value, or public API.

Real candidates derive their owner transiently. Fake candidates retain the
shared owner; fake equality/hash includes printed-label canonical identity and
consuming-owner canonical identity, while real/fake stay distinct. Preserve
existing label-materialization and first-representative semantics for
`union`, `intersect`, `except`, `set`, and `let`.

Route `siblings`, `same_pkg_direct_rdeps`, `loadfiles`, and `buildfiles`
through the identity. Root Host and legacy candidates retain their existing
graph/load/companion owners. External candidates require Host mode, resolve
the retained apparent spelling through `RootRepositoryRouteKey`, verify its
canonical repository before any compute, and only then use
`ExternalUnconfiguredPackageGraphKey` or `RepositoryPackageLoadKey`. Rewrite
BUILD/Bzl labels only after canonical repository/package verification and
preserve the owner apparent spelling. Current external loadfiles remain empty;
do not activate an external Bzl loader or claim external fake Bzl companions.
An external owner must never reach a root graph/load/companion fallback.

Preserve Bazel 9.2 visibility semantics from the accepted source/probes:
Private and java aliases compare package fragments, including across
repositories; Restricted direct/package-group membership receives the full
canonical `PackageIdentifier`. Root group traversal remains existing behavior.
External visibility dependency labels and external restricted-group traversal
remain rejected pending separate visibility-content evidence. A fake target
remains visible and non-loadable.

Add only the accepted focused cases:

- `query_package_identity_canonical_equality_retains_first_apparent_route`
- `fake_candidate_owner_identity_is_symmetric_and_route_preserving`
- `external_owner_visible_private_uses_fragment_and_restricted_uses_canonical`
- `external_owner_dispatches_siblings_rdeps_and_loading_files_without_root_fallback`
- `external_owner_route_lifecycle_reuses_edits_deletes_recreates_and_recovers`

Also extend only the named CLI lifecycle test. Cover root/external real/fake
identity, two apparent aliases for one canonical package, siblings order,
same-package reverse deps, external BUILD and empty loadfiles, root/synthetic
fake-owner preservation, Private/Restricted discrimination, default Text and
label/graph/label_kind/package outputs, route remap, cold/warm,
edit/delete/recreate/error/recovery, and unchanged external-pattern rejection.
Do not add a fixture row.

Run serially after implementation:

- `cargo test -p slug_query_v2 provenance`
- `cargo test -p slug_query_v2 loading_query`
- `cargo test -p slug_cli_v2 direct_external_query_matches_one_shot_and_retained_daemon_output_and_events`
- `cargo check -p slug_cli_v2`
- `cargo test -p slug_query_v2 --target x86_64-pc-windows-gnu --no-run`
- `cargo test -p slug_cli_v2 --target x86_64-pc-windows-gnu --no-run`
- `cargo fmt --check`
- `scripts/v2_archive_status.sh`
- `git diff --check`

Clean stale `slugd` before and after the CLI smoke. Obtain one independent
latest-diff implementation review after root validation.

Stop with **REPLAN** rather than expanding scope if implementation needs a
second shared allocation or retained duplicate owner/route, new
interner/cache/lock/key, apparent-route semantic equality, public cross-crate
API, another source/observation/graph owner, direct filesystem access, a root
fallback for external provenance, unbounded discovery, external visibility
content, a cross-package/repository Bzl companion, or partial generic output.
Test/executable rules, suites, implicit/user dependencies, generated outputs,
external patterns/discovery, configuration, analysis/actions/execution,
repository rules/extensions, `@bazel_tools`, JVM, Java bytecode, and Bazel
delegation remain out of scope.
