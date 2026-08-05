# Current Slug V2 Packet

Packet: `WP-6-m2-positive-string-build-setting-transition-design`
Milestone: M2 semantic target configuration inputs and transitions
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design-only ownership and implementation-boundary decision
Evidence: accepted positive Bazel 9.2 fixture `b12774b9`; recursive configured
analysis; bounded root Starlark-label cquery; invalid-transition checksum stop.

Design only the successful root string build-setting and outgoing user-
transition vertical proven by
`tests/v2_oracle/fixtures/string-build-setting-transition/`. Add no Rust,
fixture, expected artifact, Cargo, wire, command, or harness change.

The design must decide and freeze:

1. the immutable semantic representation for one root string build-setting
   value, its equality/hash/`Allocative` behavior, and its relationship to the
   existing opaque `ConfigurationKey` without claiming Bazel's checksum;
2. the DICE/command input owner for the default and explicit
   `--//:setting=<value>` states, including same-daemon A-to-B-to-A restoration
   with no process-global or direct filesystem state;
3. the loading and evaluator owner for `config.string(flag = True)`,
   `build_setting_default`, `ctx.build_setting_value`, transition definitions,
   and `attr.label(cfg = transition)` in exactly the accepted fixture shape;
4. how recursive analysis applies each transition before constructing the
   dependency `ConfiguredTargetKey`, so two edges to the same label compute
   distinct values through the existing analysis graph and preserve declared
   attribute order;
5. whether a truthful bounded command observation can reuse the current cquery
   boundary, or whether Starlark-file/provider rendering must remain an oracle-
   only acceptance gate and the first implementation stay internal;
6. exact activation/equality evidence for default, command override, two
   transitioned children, warm reuse, transition edit/restoration, and default
   edit/restoration;
7. the precise production/test allowlists, formatted line caps, serial
   validation, GNU-Windows applicability, and independent review boundary; and
8. retained utility reuse under the Stage 9/Buck2 audit, with no default hash
   collection, duplicated label/configuration identity, or unbounded owned-
   string graph churn.

Successful semantics only. Invalid or missing transition programs and every
configured-analysis failure diagnostic remain deferred because Bazel's error
envelope prints the unavailable configuration checksum. The design must make
that unsupported boundary explicit without normalizing, parsing, fabricating,
or exposing `first-build`.

Use parallel read-only live-owner and pinned Bazel/Buck2/utility audits, root
synthesis, and independent reserved review. Reuse the accepted oracle; do not
invoke Bazel. Return `REPLAN` if the bounded semantic slice needs general
native option modeling, default/label cquery output, arbitrary cquery Starlark
execution, a second configured graph/key family, command-owned analysis,
direct filesystem discovery, exec/host/split/repository transitions,
`select`/`config_setting`, platform/toolchain/action execution, REAPI, or a
lock across DICE computation.
