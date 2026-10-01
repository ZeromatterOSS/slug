# DICE and incremental state

DICE is Slug's incremental computation graph. Use it for semantic build state
that must be cached, invalidated, replayed, or shared across requests.

This document is the long-form reference. `AGENTS.md` carries only a pointer to
it, because the locking hazard below is specialized and does not need to be in
every agent's context.

## Ownership principles

- Represent semantic inputs as DICE keys or tracked DICE dependencies. Do not
  make a value "DICE-owned" by computing it outside DICE and injecting it after
  startup.
- Key equality and hashing must include every input that can change the result:
  command policy, environment policy, file identity/digest, repo mapping,
  toolchain/platform policy, lockfile policy, and any relevant mode flags.
- Prefer explicit invalidation edges over process-global mutable state,
  singleton caches, marker-file trust, or best-effort digests.
- A warm cache hit is correct only when the key or tracked dependency graph
  explains why every Bazel-relevant input is unchanged.
- Use process globals only for non-semantic instrumentation or short-lived
  plumbing, and document why they cannot affect correctness.
- When modeling Bazel Skyframe behavior, mirror the ownership boundary:
  parse inputs, resolution outputs, repo mappings, repository specs,
  materialization state, and lockfile policy should each have auditable
  producers and dependencies rather than being bundled into an opaque session
  object.

## Live computation identity

`DiceTransaction::live_context_id()` returns an opaque, process-local
`DiceLiveContextId` for the existing engine, semantic version and live-cache
epoch. Simultaneous transactions sharing that cache have equal tokens. Dropping
the last transaction and reopening the same semantic version creates a different
token, even while old workers finish cancellation. Copying a token retains no
transaction or cache. Use it only to scope runtime plumbing such as cycle
detectors; semantic key/value equality, content identity, `DiceEquality`, and
persisted materialization namespaces keep their existing owners. Detector
binding must use the committed transaction's live identity rather than version
number alone. This getter does not install a detector or admit repository cycles.

### Request-scoped detector binding

`RequestRevisionRuntime` installs a fresh deferred detector in each request's
userdata, then binds it to the committed transaction before any computation.
Its weak registry shares one actual detector for an equal `DiceLiveContextId`.
Different engines, semantic versions and reopened live-cache epochs stay
isolated; request environment, effect tracking and observations remain in their
own userdata. A bound proxy cannot migrate to another live context. Equality-only
updaters need no binding until they become compute-capable transactions.

The generated-repository graph retains full concrete key identities and active
attempts. A finishing child releases its outgoing edges and notification state.
Incoming logical dependencies belong to active parents: DICE can request a
replacement child before the cancelled predecessor's finish callback. Preserve
that pending edge until its parent finishes, traverse it only while the target
is active, and compare exact attempt IDs when old guards drop. The legacy `.bzl`
and configured-analysis actors similarly retain referenced targets as empty
placeholders and compact unreferenced completed nodes. No lock spans a compute.

Native synchronous attempts drop transactions and drain cancellation workers
before retry, acceptance or restoration. Legacy detector actors belong to the
retained workspace Tokio runtime; request callers do not own detached cleanup
tasks. Mixed cycle errors record the existing transient poison dependency, so
an interrupted wait cannot turn a cycle diagnostic into a permanent cache hit.

## Don't hold a shared mutex across a DICE computation

The precise hazard is a lock held across a `.await` that re-enters DICE where
that same lock can be requested again. Because DICE schedules across a
multi-threaded tokio runtime, freely re-enters other keys, and has NO cycle
detection for analysis/native keys, such a re-entrant acquisition deadlocks
SILENTLY (daemon at low CPU, threads parked in `futex_wait`) rather than
erroring. Default to NOT holding any shared lock across a DICE compute; the
exceptions below are narrow.

- HARD rule: never hold a *thread-blocking* lock (`parking_lot::Mutex`,
  `std::sync::Mutex`) across a `.await` that runs (directly or transitively) a
  DICE computation — `ctx.compute(...)`, `try_compute_join`, `get_*` helpers,
  Starlark evaluation, repository-rule execution, etc. A blocking guard pins an
  OS worker thread; once the runtime can't poll the holder, you deadlock. The
  workarounds people reach for are all wrong here too: `blocking_lock` panics,
  `block_in_place` starves the runtime, and `parking_lot` hard-deadlocks.
- An *async* lock (`tokio::sync::Mutex` / `futures::lock::Mutex`) yields the
  worker instead of blocking it, so holding one across an await is not an
  automatic deadlock — but it still deadlocks if the critical section can
  re-request the same lock (self-recursion or via a dependency key), and a
  global async lock still serializes otherwise-independent DICE work and
  undermines incrementality. Treat it as a last resort, never global, and prove
  the critical section cannot re-enter the same key.
- Per-key locks are typically NON-REENTRANT. If an async path acquires a
  per-key lock and then calls a helper that re-acquires the same key's lock,
  that single computation self-deadlocks. Thread an "already-held" flag (or
  restructure) so the inner call does not re-lock.

### Correct patterns, in preference order

1. Let DICE own the serialization — two requests for the same key already
   dedupe to one computation, so you usually do not need a manual lock at all.
2. Make the serialized state its own DICE key.
3. If you must use a manual lock, scope the guard to a synchronous critical
   section that does NO `.await` and NO compute, and drop it before awaiting.
   For an atomic publish/swap step, a `parking_lot::Mutex` held only across a
   synchronous `rename()` (atomic on Linux, so readers never see a partial
   generation or an ENOENT gap) satisfies pattern 3.

### Diagnosing a suspected hang

A thread parked in `parking_lot ... lock_slow` / `futex_wait` under a
`*_compute` frame is the signature of this bug.

If `ptrace_scope=0` and the daemon is owned by your user, `gdb` works without
sudo:

```
gdb -p <slugd_pid> -batch -ex "set pagination off" -ex "thread apply all bt"
```
