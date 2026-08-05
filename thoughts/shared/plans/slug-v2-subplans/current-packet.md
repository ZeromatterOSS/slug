# Current Slug V2 Packet

Packet: `WP-6-m2-host-input-observation-contract-design-retry`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only design for the smallest immutable process Host snapshot and
separate request-scoped option-path fact projection needed by the five
accepted Host-context native option routes.

Predecessors are authoritative: the accepted 287/8/5/41 partition, private
pure native kernel, two-context Host/repository design, option-label seam in
`b035dfbb`, exact Bazel 9.2 Host source audit, Windows option-path observation
design, and implemented producer-free `WindowsOptionPathLongName` Host/DICE
fact. The five Host descriptors remain `cpu`, `host_cpu`,
`shell_executable`, `platform_mappings`, and `default_test_resources`. Add no
Rust, probe, fixture, Host read, DICE, converter, or runtime behavior.

Read `docs/developers/dice.md` and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Recheck the pinned Bazel
`9.2.0` / JDK owners only where needed to close:

- exact process-lifetime/lazy timing of OS, architecture,
  `LocalHostCapacity` CPU/RAM, and `user.home`, including one-shot versus
  persistent server behavior and every failure/unsupported case;
- AutoCPU's finite OS/architecture token table and whether the retained input
  can losslessly collapse source spellings that produce the same observable
  token;
- CPU/RAM byte-to-MiB, `ceil`, Java numeric-cast, and resource-expression
  double boundaries; and
- valid-Unicode Rust input versus Java UTF-16 `user.home`, literal leading
  `~/` detection, replace-every-`~` order, and the explicit unsupported
  boundary for unpaired/lossy Host property conversion.

Audit every live one-shot/daemon owner that could create and share a
process-scoped snapshot. Select one explicit process owner—not a global
static—that is constructed once per one-shot process or daemon process and
shared into workspace/request runtimes. Freeze when lazy CPU/RAM capture
occurs, whether OS/architecture/home are captured atomically with it, and how
fresh-process versus later-same-process changes behave. A workspace runtime
must not independently recapture process facts, and configuration conversion
must never read process/environment state.

Freeze the smallest configuration-owned immutable schema. It must separate:

1. a process-scoped Arc-backed Host platform/resource/home snapshot; and
2. a request-scoped, complete, raw-UTF-16-sorted/deduplicated Arc projection of
   `WindowsOptionPathLongName` results whose outcome keeps `Resolved` distinct
   from `IOExceptionFallback` until the pure converter performs later lexical
   normalization.

Specify exact field types; producer/consumer direction; structural
equality/order/hash; `Allocative`; Arc sharing and `Dupe`; clone and retained
memory cost; invalid/unsupported inputs; and conversion of the workspace-owned
operational outcome into a configuration-owned DICE/PathBuf/OsString-free
fact. Prefer compact enums/scalars/Arc slices already retained in V2; add no
map, interner, cache, registry, second raw-input copy, or new shared crate.

The only permitted future dependency direction is core -> configuration:
`slug_configuration_v2` owns pure schema/conversion, `slug_core_v2` owns Host
capture and bridges existing workspace observations into complete supplied
facts, and configuration has no core/workspace/IO/DICE dependency. Explain
how a later DICE configuration key depends on the supplied immutable value and
how each one-shot/daemon request gets a fresh option-path epoch without holding
a lock across compute/retry. Do not design configured-target cycle semantics.

Return one bounded serial implementation sequence or `REPLAN`. Separate the
pure retained schema, Host producer, option pre-scan/retry bridge, contextual
conversion, and command activation whenever their owners or validation differ.
The immediate next packet must be the smallest owner that can be implemented
and validated without Host IO or activation if such a split is exact.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 520 net documentation lines and 600 total changed lines. No Rust, test,
Cargo/lockfile, dependency, fixture, oracle/probe, generated source,
filesystem/environment read, DICE key/compute/invalidation edit, Host snapshot
implementation, option scan, path converter, request/wire, CLI/server/daemon,
configuration conversion, target, loading/materialization,
command-tokenization, checksum, or downstream activation edit.

Acceptance requires pinned source closure, live process/daemon ownership and
crate-direction audits, one exact two-lifetime retained representation,
capture/invalidation/retry/error ownership, direct future one-shot/daemon and
Windows result discriminators, bounded implementation allowlists/caps/stops,
and independent latest-text source plus architecture review.

Stop and `REPLAN` if exact Host capture requires an unowned global/static,
configuration IO, a workspace-local recapture that differs from Bazel process
lifetime, lossy/unpaired Host property acceptance, collapsed path
success/fallback, stale cross-request path facts, a new DICE producer or lock
across computation, config -> core/workspace, a new shared crate/dependency
cycle, or any configured-target edge. Configured-target dependency cycles
remain explicitly deferred by user approval.
