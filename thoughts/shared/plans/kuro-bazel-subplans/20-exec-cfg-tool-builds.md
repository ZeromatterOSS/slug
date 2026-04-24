# Plan 20: Exec-cfg tool builds — unblock the td_generate win

> **Status:** COMPLETE (2026-04-23). Phase 20.1 + 20.2 landed in
> `68cb2097` and `41ce00a5`; 20.3 final benchmark `8a286bb6`.
> Cold `@llvm-project//clang:clang` wall: **1107.6 s** vs Plan 19's
> 1416.1 s baseline and the 1200 s plan target — target met.
> Full write-up: `benchmarks/post-plan-20-final-41ce00a5/FINDINGS.md`.

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Prerequisites: Plan 19.1–19.6 (complete). Plan 19 delivered the
> per-cfg `build_settings` plumbing and the exec_properties → exec-cfg
> wiring but the measured td_generate critical-path win was blocked by
> two independent failures that surfaced only under real integration.

## Scope

Close the two gaps that prevent exec-configured tool builds (e.g.
llvm-tblgen under `genrule.tools`) from being compiled with the
platform's opt defaults declared in Plan 19.3. Until this plan lands,
`benchmarks/post-plan-19-final-ac51eb9/FINDINGS.md` shows wall-clock at
parity with the Plan 17.2 baseline (1416 s) because tools still compile
in fastbuild mode.

**Concrete goal.** After this plan, `kuro build
@llvm-project//clang:clang` cold should produce llvm-tblgen compiled
with `-O2 -g0 -DNDEBUG -ffunction-sections -fdata-sections` and the
td_generate critical-path entries should drop from ~275 s to ~30 s
each, reaching the ≤ 1200 s wall target that Plan 19 originally
predicted.

Out of scope: changing exec-platform resolution (Plan 11), altering
the transition engine (Plan 19.2), or adding new CLI flags. All of
that is intact; the work here is unblocking the integration.

## Current State Analysis

### Blocker 1: `genrule.tools` stays at target-cfg

`app/kuro_interpreter_for_build/src/interpreter/native_rules.rs:451-466`
declares `tools` as `AttrType::dep` with a comment explaining the
temporary hold. Bazel declares it `cfg="exec"`, which is what makes
tools compile on the build host regardless of target platform and what
activates the exec_properties `compilation_mode = "opt"` default from
Plan 19.3.

Attempting to flip to `AttrType::exec_dep` during Plan 19.6 produced a
runtime failure on a clean rebuild:

    buck-out/v2/external_cells/extension_repo/llvm-project/llvm/lib/Support/SipHash.cpp:15:10:
    fatal error: siphash/SipHash.h: No such file or directory
       15 | #include "siphash/SipHash.h"

The compile command's include list contained
`-Ibuck-out/v2/gen/llvm-project/<cfg>/external/llvm-project/third-party/siphash/_virtual_includes/siphash`,
but that directory was empty. `siphash` is a plain cc_library with
`include_prefix = "siphash"` on its headers — rules_cc emits the
virtual-includes symlink tree into the *producing* target's
configuration directory.

The hypothesis: switching `tools` to `exec_dep` reshapes which cfg
downstream targets observe, and somewhere a consumer ends up looking
for the virtual_includes dir in a cfg that didn't run the
`_virtual_includes`-producing action. That may be the consumer's own
exec-cfg transition, or it may be a dep graph re-configuration
triggered by the `ExecutionPlatform` cfg-hash changing (Plan 19.3 added
build_settings to its cfg).

### Blocker 2: `--compilation_mode=opt` rules_cc analysis regression

`kuro build --compilation_mode=opt @llvm-project//llvm:Support` fails
with:

    Error running analysis for `llvm-project//llvm:Demangle (...)`
    Caused by:
      0: Action category `c_compile` contains duplicate identifier
         `external/llvm-project/llvm/lib/Demangle/DLangDemangle.cpp`;
         category-identifier pairs must be unique within a rule

The same build under fastbuild (default) succeeds. Something in the
rules_cc analysis path branches on active features and double-registers
a compile action when "opt" is enabled but not under fastbuild.

Plan 19.6 removed `"opt"`, `"dbg"`, and `"fastbuild"` from
`FeatureConfiguration::default_cc_features` (see
`app/kuro_build_api/src/interpreter/rule_defs/cc_common/feature_config.rs`
`default_cc_features()`). Before that change, all three were always
active, and the rules_cc branch that picks actions likely short-circuited
on feature presence. After the change, only the cfg-requested mode is
active; that's correct Bazel semantics but exposes the double-register.

Kuro has two native compile-action entry points that rules_cc can
reach:

- `cc_common_internal.create_cc_compile_action`
  (`actions.rs:167`) — the one rules_cc's Starlark-internal path uses.
  Registers via `actions.run(executable, mnemonic, …)`.
- `cc_common.create_compile_action` (`actions.rs:3432`) — a simpler
  public wrapper.

If rules_cc's opt-mode path calls both for the same source (say
because an "opt" feature implies a second flag-propagating action in
the Starlark-level rules_cc code that kuro's native stub translates to
a real duplicate), the "duplicate identifier" error triggers. The
actual rules_cc path needs tracing.

