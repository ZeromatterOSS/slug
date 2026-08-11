# Current Slug V2 Packet

Packet: `WP-6-m32-cquery-rdeps-reverse-depth-admission`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implementation after accepted normalized reverse traversal.

## Observable slice

Extend exactly Bazel 9.2
`rdeps(deps(<one concrete root-repository root>), <one concrete
root-repository seed>[, <signed Java-int reverse depth>]) --noimplicit_deps`.
Keep the universe `deps()` unbounded. Omitted depth remains unbounded; zero
returns only matching in-universe seed keys; positive values add that many
reverse BFS layers; negative values return no rows. Preserve the existing
label, Starlark-label, label-kind, and unfactored graph outputs.

## Ownership and stops

Reuse M31's completed-universe ordering, loading-only seed validation, full
configured/null key resolution, request-local reverse BFS, authoritative
normalized forward edges, and selected-induced output graph. Bound only the
reverse loop; do not add reverse adjacency/state, a DICE key/cache, interner,
lock, or retained representation.

Do not admit bounded universe `deps`, general reverse/path expressions,
wrappers or filters around `rdeps`, default implicit/tool/external/factored
topology, parser/vendor changes, exact hashes, JVM/Java, or CI.

## Validation

Use pinned Bazel 9.2 `RdepsFunction`, `AllRdepsFunction`, and
`QueryEnvironment#shouldVisit` plus the accepted delegation topology. Prove
quoted signed Java-int boundaries and overflow rejection; reverse depths zero,
one, two, negative, maximum, and omitted; both base and transitioned seed keys;
alias-layer and selected-induced graph behavior; universe-Need-before-seed
ordering; and command/server plus rebuilt CLI one-shot/daemon symmetry. Run
formatting, archive, diff, and stale-daemon checks serially. Suggested caps: 60
production / 220 test / 280 total Rust lines. Bundle this bookkeeping with the
functional commit; no standalone documentation commit.
