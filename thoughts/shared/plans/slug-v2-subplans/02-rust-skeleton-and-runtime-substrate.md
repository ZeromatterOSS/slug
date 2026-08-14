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

### 2.5A Request revisions, source certificates, and memory lifetimes

The next M1 architecture packet is scheduled immediately after the fixed M7
source-consumer cutover: acceptance of the first private core repository
source-observation consumer, including only any smallest prerequisite selected
by the current audit. Its only advance evidence gate is the focused
mutation/concurrent-request oracle plus the applicable Buck2 DICE transaction
and publication audit. Unrelated Starlark/provider/action/toolchain oracle
subsets do not block M1.

That packet must replace coarse production snapshot authority with a
Buck2-DICE-native request/revision contract. Zabel's versioned-session design
is concept and test evidence only; do not port its custom engine, scheduler,
token model, or source-certificate implementation.

#### Required request contract

One long-lived workspace runtime owns committed semantic state. Each command
opens a request that owns:

- an accepted base input revision and originating runtime identity;
- one immutable complete request overlay containing normalized command options,
  relevant client environment, cancellation/deadline state, and typed injected
  roots;
- narrow equality projections consumed by semantic producers, while command
  expressions, output modes, diagnostics, profiles, and presentation remain
  request-local;
- lazy typed observations of exactly demanded files, directories, globs,
  environment values, registry/lockfile inputs, repo mappings, repository-rule
  inputs, and materialized immutable roots; and
- a source certificate that is the union of those exact observations and
  tracked semantic dependencies.

The repository capability, path, source-input, and source-observation
certificates accepted before this packet are leaf producer facts. They do not
satisfy this request-level certificate contract until a request unions all
demanded observations and tracked dependencies, performs final reobservation,
and accepts or retries one compatible effective revision.

Source-facing computations remain request-private until final validation:

1. compute from immutable observed values in a provisional transaction or the
   smallest equivalent Buck2 DICE primitive;
2. retain dependency edges and the relevant option projection;
3. reobserve every mutable source-certificate input before returning
   user-visible output;
4. atomically accept compatible values into a committed successor revision or
   discard and retry from new observations; and
5. release provisional publications, borrowed inputs, continuations, RPC
   interests, and async-transfer ownership on retry, cancellation, failure, or
   request teardown.

Never return a result mixing observations from incompatible host epochs.
Cross-request reuse is permitted only when key identity, relevant request
projection, dependency publication, and effective revision are compatible.
The host filesystem does not provide historical reads for unobserved paths;
unsupported history must not be fabricated by a workspace-wide snapshot.
Watcher events may accelerate dirtying but are only candidates. Exact
reobservation and final validation are the correctness boundary.

#### Memory-lifetime contract

Packets that add retained or asynchronous runtime state classify every new
allocation as one of:

1. service/container memory;
2. DICE-retained immutable semantic memory;
3. service-retained nonsemantic cache memory;
4. command-retained memory;
5. phase, Starlark call, action, or RPC scratch; or
6. transfer-owned asynchronous memory.

For every applicable class, state publication, equality-cutoff, replacement,
invalidation, eviction, command reset, cancellation, worker/task join, and
shutdown release behavior. Retained values may borrow only authenticated
retained dependency ownership, never evaluator heaps or command scratch.
Dropping a nonsemantic cache must not remove semantic truth. Continue using
`Allocative` and the Buck2-derived utility review for retained Rust data;
custom arenas or allocator protocols are not implied by this taxonomy.

#### Design packet proof and stops

Before Rust changes, the design packet must:

- audit Buck2 DICE transaction, projection, invalidation, cutoff, cancellation,
  duplicate-work, and publication tests rather than infer an API from Zabel;
- inventory current whole-workspace file/directory snapshots and every bzlmod,
  environment, command-policy, lockfile, mapping, materialization, cquery, and
  aquery input still outside the semantic spine;
- choose the smallest vertical migration that proves two overlapping requests
  with different relevant and irrelevant overlays, a file change during one
  compute, atomic retry, and compatible warm reuse;
- name the source-certificate representation and final-validation owner without
  adding a global command lock or holding a lock across DICE work;
- classify exact observable isolation/invalidation, Slug-native revision and
  certificate identity, and unavailable historical snapshots as unsupported;
  and
- supply exact file allowlists, retained-memory accounting, cancellation proof,
  growth caps, and `REPLAN` conditions.

Stop on a second semantic graph, command-side input replay, mutable global
options, manual semantic side store, source read outside a tracked producer,
watcher events as proof of correctness, retained evaluator values, or a custom
DICE scheduler. The existing coarse snapshots remain accepted scaffolding only
until a separately scheduled packet replaces them; this section authorizes no
runtime edit by itself.

#### Accepted post-cutover audit - `WP-2-m1-mutation-concurrent-request-dice-audit` (2026-08-13)

The source-consumer cutover is accepted in `53152727`. The bounded read-only
audit selected
`WP-1-2-m1-mutation-concurrent-request-oracle-design` before any
request-revision Rust because the existing Bazel evidence is serial and no
fixture yet discriminates an in-flight source mutation.

The accepted `load-invalidation` fixture remains the exact Bazel 9.2
retained-server baseline for serial mutation, warm nonreplay, invalidation, and
A/B/A restoration. It is not concurrent-request evidence: the fixture model
attaches mutations to one command, and the runner applies them before a
blocking `subprocess.run` inside a serial command loop. The selected design
must separately pin Bazel's public in-flight mutation result and its
same-output-base client lock/serialization boundary. True overlapping Slug
request computation and its barrier/cancellation proof are Slug-native and may
not be inferred from contending Bazel clients.

The live input inventory has two incomplete paths:

- legacy `evaluate_observations` and loading-query adapters inject whole
  `WorkspaceSnapshot`, `WorkspaceRawSnapshot`, and
  `WorkspaceDirectorySnapshot` values collected before the request, plus
  Bzlmod request inputs; they have no final reobservation;
- production build/query/cquery use `NativeDemandSessionOwner`, whose single
  `Busy` lease serializes commands and whose manual accepted snapshot retains
  request inputs, synthetic workspace/registry/repository generations,
  repository results, path observations, and selected demands;
