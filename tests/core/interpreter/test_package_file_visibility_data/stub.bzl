stub = rule(
    impl = lambda ctx: [DefaultInfo()],
    attrs = {
        "deps": attr.label_list(default = []),
    },
)
