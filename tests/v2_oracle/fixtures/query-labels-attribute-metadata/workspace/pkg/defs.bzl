def _impl(ctx):
    return [DefaultInfo()]

probe_rule = rule(
    implementation = _impl,
    attrs = {
        "one": attr.label(),
        "many": attr.label_list(),
        "with_default": attr.label_list(default = [":default.txt"]),
        "note": attr.string(default = "plain-text"),
        "_implicit": attr.label(default = ":implicit.txt"),
        "chosen": attr.label_list(),
        "combined": attr.label_list(),
        "string_labels": attr.string_keyed_label_dict(),
        "label_strings": attr.label_keyed_string_dict(),
        "label_lists": attr.label_list_dict(),
    },
)

output_rule = rule(
    implementation = _impl,
    attrs = {
        "out": attr.output(mandatory = True),
        "outs": attr.output_list(mandatory = True),
    },
)

must_rule = rule(
    implementation = _impl,
    attrs = {"required": attr.label(mandatory = True)},
)
