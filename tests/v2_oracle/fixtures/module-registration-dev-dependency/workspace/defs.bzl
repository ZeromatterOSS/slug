def _tc_impl(ctx):
    return [platform_common.ToolchainInfo(message = "dev toolchain selected")]


tc_impl = rule(
    implementation = _tc_impl,
)


def _uses_toolchain_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.toolchains["//:tc_type"].message)
    return [DefaultInfo(files = depset([out]))]


uses_toolchain = rule(
    implementation = _uses_toolchain_impl,
    toolchains = ["//:tc_type"],
)
