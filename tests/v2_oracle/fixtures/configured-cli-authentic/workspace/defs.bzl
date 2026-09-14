def _toolchain(ctx): return [platform_common.ToolchainInfo()]
toolchain_impl = rule(implementation = _toolchain)
def _writer(ctx):
    out = ctx.actions.declare_file("shared.txt")
    ctx.actions.write(out, ctx.attr.content)
    return [DefaultInfo()]
writer = rule(implementation = _writer, attrs = {"content": attr.string()}, toolchains = ["//:kind"])
def _root(ctx):
    out = ctx.actions.declare_file("runner.sh")
    ctx.actions.write(out, "#!/bin/sh\nexit 0\n", is_executable = True)
    return [DefaultInfo(executable = out)]
root_rule = rule(implementation = _root, executable = True, attrs = {"deps": attr.label_list()}, toolchains = ["//:kind"])
