"""Rule definitions for Bazel-compatible query tests."""


def _simple_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.label.name)
    return [DefaultInfo(default_output = out)]


lib_rule = rule(
    implementation = _simple_impl,
    attrs = {
        "deps": attr.label_list(default = []),
    },
)

app_rule = rule(
    implementation = _simple_impl,
    attrs = {
        "deps": attr.label_list(default = []),
        "srcs": attr.label_list(default = []),
        "app_name": attr.string(default = ""),
    },
)