- that driver retries repository/path Needs and commits a selected snapshot,
  but it does not finally reobserve every mutable unscoped host path before
  moving events and the terminal into user-visible output;
- command and environment policy, lockfile mode, registry URLs, root package
  policy, repository/materialization generations and results, and path epochs
  enter DICE as typed injected keys; root string settings and semantic
  query/cquery options participate through root identity;
- process-host configuration, repository/materializer session state,
  observation I/O, accepted-demand state, revision counters, command
  presentation, and event buffers remain runtime-owned. Mapping, lockfile, and
  repository graph values derived from admitted injected inputs already remain
  DICE-owned and must not be copied into a request replay store.

The applicable vendored DICE contract is narrower than the old synthetic
driver assumes. One updater can record typed `changed_to` batches and one
commit publishes the resulting version; equal values can preserve the existing
version. Each transaction is fixed at creation and must never be retained by a
computation or result. Same-key/same-version work is deduplicated, distinct
computations can run in parallel, and a shared computation survives one waiter
dropping while another remains. DICE cancellation tests also prove cleanup and
reuse after the last waiter drops. DICE has version-tagged active/in-flight
machinery, but this audit found no public contract/test or historical-host-read
owner sufficient for M1 final validation/publication to rely on concurrent
independently mutated versions without separate proof.

A single DICE key therefore cannot own final reobservation and successor
publication: key computation has no authority to observe mutable host state and
commit a new transaction. The future cohesive owner must pair a private
immutable request overlay and DICE-produced source certificate with a runtime
request coordinator. Provisional work runs without a command-global lease.
Only a narrow final-validation/publication critical section after DICE work may
compare the accepted base, reobserve the exact certificate, commit changed
typed observations, and either accept or retry. It must not span
`compute`, Starlark evaluation, or another DICE computation, and it cannot
become a manual semantic store.

#### Accepted focused oracle design - `WP-1-2-m1-mutation-concurrent-request-oracle-design` (2026-08-13)

The design selected the single Bazel-only
`loading-inflight-source-lock` fixture. Bazel 9.2
`client_test.sh.test_noblock_for_lock_reuse_server` supplies its public FIFO
package-loading and same-output-base nonblocking-client theme. The exclusive
output-base lock in `blaze.cc`, transitive Starlark package-dependency test,
and serial local-diff tests bound the other claims.

The fixture's V1 `a/defs.bzl` supplies both a versioned source label and the
`//b:b` edge. A writer opening FIFO `b/BUILD.bazel` can therefore
acknowledge only after V1 was demanded. While the primary query remains
blocked, the harness changes that source to V2 and records a same-output-base
`--noblock_for_lock info` contender before releasing the gate. Subsequent
serial rows prove V2, compatible warm V2 nonreplay, and V1 restoration.

This is a causal ordering observation, not evidence that Bazel final-reobserves
an already demanded source. Pinned generation and two fresh-root replays must
all return one unmixed V1 primary result; any V2/mixed/variable terminal is
`REPLAN`. Same-output-base Bazel clients serialize, and the contender's
exit-9 diagnostic is exact Bazel client behavior. Neither result is a Slug
request-concurrency parity requirement.

The implementation may add one optional `concurrent_command_group` table
referencing adjacent ordinary primary/contender commands. It reuses exactly
one text `Mutation`, one contained absent FIFO path, and one fixed release
body. The runner owns both clients, writer, descriptors, process groups, one
absolute deadline, regular-file replacement, and terminate/kill/wait/join
cleanup. It admits no scheduler, arbitrary executable, polling, second group,
or module-extension/repository execution.

Exactly five generated records are allowed: in-flight V1, lock contender,
post-mutation V2, warm V2 without marker replay, and restored V1. All capture
one retained Bazel server epoch. Query/marker and normalized diagnostic shapes,
literal exits/order/group evidence, and inverse restoration are discriminating.

The implementation allowlist is the three existing harness/parser/test files,
one seven-authored-file fixture plus generated oracle, and canonical/current/
Stage 1/Stage 2 ledgers. Its corrected caps are 430 production-harness, 380
harness-test, 150 authored-fixture, 500 generated-oracle, 260 ledger, and
1,850 total net lines. The single allowed correction is consumed by this
cap-only increase. It authorizes no Rust, DICE, Cargo/BUILD, public
Slug command/server, network, JVM, or request-owner change.

After accepted replay, design the request-revision vertical against local Rust
barriers/counters. That later proof must run two genuinely overlapping Slug
requests, mutate an already demanded source, discard/retry any provisional
stale terminal, publish only V2-compatible results, prove warm reuse and A/B/A,
release cancellation ownership, and hold no lock across DICE/Starlark work.
Those revision/certificate identities and no-mixed-epoch rules remain
Slug-native. The Bazel FIFO and client lock create no Slug production surface.


#### Historical reviewed packet - `WP-2-m1-workspace-runtime` (2026-07-22)

This earlier whole-snapshot packet is retained as history only and is
superseded by the request-revision/source-certificate contract and accepted
post-cutover audit above.

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

#### Current review packet — `WP-2-m1-spine-exit-audit` (2026-07-23)

Review the complete Section 2.5 gate against the live checkout before changing
production code. Trace the retained `WorkspaceRuntime` and every transaction
handoff through root evaluation, loading, bzlmod, configured analysis, query,
and the command/daemon paths. Separately inventory explicit present/absent file
and directory inputs, create/edit/delete behavior, remaining production
filesystem scans or fresh DICE construction, lock ownership across
computations, and same-daemon tests that explain reuse and invalidation.

The result is an exact pass/gap evidence table, not an M1 acceptance claim.
Passing narrow fixtures and the serialized validation wrapper are supporting
evidence only. If any clause is incomplete, return `REPLAN` with the smallest
independently reviewable implementation packet, its exact file allowlist,
oracle/source anchors, lifecycle evidence, exclusions, and stop conditions.
This audit authorizes no Rust, fixture, dependency, DICE-key, or lock change.

