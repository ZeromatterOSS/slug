def _stub_impl(ctx):
    return [DefaultInfo()]

stub = rule(
    impl = _stub_impl,
    attrs = {},
)
