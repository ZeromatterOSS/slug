# Current Slug V2 Packet

Packet: `WP-6-m2-host-request-observation-projection-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only request-observation projection contract; no Rust or Host read.

## Goal

Freeze the request bridge that projects already-owned process facts into the
configuration boundary without changing lifetime, cache, or activation
semantics. The accepted `ProcessHostOwner` stays non-reading; native capture is
still **REPLAN**.

## Required design record

Specify the request pre-scan: which eligible request occurrences demand OS,
conditional CPU, capacity, fresh home, or Windows facts, and the exact order in
which it observes them. Define fresh home and Windows fact projection without
turning either into a process-global cache; preserve lossless UTF-16, terminal
errors, and the accepted Windows resolved-versus-`IOExceptionFallback` shape.

Freeze the single one-way core -> configuration dependency and ownership: core
retains the injected owner, configuration consumes only an explicit
request-scoped projection, and neither layer creates another owner. Define the
DICE contract precisely: epoch/input identity, owner and projection lifetime,
success/error caching and invalidation, failure replay or retry, and observable
ordering. The design must cite the accepted Host lifetime partition and
conversion-input schema, and explain why the bridge neither introduces a DICE
cycle nor changes the user-approved configured-target-cycle deferral.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Stop conditions

Do not edit Rust, add a Host source/capture/read, change `ProcessHostOwner`,
add a converter, configuration implementation, DICE key/compute, request
activation, command/configured-target behavior, Cargo/dependency, fixture, or
generated output. Stop and REPLAN if the contract requires a native
HotSpot-equivalent mapping, converter semantics, or an unresolved
configured-target cycle.

## Completion and next boundary

Complete only with the bounded design record and synchronized scheduling.
Native capture remains REPLAN; implementation requires a separately accepted
packet after this contract is reviewed.

## Diff budget

- Documentation: at most 160 net lines.
- No Rust or Cargo changes; no generated, fixture, baseline, or unrelated
  changes.
