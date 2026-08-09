def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "tool\n")
    return [DefaultInfo(executable = out)]

exec_arbitrary = rule(
    implementation = _impl,
    executable = True,
)

plain_rule = rule(
    implementation = lambda ctx: [DefaultInfo()],
)

missing_exec = rule(
    implementation = lambda ctx: [DefaultInfo()],
    executable = True,
)
