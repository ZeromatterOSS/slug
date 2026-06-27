def _tc_impl(ctx):
    return [platform_common.ToolchainInfo(value = "linux")]

linux_toolchain_impl = rule(implementation = _tc_impl)

def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.toolchains["//:demo_type"].value + "\n")
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(
    implementation = _probe_impl,
    toolchains = ["//:demo_type"],
)