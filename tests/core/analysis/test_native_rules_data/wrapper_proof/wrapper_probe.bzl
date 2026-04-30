# Plan 28.4 Stage 2 acceptance fixture. The Starlark rule below is
# trivial — its impl writes a sentinel and returns DefaultInfo. With
# the no-op wrapper wired in, the sentinel reaches the output
# unchanged: this is the "byte-for-byte equivalent provider results"
# acceptance bullet from Plan 28.4. Phase 28.4 Stage 3+ will move
# real ctx-method bodies through the wrapper.

_SENTINEL = "wrapped-via-rule-implementation-wrapper-noop"

def _wrapper_probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, _SENTINEL + "\n")
    return [DefaultInfo(default_output = out)]

wrapper_probe = rule(
    implementation = _wrapper_probe_impl,
    attrs = {},
)
