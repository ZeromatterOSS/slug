def _emit_impl(ctx):
    return []

emit = rule(
    implementation = _emit_impl,
    attrs = {
        "out": attr.output(mandatory = True),
    },
)
