def _tc_impl(ctx):
    return [platform_common.ToolchainInfo(value = "exec-group")]

linux_toolchain_impl = rule(implementation = _tc_impl)

def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        command = "printf exec-group > $1",
        arguments = [out.path],
        exec_group = "compile",
    )
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(
    implementation = _probe_impl,
    exec_groups = {
        "compile": exec_group(toolchains = ["//:demo_type"]),
    },
)