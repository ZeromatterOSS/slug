def _root_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.expand_template(
        template = ctx.file.dep,
        output = output,
        substitutions = {},
    )
    return [DefaultInfo(files = depset([output]))]

root_consumer = rule(
    implementation = _root_consumer_impl,
    attrs = {"dep": attr.label(allow_single_file = True)},
)
