# Current Slug V2 Packet

Packet: `WP-5-m1-bazel-tools-query-repository-closure-owner-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted external test-base closure `REPLAN`, finite source-pinned
direct test-base edges, prior Bazel 9.2 44-target closure probe, and existing
Bzlmod discovery/materialization owners recorded in the owner plan.

Design only the smallest exact DICE-owned repository-closure owner needed by
unconfigured query for the pinned Bazel 9.2 installed `@bazel_tools` tree and
its required contextual resolved and extension-generated repositories. Audit
and reuse the existing Bzlmod graph, repository mapping, registry/lockfile,
source preparation, materialization, and loading owners. Freeze exact route
identity, semantic equality/invalidation, installed `BUILD.tools` layout,
verbatim source provenance, generated-repository identity, cross-package and
repository-qualified BUILD/Bzl loading, lifecycle behavior, consumer breadth,
bounded allowlist/cap, evidence, and stop gates.

Do not edit Rust, fixtures, tools, plans beyond the reviewed design result, or
generated evidence, and do not run Bazel unless a later reviewed evidence
packet explicitly authorizes it. Do not activate the external test rule, test
metadata, suite membership, analysis, actions, execution, configured toolchain
resolution, JVM, Java bytecode, or Bazel delegation. Port `@bazel_tools`
content verbatim only; never synthesize it.

Obtain independent reserved-boundary review before scheduling implementation.
Stop with **REPLAN** if exact ownership requires a parallel source owner,
direct filesystem or fresh-graph observation, invented installed-tools
metadata, unbounded repository discovery or native-rule semantics, configured
toolchain resolution, analysis/actions/execution, JVM, Java bytecode, or Bazel
delegation.
