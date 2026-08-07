# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-prime-root-only-nobuild-diagnosis`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one credential-free diagnosis of the frozen cache prime option vector.

## Goal and required design

From the clean scheduling commit, require a clean Linux x86_64 checkout, no
`slugd`, and one fresh private mode-0700 root. Generate one unprinted nonce and
run exactly one root-RC-only `bazel test` invocation:

```text
env -u BAZELRC bazel \
  --nosystem_rc --nohome_rc --noworkspace_rc \
  --bazelrc=<repo>/.bazelrc --output_base=<private>/output test \
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
  --remote_cache= --remote_executor= --remote_instance_name= \
  --bes_backend= --bes_results_url= --disk_cache= --nofetch --nobuild \
  //app/slug_cli_v2:slug
```

Redirect both terminal streams to private mode-0600 files. The startup options
and unset `BAZELRC` forbid ambient authentication; the final empty service/cache
overrides forbid configured BuildBuddy traffic; `--nofetch` forbids repository
fetching; and `--nobuild` forbids action execution. Private stderr may be
inspected transiently only to identify a public checked-in non-remote flag or
flag combination. Never paste, retain, or commit the raw stream or any private
path.

Emit only `ROOT_ONLY_NONREMOTE_DIAGNOSED` plus the public identifier when exit
two is attributable, `ROOT_ONLY_NONREMOTE_ACCEPTED` for exit zero,
`ROOT_ONLY_UNEXPLAINED` for unattributable exit two, or `REPLAN` otherwise.

## Stops and budget

Do not retry, bisect options, change code/config/profile/backend, contact a
remote service intentionally, or infer cache/RBE/test behavior. Always invoke
private-output-base shutdown with all RC files ignored, delete only the exact
private root (making only it owner-writable if needed), and recheck Git
cleanliness and no `slugd`. Cleanup failure is `REPLAN` regardless of process
exit. Only owner/canonical/current docs may record the fixed result, at most
120 changed lines.
