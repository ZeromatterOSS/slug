# Current Slug V2 Packet

Packet: `WP-8-m7-filewrite-run-handoff-design`
Milestone: M7 design
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the first executable FileWrite `run` handoff over the accepted
analysis and REAPI objects without inventing runfiles or local build execution.

## Scope

Design only the admitted POSIX `run-basic` shape: exactly one requested
configured rule, executable capability true, a built-in `DefaultInfo` whose
`executable` and `files_to_run.executable` name the same normalized artifact,
default/data runfiles containing no artifact except that executable, and exactly
one executable FileWrite action that produces it.

Freeze one request-local `ResolvedRunSemanticView` owned by
`BuildCommandEvaluation`. It must borrow the sole requested analyzed node,
its retained `DefaultInfo`, the matching resolved FileWrite semantic view, and
the executable artifact relation. It creates no DICE key or second command
model. Build execution continues through `execute_file_write` and NativeLink;
after verified owner-derived materialization, the CLI launches only that
declared executable with `RunRequest.program_args`.

Decide the one-shot and daemon ownership boundary, current-directory and
environment policy, stdout/stderr streaming, signal/exit propagation,
post-build launch failure classification, and evidence needed to distinguish
the user program process from a forbidden direct-local build action. Do not
implement in this packet.

## Compatibility boundary

- **Exact:** admitted target/executable validation; `--` program-argument
  preservation; successful program stdout/stderr bytes and terminal exit code;
  and the already accepted FileWrite REAPI bytes, digests, properties, and
  zero-direct-local build boundary.
- **Slug-native:** owner-derived executable path, process launch mechanism,
  environment/current-directory policy until a Bazel-exact bounded policy is
  demonstrated, diagnostics, signal envelope, and command evidence formatting.
- **Unsupported/deferred:** additional runfiles, repository mapping manifests,
  data dependencies, source/generated executable producers other than the sole
  FileWrite, multiple actions/targets, RunShell/Spawn executables, Windows,
  tests, coverage, terminal integration, exact Bazel output/configuration bytes,
  and broad `bazel run` flag/environment parity.

Any missing or conflicting executable relation, non-executable FileWrite,
additional runfile/artifact/action, mixed closure, ambiguous platform, absent
remote executor, or path escape fails closed before launching a program.

## Evidence

Audit pinned Bazel 9.2 `RunCommand`/command-line construction and the
retained `run-basic` oracle before freezing policy. Refresh that fixture with
pinned Bazel 9.2 only if its checked-in expectation is stale.

The design must require discriminating implementation proof for:

- exact requested-target/DefaultInfo/executable/FileWrite relation and every
  fail-closed negative;
- REAPI build evidence with one action and zero direct-local actions;
- program arguments after `--`, stdout/stderr, success and nonzero exits;
- executable mode and owner-derived path integrity before launch;
- one-shot plus stable-PID daemon build/launch A/B/A restoration; and
- proof that program launch is not counted as a build action or cache event.

Reuse `run-basic` unless source audit demonstrates a missing discriminator.
No public protocol wire, retained state, JVM artifact, or implementation edit is
allowed in this design packet.

## Allowlist and caps

Edit only:

- this manifest;
- Stage 8 for an accepted design decision; and
- the canonical V2 plan only if review changes scheduling.

Read-only inspection may cover the existing run parser/CLI placeholder,
configured analysis/providers, FileWrite executor, `run-basic` fixture and
expected oracle, pinned Bazel 9.2 source/tests, and REAPI evidence code.

Do not edit Rust, fixtures, expectations, generated protocol, Stage 9, workspace
dependencies, or any other plan. Cap new Stage 8 design text at 180 lines. No
new files or dependencies.

## Validation and review

Validate source citations, existing API ownership, fixture provenance,
exact/Slug-native/deferred classification, allowlist/cap, archive boundary, and
`git diff --check`. Do not run Cargo or Slug/NativeLink for a docs-only design;
a pinned Bazel oracle refresh is allowed only if the existing evidence is stale.

Require one independent Sol design review because this packet reserves a new
public semantic view and local user-program process boundary. The reviewer must
verify no second executable/action model, no raw FileWrite executor, no hidden
direct-local build action, exact failure guards, truthful process-policy claims,
bounded public API, and discriminating one-shot/daemon evidence.

At `ACCEPT`, append the reviewed design to Stage 8 and schedule only its
bounded implementation packet. At `REPLAN`, record the missing prerequisite
and schedule that prerequisite's design packet.
