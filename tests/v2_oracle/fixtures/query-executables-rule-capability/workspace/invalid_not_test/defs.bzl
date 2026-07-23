def _impl(ctx):
    return [DefaultInfo()]

not_test_suffix = rule(
    implementation = _impl,
    test = True,
)