Audit result: `REPLAN`. One retained `WorkspaceRuntime` exists per daemon and
implemented root/loading/configured-analysis/query paths share its committed
transaction. File/directory values preserve present, absent, and read-error
states, and no blocking application lock crosses DICE or Starlark work.
However only the two coarse workspace snapshots are injected; bzlmod,
environment, command policy, lockfile, repository mapping, materialization,
cquery, and aquery are absent from the semantic spine. The recursive observer
still scans the full workspace for every production request, deletion is map
omission rather than a named per-path change, and daemon reuse evidence is
partly the adapter's compatibility counter rather than key activation.

The proposed direct Rust bridge was rejected because Stage 5's input bundles
are value-only records, not real DICE keys, and their owning fixtures still
encoded Bazel 9.1.1 behavior. Commit `911f16f2` accepts the pinned Bazel 9.2
six-fixture runtime-input oracle prerequisite after generation, two independent
clean replay sets, source-anchor checks, and final evidence review. The current
packet is the read-only Stage 5
`WP-5-m1-root-module-dice-bridge-design`; no Rust is authorized until it names
the exact input owners, equality, evaluator boundary, runtime handoff, and
same-daemon evidence. M1 remains partial.

The root-module bridge design review then returned `REPLAN`. The raw file
input boundary is correctly semantic, but it is owned by `slug_loading_v2`;
making Stage 5 depend on loading and later making loading consume the resolved
mapping would create a crate cycle, while putting Stage 5 keys in loading would
invert ownership. Independent review accepted
`WP-2-m1-shared-workspace-file-input-owner`: move only the existing file
snapshot/value/key and unchanged `Key` implementation into neutral
`slug_workspace_v2`, preserve loading re-exports, and prove no semantic change.
Directory inputs and all bzlmod behavior remain outside that prerequisite.

Commit `00422fdc` accepts that prerequisite. `slug_workspace_v2` now owns the
unchanged compact file snapshot/value/key and DICE propagation; loading
preserves exact public re-exports and core imports the neutral owner directly.
Independent validation passed two owner tests, 48 loading tests, 13 core tests,
server/CLI compilation, formatting, diff, and archive checks. The current work
returns to the read-only Stage 5 root-module vertical final design. Follow-up
`97f0fe2a` adds the newly tracked V2 crate to the archive checker's explicit
allowlist; the post-commit archive gate is green.

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

### Accepted request-revision/source-certificate design (2026-08-13)

The focused prerequisite is accepted in `2ffad088`, and the reviewed design
is accepted in `94324880`. Oracle generation and two
independent fresh-root replays preserve the five-record V1/exit-9/V2/warm/V1
sequence in one Bazel 9.2 server epoch. It supplies deterministic source-demand
ordering and the serialized client boundary only; Slug final reobservation and
overlap remain Rust-native.

A fresh live audit found no existing request owner that can be extended
unchanged. `NativeDemandSessionOwner` serializes with `Busy` and retains
`AcceptedNativeDemandSnapshot`; `RepositoryMaterializer` separately
serializes one active repository session. Legacy observation adapters still
inject whole workspace text/raw/directory snapshots. The first request-revision
vertical must create a new private owner rather than rename either lease or
treat either accepted snapshot as the source certificate.

The applicable DICE API supplies fixed transactions, exact equality tokens,
`existing_state`, batched `changed_to`, and successor commits. It supplies
neither a compare-and-swap commit nor a public changed-key compatibility diff.
`DiceTransaction::into_updater` also creates a normal updater against the
engine's newest state; it is not a conditional successor of the consumed
transaction. Therefore every production updater/commit that can race the
admitted private request family must pass through one runtime revision owner.
The owner may lock only around current-version comparison, exact final host
reobservation, typed injection, commit, and publication. DICE/root/Starlark
compute remains outside it.

The selected first vertical admits one private Host-namespace file-bytes
request. Its root key includes runtime/workspace identity, a contained path,
and one relevant semantic overlay projection. Presentation-only overlay data
is request-local. A provisional root consumes the injected request revision
and `PathObservationKey`; its exact demand and result form the one-entry
source certificate. Directory/glob unions, repository/materialization
namespaces, loading migration, and public command/server activation are later
packets.

On path Need or final mismatch, the coordinator reobserves only the exact typed
demand, batches the complete one-entry `PathObservationEpoch` and a
Slug-native successor revision, commits once, discards provisional effects,
and retries. It accepts only if the provisional base still equals
`existing_state` and final reobservation equals the certificate. A version
advance, relevant mutation, observation/injection/publication failure,
cancellation, or bounded nonprogress publishes no terminal. Transactions,
barriers, effects, and certificates are attempt/command memory and never enter
a DICE result or service cache.

The future implementation is confined to a new
`runtime/request_revision.rs`, `runtime/mod.rs`, `runtime/dice.rs`, and,
only if a named Host-only entry point is required,
`runtime/path_observation.rs`. Existing crate globs and dependencies require
no Cargo/BUILD edit. Proposed caps are 560 production, 700 in-module test, 260
ledger, and 1,520 total net lines with one bounded correction.

Acceptance must prove two requests genuinely overlap on one `Arc<Dice>`;
relevant overlays separate while irrelevant overlays share; V1 demanded before
mutation is discarded; only V2 publishes; warm V2 and A/B/A reuse; exact
reobservation/commit/retry/discard counters; one-waiter and last-waiter
cancellation; injected observation/injection/publication/nonprogress failures;
no publication or retained buffer on failure; and no revision-owner lock at
compute barriers. This remains an M1 partial vertical, not loading or command
migration.

`REPLAN` if the production commit boundary cannot be closed for the admitted
family, DICE overlap requires a second graph or historical read, the vertical
needs repository-session concurrency, a stale terminal becomes visible, or
cancellation retains request ownership. Stop on a global command lease, manual
semantic side store, accepted-snapshot reuse, command-side replay, watcher
correctness, retained transaction/evaluator values, custom scheduler, lock
across compute/Starlark, public command/server/Bzlmod edits, or cap excess.

### Active first root-host request-revision implementation (2026-08-13)

Packet `WP-2A-m1-root-host-request-revision` implements only the accepted
private one-file Host vertical. Its exact Rust allowlist is
`runtime/request_revision.rs`, `runtime/mod.rs`, and `runtime/dice.rs`;
the existing `path_observation::observe_native` kernel is already
`pub(super)`, so no fourth Rust path is authorized.

