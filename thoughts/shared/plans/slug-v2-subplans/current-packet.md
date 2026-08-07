# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-vertical-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a fail-closed build-only BuildBuddy prime/replay cache driver.

## Goal and required design

Add only `tools/v2_oracle_lib/buildbuddy_build_cache.py` (250 lines),
`tools/v2_oracle/buildbuddy_build_cache_gate.py` (40), and
`tests/v2_oracle/test_buildbuddy_build_cache_gate.py` (360): 650 maximum.

Each `prime`/`replay` phase uses a distinct fresh output base and this same
ordinary-RC command shape, differing only in private paths:

```text
bazel --output_base=<private>/<phase>/output build \
  --config=buildbuddy-cache \
  --@rules_rust//rust/toolchain/channel=nightly \
  --remote_executor= --bes_backend= --bes_results_url= --disk_cache= \
  --noremote_local_fallback \
  --action_env=SLUG_BUILDBUDDY_BUILD_CACHE_NONCE=<shared-fresh-64hex> \
  --build_event_json_file=<private>/<phase>/bep.json \
  --execution_log_json_file=<private>/<phase>/execution.json \
  //app/slug_cli_v2:slug
```

Ordinary RC is consumed only by Bazel. Raw terminal/BEP/execution data remains
private; the driver builds a closed summary from parsed BEP/execution values and
always performs RC-disabled shutdown plus descriptor-safe exact-root cleanup.

`PROVED_BUILD_CACHE` requires both process/BuildFinished/target successes,
exactly one executable regular `*/bin/app/slug_cli_v2/slug` per output base,
nonempty matching eligible digest multisets, prime local/worker/linux-sandbox
misses, replay remote-cache hits, empty statuses, zero exits/cache-field errors,
zero persistent action-cache hits, clean Git/no-`slugd`, and complete cleanup.
The closed schema is only `schema_version=1`, fixed
`mode=buildbuddy-build-cache-only`, `classification`, and fixed-key
`prime`/`replay` summaries for process/BuildFinished/target/output counts,
persistent hits, and eligible-spawn count/digest-multiset hash/error/runner
counts. It never emits a path, label, nonce, individual digest, command,
endpoint, terminal data, or raw value. Classifications are exactly
`PROVED_BUILD_CACHE`, `CONFIG_DRIFT`, `REMOTE_UNAVAILABLE`,
`COMMAND_LINE_FAILURE`, `TARGET_FAILURE`, `CACHE_MISS_OR_MIXED_REPLAY`,
`EVIDENCE_INCOMPLETE`, and `SANITIZER_REJECTED`.

## Stops and budget

Synthetic/mocked tests cover command minimality/shared nonce, JSON sequences,
runner/cache/digest/target/materialization near misses, fixed failure classes,
closed schema, raw suppression, modes, shutdown/read-only/swap-safe cleanup,
Git/no-`slugd`, and CLI stderr. Run only focused offline tests, Python
compilation, caps/diff checks, and independent review. Do not run Bazel, use
normal/home RC, contact BuildBuddy, modify old cache/config/targets, or make a
live attempt. One later packet owns the build pair; build-only RBE and full
43-test expansion remain required successors.
