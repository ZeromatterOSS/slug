def _impl(ctx):
    return [DefaultInfo()]

suffix_test = rule(
    implementation = _impl,
)
