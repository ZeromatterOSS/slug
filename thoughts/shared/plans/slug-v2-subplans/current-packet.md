# Current Slug V2 Packet

Packet: `WP-6-m2-process-host-owner-state-and-injection`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: core-only ProcessHost owner state machine, unsupported native placeholder,
and explicit runtime injection; no Host read.

## Goal

Implement the accepted `ProcessHostSource`/`ProcessHostOwner` state and inject
one owner before each production runtime. The native backend must return typed
`Unsupported` without reading the Host.

## Required implementation

In core, define a test-injectable source with lossless UTF-16 property results
`Present(Arc<[u16]>)`, `Absent`, and `ReadError`, plus signed Java-long memory
bytes, processor count, and an after-resource capacity-completion hook so tests
can distinguish retryable pre-assignment failure from resource-class failure.
Model independent OS/CPU `ClassCell`s with distinct `InitialFailure` and later
`ErroneousReuse` access errors, OS-derived `HostPathFlavor`, OS-first/conditional-CPU AutoCPU,
and a RAM-then-CPU `LocalHostResource` class state. Model `LocalHostCapacity`
as successful-resource-value-only: its source-class initialization reads
signed-long RAM bytes, immediately divides to a memory-MiB `double`, then reads
processors. A retryable pre-assignment failure remains unassigned; the
resource class's erroneous state replays its terminal error. Derive the
route-specific CPU/RAM keyword i32 values only afterward with exact `ceil` and
Java `double`-to-`int` narrowing. Read home
fresh for each eligible occurrence from the source; never cache it.

Freeze `ClassCellState<T>` as `Vacant | Initializing { thread } | Ready(T) |
Failed(Arc<ClassInitFailure>)`; the capacity cell restores `Vacant` only for a
retryable pre-assignment failure. The initializing caller returns
`InitialFailure`; subsequent access returns `ErroneousReuse`. Reject
same-thread reentry with a typed internal error rather than Condvar-waiting on
itself; the private source trait must not reenter its owner. Keep the source
trait and raw OS/CPU/resource types core-private. Only
`Arc<ProcessHostOwner>` and its public non-reading constructor cross into
runtime/server ownership; later configuration mapping is out of scope. Clone
shares that one Arc, and `WorkspaceRuntime` remains non-`Clone`.

Use short `Mutex`/`Condvar` transitions only. Release every lock before a
source call, DICE compute, or retry; a condition wait must atomically release
the owner-local guard while blocked. The native implementation is a non-reading
placeholder returning `Unsupported` for all demands; no Rust environment,
filesystem, cgroup, processor, capacity, or home API is allowed.
Mutex poison or source unwind must return a typed owner failure, notify any
waiter, and never leave a cell stuck in `Initializing`.

Create one `Arc<ProcessHostOwner>` at each of the six one-shot production sites
(four `runtime/mod.rs`, two `runtime/dice.rs`) before `WorkspaceRuntime`.
`Daemon::new` is the sole daemon construction site; `serve` is unchanged.
`WorkspaceRuntime` receives the Arc and never recaptures sources.

## Required tests

Cover class first-use/error and erroneous reuse, OS-before-conditional-CPU,
RAM-before-CPU, successful capacity reuse versus retryable/permanent failure,
fresh mutable home, lossless present/absent/read-error UTF-16, unsupported
native placeholder non-reading behavior, distinct owner isolation, injected
runtime Arc identity, distinct initial-failure/erroneous-reuse errors,
same-thread reentry rejection, exact bytes-to-MiB timing and
ceil/Java-narrowing boundaries,
and fail-closed poison/unwind without a stranded waiter. In server tests, use a
`#[cfg(test)]` retained copy of the Arc created by normal `Daemon::new` and its
strong count plus the core runtime identity test to prove one shared daemon
owner; do not expose the private source trait or add another constructor.
Tests must not attempt native Host capture.

## Allowed paths

- `app/slug_core_v2/src/runtime/process_host.rs`
- `app/slug_core_v2/src/runtime/mod.rs`
- `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_server_v2/src/lib.rs`
- inline tests in `app/slug_core_v2/src/runtime/process_host.rs` or
  `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_core_v2/tests/runtime.rs`
- `app/slug_server_v2/src/tests.rs`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Stop conditions

Do not implement native capture, property/environment reads, home lookup,
`sysconf`, procfs/cgroup access, OS/CPU/capacity approximation, config
dependency, Cargo change, DICE key/compute change, request scanning, Windows
projection, converter, command/configured-target activation, fixture, or
generated output. Stop and REPLAN if exact native/HotSpot behavior is needed
for this packet, a lock crosses source/DICE work, another daemon owner appears,
or any configured-target edge is required.

## Validation

Run focused core/server state and runtime tests, affected crate tests/checks,
the applicable GNU-Windows no-run checks, formatting, archive status, scope,
cap, no-Cargo, and `git diff --check`. Do not run daemon smoke, oracle, Bazel,
or configured-target tests.

## Completion and next boundary

Complete only with the state/injection shape and an unambiguously non-reading
unsupported native placeholder. Native capture remains REPLAN until a
HotSpot-equivalent mapping is proven. The later request bridge may add the
one-way core -> configuration dependency; configured-target cycle deferral
remains in force.

## Diff budget

- Production Rust: at most 600 net lines.
- Test Rust: at most 620 net lines.
- Documentation: at most 160 net lines.
- Total net change: at most 1,380 lines; no Cargo, dependency, fixture,
  generated, baseline, or unrelated changes.
