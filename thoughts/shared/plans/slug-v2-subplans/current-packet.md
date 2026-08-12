# Current Slug V2 Packet

Packet: `WP-8-m7-filewrite-run-handoff-implementation`
Milestone: M7 implementation
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: activate one executable FileWrite `run` vertical through the accepted
REAPI executor and client-owned process launch, including a bounded daemon
launch-intent wire, without inventing runfiles or direct-local build actions.

## Scope

Implement only the admitted POSIX `run-basic` shape frozen in Stage 8. Add one
request-local `ResolvedRunSemanticView<'a>` owned by
`BuildCommandEvaluation`. It must borrow the sole requested configured
executable non-test rule, its built-in `DefaultInfo`, the existing sole
resolved FileWrite semantic view, and their exact shared executable artifact.
Add no DICE key, retained state, second executable model, or reconstructed
action.

Fail closed unless `DefaultInfo.executable`,
`files_to_run.executable`, default files, default runfiles, and data runfiles
all identify exactly the sole executable artifact; both manifest fields,
symlinks, empty files, diagnostics, extra action-bearing owners, and other
actions are absent. Require the FileWrite output relation and executable bit,
and retain every accepted platform/constraint/property/identity guard.

Route the build only through `FileWriteReapiPlan::from_resolved` and
`execute_file_write`; raw FileWrite execution stays rejecting. After
owner-derived materialization, validate root confinement, no symlink component,
regular-file type, and Unix execute mode. Do not scan `bazel-bin`, create a
mirror, or accept an arbitrary executor path.

The CLI alone launches the program with locally retained
`RunRequest.program_args`, inherited stdin/stdout/stderr, workspace cwd, and
the client environment minus the five fixed runfiles locator variables. Preserve
normal numeric exits; map signals to `128 + signal`; classify post-build launch
failure as Slug-native exit 1 without relabeling build terminal failures. The
program is command behavior and must not increment build action/cache evidence.

## Daemon boundary

Add `DaemonRequest::Run(BuildRequest)` with no program arguments or client
environment. Add a defaulted, absent-when-none
`DaemonResponse.run_launch_plan: Option<RunLaunchPlan>` whose payload contains
only absolute executable path, absolute working directory, and the fixed
environment-clear names. The daemon builds, materializes, validates, and never
launches a user-program child.

A plan is legal only on successful Run and is the sole launch authorization.
The client rejects missing, cross-command, nonzero-response, relative-path, or
unexpected-clear plans and repeats the final file/mode check before spawn.
Program args remain local. Serialize no complete environment, environment
value, secret, content, digest, or arbitrary executor metadata. One-shot
constructs the same plan locally and uses the same launcher.

## Compatibility boundary

- **Exact:** admitted one-target executable/provider/action relation, accepted
  FileWrite REAPI semantics, arguments after `--`, noninteractive program
  stdout/stderr bytes, and normal numeric exit.
- **Slug-native:** owner-derived path bytes, launch-plan wire, workspace cwd,
  inherited-minus-clear environment, process/diagnostic/evidence envelope, and
  signal mapping.
- **Unsupported/deferred:** additional runfiles/manifests/symlinks/empty files,
  `RunEnvironmentInfo`, exact `BUILD_*`, target binary args, `run_in_cwd`,
  `run_under`, `script_path`, tests/coverage, interactive parity, multiple
  targets/actions, other executable producers/action kinds, Windows, and exact
  Bazel output/configuration identity bytes.

## Allowlist and caps

Edit only:

- `app/slug_commands_v2/src/run.rs` and existing command tests;
- `app/slug_cli_v2/src/commands/run.rs` and existing CLI tests;
- `app/slug_core_v2/src/runtime/{dice.rs,mod.rs}` and existing focused tests;
- `app/slug_reapi_v2/src/{executor.rs,lib.rs}` and existing focused tests;
- `app/slug_server_v2/src/{lib.rs,reapi.rs,server.rs,tests.rs}`;
- all six existing files under `tests/v2_oracle/fixtures/run-basic/`; and
- canonical/current-packet/Stage 8 bookkeeping.

Cap formatted net Rust growth at 270 production, 340 tests, and 610 total.
Cap fixture growth at 150 lines and bookkeeping at 120. Add no new file,
dependency, DICE state/key, direct-local/raw build executor, runfiles
materializer/tree, JVM artifact, other action kind, broad run flag, Stage 9
change, workspace dependency, or CI. One material correction maximum; a second
is `REPLAN`.

## Evidence and validation

Extend only `run-basic`: retain one executable FileWrite topology, add one
registered execution platform, and add discriminators for ordered `--`
arguments, stdout/stderr, normal nonzero exit, selected properties, and script
content A/B/A restoration. Refresh and replay with pinned Bazel 9.2.

Require:

- core positive plus fail-closed provider/runfiles/action/path tests;
- exact REAPI bytes/digests/mode/properties, one remote action, zero
  direct-local actions, and no launch action/cache event;
- focused one-shot argument/stream/exit/launch-failure/environment tests;
- direct wire round trips proving args/environment values absent,
  cross-command plan absence, and malformed authorization rejection;
- rebuilt `slug_cli_v2` plus one-shot fixture execution; and
- stable-PID daemon A/B/A proving owner path, program output, action/cache
  evidence, invalidation, and exact restoration.

Run focused command/core/REAPI/server/CLI suites, direct compile dependents,
`cargo fmt --all -- --check`, archive/scope/cap/credential/diff checks, and
clean stale `slugd` before and after daemon evidence. Do not run Cargo tests
in parallel on a shared target directory.

Require independent final Sol review because this activates a public semantic
view, daemon request/response wire, and user-program process boundary. Review
must verify no second model/raw executor/direct-local build action, no daemon
child or secret serialization, exact guards and build failure classifications,
client-only args/terminal ownership, path integrity, caps/allowlist, and
discriminating one-shot/daemon evidence. At `ACCEPT`, record Stage 8 evidence,
update canonical/current scheduling, commit, and continue the canonical plan.
At a first bounded correction, fix and rereview; at a second, `REPLAN`.
