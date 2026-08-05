# Current Slug V2 Packet

Packet: `WP-6-m2-windows-option-path-long-name-observation-primitive`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: a producer-free Host/DICE observation primitive that preserves the
pre-lexical Windows option-path long-name success/fallback branch.

Predecessors are authoritative: pinned Bazel tag `9.2.0`, the accepted Windows
option-path short-name resolution design, and the existing lossless
`WindowsLongPath` Host observation. Read
`.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing retained
representation. Reuse the existing Arc slices, sorted epoch, exact injected
key, transient `Need`, outside-DICE observer, and retry model. Add no option
producer, converter, command activation, or Host read beyond the demanded
native operation.

Add one option-specific operation and exact outcome equivalent to:

```text
WindowsOptionPathLongNameOutcome =
    Resolved(Arc<[u16]>)
  | IOExceptionFallback
```

The outcome is public, immutable, structurally `Eq`/`Ord`/`Hash`,
`Allocative`, and cheaply cloned only through its Arc payload. `Resolved`
retains the exact UTF-16 returned by the existing Bazel-equivalent native
resolver after extended-prefix removal and backslash-to-slash conversion but
before separator/dot/drive lexical normalization. Every resolver failure is
the distinct payload-free `IOExceptionFallback`; do not retain an OS error or
turn it into a configuration error. `Resolved(raw-equivalent)` and fallback
remain unequal.

Add a dedicated `PathObservationOperation`/`PathObservationResult` route and
dedicated demand constructor. The demand is always in the Host namespace and
structurally retains the caller-supplied normalized-absolute observation
identity plus the exact complete, expanded, non-normalized raw UTF-16 input.
Generic construction must reject both UTF-16 operations. The operation kind,
identity path, and raw code units all participate in demand `Eq`/`Ord`/`Hash`;
the new operation cannot collide with existing `WindowsLongPath` for the same
path/input. Share the raw Arc field if clean; add no second copy, map, cache,
interner, global, serializer, or wire form.

The core observer must call the existing raw long-path resolution helper once.
On success it returns the pre-lexical transformed UTF-16 under `Resolved`; on
any ineligible/native sizing/fill failure it returns `IOExceptionFallback`.
The Unix adapter returns fallback without filesystem access only as defensive
primitive behavior; normal option conversion must never create this demand
outside Windows host policy. Keep the existing `WindowsLongPath` operation's
final lexical-normalization behavior and all of its consumers unchanged.
Exhaustive-match edits outside workspace/core may only reject the new result
as impossible on their existing routes.

Tests must discriminate:

- Host-only demand identity, exact raw UTF-16 including unpaired surrogates,
  raw slash/backslash spelling, identity path, and operation-kind differences;
- result/operation mismatch and duplicate-demand rejection;
- `Resolved` versus fallback inequality even when later lexical normalization
  would agree, plus exact pre-lexical returned spelling;
- one native sizing/fill success and the existing ineligible, zero-size,
  oversized, zero/overflow/unterminated-fill fallback families without a Host
  probe;
- transient missing `Need`, exact complete result, and A -> B -> A DICE replay
  for resolved/fallback/payload changes; and
- unchanged existing `WindowsLongPath` normalization and repository consumers.

Allowlist:

- `app/slug_workspace_v2/src/lib.rs`
- `app/slug_workspace_v2/src/path_observation.rs`
- `app/slug_workspace_v2/src/path_resolution.rs` only if its exhaustive result
  rejection requires the new variant
- `app/slug_core_v2/src/runtime/path_observation.rs`
- `app/slug_core_v2/src/runtime/repository_io.rs` only for validation and test
  exhaustive matches
- `app/slug_bzlmod_v2/src/host_file.rs` only for an impossible result arm
- `app/slug_bzlmod_v2/src/repository_ignore.rs` only for an impossible result
  arm
- `app/slug_bzlmod_v2/src/source_preparation.rs` only for impossible result
  arms
- terminal scheduling updates in
  `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 340 production lines, 420 test lines, and 760 total net lines. No Cargo
or lockfile edit, dependency, new crate, fixture, oracle/probe, option scan,
home/OS/CPU/RAM capture, configuration schema/converter, request/wire, CLI,
server, daemon lifecycle, checksum, target, loading/materialization behavior,
or configured-target edge.

Validation: focused new workspace/core unit and DICE tests; full
`cargo test -p slug_workspace_v2` and `cargo test -p slug_core_v2`; direct
`cargo check -p slug_bzlmod_v2`; GNU-Windows no-run for workspace/core/bzlmod;
formatting, diff, exact allowlist/cap, no-Cargo, archive, and existing
`WindowsLongPath` semantic guards. Independent retained-representation/DICE
and source-equivalence latest-diff reviews are mandatory.

Stop and `REPLAN` on any change to existing `WindowsLongPath` observable
semantics, collapsed success/fallback, lossy UTF-16, lexical normalization in
the new producer, direct IO inside DICE, a lock across compute/await, unowned
epoch invalidation, a new global/cache/interner/map, dependency/Cargo change,
Host snapshot or option/configuration/request activation, new crate/cycle, or
any configured-target edge. Configured-target dependency cycles remain
explicitly deferred by user approval.
