# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-home-auth-rc-decision`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a token-free user confirmation that home RC contains only valid auth.

## Goal and required design

The user privately checks `~/.bazelrc` and confirms it contains the BuildBuddy
API-key header in valid Bazel 9.2 syntax, preferably
`common --remote_header=x-buildbuddy-api-key=<secret>` (or `build` scope), and
no stale endpoint, instance, profile, executor, strategy, or unsupported option.
Do not paste the actual line, token, or any derived value. The checked-in root
file already owns all non-secret service and mode configuration.

## Stops and budget

Zero repository changes and zero agent commands. Do not inspect, print, copy,
canonicalize, announce, or otherwise consume home RC; do not run the driver,
Bazel build/test, or a remote command. Record only the user's token-free
confirmation. Any broader home configuration or uncertainty remains `REPLAN`.
