ProbeInfo = provider(fields = {"marker": "selected toolchain marker"})

def _demo_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

demo_toolchain_impl = rule(
    implementation = _demo_toolchain_impl,
    attrs = {
        "marker": attr.string(mandatory = True),
    },
)

def _probe_impl(ctx):
    return [ProbeInfo(marker = ctx.toolchains["//:demo_type"].marker)]

probe_rule = rule(
    implementation = _probe_impl,
    toolchains = ["//:demo_type"],
)
