def _test_impl(ctx):
    return [DefaultInfo()]

oracle_test = rule(
    implementation = _test_impl,
    test = True,
)
