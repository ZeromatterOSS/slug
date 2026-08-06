# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-repository-config-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a repository-safe root `.bazelrc` with cache-default and RBE-opt-in
BuildBuddy Cloud profiles.

## Goal and required design

Add exactly the 13 accepted non-secret options to root `.bazelrc`: the five
user-approved BuildBuddy Cloud service lines; local `worker,sandboxed,local`
ordinary spawn strategy; `buildbuddy-cache` executor clearing plus synchronous
uploads; and `buildbuddy-rbe` remote-only/no-fallback execution on managed
`linux`/`amd64` rather than self-hosted executors. Authentication remains solely in
`~/.bazelrc`; never inspect, print, copy, or infer it. Validate both profiles with
Bazel 9.2 using only the explicit root RC, no home/system/workspace RC discovery,
and command-line endpoint clearing that prevents remote contact. Preserve the
43-green/one-expected-red boundary, blocked core unit, and deferred cycle.

## Stops and budget

Only `.bazelrc`, this owner plan, and the canonical/current scheduling documents
may change: at most 13 configuration lines, 80 authored documentation lines, four
files, and 150 total changed lines. Do not contact a remote service; inspect effective
home options; add a header/import/secret, CI, evidence, code, BUILD/MODULE, lock, or
custom platform/container; or enter core host tools, self-hosting, JVM, Bazel 8,
WORKSPACE, and cycle semantics. Cache and RBE live evidence remain separate later
packets.
