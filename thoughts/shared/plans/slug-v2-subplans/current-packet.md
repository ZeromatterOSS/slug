# Current Slug V2 Packet

Packet: `WP-4-5-7A-builtin-optional-package-input-projection-implementation-r2-correction-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `3b73a99cf`.

Result: freeze one bounded correction which moves built-in REPO absence before
root Starlark-semantics projection, proves it without injected policy inputs,
and corrects the implementation production ceiling from 220 to 280 lines.

## Learned facts and source basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains semantic authority. `PackageLookupFunction` checks deleted-package and
repository-ignore policy before probing `BUILD.bazel` then `BUILD`; a missing
optional marker is a normal no-package result. Repository metadata likewise
permits absent `REPO.bazel` and `.bazelignore`. The accepted built-in
`@bazel_tools` snapshot contains neither root metadata file and contains exact
`BUILD` files at `src/conditions` and `tools/test`.

Slug's accepted `HostRepositoryDirectoryListingKey` already owns sorted direct
membership for direct-local, selected-registry, generated and built-in routes.
The three current consumers do not yet use that source-neutral boundary:

- routed REPO and ignore owners call the materialized-only
  `HostRepositorySourceFileKey`, so normal built-in absence becomes a
  materialization error;
- external package lookup calls the materialized-only `HostRepositoryPathKey`,
  so built-in BUILD marker discovery fails before its package terminal; and
- direct built-in source lookup intentionally reports `UnsupportedCatalog` for
  a missing exact file and must remain fail-closed rather than reinterpret that
  integrity boundary globally.

The stopped external-boundary implementation therefore produced an opaque
repository-ignore error for a valid built-in package decision. Its complete
unaccepted Rust diff was removed; no split terminal or public boundary module
is retained.

The first prerequisite candidate also ordered `RootRepoFileSemanticsProjectionKey`
before its built-in root listing. That makes an absent built-in `REPO.bazel`
depend on unrelated Starlark policy and lets missing policy fail before normal
absence. The focused test initially masked this by injecting policy inputs.
Pinned Bazel requests REPO semantics only after discovering a REPO file.

Buck2 DICE guidance in `docs/developers/dice.md` requires each natural owner to
depend on the accepted listing key, keep Need transient, forward complete
observations and avoid locks across compute. No new oracle or fixture is
needed: accepted Bazel package-marker/metadata evidence and the verbatim
built-in catalog discriminate this correction.

Zabel's `session_directory_package_presence.zig` is concept/test guidance. It
keeps authenticated direct membership and producer-computed package presence
together while preserving followed-symlink decisions separately. Slug should
likewise use catalog membership only for the built-in disposition and leave
the existing materialized path/source producers unchanged. Do not copy
Zabel's session store, source IDs, allocator model, diagnostics or parity
claims.

## Accepted decision

Keep the accepted routed directory listing as the sole shared primitive; add
no second retained entry-kind key or source tree. Each existing semantic owner
adds only the built-in branch appropriate to the fact it already owns:

- `HostRouteRepoFileKey` obtains the built-in root listing. Absence of
  `REPO.bazel` yields the existing empty REPO value **before** computing root
  REPO Starlark semantics. Materialized routes keep their current policy,
  source-file, evaluation, event and observation behavior.
- `HostRouteRepositoryIgnoreKey` obtains the same built-in root listing after
  its REPO predecessor. Absence of `.bazelignore` contributes no additional
  prefixes. Materialized routes keep their current source-file and parser
  behavior.
- `ExternalRepositoryPackageLookupKey` obtains the built-in candidate-package
  listing and selects exact file entries in `BUILD.bazel`, then `BUILD` order.
  A missing directory or absent/file-ineligible marker yields `NoBuildFile`.
  Materialized routes keep the existing followed-path producer, including
  symlink and special-file behavior.

Use the legacy or observed listing sibling matching each parent key. Observed
outer error precedes Need, which precedes the semantic terminal; merge the
listing epoch exactly. Built-in catalog listings currently contribute an empty
path epoch because snapshot and manifest identity are structural in the route.
Do not inspect the catalog directly from any consumer.

Unexpected built-in `REPO.bazel` or `.bazelignore` file content is not silently
evaluated through a fabricated absolute path. Return a repository-relative,
typed fail-closed error and `REPLAN` the logical-source identity boundary when
a future authenticated snapshot actually adds either file. A directory-valued
`.bazelignore` retains the accepted no-additional-ignore behavior; an
unexpected wrong-kind REPO entry remains an error. This is an explicit future
snapshot stop, not a fallback.

After this correction, resume the already-reviewed external package-boundary
projection. Built-in package source bytes/evaluation remain a separate generic
source-identity concern; the resumed boundary packet may report package
presence but must not claim selected-external package loading or traversal.

Do not add traversal, package loading, target-pattern expansion, registration
activation, evaluator changes, language builtins, configured semantics, rules
or actions. Bazel 9 BCR Starlark remains the source of rule definitions,
including `cc_internal`; `cc_common` is only one consumer of the generic Rust
host builtin ABI. No C++ rule implementation belongs in Rust.

## DICE, identity and retained-state contract

The complete authenticated `RootRepositoryRoute` remains in every existing
parent key. Root or candidate `PackagePath` listing keys retain the same route,
so selected mapping, source disposition, immutable generation and built-in
snapshot identity all participate structurally in equality and invalidation.
No caller projects to display repository text or reconstructs a physical root.

No new retained value, cache, interner, global state or manual lock is added.
Parents retain only their existing semantic Result plus the existing immutable
observation epoch. Listing values remain shared `Arc`-backed immutable entry
slices and release with their DICE graph versions. Overlapping requests share
only immutable DICE results and retain independent injected request revisions;
cancellation publishes no partial terminal.

The route was authenticated by an earlier owner. A larger observed caller
continues to merge its route predecessor epoch; this packet owns only the
newly consulted listing epoch. There is no historical-filesystem fallback and
no async transfer or shutdown lifetime.

## Compatibility

- **Exact:** within the admitted point-lookup slice, missing built-in optional
  metadata and marker priority follow Bazel 9.2; the catalog remains verbatim
  upstream content. Existing materialized-route semantics remain exact.
- **Slug-native:** DICE keys, route/catalog identity, typed redacted error
  projection and observation carrier.
- **Unsupported/deferred:** built-in package-source logical identity and
  evaluation, selected-external recursion, target-pattern expansion,
  registration activation, configured validation, language builtin breadth,
  rules and actions.

## Correction-design scope

The exact rejected Rust candidate remains in the worktree. It compiles and its
focused built-in optional-input matrix passes only because that test injects
policy inputs before the REPO check. It must not be changed, accepted or
committed during this docs-only design. The candidate adds 252 production and
148 proof lines across three of the four allowed Rust files.

This correction may change only this manifest, the canonical plan, Stage 6,
`.codex/skills/slug-agent-orchestration/references/routing-log.md`, and
`.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`.
Require independent ordering/cap/scope review. If accepted, select
`WP-4-5-7A-builtin-optional-package-input-projection-implementation-r2` at the
docs commit, make only the ordering/test correction, then rerun validation.

## Preserved implementation scope

The active implementation allowlist is:

- `app/slug_bzlmod_v2/src/repo_file.rs` for built-in root REPO absence;
- `app/slug_bzlmod_v2/src/repository_ignore.rs` for built-in root ignore
  absence;
- `app/slug_bzlmod_v2/src/host_package.rs` for built-in marker selection and
  typed source-disposition/error projection; and
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs` only when the
  existing owner-local tests cannot express the observed cross-owner proof.

