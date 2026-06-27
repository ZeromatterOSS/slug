MyInfo = provider(fields = ["value"])


def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "custom\n")
    files = depset([out])
    return [
        DefaultInfo(files = files, runfiles = ctx.runfiles(files = [out])),
        MyInfo(value = "custom"),
        OutputGroupInfo(validation = files),
    ]


custom_rule = rule(implementation = _impl)