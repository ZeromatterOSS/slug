def _subject_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    marker = ctx.file.marker
    ctx.actions.expand_template(
        template = marker,
        output = output,
        substitutions = {},
    )
    return [DefaultInfo(files = depset([output]))]

subject_consumer = rule(
    implementation = _subject_consumer_impl,
    attrs = {"marker": attr.label(allow_single_file = True)},
)
