def _impl(ctx):
    return [DefaultInfo()]

test_false_test = rule(
    implementation = _impl,
    test = True,
    executable = False,
)