### What's already wired (don't redo)

- Plan 19.3: `@local_config_platform//:host` declares
  `exec_properties = {"@bazel_tools//tools/cpp:compilation_mode":
  "opt"}` via `app/kuro_external_cells_bundled/build.rs`. The
  ExecutionPlatform's `ConfigurationData.build_settings` carries it.
- Plan 19.5: `ctx.fragments.cpp.compilation_mode` and
  `ctx.var["COMPILATION_MODE"]` read from the analyzing target's cfg.
  An exec-cfg analysis will see "opt" here as soon as exec_dep
  routing works.
- Plan 19.6: `create_cc_compile_action` emits the opt flag set when
  the feature_configuration says "opt". Verified in unit tests
  (`feature_config.rs tests`). The flags will reach the compile
  command automatically once 20.1 lets exec-cfg traversal actually
  reach the tool compile.

## Desired End State

- `genrule.tools` = `exec_dep`. Tools resolve under the exec platform's
  cfg. Compile commands for llvm-tblgen sources include `-O2 -g0
  -DNDEBUG -ffunction-sections -fdata-sections`.
- `kuro build --compilation_mode=opt @llvm-project//llvm:Support`
  succeeds. Every c_compile action is registered exactly once, the
  command line carries the opt flag set.
- Cold `@llvm-project//clang:clang` wall ≤ 1200 s. td_generate
  critical-path per-action ~30 s. clang:clang link still dominates
  critical path tail, matching Bazel's profile.

## Phases

### 20.1 Exec-cfg traversal of `genrule.tools` (OPEN, blocks win)

Flip `genrule.tools` to `AttrType::exec_dep` and resolve the
`siphash/SipHash.h` path gap.

**Investigation first.** Before changing the attribute type, run the
failing build under a fresh daemon with `kuro --verbose audit providers
@llvm-project//llvm:Support` to record the exact cfg hashes of
`llvm:Support`, `siphash`, and anything in-between. Compare the
pre-flip and post-flip cfg hashes. The bug probably lives in one of:

1. `AttrConfigurationContextImpl::configure_target` vs
   `configure_exec_target` (`app/kuro_node/src/attrs/configuration_context.rs`).
   A target reached via an exec-dep edge should keep subsequent
   target-cfg edges at *its own* target cfg (i.e. the exec cfg), not
   flip back to the original top-level target cfg. Confirm that path.
2. Virtual-includes dir synthesis in rules_cc's
   `cc_common.create_compilation_context` / `cc_common.compile`. The
   include path is computed from the compiling target's cfg; check
   whether that matches the producing cfg.
3. rules_cc's cc_library providing virtual_includes via a
   `cc_info.compilation_context.includes` depset that is propagated
   **as strings, not artifacts**. If the strings were baked in under
   one cfg and consumed under another, the -I flag is stale.

**Tasks.**

1. Repro under verbose logging. Flip `tools` to `exec_dep`. Capture
   the cfg hash of siphash in the failing build vs the cfg hash baked
   into the `-Iexternal/.../_virtual_includes/siphash` flag.
2. If hashes differ: the include-path synthesis needs to pull from the
   dep's own cfg, not the consuming target's. Fix in
   `include_flag_for_dir_impl` or wherever the `<cfg>` component is
   interpolated (likely
   `app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs`
   around `_virtual_includes` synthesis).
3. If hashes match but the dir is empty: the producing action never
   ran under the dep's cfg. Likely a depset-traversal bug where
   `exec_dep` paths don't register transitive compile actions. Fix in
   the analysis path that collects outputs from cc_library deps.

**Success criteria.** `kuro build @llvm-project//llvm:Support`
succeeds with `genrule.tools` = `exec_dep`. `kuro log what-ran` on a
td_generate action shows `/usr/bin/gcc … -O2 -DNDEBUG …` for the
llvm-tblgen compile.

### 20.2 `--compilation_mode=opt` rules_cc compile-action dedup (OPEN)

Find why opt-mode triggers a duplicate `c_compile` identifier for the
same source file and fix the double-registration.

**Investigation first.** The failing source file (`DLangDemangle.cpp`)
is registered via two different analysis paths when opt is active. One
approach:

1. Add a `tracing::debug!` in
   `create_cc_compile_action` (`actions.rs:167`) and
   `create_compile_action` (`actions.rs:3432`) that logs
   `(source_path, mnemonic, feature_configuration.enabled_features)`.
2. Rebuild and re-run with `--compilation_mode=opt`. Capture the two
   registration call stacks for `DLangDemangle.cpp`.
3. The stack trace will identify whether rules_cc calls both, or one
   path is called twice by the same source.

**Likely fixes, in order of probability.**

1. rules_cc's `cc_common.compile` iterates `srcs` *plus* some
   derived list (headers? module sources?) when a specific feature
   is active. If kuro's stub for that derived list isn't returning
   empty, we register twice. Add identity deduplication in the
   action-registration layer keyed by `(source_path, action_name)`.
