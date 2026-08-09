def _string_setting_impl(ctx):
    return [config_common.BuildSettingInfo(value = ctx.build_setting_value)]

string_setting = rule(
    implementation = _string_setting_impl,
    build_setting = config.string(flag = True),
)

def _transition_impl(settings, attr):
    return {"//:setting": "transitioned"}

to_transition = transition(
    implementation = _transition_impl,
    inputs = [],
    outputs = ["//:setting"],
)

def _ordinary_impl(ctx):
    if ctx.attr.out:
        ctx.actions.write(ctx.outputs.out, ctx.label.name + "\n")
    return [DefaultInfo(files = depset([ctx.outputs.out] if ctx.attr.out else []))]

ordinary_rule = rule(
    implementation = _ordinary_impl,
    attrs = {
        "normal": attr.label(),
        "transitioned": attr.label(cfg = to_transition),
        "aliased": attr.label(),
        "src": attr.label(allow_single_file = True),
        "generated": attr.label(allow_single_file = True),
        "out": attr.output(),
    },
)