The live competing production commits to close through the async revision owner
are `WorkspaceRuntime::drive_command` attempt injection,
`query_observations_with_policy_and_bzlmod_inputs_and_output_completion`,
`WorkspaceRuntime::evaluate_observations_with_directory_probes_and_bzlmod_inputs`,
`NativeDemandCommand::discard_in_place`, and
`commit_selected_native_demand_snapshot`. Existing-state-only readers and
`#[cfg(test)]` direct transactions are not production publication sites.
The shared owner uses `tokio::sync::Mutex`; no held path computes, invokes
Starlark, calls the repository materializer, or can reacquire the owner.

Caps are three Rust paths, one new module, 560 production lines, 700 in-module
test lines, 260 ledger lines, and 1,520 total net lines. The compatibility,
proof, memory, STOP, and `REPLAN` contract is the compact current-packet
manifest. This activation does not accept M1 or authorize loading/public
migration.

### Accepted first root-host request-revision implementation (2026-08-13)

Commit `207fe438` implements the callerless private one-file Host vertical.
`WorkspaceRuntime` owns one `RequestRevisionRuntime` over its existing
`Arc<Dice>`. The root key structurally consumes a contained relative path,
semantic overlay projection, injected request revision, and the typed
`PathObservationKey`; presentation remains outside DICE. A complete root
retains the exact one-demand/result certificate. The coordinator accepts only
after current-version comparison and exact final host reobservation, commits a
changed observation plus successor revision atomically, and drops a stale
terminal before retry.

All five audited production updater/commit sites use the same
`tokio::sync::Mutex` owner. Its held paths are nonreentrant leaves over
`existing_state`, exact host reobservation, typed `changed_to`, and
`commit`; compute, Starlark, repository/materializer work, terminal
selection, and formatting remain outside. The runtime retains no transaction,
evaluator, accepted snapshot, worker, or semantic side cache.

The in-module proof covers contained/missing paths, relevant/irrelevant
overlays, serial V1/V2/warm/A-B-A/restoration, genuine two-request
post-demand overlap with stale V1 discarded and V2-only acceptance, exact
observation/commit/retry counters, one-waiter and last-waiter cancellation,
forced observation/injection/publication/nonprogress failure, no publish
before validation, lock state, idle cleanup, and gate ownership. Focused tests
pass 7/7 and `cargo check -p slug_core_v2` passes.

The full crate passes 210 unit and 12 integration tests when two independently
reproducible inherited failures are skipped: an older external-repository
visibility diagnostic expectation and the legacy configured-analysis
`Needs` boundary. Those fixes are repository/public-loading work outside the
packet. Strict `clippy -D warnings` stops first in unchanged
`allocative_derive`; `--all-targets --no-deps` finishes without a
`request_revision` warning. The targeted Bazel Rust test cannot analyze
without a matching `rules_rust` toolchain. Diff hygiene and two independent
reviews pass.

Formatted cfg accounting is 456/560 net production, 648/700 in-module test,
and 1,104/1,520 total lines. No correction was consumed. Exact remains limited
to serial Host file present/bytes/absence/error behavior and oracle-backed
invalidation, warm reuse, and restoration. Overlay identity, overlap
isolation, revision numbers, final reobservation, no-mixed-epoch publication,
and provisional suppression remain Slug-native. Directory/glob,
repository/materialized, and public overlapping behavior remain deferred.
This acceptance is M1 evidence, not M1 completion.

### Active loading/public migration audit (2026-08-13)

Packet `WP-2A-m1-loading-public-migration-audit` is documentation-only.
Trace exact symbols from public/daemon requests through `WorkspaceRuntime`
and loading/Bzlmod/query adapters; compare the accepted direct root
exported-source/filegroup `BuildCommandRootKey` source-terminal path with root
`MODULE.bazel`, BUILD, one `.bzl` load, and loading-query candidates.
Select exactly one contained one-file Host consumer with preserved
Need/error/output ordering, immutable
overlay ownership, cancellation/publication lifetime, evidence, future
allowlist/caps/proof, and compatibility boundaries, or record a prerequisite
`REPLAN`.

The only writable files are canonical, current-packet, and this Stage 2 ledger
under 40/220/220/480. No Rust, Cargo/BUILD, oracle, generated evidence, public
activation, snapshot replacement, new DICE key/store, second graph,
directory/glob union, repository/materialization, watcher, historical-host,
or JVM work is authorized.

### Loading/public audit selects a private native-demand bridge design (2026-08-13)

The audit compared the accepted direct root exported-source path with root
module, selected BUILD, one `.bzl` load, native loading-query, legacy query
adapters, and the direct-local external source branch. Root module owns
`include()` and lockfile breadth over snapshot inputs; BUILD loading includes
package/directory/build-file selection; `.bzl` is recursive; query owns the
root anchor plus arbitrary environment Needs; legacy adapters inject whole
text/raw/directory snapshots; and external source adds route/materialization
ownership.

The uniquely smallest candidate is an explicit root
`TargetPattern::Single` selected as `PackageTargetKind::ExportedFile`.
`BuildCommandRootKey` completes the root-module anchor and
`RootPackageLoadKey`, then target lookup and kind selection, before its sole
contained Host `PathObservationKey(FileBytes)`. Need flows through the native
driver; Complete becomes `BuildTargetCompletion::ObservedExportedSource`.
Attempt effects and demands are selected before the selected native snapshot is
committed, and CLI/server output is projected only from `AcceptedCommand`.
A root filegroup is only `LoadedOnly` and is not this source consumer.

The certificate can be retained privately in
`BuildRequestedTarget`/`BuildCommandEvaluation`; no cross-crate or public
ABI is required. Direct reuse of `read_host_file` is nevertheless invalid:
its mismatch path publishes a one-entry epoch, while the native attempt and
selected snapshot own the full root-anchor/package/source epoch. Selection
also moves the effect owner to terminal state, so version/source retry needs a
new suppression/reset transition. Final reobservation and selected-snapshot
commit must share one continuous async-owner linearization.

### Active native-demand revision-publication bridge design (2026-08-13)

Packet `WP-2A-m1-native-demand-revision-publication-bridge-design` freezes
only a private core state machine. It must cover branch-only revision
consumption after earlier errors, nonduplicating initialization, selection and
updater preparation outside the lock, current comparison and exact reobserve
inside, unchanged full selected-epoch plus successor-revision commit, changed
full-epoch merge from a fresh updater, version/source retry, sealed-effect
reset, abort/restoration ordering, cancellation, bounded nonprogress, and no
terminal/event exposure before acceptance.

