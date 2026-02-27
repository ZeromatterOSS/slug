def _stub_impl(_ctx):
    return [DefaultInfo()]

stub = rule(
    impl = _stub_impl,
    attrs = {
        "configured_deps": attrs.list(attrs.configured_dep(), default = []),
        "deps": attrs.list(attrs.dep(), default = []),
        "exec_deps": attrs.list(attrs.exec_dep(), default = []),
        "labels": attrs.list(attrs.string(), default = []),
        "toolchain_deps": attrs.list(attrs.toolchain_dep(), default = []),
    },
)
