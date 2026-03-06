"""Test rules for native_rules tests."""


def _write_list_impl(ctx):
    """Writes a list of strings to an output file, one per line."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(ctx.attr.items) + "\n")
    return [DefaultInfo(default_output = out)]


write_list = rule(
    implementation = _write_list_impl,
    attrs = {
        "items": attr.string_list(default = []),
    },
)


def _collect_files_impl(ctx):
    """Collects files from deps and writes their names to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    names = []
    for f in ctx.files.deps:
        names.append(f.basename)
    ctx.actions.write(out, "\n".join(sorted(names)) + "\n")
    return [DefaultInfo(default_output = out)]


collect_files = rule(
    implementation = _collect_files_impl,
    attrs = {
        "deps": attr.label_list(allow_files = True, default = []),
    },
)


def _select_value_impl(ctx):
    """Writes the selected string value to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value + "\n")
    return [DefaultInfo(default_output = out)]


select_value = rule(
    implementation = _select_value_impl,
    attrs = {
        "value": attr.string(default = ""),
    },
)


def _bool_setting_impl(ctx):
    """A boolean build setting (no output)."""
    return []


bool_flag = rule(
    implementation = _bool_setting_impl,
    build_setting = config.bool(flag = True),
)


def _string_setting_impl(ctx):
    """A string build setting (no output)."""
    return []


string_flag = rule(
    implementation = _string_setting_impl,
    build_setting = config.string(flag = True),
)
