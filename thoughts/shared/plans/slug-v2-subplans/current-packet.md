# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-external-build-source-target-activation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only reserved design for the smallest direct-local external build
activation
Evidence: accepted external package source/loading and query owners; accepted
query-only unsupported-cycle boundary in `ea2019f8`; and the explicit Stage 5
reservation that future external-build activation requires a separate design.

Do not implement or edit Rust, tests, fixtures, or oracle assets. Audit the live
build command/root and accepted external loading path, obtain the reserved Sol
pre-review, and record `ACCEPT`, `REPLAN`, or a narrower successor design in the
canonical and owner plans.

The candidate observable slice is one explicit command target,
`build @dep//:file`, where `dep` is a direct `local_path_override` repository
and `file` is declared by `exports_files`. Decide from live code and pinned
Bazel 9.2 evidence whether this can reuse
`RepositoryPackageLoadKey -> RepositoryPackageSourceKey`, the existing build
root, accepted-command retry/event ownership, and current source-target output
semantics without a new source graph or configured analysis.

The design must enumerate typed propagation of ordinary external package-load
failures versus the accepted unsupported-cycle status into `BuildCommandError`;
mixed root/external Need and failure precedence; accepted-demand, event, and
retained-daemon lifecycle; exact outputs for a source target; and one-shot /
daemon recovery across MODULE, BUILD, and source create/edit/delete states.
Use an ad hoc pinned Bazel 9.2 probe only if existing accepted evidence does not
discriminate the observable result. Add no fixture or generated oracle record.

Freeze the smallest likely public boundary before implementation: exact allowed
production/test files, DICE direction and identities, equality/invalidation
contract, public error/rendering contract, focused evidence, formatted net
caps, and platform constraints. Verify whether a single route can be carried
through existing root/configured-target identities; do not assume that query's
request-local identity is sufficient for build analysis.

Stops: no implementation; no new DICE key; no package-all, recursive, or mixed
pattern breadth; no external dependency traversal, `filegroup`, `alias`,
Starlark-rule configured analysis, actions, execution, `run`, `test`, `cquery`,
or `aquery`; no registry/contextual mapping/`@bazel_tools`; no root-loader
rewrite; no private evaluator export or standalone `ExternalBzlModuleEvalKey`
caller; and no reopening the accepted native-Windows or JVM boundaries.
