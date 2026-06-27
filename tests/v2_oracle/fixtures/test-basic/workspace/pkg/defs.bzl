def _is_windows(ctx):
    return ctx.configuration.host_path_separator == ";"

def _script_test_impl(ctx):
    is_windows = _is_windows(ctx)
    out = ctx.actions.declare_file(ctx.label.name + (".bat" if is_windows else ".sh"))
    if is_windows:
        content = "@echo off\necho test-basic-pass\nexit /b 0\n"
    else:
        content = "#!/usr/bin/env bash\nset -euo pipefail\necho test-basic-pass\n"
    ctx.actions.write(out, content, is_executable = True)
    return [DefaultInfo(executable = out)]

script_test = rule(
    implementation = _script_test_impl,
    test = True,
)