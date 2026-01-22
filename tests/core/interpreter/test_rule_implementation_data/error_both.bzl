# Rule with both impl and implementation - should fail

def _simple_impl(ctx):
    return [DefaultInfo()]

both_params_rule = rule(
    impl = _simple_impl,
    implementation = _simple_impl,
    attrs = {},
)
