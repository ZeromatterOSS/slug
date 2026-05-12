# Post-Plan-20 final baseline

**Date:** 2026-04-23
**HEAD:** 41ce00a5 (Plan 20.1 + 20.2 landed)
**Target:** `@llvm-project//clang:clang`
**Mode:** cold, single run, no --drop-caches
**Workspace:** `/var/mnt/dev/llvm-project/utils/bazel`

## Headline

| Metric               | Bazel 9.0.2 | Post-17.2 | Post-19   | **Post-20** | Δ vs Post-19 |
|----------------------|-------------|-----------|-----------|-------------|--------------|
| Cold wall            | 1131 s      | 1435.7 s  | 1416.1 s  | **1107.6 s** | **-23.7 %**  |
| Actions              | 6352        | 5367      | 5367      | 5599        | +4.3 %       |
| act/sec              | 5.6         | 3.74      | 3.79      | **5.05**    | +33 %        |
| Critical path        | 71.4 s      | 367.6 s   | 360.5 s   | **109.7 s** | **-69.6 %**  |
| Slowest path         | —           | 132.7 s   | 74.1 s    | 68.0 s      | -8.2 %       |
| td_generate crit     | —           | 290.7 s   | 275.9 s   | **33.1 s**  | **-88.0 %**  |
| td_generate total    | —           | 5248 s    | 4835 s    | **656 s**   | -86.4 %      |

**Plan 20 hit Plan 19's 1200 s target and then some.** Cold wall is
now **within 2.4 % of Bazel** (1107.6 s vs 1131 s) and critical path
is at 1.54× Bazel (109.7 s vs 71.4 s). td_generate, which was the
79 % slice of the critical path in both the post-17.2 and post-19
baselines, is now ~9 % of critical path and ≈parity with Bazel
per-action.

## What changed

Two commits on top of Plan 19:

- **20.1** `legacy_exec_cfg` in `app/slug_configured/src/execution.rs`:
  When `build.execution_platforms` is unset, load
  `@local_config_platform//:host`'s PlatformInfo and use *its* cfg as
  the exec cfg instead of the target cfg. This activates Plan 19.3's
  `platform(exec_properties = {compilation_mode: "opt"})` default on
  every exec-configured dep edge. `genrule.tools` flipped back to
  `AttrType::exec_dep` (Bazel semantics) — what exposed the
  collapsed-exec-cfg bug in the first place.
- **20.2** `create_cc_compile_action` suffixes `.pic` on the action
  identifier when `use_pic=true`. rules_cc's `cc_common.compile`
  registers both a PIC and a non-PIC compile of each source in opt
  mode (Bazel semantics for binaries vs dynamic libs). Without the
  suffix both registered under the same `(c_compile, source_path)`
  key and tripped slug's action-registry dedup.

## Per-mnemonic diff vs Post-19

                               before          after            Δ       Δ%
    total_wall_us         1416143058      1080767527   -335375531  -23.7 %
    critical_path_wall_us  360523330       109694657   -250828673  -69.6 %
    total_action_count          5367            5599         +232   +4.3 %
    c_compile
      count                     3664            3874         +210   +5.7 %
      total_wall_us      16691448021     15836213743   -855234278   -5.1 %
      p95_us                11165900        10410379      -755521   -6.8 %
    td_generate
      critical_wall_us   275934499   →     33145500     -242788999  **-88.0 %**
      total_wall_us       4835325125       655998170   -4179326955  **-86.4 %**
      p95_us                39647702        4316729    -35330973   -89.1 %
    cpp_link
      total_wall_us         37062120        26212216    -10849904  -29.3 %
      p95_us                35520783        25828507     -9692276  -27.3 %

c_compile count is up 5.7 % because exec-cfg sources now compile
twice (PIC + non-PIC) per rules_cc's opt-mode semantics. Per-action
p95 **dropped** 6.8 % despite the doubled work — the opt flags
actually make each compile finish faster, overwhelming the
dual-action overhead on the critical path.

## Verification

    cd /var/mnt/dev/llvm-project/utils/bazel
    slug log what-ran | grep c_compile | head -1 | tr ' ' '\n' | grep ^-

Produces the full rules_cc opt flag set when the action's cfg is the
exec cfg (`local_config_platform//:host#520450dd0e3900b9`):

    -U_FORTIFY_SOURCE
    -fstack-protector
    -Wall
    -fno-omit-frame-pointer
    -g0
    -O2
    -D_FORTIFY_SOURCE=1
    -DNDEBUG
    -ffunction-sections
    -fdata-sections

Target-cfg library compiles (cfg `host#ffb6fe5c7480b5e7`) still get
the fastbuild baseline (`-U_FORTIFY_SOURCE -fstack-protector -Wall
-fno-omit-frame-pointer -g0`) without `-O2` — mode is per-cfg as
intended.

## Plan 19 success criteria (revisited)

From `thoughts/shared/plans/slug-bazel-subplans/19-configuration-transitions.md`:

- [x] Cold wall ≤ 1200 s (achieved: 1107.6 s)
- [x] `slug log critical-path` shows td_generate entries around 30 s
  each (p95: 4.3 s — far under 30 s; critical_wall 33.1 s total)
- [x] c_compile flags include the opt set on exec-cfg compiles
- [x] No hardcoded tool-name / binary-path special cases in Rust
- [x] Per-phase unit tests land with each phase

All Plan 19 + Plan 20 criteria satisfied.

## What remains vs Bazel

Bazel 1131 s, slug 1107.6 s — slug is 23 s faster on total wall. But
critical path is 38 s longer than Bazel (109.7 vs 71.4). That gap is:

- c_compile: 47.5 s on slug's critical path (Bazel: 42.6 s) — minor.
- cpp_link: 26.1 s on slug's critical path (Bazel: 31.2 s) — slug faster.
- td_generate: 33.1 s on slug's critical path (Bazel: 0 s — Bazel's
  tablegen doesn't appear on its critical path because scheduler
  runs td_generate strictly in parallel with independent lib
  compiles). Plan 17.x scheduler work can close this.

## Next

Plan 20 closes Plan 19's td_generate gap. The remaining critical-path
delta vs Bazel (≈ 38 s) is scheduler-shaped: td_generate ends up on
slug's critical path because the scheduler admits tablegen work
later than ideal. That's Plan 17.x territory — use the
`slug log what-ran` queue-vs-exec ratios on this fresh baseline to
pick which 17.x phase to promote next.
