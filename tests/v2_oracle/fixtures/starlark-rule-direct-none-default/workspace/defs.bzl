def _impl(ctx):
    return []

probe = rule(
    implementation = _impl,
    attrs = {
        "text": attr.string(default = "sentinel"),
        "levels": attr.string_dict(default = {"dbg": "0"}),
        "dep": attr.label(default = Label("//:fallback")),
        "_x": attr.string(default = "private"),
    },
)

mandatory_probe = rule(
    implementation = _impl,
    attrs = {"required_dep": attr.label(mandatory = True)},
)
