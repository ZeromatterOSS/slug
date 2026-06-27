MyInfo = provider(fields = ["value"])

def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    hidden = ctx.actions.declare_file(ctx.label.name + ".hidden.txt")
    ctx.actions.write(out, "default\n")
    ctx.actions.write(hidden, "hidden\n")
    return [
        DefaultInfo(files = depset([out])),
        OutputGroupInfo(hidden_top_level = depset([hidden])),
        MyInfo(value = "provider"),
    ]

probe_rule = rule(implementation = _probe_impl)