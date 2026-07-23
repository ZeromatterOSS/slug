def _impl(ctx):
    return []

custom_rule = rule(
    implementation = _impl,
    attrs = {"dep": attr.label()},
)
