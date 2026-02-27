def _trivial_build_impl(ctx):
    out = ctx.actions.declare_output("out.txt")
    ctx.actions.write(out, "trivial")
    return [DefaultInfo(default_output = out)]

trivial_build = rule(
    impl = _trivial_build_impl,
    attrs = {},
)
