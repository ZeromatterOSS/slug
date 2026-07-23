# Stage 2: Rust Skeleton and Runtime Substrate

## Goal

Create the minimal Slug V2 Rust binary/server skeleton while reusing Buck2
runtime crates for DICE, starlark-rust, events, and REAPI clients without
exposing Buck user semantics.

## Scope

- CLI entrypoint with `version`, `help`, `build`, `query`, `test`, and `run`
  command placeholders only until the first real build slice replaces `build`.
- Server or daemon boundary only where it helps DICE and warm-state validation.
- Buck2 crate reuse policy and wrapper crates.
- Basic diagnostics, event logging, and test fixture wiring.

## Reuse Policy

Reuse infrastructure:

- `dice`
- `starlark-rust`
- remote execution client/materializer pieces
- event and superconsole infrastructure where it does not leak Buck concepts

The retained crates must be normal dependencies of the V2 runtime before a
later stage can call its substrate complete. A trait named after DICE or
Starlark is not reuse; the first real build must execute an actual DICE compute
and parse/evaluate through starlark-rust. Re-import REAPI protocol/client code
only as a small V2 wrapper after proving it does not retain V1 `slug_*`
dependencies.

Do not expose or depend on:

- Buck cells as the semantic repository model;
- BUCK/TARGETS file discovery;
- Buck target-pattern semantics;
- Buck executor configuration as a user-facing compatibility layer.

## Implementation Slices

### 2.1 Root Crate Layout

Create a small V2 crate set before porting feature code:

| Crate | Purpose |
|-------|---------|
| `slug_cli_v2` | argument parsing, command dispatch, version/help text |
| `slug_core_v2` | shared errors, labels to be replaced by Stage 3, runtime handles |
| `slug_server_v2` | optional daemon boundary and DICE transaction lifecycle |
| `slug_oracle_v2` | test-only adapter for Stage 1 harness invocation |

The exact crate names can change, but the separation must stay: CLI should not
know Buck/Starlark internals, and server/runtime should not parse user-facing
Bazel semantics directly.

Initial concrete files:

- `app/slug_cli_v2/Cargo.toml`
- `app/slug_cli_v2/src/main.rs`
- `app/slug_cli_v2/src/lib.rs`
- `app/slug_cli_v2/src/commands/{mod.rs,version.rs,build.rs,query.rs,test.rs,run.rs}`
- `app/slug_cli_v2/tests/cli.rs`
- `app/slug_core_v2/Cargo.toml`
- `app/slug_core_v2/src/{error.rs,build_info.rs,lib.rs}`
- `app/slug_core_v2/src/runtime/{mod.rs,dice.rs,events.rs,starlark.rs,reapi.rs}`
- `tools/v2_oracle/README.md` documenting `SLUG_V2_BIN`

Delay `app/slug_server_v2` until a Stage 4 or Stage 5 same-daemon fixture
requires it. Keep `RuntimeMode::{OneShot,Daemon}` in `slug_core_v2` so the
CLI contract does not have to change when the daemon appears.

### 2.2 Runtime Wrappers

- Add wrapper modules for `dice`, starlark-rust, event logging, and REAPI
  clients.
- The wrappers should expose Bazel-neutral capability traits such as
  `IncrementalEngine`, `StarlarkEvaluator`, `EventSink`, and `RemoteExecutor`.
- Do not pass Buck2 `CellResolver`, Buck target labels, or
  `CommandExecutorConfig` through V2 public APIs.

### 2.3 CLI and Process Contract

- `slug version` reports:
  - Slug V2;
  - Bazel compatibility floor: `9.0.0`;
  - commit or build info when available.
- `slug help` lists only supported V2 commands and marks unimplemented commands
  as planned rather than silently accepting them.
- `build`, `test`, `run`, and query commands may initially return a structured
  `not yet implemented` error, but the command parser must preserve argv for
  the oracle harness.

### 2.4 Daemon Policy

- Start without a daemon if that keeps Stage 1 simple.
- Introduce `slug_server_v2` only when Stage 4/5 need same-daemon invalidation.
- The first daemon must expose a clean shutdown command and a test-only
  `clear-dice` or `new-transaction` control so oracle fixtures can assert warm
  behavior.

