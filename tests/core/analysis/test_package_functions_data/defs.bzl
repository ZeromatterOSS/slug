"""Test rules for package function tests."""


def _write_value_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value)
    return [DefaultInfo(default_output = out)]


write_value = rule(
    implementation = _write_value_impl,
    attrs = {
        "value": attr.string(default = ""),
    },
)


def _collect_files_impl(ctx):
    """Collects files from deps and writes their names to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    names = [f.basename for f in ctx.files.deps]
    ctx.actions.write(out, "\n".join(sorted(names)))
    return [DefaultInfo(default_output = out)]


collect_files = rule(
    implementation = _collect_files_impl,
    attrs = {
        "deps": attr.label_list(allow_files = True, default = []),
    },
)
