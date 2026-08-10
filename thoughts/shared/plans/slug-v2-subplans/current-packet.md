# Current Slug V2 Packet

Packet: `WP-6-m2b-command-root-setting-preparation`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: second functional packet from the accepted configured-node ownership
design after single-owner consolidation.

## Observable slice

Resolve default and explicit root string settings in the existing Build/Cquery
command-root DICE preparation before constructing `ConfiguredNodeAnalysisKey`.
Remove the temporary unresolved request mode and admit only structural
Slug-native configurations at the production key boundary.

## Ownership and stops

Keep `ConfiguredNodeAnalysisKey` as the sole configured-analysis DICE owner.
Move its root-setting/default package preflight into shared analysis preparation
called from both existing command roots, unioning Needs before semantic errors.
The key must contain only a resolved target and a `SlugConfiguration`-backed
configuration; remove the request input enum and reject legacy/opaque
configuration before key construction. Do not add another DICE graph, node
kinds, cquery traversal, graph output, exact Bazel hash bytes, filesystem
bypass, JVM/Java, CI, or compatibility behavior. BUILD, `.bzl`, and MODULE
evaluation continues through vendored `starlark-rust`.

## Validation

Allow at most eight production and eight test files, 700 formatted net
production lines, 900 formatted net test lines, and 1,600 total. Prove
default/explicit/missing/edit/restoration and convergent transition behavior,
Need-before-error, structural-key rejection of opaque configurations, one
configured-node activation per resolved identity, and unchanged one-shot/
daemon output. Run focused configuration, analysis, core, server, and CLI tests
serially; rebuild `slug_cli_v2` before CLI tests and clean `slugd` around daemon
tests. Stop if either command root needs a second key/cache or if upstream
preparation cannot return the final structural configuration.
