# Plan 25: Remote execution against BuildBuddy

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follow-up to Plans 18 (BEP) and 22 (CLI flag compat). Plan 18.4 made
> kuro's BES upload land in BuildBuddy; this plan makes kuro's *actions*
> land on BuildBuddy's remote executor instead of running locally.

## Scope

Make `kuro build … --config=remote` (with a Bazel-shape `.bazelrc`
declaring `common --remote_executor=grpcs://remote.buildbuddy.io` and
`common --remote_header=x-buildbuddy-api-key=…`) dispatch action
execution to BuildBuddy's RBE workers, with action inputs uploaded to
the BuildBuddy CAS and outputs downloaded back. The build report should
read `Commands: N (cached: X, remote: Y, local: Z)` with `Y > 0`.

Out of scope: workflow runner integration, multi-platform exec
selection, `--strategy=…` per-mnemonic override, persistent worker over
remote, remote sandboxing options.

## Current State Analysis

Verified at the protocol layer (smoke-tested via
`cargo run -p kuro_re_configuration --example probe_re -- grpcs://remote.buildbuddy.io "x-buildbuddy-api-key=…"`):

- `KuroOssReConfiguration` populates from buckconfig section
  `kuro_re_client` (`address`, `cas_address`, `engine_address`,
  `action_cache_address`, `tls`, `http_headers`, `instance_name`, …).
- `CommonBuildConfigurationOptions::config_overrides` translates
  Bazel-shape `--remote_executor=URI`, `--remote_cache=URI`, and
  `--remote_*_header=KEY=VALUE` into the matching `kuro_re_client.*`
  buckconfig overrides (`KEY=VALUE` → `Key: Value` for the
  `HttpHeader::from_str` parser; multi-value into a comma-separated
  list).
