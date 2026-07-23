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

#### Reviewed next packet — `WP-2-m1-directory-observations` (2026-07-22)

Work packet ID: `WP-2-m1-directory-observations`

Owner stage and plan: Stage 2,
`thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`;
prepares Stage 4 section 4.2 without yet wiring Starlark `glob()`.

Goal and gate link: add explicit, immutable direct-directory observations and
a per-directory DICE propagation boundary to the M1 workspace runtime. This is
the first half of the reviewed directory/glob packet; it does not claim the
M1 glob or unchanged-reuse clauses complete.

Prerequisites and current state: `3659b0f9` supplies the retained workspace
DICE/file-input spine. `5ebf8db1` supplies the generated and independently
verified Bazel 9.2.0 create/rename/delete oracle. Production observation still
collects only regular files, and `GlobExpansionKey` remains data-only.

Oracle-first artifact:
`tests/v2_oracle/fixtures/glob-directory-invalidation/expected/oracle.json`,
generated at Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. This packet uses its directory
transition semantics but does not run Slug query parity.

Reuse audit:

- adopt retained DICE `InjectedKey`, `Key`, `changed_to`, and equality
  propagation behind V2-owned keys;
- selectively port Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`'s compact
  `FileType`/sorted `Arc<[SimpleDirEntry]>` shape from
  `app/buck2_common/src/file_ops/metadata.rs`;
- keep Buck2 `app/buck2_common/src/file_ops/dice.rs` and
  `app/buck2_file_watcher/src/fs_hash_crawler.rs` reference-only for
  per-directory dirtying and observer separation;
- reject V1 commit `e218054d4c796655939b968d90208b185decb352`
  `app/slug_interpreter_for_build/src/interpreter/globspec.rs` and
  `app/slug_file_watcher/` because their Buck package listings, cells, ignores,
  and watcher freshness policy violate the V2 boundary; and
- treat Bazel 9.2.0
  `DirectoryListingValue.java`/`DirectoryListingStateFunction.java` as semantic
  authority, not source to port.

Sol-low approved this reuse decision and a compact name-sorted entry containing
`RegularFile | Directory | Symlink | Other`, plus a directory value of
`Present(entries) | Absent | ReadError`. Symlink identity is observed but not
resolved. Canonical workspace plus normalized contained absolute key paths is
approved for this root-repository-only packet; it must not become the Stage 5
repository-aware public identity.

Exact scope:

- `app/slug_loading_v2/Cargo.toml`;
- `app/slug_loading_v2/src/{keys.rs,bzl_module.rs}`;
- `app/slug_core_v2/src/runtime/{dice.rs,mod.rs}` and
  `app/slug_core_v2/tests/runtime.rs`;
- `app/slug_server_v2/src/{lib.rs,tests.rs}`; and
- focused downstream compile fixes caused directly by the observation API.

Exclude glob matching/traversal, Starlark globals, package construction,
external repositories, repo mapping, query, analysis semantics, execution,
symlink traversal, and a production file-watcher replacement.

Decisions reserved for design reviewer: the subsequent Starlark `glob()` bridge
remains unapproved. No blocking bridge, nested runtime, direct filesystem
compute, injected input during compute, or lock across a DICE/Starlark await is
permitted.

Implementation steps:

1. Add allocative directory entry/value/snapshot/key types using
   `CompactString`, a sorted `Arc` slice, and a compact deterministic snapshot.
2. Extend the migration observation adapter to record each directory's direct
   listing or explicit absence/read error without matching globs, following
   symlinks, swallowing failures, or deciding freshness.
3. Batch file and directory injected snapshots into the same updater/commit;
   make `WorkspaceDirectoryKey` compute only from its injected snapshot and use
   per-value equality as the propagation boundary.
4. Add focused sorted-kind, absent/read-error, containment, create/rename/delete
   transition, and same-revision tests. Leave computation counters and
   glob/package reuse evidence for the second half, where consumers exist.

Focused validation:

- `CARGO_TARGET_DIR=/tmp/slug-m1-directory-target CARGO_BUILD_JOBS=1 cargo test
  -p slug_core_v2 -p slug_loading_v2 -p slug_server_v2 -p slug_analysis_v2
  -p slug_cli_v2`;
- `cargo fmt --all -- --check`;
- ownership greps proving filesystem directory reads remain only in the
  pre-transaction observation adapter and the DICE runtime is built only by
  `WorkspaceRuntime`; and
- `scripts/v2_archive_status.sh` plus `git diff --check`.

Evidence and plan update: after Sol post-review acceptance, record the exact
test results, observed transitions, utility reuse, residual migration scanner,
and accepted commit here and in the Stage 9 ledger.

Stop conditions: stop on an ungenerated/stale oracle, external-repository
identity, silent symlink traversal, swallowed read failures, an injected input
requested before initialization, injection during compute, a lock crossing a
compute/evaluator await, or any need for the unreviewed Starlark bridge.

Accepted implementation evidence (2026-07-22):

- `WorkspaceDirectoryValue` now preserves sorted compact direct entries,
  absence, and read failures. Entry names use `CompactString`; present values
  retain a sorted `Arc` slice; symlinks and special files stay distinct; and an
  invalid UTF-8 name becomes an explicit read error rather than a lossy key.
- The migration observer records direct listings without following symlinks.
  Normalized contained paths are required, including rejection of symlink
  aliases. Filesystem reads remain outside DICE computations.
- File and directory snapshots are scheduled through two typed `changed_to`
  calls on one updater and become visible through its sole commit.
  `WorkspaceDirectoryKey` reads only the injected snapshot and compares its
  per-directory value.
- Production root/package evaluation deliberately requests zero directory
  keys. A private same-module probe exists only for focused tests until the
  reviewed Starlark/glob consumer is implemented; no migration evidence leaks
  into `WorkspaceBuildEvaluation`.
- Focused tests prove sorted kinds, absence versus read error, normalized and
  symlink-alias rejection, selected-key revision coherence, and retained-runtime
  create/rename/delete transitions with an unchanged unrelated directory.
  Sol-low post-review rejected the first eager all-directory result shape; the
  corrected demand-driven boundary was accepted.
- Root validation passed
  `CARGO_TARGET_DIR=/tmp/slug-m1-directory-target CARGO_BUILD_JOBS=1 cargo test
  -p slug_core_v2 -p slug_loading_v2 -p slug_server_v2 -p slug_analysis_v2
  -p slug_cli_v2`, `cargo fmt --all -- --check`, and `git diff --check`.
  `scripts/v2_archive_status.sh` retains known hygiene failures for the absent
  local `v1-archive` branch and its broad non-V2 path matcher; neither is caused
  by this runtime packet.
- Residual after the accepted Stage 4 consumer: the recursive observer remains
  a full-workspace migration adapter. `PackageListingKey` now provides the
  semantic directory consumer and retained-DICE activation evidence, while
  production glob filtering is pure over its listing. Fine-grained watcher
  input, symlink resolution, full Bazel glob syntax, and repository-aware
  listing identity remain later work.

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
