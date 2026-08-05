CustomInfo = provider(fields = {"value": "custom provider value"})
SummaryInfo = provider(fields = {
    "implicit_value": "custom value from the implicit dependency",
    "explicit_value": "custom value from the explicit dependency",
    "implicit_files": "DefaultInfo file count from the implicit dependency",
    "explicit_files": "DefaultInfo file count from the explicit dependency",
})

def _implicit_impl(ctx):
    return [CustomInfo(value = "implicit")]

implicit_rule = rule(implementation = _implicit_impl)

def _explicit_impl(ctx):
    return [
        CustomInfo(value = "explicit"),
        DefaultInfo(),
    ]

explicit_rule = rule(implementation = _explicit_impl)

def _consumer_impl(ctx):
    implicit = ctx.attr.implicit
    explicit = ctx.attr.explicit
    return [SummaryInfo(
        implicit_value = implicit[CustomInfo].value,
        explicit_value = explicit[CustomInfo].value,
        implicit_files = len(implicit[DefaultInfo].files.to_list()),
        explicit_files = len(explicit[DefaultInfo].files.to_list()),
    )]

consumer_rule = rule(
    implementation = _consumer_impl,
    attrs = {
        "implicit": attr.label(),
        "explicit": attr.label(),
    },
)
