"""Test rule + helpers for `--extra_execution_platforms` resolution."""

def _trivial_run_impl(ctx):
    out = ctx.actions.declare_output("out.txt")

    # `actions.run` triggers exec-platform resolution for an action-bearing
    # target. The action body itself doesn't matter — what matters is that
    # analysis succeeds, which proves the resolver picked a candidate
    # platform compatible with `exec_compatible_with`.
    ctx.actions.write(out, "ok")
    return [DefaultInfo(default_output = out)]

trivial_run = rule(
    impl = _trivial_run_impl,
    attrs = {},
)
