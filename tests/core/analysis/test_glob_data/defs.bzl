"""Test rules for glob() function tests."""


def _list_files_impl(ctx):
    """Writes the sorted list of file basenames to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    names = sorted([f.basename for f in ctx.files.srcs])
    ctx.actions.write(out, "\n".join(names))
    return [DefaultInfo(default_output = out)]


list_files = rule(
    implementation = _list_files_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
    },
)
