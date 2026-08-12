CONTENT = "hello from an action\n"

def _write_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

write_toolchain_impl = rule(
    implementation = _write_toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _write_file_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, CONTENT)
    return [DefaultInfo(files = depset([out]))]

write_file = rule(
    implementation = _write_file_impl,
    toolchains = ["//:write_type"],
)
