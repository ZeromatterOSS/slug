MyInfo = provider(fields = ["value"])


def _provides_missing_impl(ctx):
    """A rule that declares MyInfo in provides but does NOT return it."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "missing provider\n")
    return [DefaultInfo(default_output = out)]


provides_missing_rule = rule(
    implementation = _provides_missing_impl,
    provides = [MyInfo],
    attrs = {},
)