The future proposal may consider only
`runtime/request_revision.rs`, `runtime/dice.rs`, and
`runtime/events.rs`; the current packet authorizes no Rust. Public commands
remain serialized by the existing native lease, and that lease/repository
session cannot become certificate storage or change behavior. Exact public
serial source/output/Need-error behavior remains fixed; final reobservation,
revision, retry, suppression, and future overlap remain Slug-native.

The only writable files are canonical, current-packet, and this Stage 2 ledger
under 40/260/240/540. Stop on code, public output/overlap, lease or repository/
materializer change, snapshot/loading migration, accepted-snapshot certificate
reuse, one-entry full-epoch overwrite, callback or compute under the owner,
oracle generation, cross-crate API, JVM work, or cap excess.

### Accepted native-demand revision-publication bridge design (2026-08-13)

The bridge remains private to core. `SourceCertificate` adds sibling-only
construction/borrowing. An exactly one-target root
`PackageTargetKind::ExportedFile` retains it in
`BuildRequestedTarget`; a completed root-source error retains the same
certificate beside its existing observation. Success and error selectors
therefore final-validate present, absence, wrong-kind, read-error, and content
mutation without changing public result types or formatting. All other target
kinds and external sources retain no certificate.

After existing anchor/package/lookup/kind ordering, the branch computes the
private request revision before its existing Host FileBytes path key. A private
`NativeCommandRoot` hook sends every syntactically sole-root
`BuildCommandRootKey` through `commit_native_attempt`; multi-target,
package-all, query, cquery, synthetic, non-root, and external driving retain
`commit`. Sole-root rules and filegroups acquire no revision dependency or
certificate. The native-attempt leaf receives an updater that already contains
the full native input/repository/path snapshot. On its first use it adds only
the initial revision and commits once; it never duplicates or empties the path
epoch. Subsequent admitted attempts use the same owner leaf. The callerless
initializer and the other four routed production commits remain bounded as
before.

Event selection returns an armed private terminal token. Its sole retry
transition consumes matching `Terminal(id)` into `Idle`, drops provisional
events/demands, and permits a fresh attempt ID. Acceptance disarms it; armed
drop supplies cancellation cleanup. The token remains command-owned through
snapshot construction, final validation, materializer acceptance, and native
session replacement, so no provisional batch reaches output.

Native preparation selects activation/events/demands, constructs the selected
snapshot and repository validations, and injects an uncommitted full selected
updater without the revision owner. `finalize_native` then owns current-state
comparison and exact reobservation. Unchanged adds a successor revision to that
prepared updater and commits once. Version advance drops it and retries with no
commit. Changed drops it, replaces the certificate demand inside the full
command epoch, commits that merged epoch plus successor revision from a fresh
updater, returns the merged epoch to command state, and retries. The mismatch
commit relies on the current attempt version for unchanged non-path inputs; it
does not accept the stale selected snapshot.

Bridge-only terminal retries are capped at eight. Every error/cancellation
before irreversible acceptance drops the updater/certificate/selected state
and resets/suppresses effects; the existing abort guard restores the prior
snapshot or fails closed. No lock spans DICE root/activation compute, Starlark,
selection/formatting, user-data/native injection, repository/materializer work,
restoration, or a callback. `AcceptedNativeDemandSnapshot` never becomes
certificate storage.

No newly widened Bazel parity is claimed. The root-source public surface remains
a byte-for-byte regression/non-widening invariant. `42f4a64b` supplies only
the shared completion/lifecycle boundary and its accepted external-source
slice, not new root-specific parity. Certificate identity, request revision,
final reobservation, retry/reset, and future overlap are Slug-native. Public
commands remain lease-serialized.

### Active native-demand revision-publication bridge implementation (2026-08-13)

Packet `WP-2A-m1-native-demand-revision-publication-bridge` edits exactly
`runtime/request_revision.rs`, `runtime/dice.rs`, and `runtime/events.rs`,
plus canonical/current/Stage 2 ledgers only at completion. Caps are 600 net
production, 750 in-module test, and 1,350 total added Rust lines. The separate
ledger cap is 260 net lines, with one cap-only correction.

Proof covers atomic first initialization, branch/error ordering, exact success
and source-error certificates, unchanged acceptance, V1-to-V2/absence/error
and absence-to-present suppression, full-epoch preservation, version retry,
selected event reset/fresh ID, exact counters, warm/restoration, forced
initialization/observation/injection/publication/reset/materializer/session/
restoration failure, cancellation, nonprogress, lock barriers, leak checks,
full crate validation, Clippy, targeted Bazel Rust if available, formatting,
diff/artifact/archive/cap checks, and independent ownership/event cleanup
review.

Stop on every other file, CLI/server/public output/API/overlap, lease removal,
repository/materializer change, other root/target kinds, loading/snapshot
migration, new DICE key/store/graph, accepted-snapshot certificate storage,
one-entry full-epoch overwrite, unbounded retry, owner-held callback/compute/
Starlark/event/repository work, oracle growth, watcher, historical host state,
or JVM work.

### Accepted native root-source revision-publication bridge (2026-08-13)

Commit `f0849151` closes
`WP-2A-m1-native-demand-revision-publication-bridge` in exactly
`runtime/request_revision.rs`, `runtime/dice.rs`, and
`runtime/events.rs`. A syntactically sole-root build initializes revision
with its already-full native attempt updater. Only a later root
`ExportedFile` success or completed source error consumes revision and
retains the exact FileBytes certificate; multi-target roots, rules,
filegroups, query, external, and loading paths retain no certificate.

Selection transfers one armed terminal token through prepared-snapshot
construction and finalization. Unchanged source adds the successor revision to
the prepared full updater and commits once. Version advance commits nothing.
Changed source replaces exactly the certificate demand in the command's full
epoch, commits it with the successor revision from a fresh updater, resets the
terminal, and retries with a fresh attempt ID. Acceptance disarms only after
native session replacement; error, cancellation, and drop suppress provisional
events and restore or fail closed.

