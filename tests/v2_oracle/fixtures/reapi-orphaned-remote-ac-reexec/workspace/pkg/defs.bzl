def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(outputs = [out], command = "printf cached > $1", arguments = [out.path])
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(implementation = _impl)