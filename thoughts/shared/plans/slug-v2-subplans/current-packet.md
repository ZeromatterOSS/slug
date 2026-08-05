# Current Slug V2 Packet

Packet: `WP-6-m2-host-input-observation-contract-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only design for the smallest supplied immutable Host observation
contract needed by the five accepted Host-context native option routes.

Predecessors are authoritative: the accepted 287/8/5/41 partition, private
pure native kernel, two-context Host/repository design, and option-label seam
in `b035dfbb`. The five Host descriptors remain `cpu`, `host_cpu`,
`shell_executable`, `platform_mappings`, and `default_test_resources`. Add no
Rust, probe, fixture, converter, or runtime behavior.

Pin exact Bazel 9.2/JDK source contracts for:

- the finite OS/CPU inputs and legacy `AutoCpuConverter` tokens used when
  `cpu` or `host_cpu` is empty;
- Host CPU and RAM observation APIs, units, rounding/ceil timing, and the
  doubles supplied to `HOST_CPUS`/`HOST_RAM` resource expressions;
- valid-Unicode `user.home`, the exact input-starts-`~/` trigger, and the
  source replace-all-`~` behavior before lexical path normalization; and
- the finite lexical Host path policy required by `shell_executable` and
  workspace-relative `platform_mappings`, without filesystem access.

Audit live Slug crate dependencies and every one-shot/daemon request assembly
path that could produce, carry, or consume the snapshot. Decide one lowest
lawful public owner and a one-way producer -> core request -> configuration
consumer handoff. `slug_configuration_v2` is currently an isolated leaf while
`slug_core_v2` owns request/DICE assembly without a configuration dependency;
the design must not assume a new reverse edge or hide Host IO in configuration.

Freeze one structural retained shape equivalent to:

```text
HostConversionInputs {
  os, cpu, host_cpus, host_ram_mb,
  host_path_policy, user_home_unicode,
}
```

Specify field types, invalid/unsupported boundaries, structural
equality/order/hash, `Allocative`, Arc sharing/clone cost, and whether capture
occurs once per process, daemon lifetime, or request. One-shot and daemon modes
must receive structurally equivalent supplied snapshots for the same observed
Host; a converter may not reread process state. `Dupe` is allowed only for an
Arc-backed wrapper, not for deep snapshot clones. Add no global, cache,
interner, map, descriptor registry, or DICE key.

Return the smallest bounded implementation sequence or `REPLAN`. Separate the
pure retained schema from any later producer/request wiring if their owners or
validation differ. Contextual native conversion remains later.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 360 net documentation lines and 420 total changed lines. No Rust, test,
Cargo/lockfile, dependency, fixture, oracle run, Host probe, generated source,
filesystem/environment read, serializer/wire, DICE, loading/materialization,
configuration conversion, target, command-tokenization, or downstream
activation edit.

Acceptance requires exact pinned source anchors, a live dependency/owner audit,
one immutable representation/capture policy, retained-cost rules, direct future
one-shot/daemon discriminators, and bounded implementation allowlists/caps/
validation/stops. Independent latest-text review is mandatory.

Stop and `REPLAN` if source cannot close the OS/CPU/RAM/home contract without
executing a Host probe; if the selected owner requires a core/configuration
cycle, new shared crate, or unapproved dependency; or if exact capture requires
filesystem/env IO, a DICE producer, daemon lifecycle mutation, configuration
conversion, repository/package loading, command tokenization, wire/checksum,
target construction, any dependency cycle, or any configured-target edge.
Configured-target dependency cycles remain explicitly deferred by user
approval.
