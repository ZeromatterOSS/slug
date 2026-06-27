def _write_file_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "hello from an action\n")
    return [DefaultInfo(files = depset([out]))]

write_file = rule(implementation = _write_file_impl)