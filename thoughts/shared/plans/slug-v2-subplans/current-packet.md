# Current Slug V2 Packet

Packet: `WP-5-m1-route-policy-and-package-lookup-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: atomic private route policy and external package lookup
Evidence: the accepted `HostRepositoryPathKey` in `00e85153`; accepted nonroot
package-policy and repository-path-state evidence; pinned Bazel 9.2 deletion,
external `REPO.bazel`/`.bazelignore`, and BUILD-marker ordering; and existing
root policy/evaluator helpers. Add no new oracle.

Edit exactly these four Rust files:

- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/package_policy.rs`
- `app/slug_bzlmod_v2/src/repo_file.rs`
- `app/slug_bzlmod_v2/src/repository_ignore.rs`

The packet cap is **650 production lines / 1350 test lines / 2000 total
lines** (formatted net additions; slack authorizes no behavior, evidence
family, or file beyond this contract). Route policy and lookup land atomically;
do not leave a second include-only policy graph. Lookup identity is the full
`RootRepositoryRoute` plus a canonical external `PackageIdentifier` whose
repository equals `route.canonical_repo()`. Apparent spelling and source spans
remain adapter diagnostics only.

Add the minimal root-policy projection containing only canonical deleted
packages; root package roots, vendor policy, root `REPO.bazel`, and root
`.bazelignore` do not enter external identity or equality. Compute in this
exact order: validate package name; request the minimal deletion projection;
if canonically deleted, return typed `Deleted` before any materialization,
path/source request, route-local Need, or policy event; otherwise evaluate the
routed `REPO.bazel`, apply that repository's `.bazelignore`, then inspect
`BUILD.bazel` and only after a complete missing/non-file primary inspect
`BUILD`. Apparent `@name//pkg` does not match direct-local canonical
`@@name+//pkg`. Regular and special resolved files are markers; directory and
other non-files are not. A primary Need or typed error stops before fallback.
Do not read BUILD marker bytes.

The routed REPO policy owner owns its complete evaluation child event batch,
including print/error behavior. Lookup and ignore add no event batch; parents
select child batches through activation, events never enter semantic equality,
and no route-local batch exists on the global-deletion short circuit. Forward
all `SourcePreparationNeeds` unchanged. Keep validation, materialization/path,
source, and REPO/ignore failures typed, with typed `Deleted` and `NoBuildFile`
outcomes. Complete values/errors use semantic equality; transient Needs remain
invalid and self-unequal.

Cover canonical-versus-apparent deletion and its no-route/no-event short
circuit, REPO and ignore edits, BUILD priority/fallback, missing/directory/
regular/special/symlink/error states, route A-to-B-to-A, and
create/delete/recovery under a retained engine. Preserve current root-only
owners and current default empty deleted set.

Stops: no marker-byte read, public export or loading migration, root lookup
reuse as external policy, direct filesystem IO, second materialization/path
owner, fragment horizon/closure, evaluator/default/print changes, contextual
mapping, registry/JVM transport, fixture/oracle, or file outside the allowlist.
Run focused serial owner tests, direct dependent tests, formatting, and
scope/cap/diff gates. The sole successor is the four-file public selected-BUILD
source/loading migration (260/650/910); package horizon,
occurrence-preserving closure, and evaluator/event correction remain later.
