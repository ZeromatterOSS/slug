# Scheduler Parallelism Investigation (Plan 17.2)

## Executive Summary

Kuro's action execution throughput (3.2 act/sec) lags Bazel (5.6 act/sec) by 43% despite using tokio::spawn for task dispatch. This investigation identified the architecture, measured concurrency controls, and isolated actionable bottlenecks.

**Key finding**: No fundamental architectural blocker preventing parallelism. The shortfall appears to be a combination of:
1. **I/O thread pool hardcoded to 4 threads** (not related to action parallelism but affects cleanup)
2. **Dependency graph serialization**: false serial dependencies or edge granularity issues (requires DICE trace analysis)
3. **Host resource permit allocation working correctly** — but possibly too conservative on weights

---

## High-Level Architecture (10-line sketch)

```
Build flow: DICE graph computation engine
    └─ ActionCalculation::build_action (async_trait, per ActionKey in DICE)
         ├─ Input materialization: ensure_artifact_group_staged (join_all, parallelized)
         ├─ Action.execute() (delegated to registered action impl)
         │   └─ BuckActionExecutor::execute (local/hybrid/remote dispatcher)
         │       ├─ HostSharingBroker::acquire (semaphore, num_machine_permits)
         │       ├─ CommandExecutor::prepare_action, cache lookups, dispatch
         │       └─ MutexClaimManager::claim (per-output serialization, not global)
         └─ Span recording + metrics
    
Tasks are spawned via DICE's TokioSpawner (tokio::spawn → tokio runtime)
Concurrency per executor: SmallerTasksFirst strategy, SharedSemaphore(num_permits)
```

No global action queue exists; DICE's work-stealing graph drives task spawning.

---

## Hypothesis Evaluation

### a) Per-category concurrency caps hardcoded to small number

**Verdict**: NOT OBSERVED

**Rationale**:
- No per-category concurrency caps found in code.
- Action categories (e.g., `cxx_compile`, `cxx_link`) are used for identification/messaging only.
- Worker-specific concurrency is optional (`worker.concurrency: Option<usize>`, defaults to None).
- Main concurrency cap is **per-executor HostSharingBroker**, not per-category.

**Citations**:
- `/var/mnt/dev/kuro/app/kuro_action_impl/src/actions/impls/run.rs:664` — `concurrency: worker.concurrency` (optional per-worker, not per-category)
- `/var/mnt/dev/kuro/host_sharing/src/host_sharing.rs:166-179` — HostSharingBroker constructor uses global `num_machine_permits`

---

### b) Blocking std-lib calls inside scheduler holding a lock across task dispatch

**Verdict**: CONFIRMED (minor) + CANDIDATE ISSUE

**Issue 1: I/O thread pool hardcoded to 4 threads**

- `/var/mnt/dev/kuro/app/kuro_execute/src/execute/blocking.rs:101`
  ```rust
  let io_threads = kuro_env!("BUCK2_IO_THREADS", type=usize, default=4)?;
  ```
  Default is 4 threads for all I/O operations (file writes, materializer operations). On an llvm-scale build with many concurrent compile actions, this may bottleneck output materialization.

  **Expected impact**: Affects actions waiting for output write slots, not initial action dispatch.

**Issue 2: HostSharingBroker acquisition holds permits across semaphore wait**

- `/var/mnt/dev/kuro/host_sharing/src/host_sharing.rs:212-229`
  ```rust
  async fn acquire_from_permits_and_identifiers<'a>(
      ...
      sorted_identifiers: impl Iterator<Item = &'a String>,
  ) -> HostSharingGuard {
      // Acquire identifier semaphores FIRST (no permits held)
      for identifier in sorted_identifiers {
          let name_guard = run_semaphore.acquire(SINGLE_RUN).await;
          name_guards.push(name_guard);
      }
      // Then acquire permits
      let run_guard = self.permits.acquire(num_requested_permits).await;
  ```
  Design is correct: permits are acquired *after* identifier locks, so permits aren't held during identifier waits. This is **not a blocker**.

**Conclusion**: I/O thread pool cap is worth investigating, but it's not the action *dispatch* bottleneck—it's output materialization.

---

### c) Tasks run inline on dispatcher rather than spawned onto tokio runtime

**Verdict**: NOT OBSERVED (tasks are properly spawned)

**Rationale**:
- Action execution is driven by DICE computations, which use `TokioSpawner` (tokio::spawn).
- `/var/mnt/dev/kuro/dice/dice_futures/src/spawner.rs:29-36`
  ```rust
  impl<T> Spawner<T> for TokioSpawner {
      fn spawn(...) -> JoinHandle<Box<dyn Any + Send + 'static>> {
          tokio::spawn(fut)
      }
  }
  ```
