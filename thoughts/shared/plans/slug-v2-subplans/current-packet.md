# Current Slug V2 Packet

Packet: `WP-6-m2-process-host-owner-capture-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only exact native process Host source, error-state, and ownership
design.

## Goal

Design the core-owned `ProcessHostOwner` and its native source boundary before
any Host read or runtime activation is implemented.

## Required source closure

Read `docs/developers/dice.md` and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Recheck pinned Bazel 9.2,
its selected JDK/runtime, and launcher sources only as needed to freeze:

- OS initialization from `blaze.os`/`os.name`, architecture initialization,
  AutoCPU's OS-first/conditional-CPU order, and path flavor as a derivation of
  the same OS state;
- the native Rust observation corresponding to each supported default property,
  every override/mutation boundary, and first-initialization versus
  erroneous-class reuse diagnostics;
- `LocalHostResource` RAM/CPU acquisition order, MiB division, `ceil`, Java
  `int` narrowing, successful `LocalHostCapacity` memoization, retryable
  pre-assignment failures, and permanent source-class failure; and
- the process default for `user.home`, its fresh read on every eligible
  conversion, valid UTF-16 transport, missing/read failure, unpaired-surrogate
  `Unsupported`, and a deterministic injectable test source.

## Required ownership design

Select exact field/state types for one non-global `ProcessHostOwner` in core.
Each one-shot core wrapper creates one owner before its `WorkspaceRuntime`;
`Daemon::new` creates the sole daemon owner and `serve` creates no second one.
`WorkspaceRuntime` receives an Arc. Preserve independent OS/CPU class state,
OS-derived path flavor, capacity's success-only memoization and source-class
state, and uncached home reads. Specify synchronization, poisoning/failure
behavior, clone and retained-memory cost, and why no guard crosses DICE compute
or retry. Configuration remains a pure consumer of the already accepted
`HostConversionInputs` and has no Host/source type.

Audit every live constructor/caller affected by explicit owner injection and
freeze direct future one-shot/daemon tests for first use, conditional CPU,
success caching, retryable/permanent failures, fresh mutable home, valid and
unsupported Unicode, and owner isolation. Add no request path scanning or
Windows epoch bridge in this packet.

## Preconditions

The accepted lifetime partition, producer-free configuration schema, and
existing Windows option-path observation primitive are authoritative. Return a
bounded serial implementation packet or `REPLAN`; prefer a producer/state
packet separate from real native capture if that is the smallest exact split.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Stop conditions

Do not add Rust, tests, Host I/O, environment/process access, lazy cells,
dependencies, Cargo changes, DICE, request scanning, workspace outcomes,
Windows projection, option conversion, command/configured-target activation,
fixtures, probes, or generated artifacts. Stop and `REPLAN` on any required
JVM/Java production dependency, unowned global/static, eager/atomic snapshot,
lossy property conversion, workspace-local recapture, configuration I/O,
lock across DICE, reverse config dependency/new cycle, or configured-target
edge. Configured-target cycles remain explicitly deferred by user approval.

## Validation

Validate pinned-source closure, live owner/caller coverage, exact state/error
semantics, bounded successor allowlists/caps/stops, three-file scope, archive,
and `git diff --check`. Require independent latest-text source and architecture
review.

## Completion

Complete only when every supported source has exact capture and failure-state
ownership or an explicit unsupported boundary, and the immediate implementation
packet cannot accidentally force unused sources or change daemon lifetime.

## Diff budget

- Documentation: at most 520 net lines and 600 total changed lines.
- No Rust, test, Cargo, dependency, fixture, generated, baseline, or unrelated
  changes.
