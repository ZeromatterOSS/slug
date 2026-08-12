# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-mapping-owner-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: audit the first missing post-selection extension mapping owner over the
accepted selected module routes.

## Active design contract

Perform one read-only ownership audit of Bazel 9.2 full repository mapping
construction after the accepted Bazel-dependency-only selected routes. The audit
must:

- pin extension identifier resolution against each selected module's accepted
  dependency mapping, isolation identity, and source locations;
- pin collision-safe extension unique-name construction, imported generated
  repository names, `override_repo`/`inject_repo` precedence, and contextual
  mapping augmentation order;
- inventory exactly which accepted root/nonroot evaluated extension usages,
  proxies, imports, isolation keys, and repo overrides are already retained;
- identify the smallest callerless DICE owner and compact structural identity,
  or `REPLAN` at the first missing extension semantic leaf;
- keep extension evaluation/generated repository creation, repository-rule
  execution, materialization, lockfile/final-module publication, loading,
  public route/mapping consumers, commands, analysis, and execution deferred;
  and
- freeze an explicit implementation allowlist, production/test/total caps,
  discriminating pinned-source/oracle and DICE lifecycle proof, compatibility
  classes, and terminal stops before any production work.

This packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

The root may inspect pinned Bazel 9.2 source, the checked-in
`repo-mapping-canonical-names` oracle, and live Rust owners read-only. Cap net
growth at 320 manifest lines, 300 owner-plan lines, 45 canonical lines, and 665
total. Obtain fresh independent reserved-architecture review.

No Rust, Cargo/BUILD, public API, legacy graph/catalog, registry or filesystem
I/O, route-source mutation, extension evaluation, repository rule,
materialization, lockfile/final-module publication, loading, mapping/route
consumer, command, analysis, execution, or JVM/Java work is authorized. Return
`REPLAN` on a missing retained usage/isolation/override leaf, need for a second
graph/route owner, extension evaluation or I/O, a future successor beyond three
Rust files, or inability to freeze explicit scope/caps/stops. Return `REVISE`
on one bounded design correction; a second material correction is `REPLAN`.
No production representation may begin before independent `ACCEPT` and
explicit implementation activation.

## Accepted predecessor evidence

This section is historical evidence only and grants no file, action, cap, or
scheduling authority.

Commit `6f72baaf` accepts the callerless private selected-module route owner at
328 production, 439 tests, and 767 total formatted net lines, within the
420/700/1,120 caps. It computes the accepted selected graph before the accepted
selected registry RepoSpec aggregate and retains roots-first BFS entries with
the shallow graph entry, exact canonical identity, compact context-bearing
Bazel-dependency mapping, and optional whole selected registry RepoSpec.

Exact behavior covers root, well-known, unique-version, normalized MVO
canonical names; canonical collisions; root-empty, self, and transformed
ordinary dependency mappings; registry/nonregistry/built-in source
classification; and whole predecessor identity. Need remains invalid;
completed graph errors precede selected-source work. Slug-native error wording
and deterministic completed-error selection remain explicit.

One borrowed nonregistry RepoSpec projection reuses the retained closure source
identity. Pure and real-DICE tests cover both MVO contexts, every mapping and
registry mismatch terminal, root/built-in/nonregistry zero registry work,
registry source A/B/A, warm reuse, Need, and graph-before-source precedence.
The full owner and loading suites, formatting/diff/scope/cap checks, compact
representation and AI-cleanup audits, and independent implementation review
pass. The public root route and every consumer remain unchanged.
