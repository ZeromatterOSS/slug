ORDER = "za"

def _toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

demo_toolchain_impl = rule(
    implementation = _toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _dependency_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.label.name)
    return [DefaultInfo(files = depset([out]))]

dependency_rule = rule(
    implementation = _dependency_impl,
    attrs = {"deps": attr.label_list()},
    toolchains = ["//:demo_type"],
)

def _root_impl(ctx):
    z = ctx.actions.declare_file("z-root.txt")
    a = ctx.actions.declare_file("a-root.txt")
    if ORDER == "za":
        ctx.actions.write(z, "z")
        ctx.actions.write(a, "a")
    else:
        ctx.actions.write(a, "a")
        ctx.actions.write(z, "z")
    return [DefaultInfo(files = depset([z, a]))]

root_rule = rule(
    implementation = _root_impl,
    attrs = {"deps": attr.label_list()},
    toolchains = ["//:demo_type"],
)
