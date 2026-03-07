# Test rules for testing.analysis_test()

MyInfo = provider(fields = ["value"])

def _my_rule_impl(ctx):
    return [DefaultInfo(), MyInfo(value = ctx.attr.val)]

my_rule = rule(
    implementation = _my_rule_impl,
    attrs = {
        "val": attr.string(default = "hello"),
    },
)

# Analysis test implementation - env/target are provided by the framework
def _check_my_info_impl(env, target):
    # In a full implementation, we'd use env to assert properties of target.
    # For now, just verify the test can be defined and analyzed.
    pass

check_my_info_test = testing.analysis_test(
    implementation = _check_my_info_impl,
    attrs = {
        "target_under_test": attr.label(),
    },
)