- `prepare_uri` accepts `grpcs://` (Bazel's TLS-on alias).
- `REClientBuilder::build_and_connect` succeeds against
  `grpcs://remote.buildbuddy.io` and the BuildBuddy `Capabilities` RPC
  returns ok.

What's still wrong:

- **Daemon's `static_metadata` is bound at daemon-init from
  `root_config`.** `app/kuro_server/src/daemon/state.rs:542` calls
  `RemoteExecutionStaticMetadata::from_legacy_config(root_config)?`
  exactly once. Per-invocation buckconfig overrides (the
  `kuro_re_client.address=…` we now emit from `--remote_executor`)
  arrive at the daemon as part of the build request, *after* the
  daemon has already constructed its `ReConnectionManager`. The
  override reaches per-build configuration computation but the RE
  client itself was already built without it.

- **Action scheduler always picks Local.** Looking at
  `app/kuro_server/src/daemon/common.rs::get_command_executor`,
  the dispatch is purely a function of the rule's
  `CommandExecutorConfig.executor`:
  - `Executor::None` → no executor.
  - `Executor::Local(opts)` → `LocalExecutor`.
  - `Executor::RemoteEnabled(opts)` → hybrid / RE / local depending
    on `RemoteEnabledExecutor` variant.

  Bazel's `--remote_executor=URI` is a *global* "use RBE" toggle that
  applies to every action regardless of how its rule declared its
  executor. Kuro's per-rule configs were written for Buck2's "rules
  always declare what they want" world, where remote-execution-eligible
  actions opt in via `CommandExecutorConfig(remote_enabled=True, …)`.
  Result: `kuro build … --config=remote` against an llvm-project target
  reports `local: 8, remote: 0`.

- **`--strategy=`/`--remote_only`/`--prefer_remote` paths exist but
  error rather than promote.** The executor-factory branch for
  `Executor::Local(local)` checks `self.strategy.ban_local()` and
  returns `IncompatibleExecutorPreferences`. So `--remote_only`
  doesn't promote local-config actions — it just rejects them.

## Phases

### 25.1 Daemon constraint refresh on RE config change (DONE 2026-04-24)

**Goal.** A daemon started against a buckconfig with no RE address
restarts when the user invokes `kuro build … --remote_executor=URI`.

**Implementation.**

- `app/kuro_common/src/init.rs`: added `ReConfigSnapshot` struct
  (`address`, `cas_address`, `engine_address`, `action_cache_address`,
  `tls`, `http_headers`, `instance_name`) plus `re_config:
  ReConfigSnapshot` field on `DaemonStartupConfig` with `#[serde(default)]`.
  `from_legacy_config` reads `kuro_re_client` section so a daemon
  bootstrapped from buckconfig already carries the relevant fields.
  Snapshot equality flows through `DaemonConstraintsRequest::satisfied`'s
  startup-config check — any change forces a
  `ConstraintMismatchStartupConfig` restart for free.

- `app/kuro_client_ctx/src/common.rs`: added `cli_re_config_snapshot()`
  on `CommonBuildConfigurationOptions`. Projects `--remote_executor`,
  `--remote_cache`, `--remote_*_header`, and `--bes_instance_name` into
  a `ReConfigSnapshot`, returning `None` when no relevant flags are
  set. Both `--remote-executor` and `--remote-cache` now use
  `overrides_with` for last-wins semantics so duplicate occurrences
  (e.g. a `common` line plus a `build:remote` line in `.bazelrc`)
  resolve to the final value rather than erroring.

- `app/kuro_client_ctx/src/bazelrc.rs`: removed `remote_executor` and
  `remote_cache` from `is_bazel_specific_flag()`'s drop list. They were
  silently stripped from the args before clap parsing — now they reach
  the CLI options as intended.

- `app/kuro_client_ctx/src/streaming.rs`: in `BuckSubcommand::exec_impl`,
  if `cli_re_config_snapshot()` returns `Some(...)`, merge it onto
  `req.daemon_startup_config.re_config` before the constraint check via
  a new `merge_re_config()` helper (CLI wins per field).

- `app/kuro_server/src/daemon/state.rs`: added `apply_re_config_overlay`
  (cfg-gated `not(fbcode_build)`) that mutates the
  `KuroOssReConfiguration` inside `RemoteExecutionStaticMetadata` using
  the snapshot. A bare `--remote_executor=URL` populates every unset
  service address (CAS, engine, action cache) so the bare flag works
  without further `--remote_cache` plumbing.

**Acceptance — verified.**

- `kuro build … --remote_executor=grpcs://X` against a daemon started
  with no `kuro_re_client.address` triggers a "Startup config mismatch;
  killing daemon..." restart.
- Repeat run with the same value → daemon stays up (no spurious
  restart).
- Run with a different `--remote_executor` value → daemon restarts
  again with "Startup config mismatch".

**Est. effort (actual).** ~3 hours. The hardest piece was tracking down
why `cli_re_config_snapshot()` was returning `None` even when the flag
was on the command line — the bazelrc transitional-flag filter was
silently dropping `--remote_executor`/`--remote_cache` before clap saw
them.

---

### 25.2 Promote `Executor::Local` to remote when RE is configured (OPEN)

**Goal.** When `--remote_executor` is set (equivalently:
`static_metadata.engine_address.is_some()`), every rule's local-only
`CommandExecutorConfig` is treated as if it were
`RemoteEnabled{ executor: Hybrid, prefer_remote }`.

**Changes — option A: factory-side promotion (smaller).**

`app/kuro_server/src/daemon/common.rs::get_command_executor`:

```rust
match (&executor_config.executor, self.has_re_configured()) {
    (Executor::Local(local), true) => {
        // Promote: build a hybrid executor that prefers remote.
        let remote = self.build_remote_executor(...)?;
        Some(CommandExecutorResponse {
            executor: Arc::new(HybridExecutor::new(
                local_executor_new(local), remote, HybridPreferRemote)),
            ...
        })
    }
    (Executor::Local(local), false) => { /* existing local path */ }
    (Executor::RemoteEnabled(opts), _) => { /* existing remote path */ }
    (Executor::None, _) => None,
}
```

Pros: minimal change, scoped to one factory function.
Cons: changes the invariant that `Executor::Local` means "local only" —
existing rules that explicitly *want* local-only execution lose that
guarantee.

**Changes — option B: respect rule intent, add a strategy mode (more
correct).**

Add a new `ExecutionStrategy::RemoteWhenAvailable` (or rename
`Default` to mean this when RE is configured). Rules that want strict
local declare `local_required=True` on their `CommandExecutorConfig`,
and the factory respects that. Default rules become eligible for
remote.

Pros: matches Bazel's mental model (rule says "I'd like remote, but
I'll take what I can get").
Cons: requires touching every rule-side `CommandExecutorConfig`
producer to add the explicit local-required marker; bigger blast
radius.

**Recommended.** Start with option A behind a flag (e.g.,
`buck2.force_remote_when_configured=true` or a new
`--experimental_force_remote_when_configured`) so we can A/B against a
single workspace. Promote to default once kuro's local-required-first
rules opt back in.

**Acceptance.**

- `kuro build @llvm-project//llvm:Demangle --config=remote` reports
  `Commands: 8 (cached: ?, remote: 8, local: 0)`.
- `kuro build //:foo` (no RE config) continues to run locally.
- The BuildBuddy invocation's actions tab lists each compile action
  with `executor_kind=remote`.

**Est. effort.** 4-8 hours. The hybrid-construction path already
exists for `Executor::RemoteEnabled`; reusing it is mostly wiring.

---

### 25.3 Smoke-test CAS upload + remote dispatch end-to-end (OPEN)

After 25.1 + 25.2 land, exercise:

1. Compile a single C++ source, force `cached: 0, remote: 1, local: 0`.
2. Re-run; expect `cached: 1, remote: 0, local: 0`.
3. Inspect BuildBuddy's invocation page Actions tab, confirm:
   - Each action shows up with timing.
   - `executor_kind` reads `remote` (or `cached` on the second pass).
   - Output digests are present.

Failure modes to watch for and fix incrementally:

- CAS upload size limits (`max_total_batch_size`); kuro's existing
  `BatchUploadReqAggregator` handles batching but the limit comes from
  `Capabilities`. Should "just work" given 25.1.
- Action platform constraints: BuildBuddy may reject actions that
  don't carry an `OSFamily=Linux`/`container-image=…` exec_property.
  Plan 23 already strips these from build_settings; we'd need to
  re-add them as RE platform properties on the action message.
- TLS handshake quirks against `grpcs://remote.buildbuddy.io`. Smoke
  test (probe_re example) confirmed the basic capabilities call
  works, so the cert chain is already trusted.

**Est. effort.** 2-4 hours, mostly debugging.

---

### 25.4 Remote cache hits without remote execution (OPEN, P2)

`--remote_cache=URI --remote_executor=` (cache only, no executor) is a
common Bazel pattern. After 25.2 lands the local executor path needs
to consult the action cache *before* running locally. This is partly
covered by `Executor::RemoteEnabled` with a non-`Hybrid` variant; map
the no-executor case to `RemoteEnabledExecutor::Local(local)` with
`remote_cache_enabled=true`.

Defer until a user asks for it.

---

### 25.5 `--strategy=mnemonic=remote` per-action override (OPEN, P3)

Bazel's `--strategy=CppCompile=remote` lets users force individual
mnemonics through specific strategies. Kuro accepts the flag today
(`--strategy=MNEMONIC=STRATEGY`) but doesn't apply it. This becomes
relevant only after the global-RE path works.

Defer until Plan 25.2 is done and the global-RE behavior is proven.

---

## Dependencies and ordering

```
Plan 18 (BES upload)        — landed
Plan 22.1 (--config=NAME)   — landed
Plan 23 (module ext fix)    — landed
   │
   ▼
25.1 (daemon constraint refresh) ──► 25.2 (promote local→remote)
                                          │
                                          ▼
                                     25.3 (e2e smoke)
                                          │
                                          ▼
                                     25.4, 25.5 (deferred)
```

## Open questions

- Should we keep kuro's per-rule local/remote split or move toward
  Bazel's "every action is RBE-eligible" default? 25.2 option A
  preserves the split; option B does not. The user's preferred end
  state determines whether we ever migrate to B.

- Do we want `--remote_executor` to imply
  `--prefer_remote` automatically, or should those remain independent
  flags? Bazel's flag interaction is "if you set the executor, you
  meant it." Kuro could do the same.

- `--bes_results_url` already prints the invocation URL; should
  Plan 25 also surface a remote-execution dashboard URL once the
  invocation lands its first remote action? BuildBuddy renders this
  as part of the invocation page so probably no extra work needed.

## Success criteria

- `kuro build @llvm-project//llvm:Demangle --config=remote` reports
  `remote: 8` (not `local: 8`).
- The BuildBuddy invocation page shows action-by-action remote
  execution timings and CAS hits.
- `kuro build` without RE config remains 100% local with no
  performance regression.
