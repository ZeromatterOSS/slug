# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-package-group-query`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted corrected standalone external package-group design; live
Bazel 9.2 empty/nonempty/parent/cycle literal, kind, dependency, labels, and
visibility probes; pinned package-group/label-visitation source; accepted root
contents/includes graph representation; accepted external query spine and
fixture-hygiene checkpoint
Validation tier: one-file private query projection plus focused public
query/core and four exact Bazel/Slug oracle rows

Implement only the accepted standalone external native `package_group`
projection in `app/slug_query_v2/src/graph.rs`. Reuse
`PackageTargetKind::PackageGroup`, its `Arc<PackageGroupContents>`, the accepted
external graph/key, and existing query/render consumers. Treat contents as
opaque retained data: do not evaluate, reparse, enumerate, discover, render,
or claim exact external repository identity for package specifications.

Before generic source synthesis, require every include to be a root-context
same-package label resolving in the same loaded target batch to another native
`PackageGroup`. Remap it to canonical external identity with selected apparent
rendering. Preserve source order and duplicates in
`PackageGroupInclude` edges; accept direct chains and cycles. Project
`QueryNodeKind::PackageGroup`, the retained contents, no rule capability/test
metadata/query attribute, and public visibility with no visibility edge.

Production allowlist: `app/slug_query_v2/src/graph.rs`. Tests may change only
that file and `app/slug_core_v2/src/runtime/dice.rs`. Oracle changes are
limited to the existing `module-local-override` fixture TOML,
`workspace/dep/BUILD.bazel`, and expected JSON; add no asset or fixture. Do not
alter Cargo metadata, public APIs, DICE keys, repository routes,
loading/source owners, CLI/server adapters, protocol, formatters, contents
representation, visibility evaluation, analysis, actions, or execution.

Extend the fixture with empty, nonempty, leaf/parent, and two cyclic package
groups. Add exactly four commands: parent literal, parent
`--output=label_kind`, `deps(parent)`, and `deps(cycle_a)`. Exact outputs are
the apparent parent label, `package group` kind, leaf then parent, and the two
cycle labels. Protect all 13 existing normalized rows. The bounded sixth-packet
hygiene review found every existing row discriminating; add no other row or
asset.

Focused structural/public evidence must prove opaque retained empty and
positive/negative/exact/recursive/public/private contents shape without
evaluating it; canonical/apparent labels; public kind; no attributes,
capability, metadata, or visibility edges; source-ordered includes; finite
chains/cycles; lifecycle reuse and source noninvalidation. Prove missing,
non-package-group, alias, cross-package, and named-repository includes stop
before generic source synthesis/discovery. Preserve all accepted external
source/filegroup/alias/config-setting/test-suite behavior and stop gates.

Do not activate `labels(packages|includes, ...)`, external rule visibility,
`visible()` content evaluation, package discovery, alias include resolution,
or cross-route traversal. Stop and `REPLAN` if parity needs exact external
contents identity/evaluation, another representation/key/owner/route,
filesystem observation, loads/globs/patterns, configuration, analysis,
build/execution, JVM, Java bytecode, or Bazel delegation.

Finish with focused serial query/core/CLI/server external tests, unchanged
root package-group/visibility tests, four-row Bazel generation and
distinct-root replay, rebuilt `slug_cli_v2` direct Slug replay, formatting,
`git diff --check`, archive/scope/no-Cargo and forbidden-owner scans, and one
independent terminal implementation review. Do not run a broad Cargo suite for
this packet.