Focused proof passes eleven revision tests, the sole-root bridge integration,
the fresh-runtime multi-target isolation regression, and five terminal-token
lifecycle tests. The bounded full crate passes 220 library and 12 integration
tests with the two inherited out-of-packet failures skipped. Strict Clippy
stops first in unchanged `allocative_derive`. Targeted Bazel Rust reaches
analysis and stops on six unchanged `slug_bzlmod_v2` `include_bytes!`
files omitted from Bazel `srcs`. Formatting, diff/artifact checks, and
independent DICE/event/cleanup review pass.

Conservative top-level-`cfg(test)` accounting charges 194 production plus
171 test net lines in `dice.rs`, 95 plus 72 in `events.rs`, and 266 plus
140 in `request_revision.rs`: 555/600 production, 383/750 test, and
938/1,350 total. Public bytes remain a regression/non-widening invariant;
certificate/revision/final-validation/reset behavior remains Slug-native.

### Active next source-certificate consumer audit (2026-08-13)

Run only docs packet
`WP-2A-m1-next-source-certificate-consumer-audit` under
40/220/220/480 ledger lines. Compare root MODULE, selected BUILD, one loaded
`.bzl`, and direct-local external source frontiers only enough to select one
complete bounded private certificate consumer or a precise prerequisite
`REPLAN`. Authorize no Rust, partial certificate, public API/output/overlap
or lease change, new key/store/graph, snapshot replacement, repository/
materialization activation, oracle generation, watcher, historical Host read,
or JVM work.

### Next-consumer audit records a loading-frontier prerequisite (2026-08-13)

Independent source and ownership review accepts the audit activated in
`ea36fdcc`: there is no remaining bounded one-observation consumer. The
selected root BUILD terminal depends on root-module anchoring,
`HostRootPackageLookupKey` package-root and `BUILD.bazel`/BUILD probes,
`HostFileBytesKey`, and any recursively loaded `HostBzlModuleEvalKey`
children. Root MODULE expands `include()`; one `.bzl` expands its load
closure and cycle state; direct-local external source adds route, repository,
materialization, package-discovery, and source observations.

Certifying only chosen bytes omits selection negatives and recursively
discovered sources. The core-private one-file type also cannot be produced by
loading/Bzlmod without either crossing visibility deliberately or reversing
the dependency. The audit records `REPLAN` before any implementation.

### Active loading-frontier certificate design (2026-08-13)

Run only docs packet `WP-2A-m1-loading-frontier-certificate-design` under
40/300/260/600. Freeze one app-internal deterministic compact collection of
exact Host demand/result pairs, including selection negatives and dynamically
discovered children. Choose its lowest viable one-way owner and sealed
visibility, complete-only success/error construction, Need/cancellation
suppression, equality/order/duplicate/conflict algebra, provenance, memory
lifetime, terminal carrier, and one first bounded consumer.

Design compute-free batch reobservation under the existing async publication
owner, atomic changed-entry merge into the command's full epoch plus successor
revision, selected-terminal suppression/reset, and failure/nonprogress
cleanup. Preserve accepted public serial bytes and ordering. Aggregation,
revision, validation, retry, and suppression remain Slug-native.

Authorize no Rust/Cargo/oracle writes, public API/wire/output/overlap, reverse
core dependency, generic public framework, new graph/key/store, snapshot
replacement, partial frontier, owner-held compute/Starlark/repository/event
work, repository/materializer activation, watcher, historical Host read, JVM,
combined consumers, or cap excess.

### Loading-frontier design records an observed-path key prerequisite (2026-08-13)

Source inspection under `c1d875ad`, `9d1c6b80`, and `3a627ebb` rejects
a BUILD or `NoBuildFile` implementation as partial. Package lookup depends on
policy, repository-ignore, package-root, and marker sources; package load also
depends on the root MODULE anchor; successful evaluation may add `.bzl` and
glob sources.

The existing lower values lose the exact inputs needed to compose those
frontiers. `ResolvedPathKey` exposes route/state or semantic error but not the
exact Lstat/ReadLink arcs; `HostFileBytesKey` adds FileBytes and then discards
the complete prefix. Reconstructing above workspace would be a second resolver.
Changing the legacy values would activate unrelated consumers.

The bounded answer is a callerless lower chain: one doc-hidden workspace
observed-resolution sibling uses the existing state machine and returns
complete result plus exact `PathObservationEpoch`; one Bzlmod-private
observed-Host-file sibling consumes it and adds the final FileBytes observation.
The current design packet independently forbids new keys, so it records
`REPLAN` before selecting implementation.

### Active observed-path frontier key design (2026-08-13)

Run only docs packet `WP-2A-m1-observed-path-frontier-key-design` under
40/260/220/520. Freeze exactly the two sibling key identities, complete
success/error carriers, Need/cancellation suppression, shared-Arc epoch
constructor/union and conflict algebra, sealed one-way visibility,
structural equality, compact memory lifetime, four-file implementation
allowlist/caps/tests, and hierarchical-design successor.

Authorize no Rust/Cargo/oracle write, third key/store/graph, legacy key/value
or caller migration, loading/core/public caller, request finalization,
repository/module/BUILD/`.bzl`/glob activation, generic public framework,
reverse edge, direct/historical Host read, watcher, JVM, or cap excess.

### Accepted observed-path frontier sibling-key design (2026-08-13)

Design packet `8a87ce8a` freezes exactly two callerless sibling keys.
Workspace `ResolvedPathObservationKey` has the legacy resolution key's exact
identity, shares its state machine, captures each completed exact result Arc
before transition, and returns one `ObservedResolvedPath` only for terminal
success/error. Bzlmod-private `HostFileBytesObservationKey` consumes only
that sibling, retains its epoch, and appends the exact final FileBytes result
when the resolved kind requires it.

`PathObservationEpoch` stays the sole retained compact map. A shared-pairs
constructor/union sorts exact demands, checks operation agreement, preserves
result Arcs, coalesces structurally equal duplicates, and rejects conflicting
duplicates. A doc-hidden `ObservedPathFrontierError` is the outer completed
failure for construction conflict/mismatch; legacy
`PathResolutionError`/`HostFileError` remain nested semantic results and
unchanged. Need is the only incomplete state.

