# Current Slug V2 Packet

Packet: `WP-8-m7-filewrite-run-fixture-admission-design`
Milestone: M7 fixture-boundary replan
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the complete POSIX source shape for `run-basic` on the already
admitted Slug Starlark surface, without adding host-configuration semantics or
changing the accepted production Run architecture.

## Retry stop

The endpoint-injection retry passed its focused harness tests and reached Slug
analysis. Its one fixture correction added the already-required explicit marker
leaf and forwarded that marker through `ToolchainInfo`. The next replay passed
that boundary but failed independently because the old cross-platform fixture
reads `ctx.configuration.host_path_separator`, which Slug does not admit.
Because that is separate from toolchain topology, the retry ends `REPLAN`
without acceptance or commit.

## Design scope

Audit the entire four-file `run-basic` workspace source against existing Slug
loading/analysis capabilities. Freeze a POSIX-only fixture: add the existing
`required_host_os = "posix"` fixture guard, remove `_is_windows` and the
Windows content/output branches, declare only the `.sh` artifact, and retain
the exact Bash content, executable FileWrite, DefaultInfo, toolchain marker,
registered platform, mutations, and command rows already in the worktree.

This fixture guard and source reduction are Slug-oracle-native scaffolding.
They do not change the pinned Bazel 9.2 Linux observable output and make no
Windows or host-configuration compatibility claim. Windows, host-path context,
other action producers, and broader Run surfaces remain unsupported/deferred.

Edit only this manifest, canonical/Stage 8/routing bookkeeping. The successor
may additionally edit the six existing `run-basic` files and resume the retry's
production/harness allowlist and caps. Cap this design at 60 bookkeeping lines
and the successor source correction at 20 fixture lines. Require pinned Bazel
9.2 replay, Slug replay, source grep proving no `ctx.configuration`/Windows
branch, and independent design review before implementation resumes.

## Concrete stop

The predecessor implementation and all focused Rust validation reached the
fixture replay. Pinned Bazel 9.2 passed, but every Slug command stopped before
evaluation with `run requires --remote_executor`. Source inspection found the
cause in `tools/v2_oracle_lib/runner.py::_slug_reapi_argv`: the harness starts
NativeLink for any Slug fixture declaring `reapi.remote_executor = true`, but
returns unchanged argv unless the command verb is exactly `build`.

The predecessor allowlist explicitly excluded harness edits and had already
used its one material correction. Treating the one-line verb gate as implicit
would violate the reviewed scope and correction budget, so that packet ended
`REPLAN` without acceptance or commit.

## Accepted oracle amendment

Audit only the existing endpoint injector, command argv construction, REAPI
evidence extraction, and the `run-basic` command shape. Freeze whether
`_slug_reapi_argv` may admit exactly `build` and `run`. Build retains its
current append behavior. Run inserts the generated endpoint and default
properties immediately before the first standalone `--`, or appends them when
there is no `--`, so program arguments remain untouched. Successful Run must
also use the existing extracted REAPI evidence requirement already applied to
successful Build. Do not change the production
wire, CLI parsing, NativeLink lifecycle, evidence schema, fixture expectation,
or any other command verb.

Classify dynamic endpoint/default-property injection as Slug-oracle-native
scaffolding. It is not Bazel behavior and makes no compatibility claim. The
already accepted executable/provider/FileWrite/REAPI/run relations remain
exact or Slug-native as previously classified; all broader run and REAPI
surfaces remain unsupported/deferred.

## Allowlist and validation

The retry may edit this manifest, canonical/Stage 8 bookkeeping, and the
predecessor allowlist. It may additionally edit
`tools/v2_oracle_lib/{runner.py,compare.py}` plus existing focused oracle
tests, then resume the predecessor's explicit implementation allowlist and
validation contract. The harness delta is at most 20 production and 60 test
lines; combined bookkeeping remains capped at 130 lines. Add no new
file, dependency, schema, fixture, command verb beyond `run`, or fallback
endpoint source.

Require focused regressions proving Build remains unchanged; Run with `--`
inserts endpoint/properties before it; Run without `--` appends them; a
successful remote Run requires the already extracted evidence; and
Query/Aquery/Cquery receive neither flags nor an evidence requirement. Require
independent final review after the complete retry evidence. One new material
correction is allowed in this retry; a second is `REPLAN`.

## Retained implementation contract

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
- `app/slug_analysis_v2/src/starlark_rule.rs` and existing focused tests, only
  to forward Bazel's optional `ctx.actions.write(..., is_executable)` boolean;
- `app/slug_cli_v2/src/commands/run.rs` and existing CLI tests;
- `app/slug_core_v2/src/runtime/{dice.rs,mod.rs}` and existing focused tests;
- `app/slug_reapi_v2/src/{executor.rs,lib.rs}` and existing focused tests;
- `app/slug_server_v2/src/{lib.rs,reapi.rs,server.rs,tests.rs}`;
- all six existing files under `tests/v2_oracle/fixtures/run-basic/`; and
- canonical/current-packet/Stage 8 bookkeeping.

The packet's single correction replaces the pre-implementation cap after the
formatted public-wire/client-boundary measurement: cap net Rust growth at 660
production, 280 tests, and 940 total. Cap fixture growth at 150 lines and
bookkeeping at 130. Add no new file,
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
