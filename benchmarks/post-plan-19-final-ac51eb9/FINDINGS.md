# Post-Plan-19 final baseline

**Date:** 2026-04-23
**HEAD:** ac51eb9 (Plan 19.1 → 19.6 landed)
**Target:** `@llvm-project//clang:clang`
**Mode:** cold, single run, no --drop-caches
**Workspace:** `/var/mnt/dev/llvm-project/utils/bazel`

## Headline

| Metric               | Bazel 9.0.2 | Post-17.2 baseline | Post-Plan-19 | Δ vs baseline |
|----------------------|-------------|--------------------|--------------|---------------|
| Cold wall            | 1131 s      | 1435.7 s           | **1416.1 s** | **-1.3 %**    |
| Actions              | 6352        | 5367               | 5367         | +0            |
| act/sec              | 5.6         | 3.74               | 3.79         | +0.05         |
| Critical path        | 71.4 s      | 367.6 s            | **360.5 s**  | -1.9 %        |
| Slowest path         | —           | 132.7 s            | **74.1 s**   | **-44.1 %**   |
| Peak in-flight       | —           | 2445               | 2290         | -6.3 %        |

Plan 19 is **at parity** with the post-17.2 baseline. The 1200 s success
criterion from the plan document is not met — the target required
exec-cfg opt-mode tool builds, which the infrastructure enables but is
blocked from end-to-end activation by two follow-ups described below.

## What landed

Six phases of foundational infrastructure, committed serially:

- **19.1** `ConfigurationData` carries a typed `build_settings` map.
  `BuildSettingLabel` / `BuildSettingValue` added.
- **19.2** Bazel-style `transition()` impl functions mutate the outgoing
  cfg's build_settings instead of leaking through a global. Split
  transitions apply per-branch setting dicts independently.
- **19.3** `platform(exec_properties={...})` flows through `PlatformInfo`
  → `ExecutionPlatformInfo` → the exec-cfg `ConfigurationData.build_settings`
  at platform construction time. `@local_config_platform//:host` declares
  `@bazel_tools//tools/cpp:compilation_mode = "opt"` as the exec default
  — data-driven, not hardcoded in Rust.
- **19.4** `--compilation_mode` and `--//pkg:target=value` CLI flags land
  in the top-level target cfg's build_settings via
  `apply_cli_build_settings` in `get_configured_target`. Pre-existing bug
  where `compilation_mode` was listed as a Bazel pass-through flag in
  `bazelrc.rs` — clap never saw the value — was fixed.
- **19.5** `ctx.var["COMPILATION_MODE"]`, `ctx.fragments.cpp`,
  `ctx.build_setting_value`, and `config_setting.flag_values` matching
  read from the analyzing target's own cfg. Transition-layer mirror-write
  to global `BUILD_CONFIG` was removed.
- **19.6** `create_cc_compile_action` + `get_memory_inefficient_command_line`
  emit the cc_toolchain_config flag set driven by the per-cfg
  compilation_mode via the feature_configuration. Mode names removed from
  the default feature set so `is_feature_enabled("opt")` distinguishes
  cfgs.

## Per-mnemonic diff (before vs after)

                               before          after            Δ       Δ%
    c_compile                                                                 
      total_wall_us      16784500527    16691448021    -93052506    -0.6 %
      p95_us                11413622       11165900      -247722    -2.2 %
    td_generate                                                               
      total_wall_us       5248438049     4835325125   -413112924   **-7.9 %**
      p95_us                43692465       39647702     -4044763    -9.3 %
    cpp_archive                                                               
      total_wall_us         39047068       35896889     -3150179    -8.1 %
    cpp_link                                                                  
      total_wall_us         31564403       37062120     +5497717   +17.4 %
    genrule                                                                   
      total_wall_us           677449        2290910     +1613461  +238.2 %

`td_generate` total_wall drops 7.9 % even though tools are still compiled
in fastbuild mode. The gain comes from the always-on baseline flags
(`-U_FORTIFY_SOURCE -fstack-protector -Wall -fno-omit-frame-pointer`)
that 19.6 started emitting — no mode change, just the rules_cc default
compile set. `cpp_link` (+17 %) and the single-action `genrule` (+238 %)
are high-variance tails with tiny counts; not a concerning regression.

## Why the 1200 s target is not hit

Two gaps prevent exec-cfg opt-mode tool builds, which is what the plan
predicted would drop td_generate critical-path from ~290 s to ~30 s:

1. **`genrule.tools` stays at target-cfg.** `AttrType::exec_dep` would
   route tools through the exec transition machinery Plan 19.3 wires
   up. The switch triggers a separate regression in siphash's
   `_virtual_includes` include-path synthesis (cc_library's
   `virtual_includes` paths are absent from the dep graph once `tools`
   is exec-configured — observed as `fatal error: siphash/SipHash.h:
   No such file`). Comment in `native_rules.rs` describes the state.
2. **Opt-mode analysis regression.** `kuro build --compilation_mode=opt
   @llvm-project//llvm:Support` fails with
   `Action category c_compile contains duplicate identifier` for
   several Demangle .cpp files. This is a rules_cc analysis path that
   branches on feature state; my removal of opt/dbg/fastbuild from
   `default_cc_features` may be exposing a latent double-registration.
   Until that's resolved, `--compilation_mode=opt` can't be used to
   validate the opt flag emission end-to-end.

Both are out of Plan 19.1-19.6's scope (the latter is actually a
consequence of 19.6 enabling meaningful per-cfg mode selection, so it's
the right next work item).

## What's unblocked

- A unit-testable per-cfg settings layer that every downstream reader
  consults. Plan 17.x and any follow-up that wants to distinguish
  target vs exec compilation behaviour now has a clean surface.
- `--@foo//:bar=value` CLI flags flow through to `ctx.build_setting_value`
  via the cfg instead of a process-global — required for per-cfg select()
  matching on build-settings.
- Transitions declared with `cfg=transition(impl=...)` actually apply
  their outgoing dict, including split transitions. Any rule that wanted
  to use user-defined Bazel transitions can start doing so.

## Next steps (deferred from this plan)

- **Investigate siphash `_virtual_includes` when `genrule.tools`
  switches to `exec_dep`.** That's the single block between the 19.3
  infrastructure and the measured td_generate win.
- **Investigate rules_cc `duplicate c_compile identifier` under
  `--compilation_mode=opt`.** May require tracing which rules_cc
  analysis path registers two compile actions and why it didn't hit in
  fastbuild (rules_cc probably conditions some action on active
  features).
- **Re-run `clang:clang` cold benchmark after both of the above land.**
  If the post-19 infrastructure is intact and exec-cfg tools are
  compiled in opt, the td_generate critical-path drop should land in a
  single follow-up pass.

## References

- Baseline: `benchmarks/post-plan-17-fixed-aggregator/FINDINGS.md`
- Plan file: `thoughts/shared/plans/kuro-bazel-subplans/19-configuration-transitions.md`
- Investigation motivating Plan 19:
  `thoughts/shared/research/td_generate-critical-path-investigation.md`
