# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-repository-rule-host-capability-implementation`

Milestone: M7A category 6 generated-repository prerequisite.

Base: accepted effective Host-input implementation commit `64878a1be` and
accepted innate owner certificate commit `7bcac3da3`, with the dirty selected-
context R2 candidate retained unchanged. Both predecessor packets are
terminally `ACCEPT` after independent correction review.

## Observable result

Generic authenticated BCR `repository_rule` implementations can consume the
accepted command-scoped repository platform/environment inputs through
`repository_ctx.os`, `repository_ctx.getenv`, and staged
`repository_ctx.file`. Declared and dynamically discovered names become
per-name DICE dependencies through typed retries, and the real pinned
`winsdk_configure.bzl` non-Windows branch realizes its exact two files.

This packet does not add a ruleset-specific repository implementation, Windows
SDK/path discovery, another repository-context capability, registration-row
proof, selected configured context, action, or REAPI behavior.

## Authority, learned facts, and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority. `RepositoryFetchFunction` requests every declared
`repository_rule(environ=...)` name before invocation;
`RepoEnvironmentFunction` owns one dependency per variable and preserves
absent separately from empty; `StarlarkRepositoryContext` records dynamic
`getenv`; `StarlarkOS` supplies `name`, `arch`, and the full effective
environment view; and `DigestWriter` retains OS/architecture plus declared
environment identity even when output bytes agree. Pinned
`external_integration_test.sh` supplies declared/dynamic change,
unrelated-name, override, and stable-command invalidation evidence.

The authenticated pinned BCR `tools/res/winsdk_configure.bzl` is the exact
non-Windows observable: it declares MSVC environment names, observes
`repository_ctx.os.name`, and writes empty `BUILD` plus `toolchains.bzl` whose
registration function body is `pass`. An actual Windows Host requires
unadmitted path/SDK/executable discovery and must fail before publication.

`docs/developers/dice.md`, Buck2-derived
`dice/dice_tests/src/linear_recompute.rs`,
`dice/dice/src/impls/tests/user_data.rs`, and
`dice/dice/src/transaction_update.rs` govern dependency recording, equality
cutoff, injected values, transaction data, retry, cancellation, and the
no-lock-across-compute rule.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only: provider-supplied OS values, immutable
environment views, dynamic-name recording, and staged/discardable effects are
useful separation ideas. No Zig code, layout, store, scheduler, or
compatibility claim is copied.

- **Exact:** on an admitted non-Windows Host, sorted declared views;
  absent/empty; declared and dynamic per-name invalidation; unrelated-name
  non-invalidation; `repository_ctx.os`/`getenv` access shape and values;
  canonical defining-label/export authentication; existing file path/mode
  semantics; and exact pinned non-Windows winsdk output bytes.
- **Slug-native:** Rust Host OS/architecture spelling through the exact
  Starlark fields; structural DICE/effect identity; typed monotone Host retry;
  staged effect publication; internal authorization state; and existing Busy
  command overlap.
- **Unsupported/deferred:** Windows repository execution and SDK/path/
  executable discovery; strict/action repository-environment interaction;
  bazelrc inputs; non-Unicode and Windows environment/path edge behavior;
  repository-context capabilities beyond OS/environment/file; `local` and
  `configure` scheduling policy; exact Bazel marker bytes; and selected-context
  closure.

BCR Starlark owns every rule and control-flow decision, including
`cc_internal`; `cc_common` is only a generic Host/provider ABI client. This is
not a `set` or C++ parser packet.

No new oracle fixture is authorized. Existing authenticated BCR sources and
accepted pinned Bazel tests are sufficient. Strict/action-environment and
Windows upstream variants are skipped because those surfaces are explicitly
unsupported. There is no fallback or deletion ledger.

## Natural owners and implementation

### Canonical definition and context

Add `app/slug_loading_v2/src/repository_rule_context.rs`. It owns only the
invocation-scratch repository context values/capabilities and the compact
sorted observed-Host projection used by the completed effect. Reuse the
adopted Rust/Buck2 Starlark evaluator and the existing canonical source/load
route; do not parse rule implementations or add a second loader.

The defining label routes through the existing Root/Canonical external `.bzl`
owner. Canonical definitions consume the accepted explicit ordinary/innate owner
certificate and its authenticated actual `RepoRuleId`; they use the existing
canonical load and module-evaluation keys without reconstructing the
synthetic `//:MODULE.bazel` identity. `bzl_module.rs`, despite exceeding
10,000 lines, may expose only a thin reuse boundary for that existing route; it
gains no loader, cache, semantic side table, or repository execution owner.

The context exposes:

- `repository_ctx.os.name` and `.arch` from `RepositoryPlatformKey`;
- `repository_ctx.os.environ` from the immutable full request snapshot;
- `repository_ctx.getenv(name, default=None)` from that snapshot while
  recording the dynamic name; and
- the existing staged `repository_ctx.file` capability with unchanged path and
  mode semantics.

Evaluator values and the dynamic-name recorder are invocation scratch and may
not escape. The completed effect retains only platform plus every declared and
dynamically recorded absent-or-value observation, sorted canonically. It never
retains or depends on the unobserved remainder of the full environment.

### DICE effect owner and typed retry

`module_extension_repository_file_effect.rs` remains the sole async DICE owner
of definition authentication, Host-key computation, synchronous invocation,
staged-plan discard/publication, dependency validation, and completed effect
equality. It delegates context construction and capabilities to the new
module.

