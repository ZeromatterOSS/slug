def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "runfiles\n", is_executable = True)
    return [DefaultInfo(
        executable = out,
        files = depset([out]),
        runfiles = ctx.runfiles(files = [out]),
    )]

probe_rule = rule(implementation = _probe_impl, executable = True)