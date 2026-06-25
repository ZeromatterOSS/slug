def _impl(ctx):
    out = ctx.actions.declare_output("out")
    ctx.actions.run(
        ["sh", "-c", "printf ok > \"$1\"", "--", out.as_output()],
        category = "write",
        env = {"CACHE_BUSTER": ctx.attrs.cache_buster},
    )
    return [DefaultInfo(default_output = out)]

my_rule = rule(
    impl = _impl,
    attrs = {
        "cache_buster": attrs.string(default = read_config("test", "cache_buster", "")),
    },
)