The workspace carrier/key are doc-hidden public solely for the existing
workspace-to-Bzlmod dependency. The observed Host-file carrier/key remain
crate-private. Every legacy key and caller remains untouched. Retained memory
is one semantic result plus shared epoch for DICE-value lifetime; no
transaction, evaluator, updater, event, repository result, materializer,
worker, observer lease, or lock is retained.

### Active callerless observed-path frontier implementation (2026-08-13)

Run `WP-2A-m1-observed-path-frontier-key-implementation` in exactly workspace
`path_observation.rs`, `path_resolution.rs`, `lib.rs`, and Bzlmod
`host_file.rs`. Corrected caps are 380 production, 650 in-module test, 1,030
total Rust, and 200 completion-ledger net lines; the single cap-only correction
is consumed by discriminating proof. Prove exact epoch union/conflict, complete
success/error prefixes, symlink/error/cycle/expansion and final FileBytes
frontiers, Need/cancellation suppression, A-B-A equality, compact Arc
ownership, and unchanged/unactivated legacy keys.

Stop on any other file, Cargo/BUILD/dependency, third key/store/graph, legacy
migration, loading/core/public caller, request finalization, repository/module/
BUILD/`.bzl`/glob activation, generic public framework, panic/error
laundering, reverse dependency, direct/historical Host read, watcher, oracle,
JVM, or cap excess. Completion schedules docs-only hierarchical Host-loading
frontier composition; it does not claim public final validation.

### Accepted callerless observed-path frontier implementation (2026-08-14)

Commit `308b409a` preserves both legacy keys and adds exactly the accepted
callerless observed siblings. The workspace sibling shares one resolution
driver, retains exact completed Lstat/ReadLink Arcs, and publishes no carrier
on Need/cancellation. The Bzlmod sibling consumes only it and appends the final
FileBytes Arc exactly once. Stable shared-epoch union returns typed outer
conflict/mismatch errors without partial or legacy-error laundering.

Focused observed proof is 3 workspace plus 2 Bzlmod tests. Full workspace
43, Bzlmod 367 plus all integration groups, and downstream core check pass.
Strict Clippy stops first in unchanged `allocative_derive`; archive status
retains its named missing-ref/allowlist baseline. Exact accounting is
352/380 production, 394/650 tests, and 746/1,030 total; physical files are
1,662/1,750, 4,346/4,400, 572/580, and 1,080/1,100. The single cap correction
is consumed. Independent ownership, memory, scope, artifact, formatting, and
diff reviews accept.

### Active hierarchical Host-loading frontier composition audit (2026-08-14)

Run only docs packet
`WP-2A-m1-host-loading-frontier-composition-audit` under
40/320/280/640 ledger lines. Inspect repository-ignore, root-module anchor,
and only then package-marker selection to identify complete mutable Host
predecessor closures and one bounded private successor or `REPLAN`.

Write only canonical/current/Stage 2. Authorize no Rust/Cargo/oracle, partial
frontier, loading/core/public activation, generic certificate framework,
reverse dependency, new retained container/graph/store, repository/
materializer activation, historical Host read, watcher, JVM, or combined
MODULE/BUILD implementation.

### Hierarchical audit records an observed root REPO-file prerequisite (2026-08-14)

Source inspection under `a6aaa844` records `REPLAN` before repository-ignore,
package-marker, or root-module implementation. Root
`HostRepositoryIgnoreKey` first consumes legacy `HostRepoFileKey`, then
immutable policy, then ordered `.bazelignore` probes. The legacy REPO key reads
one root `REPO.bazel`, evaluates it, stores per-transaction events, and returns
only semantic value/error; its exact Host-file epoch is erased. Reconstructing
that epoch above the key would duplicate its natural owner.

Package lookup subsequently consumes repository-ignore before ordered
`BUILD.bazel`/`BUILD` resolution probes. Root MODULE cannot precede that
work: every dynamically discovered `include()` horizon preflights its packages
through the same lookup. The root-module frontier seals only when that horizon
empties. Visible lockfile and selected package source are separate later
frontiers, not dependencies of `RootModuleLoadingAnchorKey`. Routed/nonroot
repository sources add repository/materializer ownership and remain deferred.

The uniquely smallest prerequisite is one Bzlmod-private, callerless observed
root REPO-file sibling. It can consume the accepted observed Host-file key,
retain the exact epoch beside unchanged semantic success/error, and preserve
events without activating repository-ignore or a public caller.

### Active observed root REPO-file frontier-key design (2026-08-14)

Run only docs packet `WP-2A-m1-host-repo-file-frontier-key-design` under
40/260/220/520 ledger lines. Freeze exactly one crate-private
`HostRepoFileObservationKey` and `ObservedHostRepoFile` carrier in
`repo_file.rs`. The sibling uses the same workspace identity and policy-first
ordering as the legacy key, consumes only `HostFileBytesObservationKey`, and
returns unchanged semantic result/error plus the accepted exact epoch. Need is
incomplete; dependency frontier errors are completed outer errors; policy
failure retains the legacy semantic error with an empty epoch.

Freeze shared semantic evaluation/event handling, exact Arc/epoch ownership,
complete equality/validity, one-file implementation caps, focused parity/
Need/cancellation/A-B-A/activation/event proof, and a docs-only
repository-ignore frontier successor. Preserve legacy keys, callers, values,
events, and public behavior.

Authorize no Rust/Cargo/oracle write, direct repository-ignore consumer,
package/MODULE/lockfile/BUILD/`.bzl`/loading/core/public activation,
routed/nonroot repository/materializer work, second observed key, legacy
migration, generic certificate framework, new retained container/graph/store,
reconstructed path demands, sibling-to-legacy compute, duplicated evaluator,
retained event/evaluator/transaction, historical Host read, watcher, JVM, or
cap excess.

The future Rust allowlist is exactly `app/slug_bzlmod_v2/src/repo_file.rs`.
The observed carrier retains one private
`Arc<Result<HostRepoFileValue, HostRepoFileError>>` plus the accepted epoch
and exposes only a borrowed semantic result. Formatted implementation caps are
200 production, 370 in-module test, and 570 total net lines; the physical file
ceiling is 2,328 lines. No cap correction is reserved.

### Accepted observed root REPO-file frontier-key design (2026-08-14)

