def _demo_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo()]

demo_toolchain_impl = rule(implementation = _demo_toolchain_impl)

def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.output_name)
    ctx.actions.write(out, ctx.attr.content)
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(
    implementation = _probe_impl,
    attrs = {
        "content": attr.string(mandatory = True),
        "output_name": attr.string(mandatory = True),
    },
    toolchains = ["//:demo_type"],
)
