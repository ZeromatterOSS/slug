def _toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]


toolchain_impl = rule(
    implementation = _toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)


def _string_flag_impl(ctx):
    return []


string_flag = rule(
    implementation = _string_flag_impl,
    build_setting = config.string(flag = True),
)