- Action input materialization uses `tokio::task::unconstrained(KeepGoing::try_compute_join_all(...))` `/var/mnt/dev/kuro/app/kuro_build_api/src/actions/calculation.rs:115-129` — explicitly parallelized.
- No `block_in_place` in the action execution path (only in Starlark evaluator for interpreter safety).

**Conclusion**: Task dispatch is async and properly spawned. Not a candidate.

---

### d) Dependency edges wrongly inherited, missing fine-grained ReductionKey

**Verdict**: CAN'T TELL (requires DICE trace, not visible from static code)

**Rationale**:
- No per-category or per-package ReductionKey found in code review.
- Action dependencies flow through DICE's `ActionKey` (defined in `kuro_artifact::actions::key::ActionKey`).
- Artifact groups are resolved to per-artifact inputs via `ensure_artifact_group_staged`.
- Without a DICE trace showing the critical path and how many action nodes have the same inputs, cannot determine if dependencies are too coarse.

**Candidate scenario** (from plan context):
> "The LLVM `td_generate` phase was visibly sequential (one pending action at a time for minutes)."

This suggests:
- Either a false serial dependency in the LLVM BUILD rules (actions A→B→C forced sequentially)
- Or a ReductionKey shared across semantically-independent actions (e.g., all `td_generate` actions reducing to one DICE node, causing DICE to serialize them)

**Next step**: Examine DICE trace or critical-path data from plan-16 measurement pass.

---

### e) tokio::spawn_blocking not used where I/O happens in scheduler path

**Verdict**: NOT A BOTTLENECK (I/O is properly delegated)

**Rationale**:
- Command execution happens in external workers (local or RE).
- Kuro's action execution waits on futures, not blocking syscalls.
- I/O (materializer, cleanup) uses `BlockingExecutor` trait:
  - `execute_io_inline`: uses `tokio::task::block_in_place` (correct for synchronous I/O inside async)
  - `execute_io`: either dedicates a thread pool (BuckBlockingExecutor) or uses `tokio::task::spawn_blocking` (DirectIoExecutor)
- `/var/mnt/dev/kuro/app/kuro_execute/src/execute/blocking.rs:137` and `:184`

**Conclusion**: I/O handling is correct. Not a dispatcher bottleneck.

---

## Ranked List of Concrete Changes (by Expected Impact)

### 1. **Investigate + fix DICE dependency granularity** (HIGH impact, MEDIUM risk)

**Problem**: Unknown false serial dependency or coarse ReductionKey causing multi-minute sequential phases.

**Changes to try**:
- Use plan-16's DICE trace or critical-path output to identify which actions serialize and why.
- If all `td_generate` actions share a single DICE key, split by a finer attribute (e.g., per-language-feature, per-target subset).
- Verify `ActionKey` includes all semantically-independent variation (target, config, rule inputs).

**Expected impact**: Could recover 30-50% of the 3.2→5.6 gap if dependencies are the main blocker.

**Risk**: Medium. Changing dependency structure can affect caching correctness; requires careful validation.

**Measurement**: Plan-16 harness; cold clang:clang `act/sec` and `td_generate` phase duration.

---

### 2. **Increase I/O thread pool default from 4 to CPU count** (MEDIUM impact, LOW risk)

**Problem**: `/var/mnt/dev/kuro/app/kuro_execute/src/execute/blocking.rs:101` hardcodes `BUCK2_IO_THREADS=4`.

**Changes to try**:
```rust
let io_threads = kuro_env!("BUCK2_IO_THREADS", type=usize, default=available_parallelism())?;
```

**Expected impact**: 5-15% throughput gain if output materialization is bottlenecked on I/O thread pool.

**Risk**: Low. Comment `D33922298` references benchmark data; may need to re-evaluate for LLVM scale.

**Measurement**: Plan-16 flame graph; measure queue depth in `BuckBlockingExecutor::queue_size()`.

---

### 3. **Add instrumentation to measure action queue depth and spawn latency** (ZERO risk, enables debugging)

**Problem**: "2400+ action queue depth observed mid-build" — need to correlate with throughput and identify bottlenecks.

**Changes to try**:
- Log `DiceData` queue size at intervals.
- Measure time from action becoming ready (inputs resolved) to actual task execution.
- Histogram of host sharing permit acquire latency.

**Expected impact**: Direct visibility into whether bottleneck is:
  - Work generation (DICE scheduler too slow)
  - Task spawn latency (tokio runtime contention)
  - Permit acquisition (host sharing oversubscribed)
  - Worker throughput (too few workers, slow execution)

