# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-package-query-activation-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted dormant external Bzl owner `0463cb17`, accepted
request-local external query package identity `845e89b7`, accepted activation
design in the owner tail, accepted direct missing/cycle/load evidence, and the
accepted 20-row `module-local-override` macro-query oracle.

Implement only the previously accepted external Bzl package/query activation.
`RepositoryPackageLoadKey` remains the sole package owner: normalize every
direct same-package external load before computing any child, compute
`ExternalBzlModuleEvalKey` children sequentially, and transfer the raw-load/
frozen-module pairs through the existing package-attempt owner. Accept a
nonempty-load package only when every produced target is `ExportedFile` or
native `Filegroup`; all other kinds remain whole-package typed stops.

The exact edit allowlist is `app/slug_loading_v2/src/bzl_module.rs`; a
test-only retained-lifetime accessor in `app/slug_loading_v2/src/package.rs`;
`app/slug_loading_v2/src/host_package_load_tests.rs`;
`app/slug_query_v2/src/loading_environment.rs`;
`app/slug_query_v2/tests/loading_query.rs`; and the existing direct-external
lifecycle test in `app/slug_cli_v2/tests/cli.rs`. The four accepted
`module-local-override` fixture paths are validation-only and byte-frozen.
Total growth must remain at or below the accepted `+2100`-line cap. Do not add
a source owner, DICE key, lock, public identity/API, graph/provenance/generic-
traversal/output module change, or cycle-detector/Bzlmod/core-event change.

Query production may change only `loading_environment.rs`: expose the accepted
external BUILD and reachable Bzl identities using the retained external
`QueryPackageIdentity`; `loadfiles()` returns only Bzls, and `buildfiles()`
reuses the already-emitted external BUILD companion without root discovery.
Preserve apparent `@dep` rendering, printed-label dedupe, and existing fake
source-file leaf behavior for all enabled consumers and formats.

Required focused coverage is manifest and frozen-lifetime transfer;
validation of all direct loads before any child source request; typed missing/
cycle preparation; Need/equality/validity; edit/delete/recreate and fresh-
detector same-DICE recovery; evaluation-only event publication with no Reused
replay/recapture; every enabled fake-candidate consumer; apparent output; and
a real root BUILD proving no root companion fallback. Extend only the named
existing tests. Rebuild `slug_cli_v2` before the accepted Slug fixture replay
and clean stale `slugd` before and after. Run focused loading/query/CLI tests
and direct checks, both affected loading/query GNU-Windows no-run gates,
`cargo fmt --all -- --check`, `scripts/v2_archive_status.sh`, and
`git diff --check`. Do not rerun a Bazel oracle or a workspace-wide Cargo
suite; CLI GNU-Windows no-run remains excluded by the existing Unix-socket
transport blocker.

Stop with **REPLAN** rather than widening if implementation needs cross-
package/repository loads, mapping/discovery, non-local overrides, globs,
external patterns, visibility content, target kinds beyond `ExportedFile`/
`Filegroup`, tests/executables/suites/generated/Starlark rules, configuration,
analysis/actions/execution, repository rules/extensions, another production
file, source owner, DICE key, lock, public identity/API, graph/provenance/
generic traversal/output/core-event/cycle-detector/Bzlmod changes, JVM, Java
bytecode, Bazel delegation, a fixture edit, or more than `+2100` lines.
