"""Rules for testing glob() in BUILD files."""


def _files_list_impl(ctx):
    """Writes sorted file basenames to output for glob verification."""
    names = sorted([f.basename for f in ctx.files.srcs])
    out = ctx.actions.declare_file("files.txt")
    ctx.actions.write(out, "\n".join(names))
    return [DefaultInfo(default_output = out)]


files_list = rule(
    implementation = _files_list_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)