Before invocation, compare authenticated declaration `environ` names with the
transaction frontier. Batch every unknown name into one typed environment
Need, without constructing a context or invoking Starlark. On retry, compute
the platform and declared per-name keys in sorted order. Only
`Observed(value)` that matches the same transaction snapshot is admissible;
`Unauthorized` or any frontier/cell/snapshot mismatch fails closed.

After synchronous evaluation, compare all recorded dynamic names with the
same frontier. If any are unknown, discard the staged plan and return one
batched typed Need. On the successful replay, compute every recorded dynamic
per-name key and verify the same snapshot values before publication. Any
definition, projection, Host input, dependency, invocation, effect,
cancellation, or mismatch failure discards the plan. Repeated equal Needs are
existing environment non-progress; no repository/path Need may be fabricated.

Complete effect equality includes the compact observed-Host projection beside
the file plan, so a relevant Host change invalidates even when generated bytes
agree. An unrelated name is not a dependency and retains the cached effect.
The packet-1 core lifecycle remains the sole production injector and restores
rejected/cancelled extra cells to `Unauthorized`; loading test code may inject
the shared lower keys directly to prove that completed effects cannot warm-
reuse across those transitions.

The existing command lease/Busy boundary remains before allocation. No mutex
may be held across DICE compute/await, Starlark invocation, retry, effect,
cancellation, or publication. Injected keys/effects are DICE-retained semantic
memory; the accepted request snapshot/frontier remain core session-retained;
context values, recorded names, and staged file plans are command/phase
scratch until publication or discard. Workspace shutdown releases retained
state; no new eviction policy, interner, registry, or mutable cache is added.

### Non-Windows winsdk realization

Execute the exact selected BCR `winsdk_configure.bzl` through the generic path.
On non-Windows, publish exactly empty `BUILD` and exact `toolchains.bzl` with
`register_local_rc_exe_toolchains(): pass`. Do not special-case
`local_config_winsdk`. A forced Windows platform fails before any file plan is
published.

## Exact allowlist, blobs, and caps

Only these existing files may change, at their exact current/base blobs:

- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`
  `7396e2a80e2079be695f860af8b077d415bd7c3c`;
- `app/slug_loading_v2/src/bzl_module.rs`
  `8309f65c379a12e66fcd53eccfc49cd9f53cb889`; and
- `app/slug_loading_v2/src/lib.rs`
  `2f614c604b52456943d5353c84cefc486804f9ed`.

The only new production file is
`app/slug_loading_v2/src/repository_rule_context.rs`. The only new separate
proof file is
`app/slug_loading_v2/tests/repository_rule_host_capabilities.rs`.

Maximum additions: 650 production Rust, 900 proof Rust and 1,550 aggregate
Rust lines. No Cargo manifest/lockfile, asset, fixture, command, server, core,
Bzlmod, analysis, selected-context, action, REAPI, or other loading file may
change. Preserve the dirty selected-context candidate exactly and stage/commit
only packet deltas.

`bzl_module.rs` receives only a thin canonical-route reuse surface. The effect
file remains cohesive as the existing async DICE/effect owner; context
construction, Starlark capability methods, dynamic recording and retained
Host projection belong in the new module. No hot-path benchmark is required:
repository-rule invocation is not a demonstrated hot path, and all retained
collections are bounded sorted immutable projections using the accepted
`CompactString`/`Arc` representation.

## Discriminating proof and validation

The new proof must cover:

- root and canonical defining labels plus projection mismatch;
- declared-present direct dependency and declared-cold-absent Need before any
  invocation, followed by successful retry;
- sorted declared view, present/absent/empty, OS name/architecture, and full
  `os.environ`;
- dynamic `getenv` with and without a default; cold dynamic-absent Need with
  staged-plan discard and authenticated retry; warm no-retry; and dynamic
  dependency replay;
- missing-to-present-to-missing A/B/A, unrelated-name cache retention, and
  retained observed-input inequality even when file bytes agree;
- staged-plan discard on every failure/cancellation;
- completed rejected/cancelled effects cannot warm-reuse after both extra-
  absent and extra-present cells restore to `Unauthorized`, using direct
  shared-key injection only in loading test code;
- unchanged file path/mode behavior and exact non-Windows pinned winsdk bytes;
  and
- forced Windows failure before publication.

Run focused context/effect tests, the full serial `slug_loading_v2` suite, and
direct `slug_core_v2` and `slug_bzlmod_v2` dependents. Then run
`cargo fmt --all`, `git diff --check`, exact blob/scope/cap/dirty-isolation
audits and `scripts/v2_archive_status.sh`. Do not run Cargo commands in
parallel on the shared target directory; clean stale `slugd` around daemon-
sensitive validation if any is added.

## Stops and successor

`REPLAN` for a whole-map/frontier dependency; ambient environment read;
daemon fallback; hidden evaluator semantic state; lock across compute/await;
new loader, fallback scan or duplicate innate-owner authentication; ruleset/winsdk special case; unknown-name
publication without typed retry; repeated Need without non-progress;
repository/path Need fabricated for environment; foreign workspace or
`Unauthorized` semantic value; staged plan surviving retry/failure/
cancellation; Host identity omitted from effect equality; Windows realization;
exact marker-byte claim; new Cargo dependency; change outside allowlist/caps;
or inability to isolate the dirty selected-context candidate.

After terminal acceptance, activate only
`WP-4-5-7A-registered-toolchain-generated-repository-proof` under the frozen
proof-only four-file allowlist and 900-line cap. Only after that packet passes
may the retained selected-context R2 candidate return to terminal review.
