# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-repository-config-decision`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a user-reviewed, docs-only choice of the repository-safe BuildBuddy
connection and executor platform before any remote configuration.

## Goal and required design

Obtain the user's explicit choice between hosted BuildBuddy and an
organization/self-hosted service. Freeze only non-secret, repository-safe facts:
opt-in cache-only and execution-enabled profile names; whether either may be a
default; exact BES/cache/executor endpoints and optional instance name; exact RBE
OS/CPU and platform/container properties, including an immutable image identifier
when required; and the 43-green/one-expected-red target command matrix. Accept
official service documentation or a user-supplied sanitized organization connection
snippet as evidence. Never inspect, print, copy, or infer `~/.bazelrc`; authentication
remains environment-owned. Preserve the blocked core unit and user-deferred cyclic-Bzl
baseline.

## Stops and budget

Stop for explicit user input rather than choose a deployment, endpoint, tenant,
instance, executor pool, platform, or container by assumption. Do not add or edit
`.bazelrc`, run Bazel remotely, inspect effective RC expansion, add CI/evidence/code/
locks, expose secrets, or enter core host tools, self-hosting, JVM, Bazel 8,
WORKSPACE, or cycle semantics. Zero non-documentation changes; at most 220 authored
documentation lines.
