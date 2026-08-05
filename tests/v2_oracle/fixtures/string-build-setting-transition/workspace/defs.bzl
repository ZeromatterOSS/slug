ConsumerInfo = provider(fields = {"value": "configured string build-setting value"})
ParentInfo = provider(fields = {"value": "left and right configured consumer values"})
SettingInfo = provider(fields = {"value": "string build-setting value"})

def _string_setting_impl(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]

string_setting = rule(
    implementation = _string_setting_impl,
    build_setting = config.string(flag = True),
)

def _consumer_impl(ctx):
    return [ConsumerInfo(value = ctx.attr._setting[SettingInfo].value)]

consumer_rule = rule(
    implementation = _consumer_impl,
    attrs = {
        "_setting": attr.label(default = "//:setting"),
    },
)

def _left_transition_impl(settings, attr):
    return {"//:setting": "left"}

left_transition = transition(
    implementation = _left_transition_impl,
    inputs = [],
    outputs = ["//:setting"],
)

def _right_transition_impl(settings, attr):
    return {"//:setting": "right"}

right_transition = transition(
    implementation = _right_transition_impl,
    inputs = [],
    outputs = ["//:setting"],
)

def _parent_impl(ctx):
    return [ParentInfo(value = "%s,%s" % (
        ctx.attr.left[0][ConsumerInfo].value,
        ctx.attr.right[0][ConsumerInfo].value,
    ))]

parent_rule = rule(
    implementation = _parent_impl,
    attrs = {
        "left": attr.label(cfg = left_transition),
        "right": attr.label(cfg = right_transition),
    },
)
