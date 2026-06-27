def _emit_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "aquery action shape\n")
    return [DefaultInfo(files = depset([out]))]

emit = rule(implementation = _emit_impl)