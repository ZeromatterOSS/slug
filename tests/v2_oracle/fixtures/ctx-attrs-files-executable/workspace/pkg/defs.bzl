def _tool_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/sh\n", is_executable = True)
    return [DefaultInfo(executable = out, files = depset([out]))]

tool_rule = rule(implementation = _tool_impl, executable = True)

def _probe_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    content = "\n".join([
        "label=" + str(ctx.label),
        "attr=" + ctx.attr.message,
        "file=" + ctx.file.src.path,
        "files=" + ",".join([f.path for f in ctx.files.srcs]),
        "exe=" + ctx.executable.tool.path,
    ]) + "\n"
    ctx.actions.write(out, content)
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(
    implementation = _probe_impl,
    attrs = {
        "message": attr.string(),
        "src": attr.label(allow_single_file = True),
        "srcs": attr.label_list(allow_files = True),
        "tool": attr.label(executable = True, cfg = "exec"),
    },
)