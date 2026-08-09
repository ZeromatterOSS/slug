# Current Slug V2 Packet

Packet: `WP-6-m4-root-cquery-label-slug-projection-implementation`
Milestone: M4 cquery
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implement the accepted Rust-only public cquery label projection.

## Boundary

Implement only the independently accepted “Root cquery Slug-projection
public-format design result (2026-08-09)” in the owner plan. Reuse the accepted
Bazel 9.2 label-layout, missing-target, and warm-replay evidence; do not run a
new oracle unless a concrete source/evidence gap appears.

The exact Bazel configuration checksum, output-directory identity, and
ActionKey bytes remain M9. This packet exposes the existing full structural
Slug projection as a namespaced display token. It does not parse, truncate, or
promote that projection into semantic identity.

## Implementation

- Admit exactly default output, explicit `--output=label`, and the existing
  `--output=starlark --starlark:expr=str(target.label)` for one root literal.
  Reject every other formatter combination before one-shot/daemon routing.
- Default and explicit label output are byte-identical:
  `//pkg:target (slugcfg-v1:<64-lowercase-hex-bytes>)\n`. Preserve existing
  Starlark-label stdout exactly as `@@//pkg:target\n`.
- Admit exactly `--//:setting=<Unicode>`, including empty and
  last-occurrence-wins. Forward it through one-shot and daemon evaluation using
  the existing structural configuration and root-setting route. Admit no other
  configuration flags.
- Keep the existing `RootConfiguredTargetAnalysisKey`; add no evaluator graph
  or DICE key. Retain the apparent display label and full projection beside the
  returned successful analysis, and derive the projection only from that
  analysis result.
- Add a required serde-validated daemon format discriminator with only
  `label | starlark_label`, plus `root_string_setting: Option<String>`. The CLI
  sends the selected mode explicitly. Do not retain a compatibility shim for
  the old prototype request.
- Preserve exact missing-target terminal behavior, command-parse ownership,
  and existing one-shot/daemon runtime-error JSON families.

## Files

Production edits are limited to:

- `app/slug_commands_v2/src/cquery.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_cli_v2/src/commands/cquery.rs`;
- `app/slug_server_v2/src/lib.rs`;
- `app/slug_server_v2/src/server.rs`.

Test edits are limited to:

- `app/slug_commands_v2/tests/commands.rs`;
- focused runtime tests in `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_server_v2/src/tests.rs`.

## Acceptance

- Parser tests cover the exact output matrix, duplicate/unknown combinations,
  setting empty/Unicode/last-wins, and bare-setting rejection.
- Runtime tests prove direct setting/default/transition resolution and that
  formatting does not recompute analysis.
- A retained daemon proves C0 -> C1 -> C0 distinct/restored label bytes and
  projection, zero source invalidations, cold C0/C1 followed by warm restored
  C0 reuse, and unchanged label/provider/action topology.
- One-shot and daemon C0 output is byte-identical. Existing Starlark and missing
  outputs remain byte-identical. Malformed wire modes fail before analysis.
- A graph comparator, if needed by these tests, normalizes only the 64 lowercase
  payload following exact `slugcfg-v1:` and never any other field.

Run focused command, runtime, server, and CLI tests serially. Rebuild
`slug_cli_v2` before daemon-sensitive CLI tests, and clean stale `slugd`
processes before and after those tests. Do not set up CI.

## Stops

Stop and `REPLAN` on a second graph/key, any Bazel-looking short-ID
approximation, truncated or caller-supplied projection, projection-as-DICE,
cache, or action identity, changed Starlark-label bytes, general Starlark
evaluation, aquery/ActionKey/platform breadth, normalization outside the exact
payload, or any JVM/Java artifact, execution, helper, or delegation.
