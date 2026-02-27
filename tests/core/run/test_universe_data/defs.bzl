def _stub_impl(ctx):
    return [DefaultInfo()]

stub = rule(impl = _stub_impl, attrs = {"deps": attr.label_list(default = [])})

def _run_python(ctx):
    return [
        DefaultInfo(),
        RunInfo(args = cmd_args("python3", "-c", ctx.attrs.script)),
    ]

run_python = rule(
    impl = _run_python,
    attrs = {
        "script": attrs.string(),
    },
)

def _transition_to_reindeer_impl(platform, refs):
    _ignore = (platform, refs)  # buildifier: disable=unused-variable
    return PlatformInfo(label = "transitioned-to-reindeer", configuration = ConfigurationInfo(constraints = {}, values = {}))

transition_to_reindeer = transition(
    impl = _transition_to_reindeer_impl,
    refs = {},
)

def _transitioned_impl(ctx):
    return [
        DefaultInfo(),
        RunInfo(args = cmd_args("python3", "-c", ctx.attrs.script)),
    ]

transitioned = rule(
    impl = _transitioned_impl,
    attrs = {
        "script": attrs.string(),
    },
    # The configuration transition.
    cfg = transition_to_reindeer,
)