### 2.5 Unified DICE Semantic Spine Gate

The accepted architecture is one long-lived DICE instance per daemon/workspace,
not a fresh `Dice::builder()` for each root evaluation and not a daemon-side
file scanner wrapped around separate evaluator caches.

- Inject file contents/absence, directory listings, relevant environment,
  command policy, repository mapping, lockfile policy, and materialization
  observations as explicit DICE inputs.
- Loading, bzlmod, configured-target analysis, query, cquery, and aquery must
  compute from the same transaction or a clearly related transaction on that
  graph. Do not reconstruct semantic subgraphs in command handlers.
- Create, edit, and delete transitions are first-class. The invalidation input
  names both the changed and deleted paths; iterating only files that still
  exist is incorrect.
- Recursive workspace scans may discover test inputs during migration, but
  cannot own production semantics, swallow read failures, or decide that a
  graph is current.
- DICE computations may not hold a blocking or re-entrant lock across
  `ctx.compute`, `try_compute_join`, Starlark evaluation, or another
  computation. Follow `docs/developers/dice.md`.
- Same-daemon tests must explain why unchanged state is reused and why each
  changed input invalidates. Instrumentation counts are evidence only when the
  key/dependency graph supplies the reason.

The existing `WorkspaceEvaluationKey` first-build path and
`slug_server_v2::Daemon::invalidate_changed()` scanner are retained scaffolds.
Their narrow `load-invalidation` result is a regression test, not acceptance of
this gate; an implementation packet may replace them rather than preserve their
shape.

#### Reviewed next packet — `WP-2-m1-workspace-runtime` (2026-07-22)

A Terra-medium source audit followed by Sol-low ownership review revised the
first M1 packet. Root evaluation and loading must be unified immediately; merely
adding file inputs around the existing fresh root DICE and private loading DICE
would preserve the split ownership bug.

- Add a V2-owned `WorkspaceRuntime` under `slug_core_v2::runtime` containing
  canonical workspace identity and the sole retained DICE instance.
- Add an injected workspace-file key whose value distinguishes present content
  from absence. Batch all observed create/edit/delete changes through
  `DiceTransactionUpdater::changed_to`, commit once, and use that transaction
  for root plus package/loading evaluation.
- Make loading compute over the supplied transaction. Remove its private
  `Dice::builder()`, private synchronous runtimes, and direct filesystem reads
  from DICE key computations.
- Remove daemon digest/scanner ownership after an explicit observation adapter
  feeds the new update API. The adapter must report absence and read errors; it
  cannot decide semantic freshness.
- Leave analysis as the current post-loading scaffold consuming results from
  the same request revision. Do not add bzlmod, configured-target, query,
  environment, lockfile, or repository-mapping keys in this packet.
- Run the Buck2 utility audit before choosing the immutable source-content
  representation; do not introduce repeated owned `String` copies into file
  keys/results by default.

Write failing tests first for unchanged reuse; loaded `.bzl` create/edit/delete;
root `MODULE.bazel` edit/delete; `BUILD.bazel` deletion with absent/present
`BUILD`; one root/package revision; and no per-build `Dice::builder()` or
semantic scanner. Stop if a computation would inject while computing, a lock
would cross a compute/Starlark await, or the implementation requires a Stage
4/5/6/8 public-interface decision.

Current worktree evidence (2026-07-22):

- `WorkspaceRuntime` now retains the workspace's sole DICE instance and commits
  one injected immutable file snapshot before root and loading computations.
  Loading accepts the caller's transaction and no longer owns a private DICE
  graph, Tokio runtime, filesystem read, or invalidation scanner.
- Focused regressions cover loaded `.bzl` create/edit/delete, read errors,
  `MODULE.bazel` edit/invalid/delete, root and package
  `BUILD.bazel`-to-`BUILD` fallback, one request revision, the production
  one-shot wrapper, and the retained daemon path.
