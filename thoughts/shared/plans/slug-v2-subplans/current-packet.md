# Current Slug V2 Packet

Packet: `WP-5-m1-host-repository-path-key-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private route-aware path-state prerequisite for external package policy
Evidence: accepted repository-path-state, direct-local route/materialization,
and whole-command retry evidence. The live `PathObservationKey`,
`ResolvedPathKey`, `RepositoryMaterializationResultKey`, and Need retry owner
already supply the sparse observation/retry substrate.

Edit exactly one Rust file:
`app/slug_bzlmod_v2/src/source_preparation.rs`. The packet cap is **170
production lines / 360 test lines / 530 total lines** (formatted net additions;
slack authorizes no behavior, test family, or file beyond this contract).

Add the crate-private, not publicly re-exported,
`HostRepositoryPathKey { route: RootRepositoryRoute, repo_relative_path }` and
crate-private constructor/value access needed by sibling bzlmod modules. It is
the sole route-keyed, bytes-free owner of a repository-relative final path
state: validate the relative path, obtain the existing route materialization
result, and use the existing `ResolvedPathKey`.
Its complete value retains the resolved-path namespace/instance, requested and
real paths, and state/lstat/kind so immutable-generation and symlink-retarget
changes cannot leave stale downstream observations. The new key must not read
bytes, create a second materialization/path observer, use direct filesystem IO,
or expose a public API.

Refactor `HostRepositorySourceFileKey` to consume `HostRepositoryPathKey` and
request `FileBytes` only after a regular or special result. Both keys provide
the existing `RepositorySourceScope`. Preserve the source key's public bytes,
logical-path, typed-error, Need, equality, and event-neutral behavior; it must
no longer materialize or resolve the route independently. The path key returns
missing and every terminal kind, including directory/non-file, as complete path
state; only the byte-reading source key maps non-file to its existing typed
`WrongKind` error.

Forward `SourcePreparationNeeds` unchanged at validation, materialization, and
resolution boundaries. Keep invalid-relative, materialization,
resolution-cycle/expansion/observation, inconsistent-state, and compute
failures typed; complete values/errors retain operational equality,
while transient Need results are invalid and self-unequal. It owns no event
batch. Cover source behavior after refactor, path-only absence/regular/special/
non-file results, no path-only `FileBytes` dependency, symlink retarget, local
and immutable operational identity, exact materialization/path Needs, route
A-to-B-to-A, and create/delete/recovery without a new oracle.

Stops: no package policy or deletion projection, `REPO.bazel`/`.bazelignore`,
BUILD marker selection, source-byte behavior change, loading caller migration,
include acquisition, evaluator/mapping/registry work, fixture change, or
Bazel/oracle work in this packet. Run focused serial Cargo tests, formatting,
and scope/cap/diff gates. The sole successors are the atomic four-file route-
policy/lookup packet (650/1350/2000), then the four-file public selected-BUILD
source/loading migration (260/650/910); package horizon, occurrence-preserving
closure, and evaluator/event correction remain later.
