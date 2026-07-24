def _alias_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output, ctx.attr.marker)
    return [DefaultInfo(files = depset([output]))]


def _ordinary_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output, ctx.toolchains["@subject//:ordinary_toolchain_type"].marker)
    return [DefaultInfo(files = depset([output]))]


ordinary_consumer = rule(
    implementation = _ordinary_consumer_impl,
    toolchains = ["@subject//:ordinary_toolchain_type"],
)


def _dev_toolchain_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output, ctx.toolchains["@subject//:dev_toolchain_type"].marker)
    return [DefaultInfo(files = depset([output]))]


dev_toolchain_consumer = rule(
    implementation = _dev_toolchain_consumer_impl,
    toolchains = ["@subject//:dev_toolchain_type"],
)


def _dev_platform_consumer_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output, ctx.toolchains["@subject//:dev_platform_toolchain_type"].marker)
    return [DefaultInfo(files = depset([output]))]


dev_platform_consumer = rule(
    implementation = _dev_platform_consumer_impl,
    toolchains = ["@subject//:dev_platform_toolchain_type"],
)


alias_consumer = rule(
    implementation = _alias_consumer_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)


def _root_string_flag_impl(ctx):
    return []


root_string_flag = rule(
    implementation = _root_string_flag_impl,
    build_setting = config.string(flag = True),
)