- Root validation passed
  `CARGO_TARGET_DIR=/tmp/slug-m1-runtime-target CARGO_BUILD_JOBS=1 cargo test
  -p slug_core_v2 -p slug_loading_v2 -p slug_server_v2 -p slug_analysis_v2
  -p slug_cli_v2`.
- This is the correctness-first file-input spine, not M1 completion. The
  observation adapter currently injects a full
  `Arc<starlark_map::SortedMap<...>>` workspace snapshot, so unchanged
  downstream reuse is not yet instrumented and fine-grained watcher/directory
  inputs remain an explicit performance follow-up.

### 2.6 First-Real-Build Promotion

Before Stage 5-8 work can advance beyond scaffold status:

- `slug_core_v2` has normal dependencies on `dice` and `starlark`, and the
  runtime executes a DICE computation for the first build fixture;
- the CLI dispatches `build` into that runtime instead of emitting a planned
  command error;
- the evaluator parses and evaluates a minimal root module/build pair through
  starlark-rust; and
- the owner plans connect the resulting typed action to the Stage 7 protobuf
  REAPI path. The daemon remains optional until the same-daemon invalidation
  portion of the canonical integration gate is reached.

These are integration prerequisites, not a claim that all Bazel commands are
implemented in Stage 2.

## Checkpoint Evidence

### Pending `WP-2-dice-starlark-root-evaluation` review — not accepted

- Gate link: advances clauses 1 and 2 of the First Real Bazel Build gate only:
  `slug build` opens a one-shot DICE transaction, then evaluates the root
  `MODULE.bazel` and `BUILD.bazel` through starlark-rust. It does not yet load
  packages, evaluate custom rules, declare actions, or execute REAPI.
- Oracle first: Bazel 9.1.1 generated and then verified
  `tests/v2_oracle/fixtures/simple-rule-action/expected/oracle.json`; it records
  the declared `bazel-bin/pkg/write_file.txt` SHA-256
  `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49`.
- Reuse audit: adopted retained `dice/dice` transaction/key APIs and retained
  `starlark-rust` `AstModule`/`Evaluator` APIs behind V2-owned runtime types.
  Inspected `slug-v1-archive:app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs`
  as reference only and rejected its Buck-cell, package-label, file-ops, and
  global-interpreter dependencies.
- Scoped implementation: `slug_core_v2::runtime::evaluate_workspace` owns a
  `WorkspaceEvaluationKey`, opens `Dice::builder()` with a real transaction,
  and evaluates the root files. `slug build` now enters that boundary and
  reports `analysis_not_implemented` only after successful evaluation.
- Focused validation passed:
  `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_core_v2 -p slug_cli_v2`
  (2 runtime tests and 5 CLI tests); `scripts/v2_archive_status.sh`; and
  `git diff --check`.
- Pending reviewer decision: this packet has no recorded Sol acceptance and no
  V2 commit yet, so it is not completed evidence and must not advance the
  integration gate. Reviewer must confirm the V2-owned one-shot boundary and
  approve the Stage 4/5 handoff before the status changes.
- Residual risk: root file reads are one-shot key inputs, not yet Stage 4/5
  tracked file/load dependencies; the temporary `module()` global is not the
  Bazel MODULE API; custom-rule analysis and REAPI execution remain open.

Stage 2 skeleton checkpoint:

- Added `slug_core_v2` with build identity, structured planned-command errors,
  `RuntimeMode`, and placeholder capability traits for DICE, events,
  starlark-rust, and REAPI wrappers.
- Added `slug_cli_v2` with `slug` binary output, `version`, `help`, and planned
  `build`, `query`, `test`, and `run` command diagnostics that preserve argv.
- Converted `tools/v2_oracle` to a Python-executable directory so
  `python3 tools/v2_oracle ...` still works and `tools/v2_oracle/README.md`
  documents `SLUG_V2_BIN`.
- Local validation used a dedicated `.codex-cargo-target` because the existing
  `target/` tree denied rustc metadata writes under the sandbox.
