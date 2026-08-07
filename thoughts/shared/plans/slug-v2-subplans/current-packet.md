# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one fixed-enum one-prime stage result.

## Goal and required evidence

From the clean scheduling commit, invoke exactly once:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_stage_probe.py
```

Inherit the environment unchanged so Bazel discovers ordinary RC files and may
privately consume the user's home authentication. Never inspect, print, copy,
hash, or persist that RC/token, `HOME`, captured terminal output, BEP/execution
contents, invocation data, or BuildBuddy UI data. Review only CLI status, empty
stderr, and its fixed normalized JSON record.

## Stops and budget

Do not retry, inspect artifacts, or modify code/config. Any nonempty stderr,
schema surprise, sanitizer result, retained state, Git/daemon drift, or cleanup
failure is `REPLAN`. Route a recorded fixed stage exactly as specified in the
owner plan; `PRIME_READY` proves only the private prime path and schedules a
separate replay-stage probe. Cache/RBE, the full 43-test expansion, and the rest
of Stage 10 remain required.
