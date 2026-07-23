def _impl(ctx):
    return [DefaultInfo()]

exec_arbitrary = rule(
    implementation = _impl,
    executable = True,
    attrs = {"deps": attr.label_list()},
)

plain_rule = rule(
    implementation = _impl,
    attrs = {"deps": attr.label_list()},
)

output_rule = rule(
    implementation = _impl,
    attrs = {"outs": attr.output_list()},
)

implicit_test_test = rule(
    implementation = _impl,
    test = True,
)

explicit_test_test = rule(
    implementation = _impl,
    test = True,
    executable = True,
)
