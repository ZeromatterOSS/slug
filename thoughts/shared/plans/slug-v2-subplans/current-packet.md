# Current Slug V2 Packet

Packet: `WP-6-m2-windows-option-path-short-name-resolution-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only design for Bazel 9.2's filesystem-dependent Windows 8.3
short-name normalization during Host-context option conversion.

Predecessors are authoritative: the accepted five-Host-route context, option
label seam in `b035dfbb`, the stopped Host observation contract, pinned Bazel
tag `9.2.0`, and the live Slug lossless Windows-long-path observation support.
Add no Rust, probe, fixture, IO, or runtime behavior.

Pin exact source contracts for:

- the Windows short-path segment predicate, normalization-level promotion,
  full-input `GetLongPath` call, successful replacement, `IOException` fallback,
  UTF-16 behavior, and subsequent separator/dot/drive normalization;
- `shell_executable`'s exact `~/`-then-replace-all-`~` order before path
  creation, including a home expansion that introduces a short-name segment;
- `platform_mappings` empty input returning its default without observation,
  while nonempty path conversion and possible long-path observation precede
  absolute rejection or explicit workspace-relative classification; and
- Unix/nonmatching Windows inputs that remain purely lexical and must not
  request an observation.

Read `docs/developers/dice.md`, then audit the live
`PathObservationOperation::WindowsLongPath`, its lossless UTF-16 demand/result,
Host namespace, retained demand ownership, epoch invalidation, command retry,
one-shot/daemon lifecycle, and current normalization consumers. Determine
whether it is source-equivalent to Bazel's option-path call or requires a new
fact/edge. Never hold a lock across a DICE computation.

Select one smallest design that lets configuration conversion consume only
supplied immutable facts while retaining every observation that can change the
converted path/configuration identity. It may propose a pre-conversion scan and
an ordered input/result projection, but must not hide IO in
`slug_configuration_v2`, read the filesystem from a converter, use a global or
best-effort cache, or collapse success/fallback. Freeze exact producer,
consumer, equality/hash/order, invalidation, retry, and error ownership.

Return one bounded implementation sequence or an explicit unsupported
boundary/`REPLAN`. Keep the general Host snapshot schema, OS/CPU/RAM/home
producer, contextual converters, checksum/wire, and activation later.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 500 net documentation lines and 600 total changed lines. No Rust, test,
Cargo/lockfile, dependency, fixture, oracle/probe, generated source, filesystem
or environment read, DICE key/compute/invalidation edit, Host snapshot, path
converter, request/wire, daemon, configuration, target, loading/materialization,
command-tokenization, or downstream activation edit.

Acceptance requires pinned source closure; a live DICE/path-observation and
dependency-direction audit; exact observed-fact representation, ownership,
invalidation, retry, and one-shot/daemon discriminators; bounded implementation
allowlists/caps/validation/stops; and independent latest-text review.

Stop and `REPLAN` on unresolved UTF-16/GetLongPath/fallback behavior, a need to
run a Host probe, an observation that cannot be represented as immutable
configuration input, filesystem access from configuration, a new global/cache,
unowned invalidation, a lock across DICE compute, a new crate/dependency cycle,
or any configured-target edge. Configured-target dependency cycles remain
explicitly deferred by user approval.
