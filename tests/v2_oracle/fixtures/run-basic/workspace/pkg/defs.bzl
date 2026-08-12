def _run_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

run_toolchain_impl = rule(
    implementation = _run_toolchain_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _script_binary_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    content = """#!/usr/bin/env bash
set -euo pipefail
echo "hello slug"
echo "%s"
echo "args:$*"
echo "runfiles:${RUNFILES_DIR-unset}"
if [[ "${1-}" == "fail" ]]; then echo "run stderr" >&2; exit 7; fi
""" % ctx.attr.marker
    ctx.actions.write(out, content, is_executable = True)
    return [DefaultInfo(executable = out)]

script_binary = rule(
    implementation = _script_binary_impl,
    attrs = {"marker": attr.string(mandatory = True)},
    executable = True,
    toolchains = ["//:run_type"],
)