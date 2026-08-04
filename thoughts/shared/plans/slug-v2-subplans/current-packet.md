# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-public-unsupported-cycle-boundary-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implement the accepted query-only public boundary
Evidence: explicit user approval of a Slug-owned public unsupported-cycle
limitation; accepted private support-gated preparation in `f2b626f2`; accepted
trusted nonregistry evaluator adapter in `c683c239`; accepted private
preparation-consuming DICE/event owner in `3cf0e441`; and a live activation
audit at clean `c69a9f93`.

The public-boundary design passed reserved Sol correction review and independent
Terra latest-text review. Implement only this query-only boundary for the
selected external package-source path under the exact allowlist, contract,
evidence, and caps below.

The implementation must keep `DirectLocalModuleEvaluationKey`, its
`Supported`/`Unsupported` value, the evaluated route-plus-module value, the
cycle capability and all provenance private. Export none of them from
`slug_bzlmod_v2::lib.rs`. Add only a crate-private support result/helper beside
the evaluation owner in `source_preparation.rs`. At the start of
`RepositoryPackageSourceKey::compute`, consume that helper before the key's own
post-gate `ExternalRepositoryPackageLookupKey` and selected BUILD source
activation. Supported evaluation continues through the existing source path.
An ordinary module-evaluation failure becomes a distinct typed source/load/query
error and never uses the unsupported discriminator.
Only the unsupported capability becomes one typed
`RepositoryPackageSourceError`, then `RepositoryPackageLoadError`, then
`QueryErrorKind::UnsupportedFeature` without converting the discriminator to a
string between those owners.

The exact Slug-owned terminal message is:

`Slug does not support MODULE.bazel include cycles in direct local_path_override repository '@{apparent_repo}' for module '{module_name}': include {repeated_raw_label:?} at {repeated_file}:{repeated_line}:{repeated_column} repeats ancestor include {ancestor_raw_label:?} at {ancestor_file}:{ancestor_line}:{ancestor_column}`

This is explicitly a Slug capability status, not a Bazel diagnostic. Query
returns exit code 7, empty stdout, and JSON `error` equal to
`unsupported_feature`. It does not append the ordinary `Evaluation of query`
context suffix. One-shot and daemon rendering are otherwise identical; daemon
JSON additionally retains `invalidated_files` through its existing renderer.

Selected evaluation events precede the terminal JSON. The unsupported status
adds no diagnostic event. Retry-only attempts publish nothing. The private
module-evaluation owner continues to store or replace its marker-conditional
empty batch on every unsupported Complete. A warm repeated unsupported query
returns the same terminal without replaying events. Removing the cycle executes
the newly supported MODULE exactly once and succeeds; reintroducing it returns
the status without stale print replay.

The boundary is query-only. Current one-shot and daemon external query paths
both reach `RepositoryPackageLoadKey -> RepositoryPackageSourceKey`, including
selected external BUILD files and their same-package external `.bzl` loads.
Build remains unchanged: external target patterns are rejected before the build
DICE root, and root-package loading rejects external `.bzl` labels. Root-only
query, `build`, `cquery`, `aquery`, `run`, and `test` must not activate the new
helper. Future external-build activation requires a separate design.

The Rust implementation may edit exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/host_package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`;
- `app/slug_query_v2/tests/loading_query.rs`;
- `app/slug_cli_v2/src/commands/query.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_server_v2/src/lib.rs`; and
- `app/slug_server_v2/src/tests.rs`.

Its formatted net cap is **260 production lines, 900 test lines, and 1160 total
lines**. No `lib.rs`, build/core-runtime production, root-loading, command
parser, server transport, fixture, or oracle file is authorized.

There is no DICE dependency cycle in this gate. The private evaluation path
uses `ExternalRepositoryPackageLookupKey` for include-package preflight and
`HostRepositorySourceFileKey` for MODULE/include bytes; it never consumes
`RepositoryPackageSourceKey`. The new direction is therefore selected package
source to private evaluation to lookup/source, never back to selected package
source. Every current product path that activates a selected external BUILD or
external `.bzl` already enters through `RepositoryPackageLoadKey ->
RepositoryPackageSourceKey`.

`ExternalBzlModuleEvalKey` can intrinsically read a `.bzl` without depending on
`RepositoryPackageSourceKey`, but it is crate-private. Every current non-test
construction is either after the gated package source or recursive from that
gated external `.bzl` root. This design therefore closes current selected
product paths, not a raw-key invariant; any future standalone caller requires a
separate gate and review.

Both current query consumers must preserve the typed discriminator. The
external package-graph path in `graph.rs` and the external `buildfiles()` /
`loadfiles()` provenance path in `loading_environment.rs` each map only the
typed load error to `QueryErrorKind::UnsupportedFeature`; neither may stringify
it first. All other load failures retain their existing classification.

Required evidence covers typed support/error propagation and source
chains; distinct ordinary evaluation failure versus Unsupported; no
`RepositoryPackageSourceKey` post-gate selected BUILD selection/read and no
BUILD or `.bzl` evaluation after Unsupported; one-shot/daemon status parity;
discriminating external `buildfiles()` and `loadfiles()` coverage through the
provenance path;
exact message and both one-based start-location renderings;
exit/stdout/context rules; event ordering and absence of a diagnostic event;
retry suppression; cold/warm, cycle removal, and cycle reintroduction;
selected-demand retention and daemon invalidation; and structural
query-only/no-export/no-build stops. Root/routed-REPO and
include-preflight marker child activations/events remain allowed and
child-owned. Reuse current fixtures and pinned private evidence; add no oracle.

Stops: no public export, no raw
capability/evaluated-module/provenance exposure, no build activation, no
registry/MVS/contextual mapping, no evaluator or event-owner semantic change,
no fixture/oracle, no new Cargo dependency, and no Bazel cycle invocation.
