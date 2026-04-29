# Plan 25: Remote execution against BuildBuddy

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follow-up to Plans 18 (BEP) and 22 (CLI flag compat). Plan 18.4 made
> kuro's BES upload land in BuildBuddy; this plan makes kuro's *actions*
> land on BuildBuddy's remote executor instead of running locally.

## Architectural successor

Plan 25 plumbs the *transport* (CAS upload, action dispatch, BES,
trace upload). The *exec platform selection* problem — how kuro
decides which RBE worker pool an action lands on, given a Bazel-shape
`.bazelrc` with `--platforms=…` and `--extra_execution_platforms=…` —
is solved in [**Plan 24: Constraint-Based Exec Platform
Resolution**](./24-exec-platform-resolution.md). 25.3.E
(`--remote_default_exec_properties=KEY=VALUE` plumbing) and 25.3.F
(reading the target_cfg's `PlatformInfo.exec_properties` inside
`legacy_execution_platform`) were point fixes to make the bare
`@llvm-project//llvm:Demangle --config=remote` smoke test pass; they
don't generalize to host-transitioned deps (e.g. building clang).
Plan 24 supersedes both with a constraint-based resolver that walks
`register_execution_platforms()` + `--extra_execution_platforms`
candidates the way Bazel does.

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

### 25.2 Promote `Executor::Local` to remote when RE is configured (DONE 2026-04-24)

