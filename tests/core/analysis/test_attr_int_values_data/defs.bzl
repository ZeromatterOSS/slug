def _int_values_test_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "stamp=" + str(ctx.attr.stamp) + "\n")
    return [DefaultInfo(default_output = out)]

int_values_test = rule(
    implementation = _int_values_test_impl,
    attrs = {
        "stamp": attr.int(default = 0, values = [-1, 0, 1]),
    },
)
