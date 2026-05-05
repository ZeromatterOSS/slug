# Plan 41: `_allowlist_function_transition` resolves under wrong cfg after transition

## Status: PROPOSED

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
that declares a `cfg = ` transition. Kuro's analysis machinery
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
2. What's kuro's current handling of `_allowlist_function_transition`?
   Search for the attribute name in `kuro_analysis`, `kuro_node`, and
   `kuro_interpreter_for_build`. Likely the attribute is being
   transitioned along with the rest of the rule's attrs, instead of
   pinned.
3. Is there a related Buck2 mechanism (e.g., `transition_must_be_exec`
   or an exec-cfg pin) that the kuro port hasn't surfaced for this
   particular Bazel-flavored attribute?

## Likely fix shape

`_allowlist_function_transition` should resolve once, in the rule's
parent (pre-transition) configuration, and be re-used as-is when the
target is also analyzed under the transitioned configuration. In
practice: short-circuit the attribute's configuration in the analysis
layer so it's exempt from the post-transition reconfigure check.

Search starting points:
- `app/kuro_analysis/src/analysis/...` — where rule-attr resolution
  validates pre/post consistency.
- `app/kuro_interpreter_for_build/src/attrs/...` — where
  `_allowlist_function_transition` is declared as an internal attr.
- `prelude/.../incoming_transition*` — any prelude-level handling.

## Verification

- Re-run zeromatter `//sdk:sdk_contents`; expect analysis to advance
  past `config_install_bin` into actual action graph generation.
- `examples/multi_package` should still build cleanly.
