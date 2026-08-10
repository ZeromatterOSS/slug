# Current Slug V2 Packet

Packet: `WP-6-m2c-configured-node-result-substrate`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: third functional packet from the accepted configured-node ownership
design after single-owner consolidation and resolved-setting preparation.

## Observable slice

Introduce the reviewed `ConfiguredNodeKey` identity with structural
configuration or `Null`, classified immutable configured edges, and the single
retained configured-node result shape. Prove structural equality, null identity,
transition-output convergence, edge order, and exact implicit/tool bits without
activating new cquery traversal.

## Ownership and stops

Keep `ConfiguredNodeAnalysisKey` as the sole configured-analysis DICE owner.
Generalize the existing `AnalysisResult` ownership into one configured-node
result rather than adding a peer cache or graph. A node key is canonical label
plus structural `ConfigurationKey` or `Null`; transition origin belongs only on
the incoming edge. Each edge owns semantic kind, target, order, and exact
`implicit`/`tool` bits from the accepted design. Reuse `Arc<[T]>`,
`CompactString`, `SmallMap`/`SmallSet`, `Dupe`, and `Allocative`. Do not yet add
native/delegating/platform nodes, cquery traversal/output, a new DICE key/cache,
exact Bazel hash bytes, filesystem bypass, JVM/Java, CI, or compatibility
behavior. Vendored `starlark-rust` remains unchanged.

## Validation

Allow at most six production and six test files, 600 formatted net production
lines, 700 formatted net test lines, and 1,300 total. Prove configured/null
identity discrimination, target/exec/host-like structural discrimination,
equal transition-output convergence, ordered edge equality, and every currently
admitted edge bit. Run focused analysis and downstream compile/tests serially.
Stop if the substrate requires a second retained result, DICE key/cache, query
graph, guessed platform/tool edge, or changes to current observable output.