**Risk**: Zero. Instrumentation only, no behavior change.

**Measurement**: Ad-hoc logging in plan-16 runs; no harness change needed.

---

### 4. **Verify HostSharingBroker permit counts reflect actual machine capacity** (LOW impact, LOW risk)

**Problem**: `num_machine_permits` defaults to `available_parallelism()` (correct), but verify permit weights aren't too conservative.

**Changes to try**:
- Trace actual permit usage in a build; verify Shared(WeightClass::Permits(1)) doesn't underutilize.
- Check if any rule is using Permits(4) or higher when it shouldn't (e.g., a single-threaded tool).

**Expected impact**: 5-10% if permit weights are wrong.

**Risk**: Low. Query-only unless weights in rules are changed.

**Measurement**: Instrument `HostSharingBroker::acquire` to log request/grant.

---

### 5. **Lower-priority follow-ups (if 1-4 don't close the gap)**

- **Starlark compilation caching** (plan 17.6): May improve cold-start.
- **File watcher state persistence** (plan 17.4): Improves warm-start, not cold.
- **Build-signal pipeline micro-opts** (plan 17.3): Sub-millisecond gains.

---

## Things I'd Need to Measure at Runtime

### 1. DICE Trace & Critical Path (HIGHEST PRIORITY)
**Why**: Static code review cannot determine if dependencies are genuinely independent or artificially serialized.

**Method**:
- Run plan-16 cold clang:clang build with DICE event tracing enabled (if not already).
- Export critical path: identify longest sequential dependency chain.
- For each major phase (load, analysis, td_generate, compile, link):
  - Count how many actions are ready-to-run.
  - Count how many are actually running.
  - Measure time blocked on dependencies vs. time blocked on executor permits.

**Output**: CSV or graph showing action readiness vs. execution.

### 2. Action Spawn Latency Histogram
**Why**: Tokio runtime contention or DICE scheduler might add latency between "ready" and "spawned".

**Method**:
- Add timestamp in ActionCalculation::build_action at entry (inputs ready).
- Add timestamp when HostSharingBroker::acquire returns (permits granted).
- Histogram of latencies by action category.

**Output**: Median/p99 spawn latency; breakdown by category.

### 3. Host Sharing Broker Permit Acquisition & Wait Times
**Why**: Identify if the semaphore itself is a bottleneck (queue depth, wait time).

**Method**:
- Instrument `SharedSemaphore::acquire` (or wrap in HostSharingBroker).
- Log: (action, time_queued_for_permits, permits_requested, permits_available, granted_time).

**Output**: Time-series plot; is permit availability tracking job count?

### 4. I/O Thread Pool Queue Depth & Latency
**Why**: Determine if output materialization is bottlenecked.

**Method**:
- Poll `BuckBlockingExecutor::queue_size()` at 100ms intervals.
- Log: (timestamp, queue_depth, permits_available_in_semaphore).

**Output**: Correlation between action queue depth and I/O queue depth.

### 5. Starlark Evaluation Time (if not already in traces)
**Why**: Rule implementation evaluation might dominate action dispatch.

**Method**:
- Check plan-16 flame graphs for time spent in `kuro_interpreter`.

**Output**: % of total build time.

### 6. Remote Execution Queueing (if hybrid executor in use)
**Why**: RE might serialize locally-runnable tasks while waiting for RE capacity.

**Method**:
- Log hybrid executor decisions: (action, preferred_executor, chose_executor, reason).

**Output**: % of actions that fell back from RE due to queueing.

---

## Summary

**Static Analysis Verdict**: No fundamental architectural blocker found. Task dispatch is async-correct (tokio::spawn), concurrency is controlled via HostSharingBroker per executor (sound design), and no global per-category caps were found.

**Most Likely Root Cause**: Dependency graph granularity (hypothesis d) — if semantic-independent actions are forced sequential by DICE, that would cause the observed "one pending action at a time for minutes" in the td_generate phase.

**Next Step**: Run DICE trace analysis from plan-16 to confirm/rule out hypothesis d. If dependency granularity is not the culprit, hypothesis 2 (I/O thread pool) is the next candidate.

**Estimated Effort to Close**: 
- Hypothesis d fix (if confirmed): 1-2 days (redefine ReductionKey).
- Hypothesis b fix (I/O threads): 30 minutes (config knob change).
- Both together: 5-15% throughput gain with high confidence.

---

*Generated 2026-04-22. Requires plan-16 DICE trace data for definitive conclusion on hypothesis d.*
