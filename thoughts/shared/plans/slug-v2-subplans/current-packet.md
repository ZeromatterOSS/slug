# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-disable-cache-reads-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one source-reviewed cache-read-disabled prime command.

## Goal and required evidence

Edit only:

- `tools/v2_oracle_lib/buildbuddy_build_cache.py`
- `tests/v2_oracle/test_buildbuddy_build_cache_gate.py`
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_stage_probe.py`
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_bep_stage_probe.py`
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_execution_stage_probe.py`
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_output_semantics_probe.py`

Add exactly one `--noremote_accept_cached` argument immediately after
`--config=buildbuddy-cache` in the one-prime command. Do not add upload, async,
replay, executor, BES, disk-cache, strategy, or other overrides. Pin the exact
argument once and preserve every existing command argument and order otherwise.

Keep `cache_hit` strict. Bazel 9.2 `ExpandedSpawnLogContext` sets the proto3
boolean on every logged spawn, and its `JsonOutputStreamWrapper` selects
`JsonFormat.printer().alwaysPrintFieldsWithNoPresence()`. Therefore prime false
is source-faithfully explicit. Test prime false accepted; prime true, absent,
null, string, zero, and one rejected; replay true accepted and replay false
rejected. A source-faithful false prime reaches `PRIME_READY`. Update only the
four frozen shared-source digest assertions made stale by the command change.

## Stops and budget

Stay within four production and 50 test changed lines. Run all
`test_buildbuddy_build_cache*.py` modules, focused cache-field/command tests,
`py_compile`, exact scope/line caps, and `git diff --check`, then independent
pinned-source review. No parser, schema, stage, lifecycle guard, CLI, RC/config,
fixture, docs, unrelated option, Bazel, network, home RC, live artifact, or
service access. A later guarded live packet is scheduled only after acceptance.
