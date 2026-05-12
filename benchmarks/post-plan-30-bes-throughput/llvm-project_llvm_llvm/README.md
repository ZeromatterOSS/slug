# Plan 30 (30.1 + 30.2 + 30.5) — BES upload throughput

`@llvm-project//llvm:llvm` warm RE build, run from `/var/mnt/dev/llvm-project/utils/bazel/`.

Slug commit: `b588318b` + uncommitted plan-30 patch (30.1 earlier stream open,
30.2 tonic flow-control + per-service Channel, 30.5 lifecycle parallelization).
Bazel: 9.0.2.
Backend: `grpcs://remote.buildbuddy.io`, default `--bes_upload_mode=wait_for_upload_complete`.
Date: 2026-04-29.

## Wall-time results

| Scenario                    | slug   | bazel  |
|-----------------------------|--------|--------|
| cold daemon, warm RE        | 14.81s | 5.92s  |
| warm daemon, warm RE        |  5.72s | 1.03s  |

Plan baseline (commit `9af3642e`, same target/network/account):

| Scenario                                       | wall  | post-build wait |
|------------------------------------------------|-------|-----------------|
| `wait_for_upload_complete` (default)           | 57.4s | 23.2s           |
| `--bes_upload_mode=nowait`                     | 24.7s |  ~0s            |

## Plan-30 effect

Default-mode wall: **57.4s → 14.81s** (-74%).

Estimated post-build BES wait now (wall − reported build phase):
14.81s − 11.9s = **~2.9s** (vs plan baseline 23.2s, -88%).

Some of the gain is from earlier improvements that landed in the build phase
itself (22.4s → 11.9s) since the plan was written. The plan's specific lever —
`BesSink::shutdown()` — went from 23.2s of post-build blocking down to ~2.9s
because (a) the BES stream was already open before the first action, (b)
~5k events streamed through a 2 MiB initial-window connection that no longer
share with the chrome-trace ByteStream upload, and (c) the four lifecycle
unary RPCs collapsed to two parallel pairs.

## Plan success criteria

- **`slug build @llvm-project//llvm:llvm --config=remote` warm: client wall ≤ 30s** — ✅ 14.81s.
- **Stretch: slug wall ≤ bazel wall** — ❌ 14.81s vs 5.92s. Remaining gap is in the build phase, not BES upload.
- Sub-second post-build BES wait — partially: ~2.9s, dominated by the chrome-trace ByteStream upload (0.3–3.2s observed) plus the residual tail of BEP events. Plan 30.3 (daemon-resident uploader) would move all of this off the client wall.

## Methodology

```bash
cd /var/mnt/dev/llvm-project/utils/bazel
# cold-daemon
slug killall && bazel shutdown
/usr/bin/time -f "WALL=%es" slug build @llvm-project//llvm:llvm --config=remote
# warm-daemon
/usr/bin/time -f "WALL=%es" slug build @llvm-project//llvm:llvm --config=remote
# Same pattern for `bazel build …`.
```

Reported phases from slug cold-daemon run:
`load=2.6s analyze=1.0s execute=8.2s materialize=4.6s total=11.9s`.
