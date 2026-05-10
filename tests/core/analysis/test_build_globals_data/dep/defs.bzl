def _write_value_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value)
    return [DefaultInfo(files = depset([out]))]

write_value_rule = rule(
    implementation = _write_value_impl,
    attrs = {
        "value": attr.string(default = ""),
    },
)
