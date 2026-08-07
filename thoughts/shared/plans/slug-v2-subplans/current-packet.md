# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-command-stderr-user-decision`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one token-free user diagnosis of Bazel's omitted failure detail.

## Goal and required design

The user, not an agent, may run this minimal reproduction and inspect its
terminal output privately:

```text
bazel test --config=buildbuddy-cache --remote_executor= --bes_backend= \
  --bes_results_url= --disk_cache= --noremote_local_fallback \
  //app/slug_cli_v2:slug
```

Reply only `minimal succeeds` or give a token-free paraphrase naming the
offending option/failure kind. Never paste the raw output, authentication
header/value, token, path, invocation URL, or other private data.

## Stops and budget

No agent command or repository change is authorized before the user's
token-free response. Agents must not read stderr, inspect/expand home RC,
contact BuildBuddy, rerun the gate, or infer a cache claim. After the response,
a separate reviewed packet may record the decision and freeze one bounded
repair or external-state action.
