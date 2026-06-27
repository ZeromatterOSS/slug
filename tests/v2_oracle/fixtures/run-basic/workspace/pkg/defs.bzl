def _is_windows(ctx):
    return ctx.configuration.host_path_separator == ";"

def _script_binary_impl(ctx):
    is_windows = _is_windows(ctx)
    out = ctx.actions.declare_file(ctx.label.name + (".bat" if is_windows else ".sh"))
    if is_windows:
        content = "@echo off\necho hello slug\necho " + ctx.attr.message + "\n"
    else:
        content = "#!/usr/bin/env bash\nset -euo pipefail\necho \"hello slug\"\necho \"%s\"\n" % ctx.attr.message
    ctx.actions.write(out, content, is_executable = True)
    return [DefaultInfo(executable = out)]

script_binary = rule(
    implementation = _script_binary_impl,
    attrs = {"message": attr.string(mandatory = True)},
    executable = True,
)