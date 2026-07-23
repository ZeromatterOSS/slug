def _passthrough_impl(ctx):
    return [DefaultInfo()]

passthrough = rule(
    implementation = _passthrough_impl,
    attrs = {"deps": attr.label_list()},
)
