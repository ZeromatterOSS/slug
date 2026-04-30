# Plan 31.2 — File-watcher filter for bazel symlinks

`@llvm-project//llvm:llvm` warm-RE build, run from
`/var/mnt/dev/llvm-project/utils/bazel/`.

Kuro: HEAD `b588318b` + plan 31.2 patch (notify + watchman component filter
extended with `bazel-bin`, `bazel-out`, `bazel-testlogs`, `bazel-bazel`,
`bazel-external`).
Bazel: 9.1.0.
Backend: `grpcs://remote.buildbuddy.io`,
default `--bes_upload_mode=wait_for_upload_complete`.
Date: 2026-04-29.

## Wall-time results

5-trial back-to-back warm-daemon runs after a single cold-daemon warmup.
Both kuro and bazel report `Network: Up: 0B Down: 0B` — pure local
overhead.

| Tool  | Trials (s)                      | Median |
|-------|---------------------------------|--------|
| kuro  | 1.05, 0.79, 0.78, 0.83, 0.89    | **0.83 s** |
| bazel | 0.97, 0.94, 0.95, 0.87, 0.86    | **0.94 s** |

Baseline (plan 30 bench, pre-31.2):

| Tool  | warm-daemon | gap to bazel |
|-------|-------------|--------------|
| kuro  | 5.72 s      | 4.7 s        |
| bazel | 1.03 s      | —            |

Plan 31 success criterion: warm-daemon kuro wall ≤ 1.97 s (closes ≥80%
of 4.7 s gap). **Hit.** Kuro now beats bazel on this scenario.

## What this measures

Both daemons are alive with their build graphs warm. With nothing
materially changed in the source tree, neither system needs to consult
the network: kuro short-circuits on DICE, bazel on Skyframe. The metric
is the local-overhead floor of a no-op rebuild.

## 31.2-specific test — bazel-write events do not invalidate kuro

Without the filter, every artifact bazel writes to `bazel-bin/` looks
like a source change to kuro's notify watcher and forces DICE to
re-evaluate ~5 k actions on the next build (the 5.72 s baseline above).
With the filter, those events are dropped at the watcher.

Test:

```bash
cd /var/mnt/dev/llvm-project/utils/bazel
# warm-daemon kuro baseline
kuro build @llvm-project//llvm:llvm --config=remote      # 0.78 s
# bazel runs, writing bazel-bin/, bazel-out/, …
bazel build @llvm-project//llvm:llvm --config=remote     # 2.21 s
# next kuro build — file events from bazel-bin should be dropped
kuro build @llvm-project//llvm:llvm --config=remote      # 0.78 s ✓
```

The post-bazel kuro build prints **no** `File changed: …bazel-bin…`
or `File changed: …bazel-out…` lines and runs zero actions, exactly
matching the no-bazel-in-between baseline.

## Cold-daemon (for reference; not the focus of 31.2)

Single trial, both daemons just killed (`kuro killall`,
`bazel shutdown`); bazel keeps its on-disk action cache:

| Tool  | wall    |
|-------|---------|
| kuro  | 15.93 s |
| bazel |  6.90 s |

Cold-daemon gap is owned by plan 31.1 (persistent on-disk action cache)
— still ~9 s. Not addressed here.

## Apples-to-apples: both forced to round-trip the action cache

Plan 31.1 isn't done yet, so kuro has no on-disk action cache —
**every** cold-daemon build round-trips to BB.io's ActionCache for each
of the ~4 850 actions in this graph. Bazel's on-disk cache was deleted
between runs to reproduce the same conditions:

```bash
bazel shutdown
rm -rf ~/.cache/bazel/_bazel_wgray/<hash>/action_cache/
kuro killall    # no on-disk cache to delete
```

| Tool  | Trial 1 | Trial 2 | Network (up / down) | actions / processes |
|-------|---------|---------|---------------------|---------------------|
| kuro  | 14.48 s | 15.20 s | ~948 KiB / ~5.1 MiB | 4 853 (4 852 cached, 1 local) |
| bazel | 48.83 s | 46.05 s | not reported by bazel | 5 886 (4 739 remote cache hit, 1 147 internal) |

Median wall: kuro **14.84 s** vs bazel **47.44 s** — **kuro ~3.2× faster**
when both must consult the remote action cache for every action.

Bazel's slowdown is not RE-throughput-bound: its `Critical Path`
reports 12.67 s and 14.14 s, all of which is action cache validation
on its own end. Kuro's RE client issues `GetActionResult` calls in
parallel; bazel's local action cache rebuild appears to be far less
parallel.

When plan 31.1 lands and kuro persists its action cache to disk, this
scenario becomes a fast-path local lookup for kuro too, and the cold-
daemon table at the top of the README closes.

BB.io invocations:

- kuro #1: https://app.buildbuddy.io/invocation/9ee8a4b3-712e-4c47-adeb-e75fdd6dbb3d
- kuro #2: https://app.buildbuddy.io/invocation/53badca0-d2ab-4b80-91eb-000a7e899170
- bazel #1: https://app.buildbuddy.io/invocation/7db90044-e6f9-4eb9-8f68-103d82278e44
- bazel #2: https://app.buildbuddy.io/invocation/a20c8a0a-be34-43d5-b99c-95c5125e4acf

## Methodology

```bash
cd /var/mnt/dev/llvm-project/utils/bazel
# clean both daemons
kuro killall
bazel shutdown
# warmup (cold-daemon, populates DICE / skyframe; not measured here)
kuro  build @llvm-project//llvm:llvm --config=remote
bazel build @llvm-project//llvm:llvm --config=remote
# 5 warm-daemon trials per tool
for i in 1..5; /usr/bin/time -f "WALL=%es" kuro  build @llvm-project//llvm:llvm --config=remote
for i in 1..5; /usr/bin/time -f "WALL=%es" bazel build @llvm-project//llvm:llvm --config=remote
```

`--config=remote` routes to `grpcs://remote.buildbuddy.io` for both tools.
The BB.io action cache is fully populated for this target prior to the
measured runs, so any action that *did* miss DICE/Skyframe would
round-trip to the BB.io ActionCache rather than execute remotely. None
did, in any of the 10 measured trials.

## Source

- Notify filter: `app/kuro_file_watcher/src/notify.rs`
  `RESERVED_OUTPUT_COMPONENTS` const + `is_reserved_output_path`.
- Watchman parity: `app/kuro_file_watcher/src/watchman/interface.rs`
  reuses `is_reserved_output_path` in `process_one_change`.
- Tests: `app/kuro_file_watcher/src/notify.rs#tests` — 6 cases covering
  every reserved component plus a similar-named-source negative.
