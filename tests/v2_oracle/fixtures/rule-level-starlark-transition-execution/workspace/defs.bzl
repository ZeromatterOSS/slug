def _empty(ctx):
    return [DefaultInfo()]

string_setting = rule(implementation = _empty, build_setting = config.string(flag = True))
int_setting = rule(implementation = _empty, build_setting = config.int(flag = True))
bool_setting = rule(implementation = _empty, build_setting = config.bool(flag = True))

def _checked(ctx):
    if ctx.attr.selected != "selected":
        fail("final configured selector was not re-resolved: " + ctx.attr.selected)
    return [DefaultInfo()]

def _identity(settings, attr):
    return None

identity_t = transition(implementation = _identity, inputs = [], outputs = [])
identity_rule = rule(implementation = _empty, cfg = identity_t)

def _idempotent(settings, attr):
    if hasattr(attr, "selected"):
        fail("selector reading a transition output was not omitted")
    if type(attr.dep) != "Label":
        fail("label-bearing transition attr was not a Label")
    return {"//:mode": attr.desired}

idempotent_t = transition(
    implementation = _idempotent,
    inputs = ["//:mode"],
    outputs = ["//:mode"],
)
idempotent_rule = rule(
    implementation = _checked,
    cfg = idempotent_t,
    attrs = {
        "desired": attr.string(),
        "selected": attr.string(),
        "dep": attr.label(),
    },
)

def _toggle(settings, attr):
    return {
        "//:mode": "changed" if settings["//:mode"] == "default" else "default",
    }

toggle_t = transition(
    implementation = _toggle,
    inputs = ["//:mode"],
    outputs = ["//:mode"],
)
non_idempotent_rule = rule(
    implementation = _checked,
    cfg = toggle_t,
    attrs = {"selected": attr.string()},
)

OUTPUTS = ["//:text", "//:count", "//:enabled"]

def _patch(settings, attr):
    return {"//:text": "patch", "//:count": 2, "//:enabled": True}

def _dict(settings, attr):
    return {"only": {"//:text": "dict", "//:count": 3, "//:enabled": True}}

def _sequence(settings, attr):
    return [{"//:text": "sequence", "//:count": 4, "//:enabled": True}]

def _missing(settings, attr):
    return {"//:text": "missing", "//:count": 5}

def _extra(settings, attr):
    return {"//:text": "extra", "//:count": 6, "//:enabled": True, "//:other": "bad"}

def _wrong(settings, attr):
    return {"//:text": 7, "//:count": 7, "//:enabled": True}

def _split(settings, attr):
    return [
        {"//:text": "a", "//:count": 1, "//:enabled": True},
        {"//:text": "b", "//:count": 2, "//:enabled": True},
    ]

def _typed_rule(implementation):
    return rule(
        implementation = _empty,
        cfg = transition(implementation = implementation, inputs = [], outputs = OUTPUTS),
    )

patch_rule = _typed_rule(_patch)
dict_rule = _typed_rule(_dict)
sequence_rule = _typed_rule(_sequence)
missing_rule = _typed_rule(_missing)
extra_rule = _typed_rule(_extra)
wrong_rule = _typed_rule(_wrong)
split_rule = _typed_rule(_split)

PLATFORMS = ["//command_line_option:platforms"]

def _platform(settings, attr):
    return {"//command_line_option:platforms": [attr.platform, "//:other_platform"]}

def _scalar_platform_label(settings, attr):
    return {"//command_line_option:platforms": attr.platform}

def _missing_platform(settings, attr):
    return {"//command_line_option:platforms": "//:does_not_exist"}

def _comma_platform(settings, attr):
    return {"//command_line_option:platforms": "//:alt,//:other_platform"}

def _comma_sequence_platform(settings, attr):
    return {"//command_line_option:platforms": ["//:alt,//:other_platform", Label("//:other_platform")]}

platform_rule = rule(
    implementation = _checked,
    cfg = transition(implementation = _platform, inputs = PLATFORMS, outputs = PLATFORMS),
    attrs = {"platform": attr.label(), "selected": attr.string()},
)
scalar_platform_label_rule = rule(
    implementation = _empty,
    cfg = transition(implementation = _scalar_platform_label, inputs = PLATFORMS, outputs = PLATFORMS),
    attrs = {"platform": attr.label()},
)
missing_platform_rule = rule(
    implementation = _empty,
    cfg = transition(implementation = _missing_platform, inputs = PLATFORMS, outputs = PLATFORMS),
)
comma_platform_rule = rule(
    implementation = _checked,
    cfg = transition(implementation = _comma_platform, inputs = PLATFORMS, outputs = PLATFORMS),
    attrs = {"selected": attr.string()},
)
comma_sequence_platform_rule = rule(
    implementation = _checked,
    cfg = transition(implementation = _comma_sequence_platform, inputs = PLATFORMS, outputs = PLATFORMS),
    attrs = {"selected": attr.string()},
)

def _ambiguous_transition(settings, attr):
    if hasattr(attr, "selected"):
        fail("AMBIGUOUS_ATTR_LEAKED_TO_TRANSITION")
    return {"//:mode": "changed"}

ambiguous_rule = rule(
    implementation = _empty,
    cfg = transition(implementation = _ambiguous_transition, inputs = [], outputs = ["//:mode"]),
    attrs = {"selected": attr.string()},
)

def _child(ctx):
    if ctx.attr.selected != "selected":
        fail("attribute transition did not observe the final owner configuration")
    return [DefaultInfo()]

child_rule = rule(implementation = _child, attrs = {"selected": attr.string()})

def _child_transition(settings, attr):
    if settings["//:parent_mode"] != "on" or attr.desired != "child_on":
        fail("attribute transition did not receive final owner settings/attrs")
    return {"//:child_mode": attr.desired}

child_t = transition(
    implementation = _child_transition,
    inputs = ["//:parent_mode"],
    outputs = ["//:child_mode"],
)

def _parent_transition(settings, attr):
    return {"//:parent_mode": "on"}

parent_t = transition(
    implementation = _parent_transition,
    inputs = [],
    outputs = ["//:parent_mode"],
)
parent_rule = rule(
    implementation = _empty,
    cfg = parent_t,
    attrs = {"dep": attr.label(cfg = child_t), "desired": attr.string()},
)