Corrected caps are 280 production and 420 proof additions, no dependency,
fixture, oracle, source asset or export. All three production files exceed the
2,000-line complexity trigger, but each change stays in its existing cohesive
semantic owner; a new central module would duplicate or expose private
consumer policy. Stop and `REPLAN` if a fifth file is required, a built-in
metadata file is present, materialized-route dependencies or terminals change,
a physical path is fabricated/exposed, direct catalog/source internals are
read by a consumer, or package source/traversal is needed.

Proof must cover built-in empty REPO, empty additional ignore, root no-package,
`src/conditions` and `tools/test` BUILD selection, exact marker priority,
listing failure projection, legacy/observed equality and complete-only
validity, empty built-in epoch, route/source A/B/A identity, and structural
dependency separation. Existing direct-local symlink/special-file, selected-
registry and generated marker/source tests must remain unchanged and green.
The built-in REPO-absence proof must use a transaction with no injected root
package-policy or REPO-semantics input; any policy dependency is a regression.
Run focused and full bzlmod tests plus downstream loading checks/tests,
formatting, diff, scope, cap, dependency, no-lock and archive-baseline gates.

## Immediate predecessor

Commit `5ec7f3c79` froze the external package-boundary projection and commit
`eb6843ebd` selected its implementation. The implementation reached its
declared source-disposition STOP before acceptance: a valid catalog-backed
route could not obtain optional metadata or package markers through the
materialized-only consumers. The candidate was removed completely and the
reviewed external-boundary design remains the successor after this prerequisite.
Commit `078518b88` freezes this prerequisite design after independent review
accepted its built-in-only listing branch, fail-closed metadata boundary,
materialized-route preservation and bounded implementation scope.
Commit `3b73a99cf` selected implementation at 220/420. The compiled candidate's
252 production and 148 proof additions exceeded only the production ceiling;
independent cap review then found the masked built-in REPO policy-order defect.
No other semantic, proof, allowlist, dependency or validation defect is known.
