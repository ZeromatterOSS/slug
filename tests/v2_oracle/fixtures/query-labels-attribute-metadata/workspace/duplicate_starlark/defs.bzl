duplicate_rule = rule(
    implementation = lambda ctx: [],
    attrs = {
        "many": attr.label_list(
            allow_files = True,
        ),
    },
)
