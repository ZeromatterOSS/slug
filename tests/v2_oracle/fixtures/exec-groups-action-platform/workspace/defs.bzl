def _tc_impl(ctx):
    return [platform_common.ToolchainInfo(value = ctx.label.name)]

toolchain_impl = rule(implementation = _tc_impl)

def _probe_impl(ctx):
    default_out = ctx.actions.declare_file("default-owner.txt")
    ctx.actions.run_shell(
        outputs = [default_out],
        command = "printf default > $1",
        arguments = [default_out.path],
        mnemonic = "DefaultOwnerProbe",
    )
    compile_out = ctx.actions.declare_file("compile-owner.txt")
    ctx.actions.run_shell(
        outputs = [compile_out],
        command = "printf compile > $1",
        arguments = [compile_out.path],
        exec_group = "compile",
        mnemonic = "CompileOwnerProbe",
    )
    return [DefaultInfo(files = depset([default_out, compile_out]))]

probe_rule = rule(
    implementation = _probe_impl,
    toolchains = ["//:default_type"],
    exec_groups = {
        "compile": exec_group(toolchains = ["//:compile_type"]),
    },
)
