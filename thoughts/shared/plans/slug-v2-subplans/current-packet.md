# Current Slug V2 Packet

Packet: `WP-6-m2-host-input-lifetime-partition-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only lifetime design for individually lazy process Host sources,
fresh eligible-conversion home observations, and request-scoped Windows path
facts.

Predecessors are authoritative: the accepted 287/8/5/41 partition, private
pure native kernel, two-context Host/repository design, option-label seam in
`b035dfbb`, exact Bazel 9.2 Host source audit, Windows option-path observation
design, and implemented producer-free `WindowsOptionPathLongName` Host/DICE
fact. The five Host descriptors remain `cpu`, `host_cpu`,
`shell_executable`, `platform_mappings`, and `default_test_resources`. Add no
Rust, probe, fixture, Host read, DICE, converter, or runtime behavior.

Read `docs/developers/dice.md` and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Recheck only the pinned
Bazel `9.2.0` / JDK owners needed to freeze the independent source contracts:

- OS and architecture token retrieval, including timing and unsupported/error
  behavior;
- `LocalHostCapacity` CPU/RAM lazy evaluation, failure, byte-to-MiB/`ceil`,
  numeric-cast, and resource-expression double boundaries; and
- `user.home`'s valid UTF-16 boundary and its per-eligible-conversion read:
  literal leading `~/`, replace-every-`~` order, fresh one-shot versus daemon
  behavior, and an explicit stop for lossy/unpaired conversion.

Design one explicit process owner—not a global/static—constructed once per
one-shot process or daemon process and shared into workspace/request runtimes.
It must retain independently lazy per-source result cells, not one atomic
snapshot: each cell records its precise source value or error and when it may
be read. Distinguish sources that permit a retained lazy cell from `user.home`,
which must produce a fresh per-eligible-conversion result rather than a reused
process cell. State how later daemon reads behave and why a workspace runtime
cannot recapture any source. Do not perform Host capture or implement the Rust
representation in this packet.

Separately freeze request-scoped option facts. A later command owner may scan
only eligible Windows-host conversions after the source-required home
expansion, demand the existing `WindowsOptionPathLongName` operation, retry
outside DICE, and project a complete raw-UTF-16-sorted/deduplicated Arc slice.
`Resolved` and `IOExceptionFallback` must remain distinct until later pure
lexical conversion; no option fact may survive into another request merely
because a daemon process survives.

Specify the smallest pure configuration-owned schema and its exact conversion
from process-source results, fresh home observations, and the request path
projection: field types, structural equality/order/hash, `Allocative`, Arc
sharing/`Dupe`, clone and retained memory cost, invalid/unsupported inputs,
and no PathBuf, OsString, workspace, IO, or DICE type. Prefer existing compact
scalar/enum/Arc-slice patterns; add no map, interner, cache, registry, second
raw-input copy, or new shared crate.

The only permitted future dependency direction is core -> configuration:
`slug_configuration_v2` owns pure schema/conversion, `slug_core_v2` owns the
process owner and bridge from workspace facts, and configuration has no
core/workspace/IO/DICE dependency. Explain how a later DICE configuration key
depends only on supplied immutable values and how no lock crosses compute or
retry. Do not design configured-target cycle semantics.

Return one bounded serial implementation sequence or `REPLAN`. Separate pure
schema, process owner, option pre-scan/retry projection, contextual conversion,
and command activation whenever their lifetimes, owners, or validation differ.
The immediate next packet must be the smallest owner that can be implemented
and validated without Host IO or activation if an exact split exists.

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

Acceptance requires pinned-source lifetime closure; a live one-shot/daemon
owner and crate-direction audit; separate process-source and request-fact
representation with exact timing/error/invalidation/retry ownership; direct
future daemon, `user.home`, and Windows-branch discriminators; bounded
implementation allowlists/caps/stops; and independent latest-text source plus
architecture review.

Stop and `REPLAN` if a source requires a global/static or atomic snapshot,
configuration IO, workspace-local recapture, lossy/unpaired Host property
acceptance, collapsed option success/fallback, stale cross-request option
facts, a new DICE producer or lock across computation, config ->
core/workspace, a new shared crate/dependency cycle, or any configured-target
edge. Configured-target dependency cycles remain explicitly deferred by user
approval.