**Goal.** When `--remote_executor` is set (equivalently:
`static_metadata.engine_address.is_some()`), the default
`CommandExecutorConfig` (returned by `get_default_executor_config` for
rules that don't specify their own platform) is treated as if it were
`RemoteEnabled{ executor: Hybrid, level: Limited }`.

**Implementation.**

- `app/kuro_re_configuration/src/lib.rs`: added
  `RemoteExecutionStaticMetadataImpl::is_re_configured()` to the trait
  and impls (both fbcode and OSS variants). Returns `true` when
  `engine_address.is_some()`.

- `app/kuro_execute/src/re/manager.rs`: added
  `ReConnectionManager::is_re_configured()` delegating to the static
  metadata.

- `app/kuro_server/src/daemon/common.rs::get_default_executor_config`:
  takes a new `re_configured: bool` argument. The previous
  `if kuro_core::is_open_source()` branch becomes
  `if !kuro_core::is_open_source() || re_configured`. OSS without RE
  configured continues to return `Executor::Local`; OSS with RE
  configured now returns the same `RemoteEnabled{Hybrid, Limited}`
  shape as the fbcode build.

- `app/kuro_server/src/ctx.rs`: at command-context construction the
  caller reads `base_context.daemon.re_client_manager.is_re_configured()`
  and passes it into `get_default_executor_config`.

**Acceptance — verified.**

- `kuro build @llvm-project//llvm:Demangle --config=remote` reports
  `Commands: 2 (cached: 0, remote: 2, local: 0)` and `Network:
  (GRPC-SESSION-ID)`. Actions reach BuildBuddy (action digests appear
  in the failure path).
- `kuro build hello_world//:main` from a workspace with no RE
  configuration reports `Commands: 4 (cached: 0, remote: 0, local: 4)`
  and `BUILD SUCCEEDED` — no regression for non-RE workflows.

**Known follow-up for 25.3.** Remote actions for the LLVM smoke test
fail with `fatal error: llvm/Demangle/Demangle.h: No such file or
directory`. Local builds succeeded because the host filesystem made
the headers visible without sandboxing. RE requires every input to be
in the action's input tree, so cc-rule transitive header propagation
(or include-path resolution under remote sandboxing) is the next
issue. Tracked in 25.3.

**Est. effort (actual).** ~1 hour. Mostly threading the
`re_configured` boolean through.

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

### 25.3 Smoke-test CAS upload + remote dispatch end-to-end (PARTIAL 2026-04-24)

**Goal.** With 25.1 + 25.2 in place, verify a real C++ compile actually
runs on BuildBuddy and lands its outputs in CAS.

**Status.** RE upload + dispatch + CAS round-trip are real (action
digests appear, BuildBuddy invocation pages populate). Most LLVM
Demangle compiles upload and dispatch correctly. One blocking issue
(documented in 25.3.A below) prevents full green: kuro's command-line
path resolution for source/header artifacts in external repos
diverges from rules_cc's `-I` flag convention, so RE workers can't
resolve `#include` lookups even though the headers are in the input
tree.

**Pre-blocker fixes shipped this session:**

#### 25.3.B Workspace re-materialization for extension repos (DONE)

`@llvm-raw//utils/bazel:configure.bzl%llvm_configure` was failing to
re-materialize after `kuro clean`. The repo's
`_overlay_directories(repository_ctx)` calls
`repository_ctx.path(Label("@llvm-raw//:WORKSPACE")).dirname` to find
its sibling repo's source tree. Kuro's `resolve_label_to_path` in
`app/kuro_interpreter_for_build/src/repository_ctx.rs` could not find
the canonical name `_main+llvm_repos_extension+llvm-raw` when the
caller asked for `llvm-raw`:

- `bazel-external/llvm-raw/` does not exist (canonical name has the
  three-segment `<owner>+<ext>+<repo>` shape).
- The directory scan only matched names matching the *first* segment
  (`<repo>+<version>`), then explicitly rejected names with more than
  two segments to avoid grabbing extension-generated spoke repos for
  bzlmod-module lookups.

Result: `Label("@llvm-raw//:WORKSPACE")` resolved to the unresolved
fallback string `llvm-raw/WORKSPACE`, which then got joined with the
working dir to give `bazel-external/llvm-project/llvm-raw/WORKSPACE`
— a path inside the repo being constructed, not the sibling. The
rule's `_extract_cmake_settings` then read from a non-existent file
and the rule failed; kuro stubbed `bazel-external/llvm-project/` as
a placeholder, which caused all subsequent loads of
`@llvm-project//llvm:...` to error with `dir does not exist`.

Fix: extend the scan to also match directory names whose **last**
`+`-segment equals the requested repo (e.g. `_main+ext+llvm-raw`
matches a request for `llvm-raw`). Matching the last segment is
unambiguous — for extension-generated spoke repos the apparent name
(what the user types) IS the last segment by canonical-name
construction. The first-segment match preserves the previous behavior
for bzlmod-module deps.

After the fix, deleting `bazel-external/llvm-project/` and rerunning
`kuro build` rematerializes it correctly via `llvm_configure`, and
local builds succeed (`Commands: 8 local, BUILD SUCCEEDED`).

#### 25.3.C `external_includes` / `system_includes` field collision (DONE)

`CcCompilationContext.external_includes` and `.system_includes` were
sharing a single backing field in `CcCompilationContextGen`
(`providers.rs`) — `get_attr("external_includes")` returned the
system_includes value. `create_compilation_context` accepted only
`system_includes` as a Rust parameter and silently dropped any
`external_includes` kwarg into `**kwargs`. Both bugs are independent
of the cc_library path that hit 25.3.A but would have produced wrong
behavior the first time a rule called `cc_common.create_compilation_context(external_includes=...)`.

Fix: split the two fields, accept `external_includes` as a named
parameter, and merge it correctly through
`merge_cc_compilation_contexts`. Also extended `create_cc_compile_action`'s
include-flag emission loop to iterate `external_includes` alongside
the existing three include kinds, so when a rules_cc upgrade *does*
populate that field, the `-I` flags emit correctly.

#### 25.3.D cc_common.compile() unwrap for tuple inputs (DONE)

The Rust `CcCommonModule::compile()` method's compile-inputs
collection only handled depsets, not `(artifact, label)` tuple
lists, and only handled depset element types, not bare list elements.
Updated to:
1. Unwrap `(artifact, label)` tuples to their first element when
   pushing into `compile_inputs` (so `cc_helper._get_public_hdrs`
   results aren't silently dropped by the actions.run input
   downcaster).
2. Iterate plain lists/tuples directly when the value isn't a depset.
3. Propagate `compilation_contexts.includes` /
   `system_includes` / `quote_includes` into `extra_flags` (was
   previously only registered globally, never emitted on the compile
   command line).

This path is currently dormant — rules_cc's cc_library compiles flow
through `_cc_internal.create_cc_compile_action` (a different Rust
entrypoint) — but other Bazel callers and future rules_cc versions
may invoke `cc_common.compile()` directly, so the fix prevents a
latent recurrence of the same bug.

#### 25.3.A Path-scheme mismatch between artifact uploads and `-I` flags (OPEN — BLOCKING)

After 25.3.B the workspace re-materializes, after 25.3.C/D the
non-rules_cc paths are sound, and after 25.1+25.2 actions actually
dispatch to BuildBuddy. The remaining failure on
`@llvm-project//llvm:Demangle --config=remote` is:

```
buck-out/v2/external_cells/extension_repo/llvm-project/llvm/lib/Demangle/Demangle.cpp:13:36:
  fatal error: llvm/Demangle/Demangle.h: No such file or directory
```

The header IS in the action's input tree. The bug is two divergent
path schemes for source artifacts in external cells:

1. `ArtifactPath::with_path` (the Starlark `artifact.path` attribute,
   used for `progress_message` and the rules_cc analytical paths)
   emits `external/<cell>/<rel>` — the Bazel-execution-time
   convention.
2. Command-line argument resolution
   (`ArtifactFs::resolve_source` →
   `BuckOutPathResolver::resolve_external_cell_source`) emits
   `buck-out/v2/external_cells/<origin>/<cell>/<rel>` — kuro's
   on-disk layout.

Locally these resolve to the same file because kuro creates a
`external/<cell> -> bazel-external/<cell>` symlink at the project
root and `bazel-external/<cell>` symlinks (or is) the buck-out
extracted location. Remotely there is no such symlink in the action
sandbox, so:

- The action's `-c <source>` argument uses path (2): GCC opens the
  source from `buck-out/v2/external_cells/.../Demangle.cpp` —
  works, the file is in the input tree at that path.
- The action's `-I` flags are emitted by rules_cc's
  `init_cc_compilation_context` using `repository_exec_path()`,
  which produces path (1): `external/llvm-project/llvm/include`.
  GCC's `#include "llvm/Demangle/Demangle.h"` lookup hits this -I
  but the file isn't at `external/llvm-project/...` in the input
  tree (it's at `buck-out/.../include/llvm/Demangle/Demangle.h`).

Two ways to fix:

- **A.** Change kuro's command-line resolution for source artifacts
  in external cells to emit `external/<cell>/<rel>` instead of
  `buck-out/v2/external_cells/<origin>/<cell>/<rel>`. The action's
  input tree builder would need to follow suit so the source is
  uploaded under the same prefix. Matches Bazel exactly.
- **B.** Add SymlinkNodes to the action's RE input tree mapping
  `external/<cell>` → `buck-out/v2/external_cells/<origin>/<cell>`.
  Lower blast radius but touches the RE input-root construction
  path, which is non-trivial.

Option A is the right long-term fix. It would also let kuro drop the
local `external/<cell> -> bazel-external/<cell>` symlink dance in
favor of the same path layout everywhere.

**Older notes (kept for context):**

`bazel-external/llvm-project` cell repeatedly stubbing out under
`kuro clean` (see "Known issues" below).

**Known issues uncovered during smoke test.**

- `llvm_configure` repository_rule's `_overlay_directories` resolves
  `repository_ctx.path(Label("@llvm-raw//:WORKSPACE")).dirname` to a
  path *inside* the llvm-project repo (`bazel-external/llvm-project/llvm-raw/...`)
  rather than the sibling llvm-raw repo, so the rule fails on
  CMakeLists.txt and kuro stubs the cell. Out of scope for Plan 25;
  needs investigation in `kuro_external_cells`/`kuro_bzlmod` repo-rule
  handling.

**Est. effort (actual so far).** ~1 hour for the cc_common fix.
Remaining smoke-test verification deferred until the cell-extraction
issue is sorted.

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