- Validation passed: `cargo check -p slug_cli_v2 -p slug_core_v2`,
  `cargo test -p slug_cli_v2`, `cargo tree -p slug_cli_v2`,
  `SLUG_V2_BIN=.codex-cargo-target/debug/slug.exe py -3 tools/v2_oracle run --fixture version-bazel9`,
  and the no-Buck-surface grep over `app/slug_cli_v2 app/slug_core_v2`.
## Exact Test Criteria

- `cargo check -p slug_cli_v2 -p slug_core_v2` passes without depending on V1
  app crates other than explicitly vendored Buck2 infrastructure.
- `cargo tree -p slug_cli_v2` shows no dependency on `app/slug`, `slug_client`,
  `slug_server`, or V1 `slug_core`.
- Before the First Real Bazel Build gate, `cargo tree` for the V2 runtime shows
  `dice` and `starlark`; a focused test proves an actual `DiceComputations`
  path, not a capability trait alone.
- A focused same-daemon test covers file create, edit, and delete without a
  production fallback scan, and proves loading plus analysis/query observe the
  same injected revision.
- `slug version` exits 0 and prints `Slug V2` plus `Bazel compatibility: >=9.0.0`.
- `slug help` does not mention `buck`, `BUCK`, `TARGETS`, `cell`, or
  `.buckconfig`.
- `slug build //:x --unknown_flag` preserves the unknown Bazel-shaped flag in a
  structured parse error for later Stage 8 handling.
- The Stage 1 harness can invoke the V2 binary through an environment variable
  such as `SLUG_V2_BIN`.

## Acceptance Criteria

- `slug version` reports Bazel-9-compatible identity policy.
- `slug help` is V2-specific and does not advertise V1/Buck behavior.
- The oracle harness can invoke the V2 binary.
- The codegraph sees V2 crates as separate from any archived V1 code.
- `build` has crossed the DICE and starlark-rust runtime boundaries for the
  canonical first-build fixture.
- One daemon-owned DICE graph is the semantic owner for module/loading,
  analysis, and query state; no per-request graph or scanner cache is counted
  as final architecture.

### `WP-2.4-slug-server-v2-daemon` — accepted 2026-07-16

- Gate link: satisfies clause 5 of the First Real Bazel Build gate
  (same-daemon DICE invalidation).
- `slug_server_v2` crate introduced with a `Daemon` struct that retains a
  `BzlModuleEvaluator` + file-digest cache across builds.
- `Daemon::invalidate_changed()` rescans `.bzl`/`BUILD.bazel` files, compares
  SHA-256 digests to the previous build, and calls `invalidate_path` /
  `invalidate_package` for changed paths. The DICE graph replays only the
  affected computations.
- Unix-socket server (`server.rs`) with `serve()`, `send_build_request()`,
  `send_shutdown()`. The CLI auto-starts the daemon when `--output_base` is
  set and connects via the socket.
- 3 focused Rust tests: first-build-invalidates-zero, bzl-edit-invalidates-one,
  third-build-after-no-edit-invalidates-zero.
- `load-invalidation` oracle fixture passes end-to-end: prime produces
  `message.txt` digest `2c8b08da.../4` (mode 0o555); after_bzl_edit invalidates
  1 file and produces `27dd8ed4.../4` (mode 0o555) — both match the Bazel 9.2.0
  oracle.

2026-07-22 qualification: this acceptance records the narrow fixture and
daemon transport only. The retained evaluator plus digest scanner does not
satisfy Section 2.5 because root evaluation can create a new DICE graph,
directory deletion is not an explicit input, and loading/analysis/query do not
yet share one semantic transaction. Keep the tests; replace the ownership
shape before M1 is accepted.

## Validation

```bash
cargo check -p slug_cli_v2 -p slug_core_v2
cargo test -p slug_cli_v2
SLUG_V2_BIN=target/debug/slug tools/v2_oracle run --fixture version-bazel9
cargo tree -p slug_cli_v2 | rg 'dice|starlark'
rg -n "buck|BUCK|TARGETS|CellResolver|buck-out" app/slug_cli_v2 app/slug_core_v2
git diff --check
```

Package names are placeholders until the root reset chooses the final crate
layout.
