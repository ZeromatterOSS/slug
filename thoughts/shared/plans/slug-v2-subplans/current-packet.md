# Current Slug V2 Packet

Packet: `WP-6-m2h-configured-query-root-traversal-activation-audit`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: read-only successor audit after root topology activation.

## Observable slice

Reconcile the accepted Bazel 9.2 delegation/toolchain `deps` evidence with the
live cquery expression, evaluator, command-root, and configured-result owners.
Identify the smallest root-only forward traversal that can consume the sole
ordered configured graph without synthesizing the stopped external
host-platform tail.

## Ownership and stops

Read the accepted topology fixtures and the live query/cquery/analysis/core
owners. Produce an exact traversal, depth, filtering, ordering, terminal, and
ownership table; identify whether an exact `--noimplicit_deps` root slice is
implementable before default traversal. Do not edit Rust, tests, fixtures, or
oracle payloads in this audit.

Literal `@bazel_tools`, `@platforms`, host-platform edges, and cross-package
reverse implementation topology remain stopped. Exclude `rdeps`, public
label/graph formatter breadth, new parser work, exact Bazel hash bytes,
JVM/Java, CI, and compatibility behavior. Vendored Buck2 `starlark-rust`
remains the sole Starlark parser/evaluator substrate.

## Validation

Every proposed traversal rule must cite accepted Bazel 9.2 evidence and one
live owner. Preserve exact depth, implicit/tool filtering, ordered structural
identity, Need/error precedence, and result ownership. Stop with `REPLAN` if no
bounded exact slice exists without synthetic external nodes or a second
graph/key/result/cache.