2. One of `create_compile_action` / `create_cc_compile_action` is
   fallback-invoked by the other. In the fastbuild case the fallback
   is a no-op; in opt the fallback triggers because a feature flag
   changes the branch. Remove the double-invocation.
3. rules_cc's `opt_compile_flags` feature implies
   `generate_dsym_file` which in turn requests a second compile. Kuro
   should suppress that second action (the dsym action is Apple-only
   and doesn't apply here). Check the feature-implies chain.

**Tasks.**

1. Reproduce the duplicate-identifier error with logging.
2. Identify the two registration sites.
3. Either dedup at the action-registry layer or suppress the extra
   registration at its call site (prefer the latter — dedup hides
   real bugs).
4. Add a small e2e test in `tests/core/analysis/` that builds a
   trivial cc_library under `--compilation_mode=opt` and asserts
   success.

**Success criteria.** `kuro build --compilation_mode=opt
@llvm-project//llvm:Support` succeeds. Compile command includes
`-O2 -D_FORTIFY_SOURCE=1 -DNDEBUG -ffunction-sections -fdata-sections
-g0 -fstack-protector -Wall -fno-omit-frame-pointer`.

### 20.3 Final benchmark and write-up (OPEN, measurement)

After 20.1 and 20.2 land, re-run the canonical cold benchmark:

    BUCKD_STARTUP_TIMEOUT=180 KURO=/var/mnt/dev/kuro/kuro \
      tools/bench/run.sh \
      --target '@llvm-project//clang:clang' --runs 1 --cold \
      --workspace /var/mnt/dev/llvm-project/utils/bazel \
      --out benchmarks/post-plan-20-final-<sha>

Diff with Plan 19's baseline:

    kuro log diff summary \
      --path1 benchmarks/post-plan-19-final-ac51eb9/.../build.pb.zst \
      --path2 benchmarks/post-plan-20-final-<sha>/.../build.pb.zst

Write `FINDINGS.md` following the format of the post-Plan-17.2 and
post-Plan-19 baselines. Required numbers: cold wall, critical path,
td_generate's critical_wall_us (should drop from 275.9 s), c_compile's
p95 (should stay flat or improve).

## Dependencies and ordering

```
20.1 genrule.tools exec_dep + virtual_includes fix
    │
    └─► 20.2 --compilation_mode=opt rules_cc dedup
            │
            └─► 20.3 Final cold clang:clang measurement
```

20.1 is the larger of the two. 20.2 is a surgical dedup-style fix
that only matters when a user passes `--compilation_mode=opt`
explicitly; most exec-cfg builds hit 20.1's path via
platform(exec_properties) defaults. 20.3 is the measurement that
closes the plan.

## Open questions

- **Does exec-cfg traversal need to re-apply `apply_cli_build_settings`?**
  Plan 19.4's CLI-injection helper runs once at the top-level
  `get_configured_target`. If the exec cfg goes through a fresh
  `get_platform_configuration` path (via exec_platform resolution),
  it won't have CLI starlark_flags. That's *probably* correct —
  exec-cfg shouldn't pick up `--//pkg:target=value` that targets
  the user's target graph — but confirm.
- **Does Plan 19.3's `platform(exec_properties)` override user
  `--host_compilation_mode`?** Plan 19 never wired
  `--host_compilation_mode`; if we want exec-cfg to honour it, that's
  a Plan 20.4 follow-up.

## Success criteria

- Cold `@llvm-project//clang:clang` wall ≤ 1200 s (Bazel: 1131 s).
- `kuro log critical-path` shows td_generate entries at ~30 s each
  (vs 275 s in the post-19 baseline).
- `kuro log what-ran | grep c_compile` on an exec-cfg tool-compile
  action contains `-O2 -D_FORTIFY_SOURCE=1 -DNDEBUG
  -ffunction-sections -fdata-sections -g0` plus the always-on
  baseline flags.
- `kuro log what-ran` on a target-cfg library compile (llvm:Support
  sources) does *not* contain `-O2` — target cfg stays at fastbuild
  unless the user opts in via `--compilation_mode`.
- `kuro build --compilation_mode=opt @llvm-project//llvm:Support`
  succeeds and its compile commands include the opt flag set.

## References

- Plan 19 completion write-up:
  `benchmarks/post-plan-19-final-ac51eb9/FINDINGS.md`
- Parent plan's Plan-17-follow-ups research:
  `thoughts/shared/research/plan-17-remaining-phases.md`
- Investigation that motivated both Plans 19 and 20:
  `thoughts/shared/research/td_generate-critical-path-investigation.md`
- Relevant source surface:
  - `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs:451`
    (`genrule.tools` declaration — flip site)
  - `app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs:167`
    (`create_cc_compile_action`)
  - `app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs:3432`
    (`create_compile_action`)
  - `app/kuro_node/src/attrs/configuration_context.rs`
    (`configure_target` / `configure_exec_target`)
  - `app/kuro_build_api/src/interpreter/rule_defs/cc_common/feature_config.rs`
    (`default_cc_features`, removed mode names in 19.6)
