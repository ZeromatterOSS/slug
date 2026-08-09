# Current Slug V2 Packet

Packet: `WP-6-m4-root-cquery-label-slug-projection-design`
Milestone: M4 cquery
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: freeze the public Slug-native default/`label` cquery format before code.

## Boundary

This is docs/source design only. Reuse the accepted Bazel 9.2 evidence at the
owner plan's “Root cquery label-output evidence” section: default and explicit
`label` have identical `label (seven-hex-short-id)` layout, warm replay is
stable, and missing-target diagnostics are already pinned. Do not run a new
oracle unless a concrete source/evidence gap appears.

Exact Bazel configuration checksum and short-ID bytes are intentionally M9.
The current structural configuration and full `slugcfg-v1:<opaque>` projection
are accepted; this packet decides how the public formatter uses that projection
without calling it a Bazel checksum, truncating it, parsing it as semantics, or
changing structural DICE identity.

## Required design

Freeze:

- accepted CLI forms for default output and `--output=label` while retaining
  the existing one-label root cquery allowlist;
- exact success stdout spelling around the canonical label and full
  `slugcfg-v1:<opaque>` display token;
- preservation of the existing
  `--output=starlark --starlark:expr=str(target.label)` bytes;
- missing-target, unsupported-flag, one-shot, daemon JSON/wire, warm replay,
  and changed/restored configuration behavior;
- graph-local comparison normalization that may replace only the opaque
  projection token and never label, graph, provider, action, platform,
  ordering, content, or failure fields; and
- exact implementation/test files for the successor, with no aquery,
  ActionKey, platform/toolchain breadth, general Starlark expressions, or
  configuration parsing.

Obtain an independent public-format/identity review. Record `ACCEPT` or
`REPLAN` in the owner plan and schedule one bounded implementation successor
only after acceptance.

## Stops

Stop and `REPLAN` on any JVM/Java helper or delegation, seven-hex or otherwise
Bazel-looking approximation, truncated projection, caller-supplied projection,
projection-as-DICE/cache identity, changed Starlark-label bytes, new evaluator
graph, aquery/ActionKey activation, or normalization outside the opaque display
token.
