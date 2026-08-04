# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-package-group-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct external source/filegroup/alias/config-setting and
suite-only test-suite queries; accepted root `PackageGroupContents`, includes
edges, visibility evaluation, and query graph representation; Bazel 9.2 oracle
infrastructure and complete external route/no-legacy guards
Validation tier: design-only source/oracle reconciliation plus independent
terminal design review

Design only a bounded standalone external native `package_group` query slice.
Do not edit Rust, Cargo metadata, fixtures, generated oracle JSON, protocol,
or another implementation file. Start from live `PackageTargetKind::PackageGroup`,
`PackageGroupContents`, the root graph projection, includes edges, visibility
consumers, and the current external prohibition on visibility dependency
labels.

Collect exact Bazel 9.2 direct-local-override evidence for package groups with
empty and nonempty `packages`, direct same-package `includes`, and an include
cycle. Probe literal output, `--output=label_kind`, `deps`, and any existing
node-local attribute or visibility consumer that would automatically become
active. Reconcile live behavior with accepted root fixtures and pinned Bazel
source, including ordering, duplicate rejection, negative/recursive package
specs, includes traversal, cycle handling, and apparent/canonical rendering.

The proposed contract must decide whether a useful standalone projection can
remain independent of rule visibility edges and external package-pattern
discovery. State exact accepted contents/include forms, graph kind/edges,
identity/rendering, observable query surface, lifecycle/publication, stop
gates, implementation/test/fixture allowlist, and serial validation. Keep
`visible()`, rule visibility labels, cross-package/repository includes, and
any package discovery stopped unless exact evidence proves they require no new
owner.

Stop and `REPLAN` if parity needs a new retained representation/key, external
package-pattern discovery, another repository route, visibility dependency
loading, filesystem observation, loads/globs, configuration, analysis,
build/execution, JVM, Java bytecode, or Bazel delegation. Append the proposed
contract only to the owner plan, obtain independent terminal design review,
and advance the canonical/current packet only after acceptance. Make no
implementation change in this packet.
