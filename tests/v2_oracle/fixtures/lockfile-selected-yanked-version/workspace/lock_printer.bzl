def _lock_printer_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.name + ".bat")
    ctx.actions.write(
        out,
        "@echo off\r\nfindstr /C:\"selectedYankedVersions\" /C:\"yyy@1.0.0\" \"%BUILD_WORKSPACE_DIRECTORY%\\MODULE.bazel.lock\"\r\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

lock_printer = rule(
    implementation = _lock_printer_impl,
    executable = True,
)