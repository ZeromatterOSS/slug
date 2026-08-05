# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-external-build-source-target-activation-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implement one explicit direct-local exported-source build vertical
Evidence: accepted design and retained Bazel 9.2 present/edit/delete/recreate/
directory evidence; accepted external route/load/source and command owners.

Allow a nonroot build request only when the complete request contains exactly
one `TargetPattern::Single`. Keep mixed root/external, multiple-target external,
package-all, and recursive requests rejected before `BuildCommandRootKey`.
After the existing root anchor, compute
`RootRepositoryRouteKey -> RepositoryPackageLoadKey`; select only
`PackageTargetKind::ExportedFile`. Every other external kind returns a private
typed unsupported-kind build error and performs no configured analysis.

Observe the exact selected source before success. External sources compute
`HostRepositorySourceFileKey(route, package/target)`; Present succeeds, Absent
is the exact ordinary missing-input failure, and a Complete `WrongKind` whose
actual kind is Directory also succeeds per the accepted Bazel evidence. Other
source errors remain ordinary. Root exported sources use the existing exact
`PathObservationKey` FileBytes demand through the native command driver. Add no
key, direct filesystem access, or second graph.

Retain a private per-target completion class that distinguishes analyzed,
observed exported-source, and existing loaded-only paths. Renderers consume the
class, never `analyzed_target_count == 0`. Observed root and external exported
sources exit 0 with empty stdout, no REAPI invocation, and exact success JSON.
One-shot stderr is:

`{"success":true,"command":"build","target_count":1,"loaded_package_count":1,"analyzed_target_count":0,"declared_action_count":0,"runtime_mode":"one-shot","completed_boundary":"dice_exported_source_file"}`

Daemon uses the same field order with
`"runtime_mode":"daemon","invalidated_files":N` before
`completed_boundary`. All terminals end with one newline. Root filegroup,
package-all, and rule paths remain unchanged.

Preserve ordinary route/load/source errors structurally in private
`BuildCommandError` variants, exit 2, and `build_runtime_error`. Only
`RepositoryPackageLoadError::is_unsupported_feature()` becomes exit 7,
`unsupported_feature`, and the already-accepted exact cycle message without a
context suffix. Daemon retains `invalidated_files`. Events precede the terminal;
retry attempts publish nothing; warm reuse replays nothing. Byte-only edits may
recompute lower source state but the build terminal remains equal and
event-free; deletion fails and recreation succeeds in one retained daemon.

The implementation may edit exactly:

- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_cli_v2/src/commands/build.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_server_v2/src/lib.rs`; and
- `app/slug_server_v2/src/tests.rs`.

Formatted net caps are 280 production, 850 test, and 1130 total lines. Required
coverage includes request shape, route/load/source Need and error precedence,
ExportedFile versus every representative unchanged kind, exact present/edit/
delete/recreate/directory lifecycle and equality, root exported-source control,
one-shot/daemon exact JSON/status/stdout/invalidated-files, cold/warm events,
unsupported-cycle typed propagation, and structural no-new-key/no-REAPI stops.

Stops: no loading/query semantic edit, new key, dependency traversal,
filegroup/alias/rule external activation, configured analysis, action,
execution, REAPI, run/test/cquery/aquery, registry/contextual mapping/
`@bazel_tools`, root-loader rewrite, evaluator export, direct filesystem read,
fixture, oracle, harness, dependency, or cap growth.