Independent source and ownership review accepts the design activated in
`7d7f0d25`. One crate-private `HostRepoFileObservationKey` shares the
legacy workspace identity and policy-first ordering but computes only
`HostFileBytesObservationKey`. Its callerless DICE value is
`PathOutcome<Result<ObservedHostRepoFile, ObservedPathFrontierError>>`.
The carrier retains one private semantic-result Arc plus the accepted exact
epoch and exposes only borrowed result/epoch accessors.

Policy projection failure retains the unchanged inner semantic error with an
empty epoch and zero Host activation. Need/cancellation publishes no carrier
or events. A lower frontier error remains a completed outer error. Completed
Host-file resolution/read errors, missing, evaluation errors, and success all
retain the exact dependency epoch. A synchronous stack-only adapter owns the
existing reporter/evaluator path and exactly one completed event-batch store;
neither DICE key computes the other, and no lock, evaluator, batch,
transaction, or extra source copy enters retained state.

The authorized retained utility sources moved to live
`gazebo/dupe/src/lib.rs` and `allocative/allocative/src/lib.rs`; the stale
`third-party/buck2/` prefixes are replaced without widening the comparison.
`Dupe` covers only cheap Arc/epoch bumps and `Allocative` preserves DICE
memory accounting. No new container, export, manifest edge, or public behavior
is introduced.

### Accepted observed root REPO-file frontier-key implementation (2026-08-14)

Commit `f2c7305f` accepts the callerless Bzlmod-private
`HostRepoFileObservationKey`. It preserves legacy policy-first semantic and
event behavior, computes only the accepted observed Host-file sibling, keeps
Need incomplete, forwards outer frontier errors before evaluation/event
storage, and retains one semantic-result Arc plus the exact child epoch.
Neither REPO sibling computes the other.

Focused proof is 4/4 plus the strengthened resolution-prefix rerun 1/1. All 564
Bzlmod unit/integration tests and the direct `slug_core_v2` compile check pass;
formatting and diff hygiene pass. Strict Clippy stops first in unchanged
`allocative_derive`; the archive checker reproduces its inherited missing
archive-ref/non-V2-thoughts baseline. Formatted net growth is 158 production,
365 in-module test, and 523 total lines; the file is 2,281 physical lines,
within 200/370/570 and 2,328.

Independent ownership and AI-cleanup review accepts the file as cohesive:
root/nonroot/routed REPO parsing/evaluation, reporters, event finalization,
private keys, and activation proof remain one owner. A split would widen
private seams without isolating another responsibility. Retained state is only
the semantic-result Arc and existing compact epoch; source bytes, evaluator,
reporter, events, transaction, and scratch do not escape computation.

### Accepted root repository-ignore frontier design (2026-08-14)

Independent source and ownership review accepts one callerless crate-private
`HostRepositoryIgnoreObservationKey` in `repository_ignore.rs`. Its sole
identity is the normalized workspace, its value is
`PathOutcome<Result<ObservedHostRepositoryIgnore,
ObservedPathFrontierError>>`, and complete equality/validity match the accepted
observed siblings. The carrier retains exactly one matcher/error Arc and the
existing `PathObservationEpoch` and exposes borrowed accessors only.

The sibling computes observed REPO first, immutable policy second, then
observed `.bazelignore` files in configured root order. Missing and directory
terminals remain exact negative probes; other Host errors, parse errors, the
first selected file, and the all-negative success retain the complete prefix.
One shared observed-capable parser additionally captures each exact Windows
long-path result Arc before interpretation. Need/cancellation drops all scratch.

One private `PathObservationEpoch::from_shared` union preserves REPO/root/line
input order, exact first-Arc retention, deterministic duplicate coalescing, and
typed outer mismatch/conflict failures. Legacy repository-ignore semantic
errors, diagnostics, source order, matcher behavior, and events do not change.
The observed REPO dependency remains the sole event-batch owner.

No public export, reverse edge, extra key family, retained container, copied
observation, evaluator, reporter, policy object, transaction, worker, or lock
is required. Exact serial repository-ignore behavior and admitted Host
observations remain exact; aggregate frontier identity is Slug-native;
routed/materialized and higher package/MODULE/source/loading/core/public
surfaces remain deferred.

The implementation allowlist is exactly
`app/slug_bzlmod_v2/src/repository_ignore.rs`. Formatted caps are 280
production, 450 in-module test, and 730 total net added lines; the physical
ceiling is 2,821 from the 2,091-line baseline. Require an independent
cohesion/cleanup review above 2,400 formatted lines. Completion ledgers are
capped at 180 net lines with no correction.

### Active observed root repository-ignore implementation (2026-08-14)

Run `WP-2A-m1-host-repository-ignore-frontier-implementation` from accepted
design `8ac5c30f`. Implement exactly one callerless crate-private
`HostRepositoryIgnoreObservationKey` and `ObservedHostRepositoryIgnore` in
`app/slug_bzlmod_v2/src/repository_ignore.rs`.

Preserve observed REPO, immutable policy, ordered observed `.bazelignore`
negative/selected probes, and Windows long-path observation order. Reuse one
shared observed-capable parser and `PathObservationEpoch::from_shared`; Need
and cancellation publish no carrier, semantic errors stay inner, and
mismatch/conflict stays a completed outer frontier error. The observed REPO
dependency remains the only event-batch owner. Retain only one semantic Arc and
the existing epoch.

Require focused success/error/Need/outer, ordered precedence, exact-Arc,
legacy-nonactivation, A/B/A/warm, create/edit/delete/recreate, policy-only,
cancellation, and cfg(windows) normalization proof; full Bzlmod validation,
direct downstream compile, formatting, Clippy disposition, diff/artifact/cap
accounting, and independent ownership plus cohesion review.

The exact Rust allowlist is `repository_ignore.rs`. Caps are 280 production,
450 in-module test, 730 total net, and 2,821 physical lines from the 2,091
baseline; require cleanup review above 2,400 lines. Completion ledgers are
capped at 180 net lines with no correction. STOP on every other code path,
legacy/public/routed/materialized/higher activation, new storage or key family,
and cap excess. Completion schedules only docs-only
`WP-2A-m1-host-package-marker-frontier-design`.
