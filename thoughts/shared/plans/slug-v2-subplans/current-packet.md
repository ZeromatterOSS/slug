# Current Slug V2 Packet

Packet: `WP-6-m33-cquery-rdeps-bounded-universe-admission`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implementation after accepted reverse-depth admission.

## Observable slice

Extend exactly Bazel 9.2
`rdeps(deps(<one concrete root-repository root>[, <nonnegative Java-int
universe depth>]), <one concrete root-repository seed>[, <signed Java-int
reverse depth>]) --noimplicit_deps`. The inner bound selects the forward
universe; the outer bound limits reverse BFS within that selected universe.
Preserve the existing label, Starlark-label, label-kind, and unfactored graph
outputs.

## Ownership and stops

Reuse the existing `CqueryDepsSpec` depth, bounded forward-closure owner, M31
universe-first loading-only seed validation, full configured/null seed matching,
M32 reverse bound, normalized forward edges, and selected-induced output graph.
Do not add reverse adjacency/state, a DICE key/cache, interner, lock, or retained
representation.

Do not admit general universe expressions, multi-root/set/variable forms,
wrappers or filters around `rdeps`, other reverse/path functions, default
implicit/tool/external/factored topology, parser/vendor changes, exact hashes,
JVM/Java, or CI.

## Validation

Use pinned Bazel 9.2 `RdepsFunction#evalWithBoundedDepth`, `DepsFunction`, and
`AllRdepsFunction` plus accepted forward-depth and delegation evidence. Prove
independent Java-int parsing and diagnostic precedence; inner depths zero, one,
two, maximum, and omitted composed with outer negative, zero, one, maximum, and
omitted; no reverse crossing outside the bounded universe; full configured-key
duplicate seeds; selected-induced edges; universe-Need-before-seed ordering;
and command/server plus rebuilt CLI one-shot/daemon symmetry. Run formatting,
archive, diff, and stale-daemon checks serially. Suggested caps: 45 production /
220 test / 265 total Rust lines. Bundle bookkeeping with the functional commit;
no standalone documentation commit.
