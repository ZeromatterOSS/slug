ORDER = "za"

def _string_setting_impl(ctx):
    return []

string_setting = rule(
    implementation = _string_setting_impl,
    build_setting = config.string(flag = True),
)

def _transition_impl(settings, attr):
    return {"//:setting": "transitioned"}

to_transition = transition(
    implementation = _transition_impl,
    inputs = [],
    outputs = ["//:setting"],
)

def _toolchain_impl(ctx):
    out = ctx.actions.declare_file("toolchain.txt")
    ctx.actions.write(out, "toolchain")
    return [
        DefaultInfo(files = depset([out])),
        platform_common.ToolchainInfo(marker = ctx.attr.marker),
    ]

demo_toolchain_impl = rule(
    implementation = _toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _actionless_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

actionless_toolchain_impl = rule(
    implementation = _actionless_toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _dependency_impl(ctx):
    if ctx.label.name == "producer" and hasattr(ctx, "outputs"):
        out = ctx.outputs.out
    else:
        out = ctx.actions.declare_file(
            "producer.out" if ctx.label.name == "producer" else ctx.label.name + ".txt",
        )
    ctx.actions.write(out, ctx.label.name)
    return [DefaultInfo(files = depset([out]))]

dependency_rule = rule(
    implementation = _dependency_impl,
    attrs = {
        "deps": attr.label_list(),
        "out": attr.output(),
    },
    toolchains = ["//:demo_type"],
)

transitioned_rule = rule(
    implementation = _dependency_impl,
    attrs = {
        "deps": attr.label_list(),
        "out": attr.output(),
    },
    toolchains = ["//:transition_type"],
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
    attrs = {
        "aliased": attr.label(),
        "deps": attr.label_list(),
        "generated": attr.label(allow_single_file = True),
        "transitioned": attr.label(cfg = to_transition),
    },
    toolchains = ["//:demo_type"],
)
