# Current Slug V2 Packet

Packet: `WP-5-m1-loading-typed-propagation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted dormant Host package/`.bzl` keys and their public bzlmod
source projection; pinned Bazel 9.2 package/BzlLoad source; existing loading,
query, and retained command-root owners
Validation tier: design/source/call-graph and exact allowlist checks

Design file:

- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: freeze the smallest root-repository typed loading boundary that lets a
later query command root consume the accepted dormant `HostPackageLoadKey`.
Name the exact public/private API, DICE value and identity, deterministic Need
propagation, local event ownership, call graph, implementation allowlist, and
focused lifecycle/downstream tests. Account for remaining root-repository file
and package-directory consumers so no Host Need can become `LoadingError`.

Prefer the direct path to simple `query` package/target reads. Add no Rust,
Cargo/dependency change, fixture, oracle, command/runtime/server activation,
analysis migration, external repository mapping/materialization, legacy
retirement, JVM, Java bytecode, or Bazel delegation. Stop if the design cannot
preserve typed Need through loading without expanding into those surfaces.

Validate pinned-source citations, existing call sites and tests, exact
design-only scope, Markdown links, `git diff --check`, archive status, and
manifest/canonical agreement. Obtain one independent reserved-boundary design
review before scheduling Rust.
