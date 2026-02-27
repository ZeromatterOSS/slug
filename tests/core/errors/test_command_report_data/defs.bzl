def _fail_build_impl(ctx):
    out = ctx.actions.declare_output("out.txt")
    ctx.actions.run(
        cmd_args(["false"], hidden = out.as_output()),
        category = "run",
    )
    return [DefaultInfo(default_output = out)]

fail_build = rule(
    impl = _fail_build_impl,
    attrs = {},
)
