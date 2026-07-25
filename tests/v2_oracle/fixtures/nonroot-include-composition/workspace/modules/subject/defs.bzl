def _copy_marker_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.expand_template(
        template = ctx.file.marker,
        output = output,
        substitutions = {},
    )
    return [DefaultInfo(files = depset([output]))]

copy_marker = rule(
    implementation = _copy_marker_impl,
    attrs = {"marker": attr.label(allow_single_file = True, mandatory = True)},
)
