# Plan 41: `_allowlist_function_transition` resolves under wrong cfg after transition

## Status: COMPLETE (2026-05-05)

Fix landed in `app/slug_configured/src/nodes.rs`:

- New helper `configured_attrs_equal_modulo_cfg` walks `ConfiguredAttr`
  values and, for label-bearing variants (`Label`, `SourceLabel`,
  `Dep`, plus `List`/`Tuple`/`Dict`/`OneOf` recursively), compares by
  unconfigured target. Other variants fall through to direct equality.
- `verify_transitioned_attrs` now uses this helper instead of `!=`.

That accommodates the legitimate cfg change on label-typed attrs that
follows from the rule's own transition while still catching genuine
non-idempotent transitions on scalar attrs (the original purpose of
the check).

The Bazel-style all-attrs sweep in `resolve_transition_attrs` is
preserved (transitions need to read attrs via `attr.<name>`); only
the post-transition idempotency comparison is now cfg-tolerant.

ZeroMatter `//sdk:sdk_contents` advances past the
`_allowlist_function_transition`, `_always_enable_metadata_output_groups`,
and `allocator_libraries` failures into real rule analysis. Next
blocker (Plan 42): `ctx.actions.expand_template` rejects build-artifact
templates (`bazel_lib`'s `expand_template` rule passes a generated
file via `template = ctx.file.template`).

## Context

After plans 38, 39 (phases 1/1.5/1.75) and 40 unblocked extension-
spoke materialization end-to-end, zeromatter's `//sdk:sdk_contents`
build advances all the way through analysis until it hits a Buck2-
classic configuration-transition consistency error:

```
Target zeromatter//sdk/config_install:config_install_bin configuration transitioned
  old: zeromatter//bazel/platforms:linux-gnu-host#ad32165a48ea416e
  new: zeromatter//bazel/platforms:linux-gnu-host#e3788043e3bf9951
but attribute: _allowlist_function_transition
  resolved with old configuration to: "bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist (linux-gnu-host#ad32165a48ea416e)"
  resolved with new configuration to: "bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist (linux-gnu-host#e3788043e3bf9951)"
```

`_allowlist_function_transition` is Bazel's special implicit
attribute that gates user-defined transitions. It has to be available
under both the pre- and post-transition configurations of every rule
that declares a `cfg = ` transition. Slug's analysis machinery
detects that the attribute resolved to two different *configured*
labels (same target, different config hash) and bails out.

This is **not** a spoke-registration or repo-rule problem; it's a
genuine analysis-layer configuration-transition behavior. The same
target build fine in Bazel.

## Investigation needed

1. How does Bazel handle this? Is the allowlist target resolved via a
   special-cased `attr.label(default=..., cfg = unconditional)` so it
   stays in the host/exec configuration regardless of incoming
   transitions?
2. What's slug's current handling of `_allowlist_function_transition`?
   Search for the attribute name in `slug_analysis`, `slug_node`, and
   `slug_interpreter_for_build`. Likely the attribute is being
   transitioned along with the rest of the rule's attrs, instead of
   pinned.
3. Is there a related Buck2 mechanism (e.g., `transition_must_be_exec`
   or an exec-cfg pin) that the slug port hasn't surfaced for this
   particular Bazel-flavored attribute?

## Likely fix shape

`_allowlist_function_transition` should resolve once, in the rule's
parent (pre-transition) configuration, and be re-used as-is when the
target is also analyzed under the transitioned configuration. In
practice: short-circuit the attribute's configuration in the analysis
layer so it's exempt from the post-transition reconfigure check.

Search starting points:
- `app/slug_analysis/src/analysis/...` — where rule-attr resolution
  validates pre/post consistency.
- `app/slug_interpreter_for_build/src/attrs/...` — where
  `_allowlist_function_transition` is declared as an internal attr.
- `prelude/.../incoming_transition*` — any prelude-level handling.

## Verification

- Re-run zeromatter `//sdk:sdk_contents`; expect analysis to advance
  past `config_install_bin` into actual action graph generation.
- `examples/multi_package` should still build cleanly.
