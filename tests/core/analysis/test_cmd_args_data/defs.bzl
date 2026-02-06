def _args_builder_impl(ctx):
    out = ctx.actions.declare_file("args.txt")

    args = ctx.actions.args()
    args.add("one")
    args.add_all(["two", "three"])
    args.add_joined(["four", "five"], join_with = ",")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_builder = rule(
    implementation = _args_builder_impl,
    attrs = {},
)
