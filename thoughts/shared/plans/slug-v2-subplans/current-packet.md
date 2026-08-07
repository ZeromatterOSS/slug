# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-rbe-developer-build-after-user-minimal-success`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one remote-only BuildBuddy-profile build of the Slug V2 binary.

## Goal and required design

From the clean scheduling commit, require a clean Linux x86_64 checkout, no
`slugd`, and one fresh private mode-0700 root. Inherit the process environment
unchanged and run exactly one build with ordinary RC discovery. Do not set,
print, expand, copy, inspect, or otherwise touch `HOME` or home RC:

```text
bazel --output_base=<private>/output build \
  --config=buildbuddy-rbe \
  --@rules_rust//rust/toolchain/channel=nightly \
  --noremote_accept_cached --noremote_upload_local_results \
  --remote_download_outputs=toplevel --remote_timeout=900 --jobs=4 \
  --build_event_json_file=<private>/build-events.json \
  --execution_log_json_file=<private>/execution.json \
  //app/slug_cli_v2:slug
```

Redirect both terminal streams to private mode-0600 files. Never display,
inspect, parse, copy, or commit terminal/BEP/execution contents. Accept only
process exit zero and exactly one executable regular file (`-type f`) matching
`*/bin/app/slug_cli_v2/slug` beneath the private output base. Emit only fixed
booleans/counts.

## Stops and budget

Return `REPLAN` on nonzero build exit, missing output, retained private root,
Git/daemon drift, or cleanup failure and do not retry or change profile/backend.
Always invoke private-output-base shutdown with all RC files ignored, delete
only the exact private root (making only it owner-writable if needed), and
recheck Git cleanliness and no `slugd`. Claim only one fresh remote-only
BuildBuddy-profile Stage 10 developer build; without parsing structured logs,
per-spawn RBE, cache reuse, the 43-test gate/classifier, CI, Stage 7 acceptance,
and self-hosting remain separate. Only owner/canonical/current docs may record
the result, at most 120 changed lines.
