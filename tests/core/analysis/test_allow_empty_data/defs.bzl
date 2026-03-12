def _nonempty_deps_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "ok\n")
    return [DefaultInfo(default_output = out)]

nonempty_deps_rule = rule(
    implementation = _nonempty_deps_impl,
    attrs = {
        "deps": attr.label_list(allow_empty = False),
    },
)

def _nonempty_strings_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ",".join(ctx.attr.items) + "\n")
    return [DefaultInfo(default_output = out)]

nonempty_strings_rule = rule(
    implementation = _nonempty_strings_impl,
    attrs = {
        "items": attr.string_list(allow_empty = False),
    },
)
