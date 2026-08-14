# Current Slug V2 Packet

Packet: `WP-1-2-m1-mutation-concurrent-request-oracle-design`
Milestone: M1 one semantic spine
Owners: `slug-v2-subplans/01-compliance-oracle-harness.md` and
`slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the smallest Bazel 9.2 mutation/concurrent-client oracle and
local Slug concurrency-proof boundary required before request-revision Rust.

The fixed source-consumer cutover is accepted in `53152727`. Scheduling has
therefore left M7 and follows the canonical M1 -> just-in-time oracle -> M7A ->
M8 -> M7B order. This packet is docs-only design. It may select one bounded
oracle implementation or return a precise prerequisite `REPLAN`; it cannot
implement the oracle or the request-revision vertical.

## Active design contract

Audit the canonical Bazel 9.2 source and tests from the pinned `9.2.0` commit
for two distinct observations:

1. one command whose demanded loading or repository source changes while work
   is in flight, including the exact terminal/retry/failure and warm-restoration
   behavior that is publicly observable; and
2. two clients contending for one Bazel output base, including the exact
   `--block_for_lock`/nonblocking boundary and whether Bazel serializes rather
   than concurrently evaluates commands.

Do not infer true overlapping server-request semantics from two Bazel client
processes. If Bazel serializes them, record that exact surface and classify
Slug's internal overlapping-request execution as Slug-native architecture over
the same exact accepted outputs.

Audit the current oracle harness before choosing a representation. The live
model applies every command's mutations before a blocking `subprocess.run` and
then runs commands serially. Freeze at most one fixture-scoped coordination
shape that can start one command, wait for a deterministic public or
fixture-owned gate, mutate one contained source, optionally launch the
contention client, collect both processes, and prove cleanup. Prefer an
existing Bazel integration-test theme and public workspace behavior. Do not add
a general process scheduler, arbitrary shell program, sleep-based race, or a
second fixture language.

The design must name:

- the exact upstream Bazel source/test anchors and any deliberately skipped
  cases;
- one fixture name, complete workspace/source roles, literal command ordering,
  mutation and restoration sequence, expected exits/channels/manifests, and
  comparison modes;
- whether the existing mutation schema is reused or one narrow concurrent
  command-group schema is required;
- the exact future tool, harness-test, fixture, and owner-plan allowlist;
- process, signal/gate, output-base, timeout, failure, cancellation, and cleanup
  ownership for Bazel and Slug runs;
- which records are Bazel-generated exact evidence and which later Rust
  barriers/counters are Slug-native proof only; and
- authored-line, file-count, record-count, and correction caps plus terminal
  `REPLAN` conditions.

The focused evidence is a prerequisite, not the request architecture. A later
Rust design must still provide two genuinely overlapping Slug requests at one
accepted base revision, different relevant and irrelevant immutable overlays,
an in-flight demanded-source mutation that discards and retries the provisional
terminal, A/B/A restoration, compatible warm reuse, cancellation release, and
zero lock across DICE or Starlark computation. Bazel output-base serialization
does not authorize Slug's current global command lease.

## Compatibility and DICE boundary

Exact compatibility is limited to the named Bazel 9.2 public command,
mutation, invalidation, diagnostic, exit, and output relationships selected by
the design. Slug-native scope is the request/revision/source-certificate
representation, true overlapping-request scheduling, DICE transaction
composition, local barriers/counters, revision tokens, memory ownership, and
diagnostic wording where Bazel exposes no stable byte contract. Historical host
snapshots, watcher correctness, unselected source families, public command
migration, transport/materialization breadth, and Zabel implementation are
unsupported/deferred.

The accepted DICE audit constrains later design:

- one updater may record typed `changed_to` batches and one commit publishes a
  version; equal injected values need not create a new version;
- a transaction is fixed at creation and must not be retained by a computation
  or result;
- identical key/version work is deduplicated, distinct computations can run in
  parallel, and dropping one of multiple waiters does not cancel their shared
  work;
- DICE has version-tagged active/in-flight machinery, but this audit found no
  public contract/test or historical-host-read owner sufficient for M1 final
  validation/publication to rely on concurrent independently mutated versions
  without separate proof;
- a DICE key cannot reobserve the host and commit a successor transaction from
  inside its computation.

Therefore the future final-validation owner must be a request/runtime
coordinator outside key computation. It may use only a narrowly scoped
publication critical section after provisional DICE work; it may not retain a
global command lease, hold a lock across `compute`/Starlark work, replay
semantic inputs in a manual side store, or create a second graph. User-visible
terminal/events/output remain provisional until exact reobservation accepts the
base revision or injects changed observations and retries.

## Scope, caps, proof, and stops

This docs-only design may edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Net documentation growth is capped at 40 canonical, 320 current, 280 Stage 1,
280 Stage 2, and 840 total added lines. Require exact live-harness and pinned
source/test maps, explicit command-versus-server concurrency classification,
one smallest future allowlist and fixture schema, proof/caps/stops, compact
predecessor retention, `git diff --check`, and independent design acceptance.

STOP on any tool, fixture, generated oracle, Bazel execution, network access,
Rust, Cargo/BUILD, public command or server change, generic asynchronous
harness, arbitrary fixture-owned executable, sleep/poll-only race, second
fixture schema, JVM/Java artifact in Slug, Zabel code, unbounded source family,
more than one selected future fixture, fifth documentation file, cap excess, or
lack of a deterministic public/test-anchored mutation gate. `REPLAN` if the
only Bazel evidence requires private test hooks unavailable to the oracle
harness, if client lock behavior cannot discriminate the intended claim, or if
the selected source mutation belongs to execution rather than the M1 semantic
spine.

## Immediate predecessor record

Implementation `53152727` accepts the first callerless private core root
repository source-observation consumer and is the fixed cutover. Focused
consumer tests pass 6/6; the direct dependent compile and formatting pass; an
independent reviewer accepted the split path-compute/observation-compute
terminal ownership and retained-Arc proof. The full core suite still has the
unrelated clean-reproducing deferred-message baseline, and the archive checker
still has the inherited missing-ref/active-guide baseline; neither widened the
three-file implementation.

The completed `WP-2-m1-mutation-concurrent-request-dice-audit` found that
`load-invalidation` is the serial retained-server mutation/warm-reuse
baseline, not concurrency evidence. The fixture model stores mutations on one
command, and the runner applies them before each blocking command. Live Slug
also has two incomplete authorities: legacy requests inject complete text/raw/
directory workspace snapshots with no final reobservation, while production
native-demand commands use a single `Busy` lease and a manual accepted
snapshot containing request inputs, synthetic generations, repository results,
path observations, and selected demands. The latter retries materialization
Needs and commits a selected snapshot but does not finally reobserve every
mutable unscoped source before exposing output.

Command/environment policy, lockfile mode, registry URLs, root package policy,
repository/materialization generations and results, and path epochs enter DICE
through typed injected keys. Root string settings and query/cquery options are
partly root-key owned; process-host configuration, repository/materializer
session state, accepted-demand state, observation I/O, presentation, and
revision counters remain runtime-owned. This inventory rules out treating the
existing global lease or selected-snapshot side store as the M1 request-level
certificate. The focused oracle design above is the smallest prerequisite
before freezing its replacement.
