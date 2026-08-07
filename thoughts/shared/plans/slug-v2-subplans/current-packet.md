# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-prime-command-vector-isolation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one token-safe classification of the frozen cache prime option vector.

## Goal and required design

From the clean scheduling commit, require a clean Linux x86_64 checkout, no
`slugd`, and one fresh private mode-0700 root. Generate one unprinted nonce,
inherit the process environment unchanged, and run exactly one `bazel test`
invocation with ordinary RC discovery. Do not set, print, expand, copy,
inspect, or otherwise touch `HOME` or home RC:

```text
bazel --output_base=<private>/output test \
  --config=buildbuddy-cache \
  --@rules_rust//rust/toolchain/channel=nightly \
  --remote_cache=grpcs://remote.buildbuddy.io --remote_instance_name= \
  --remote_executor= --bes_backend= --bes_results_url= --disk_cache= \
  --spawn_strategy=worker,sandboxed,local --test_strategy=local \
  --cache_test_results=yes --runs_per_test=1 \
  --test_sharding_strategy=disabled --noremote_local_fallback \
  --build_event_publish_all_actions \
  --build_event_json_file=<private>/build-events.json \
  --execution_log_json_file=<private>/execution.json \
  --action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE=<nonce> \
  --test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE=<nonce> \
  --noremote_accept_cached --remote_upload_local_results \
  --noremote_cache_async \
  //app/slug_cli_v2:slug
```

Redirect both terminal streams to private mode-0600 files. Never display,
inspect, parse, copy, or commit terminal/BEP/execution contents. Emit only one
fixed result: exit zero is `PRIME_VECTOR_ACCEPTED`; exit two is
`PRIME_VECTOR_EXIT_2`; any other process result is `REPLAN`.

## Stops and budget

Do not retry, bisect options, change code/config/profile/backend, or infer cache,
RBE, test-suite, or classifier behavior. Always invoke private-output-base
shutdown with all RC files ignored, delete only the exact private root (making
only it owner-writable if needed), and recheck Git cleanliness and no `slugd`.
Cleanup failure is `REPLAN` regardless of process exit. Only owner/canonical/
current docs may record the fixed result, at most 120 changed lines.
