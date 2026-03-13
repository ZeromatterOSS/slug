"""Tests for select() with config_setting using values, define_values, and flag_values."""


def _write_value_impl(ctx):
    """Rule that writes its 'value' attribute to a file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value)
    return [DefaultInfo(default_output = out)]


write_value_rule = rule(
    implementation = _write_value_impl,
    attrs = {
        "value": attr.string(default = "unset"),
    },
)


def _write_list_impl(ctx):
    """Rule that writes its 'items' list to a file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ",".join(ctx.attr.items))
    return [DefaultInfo(default_output = out)]


write_list_rule = rule(
    implementation = _write_list_impl,
    attrs = {
        "items": attr.string_list(default = []),
    },
)


def _write_deps_impl(ctx):
    """Rule that writes the number of deps to a file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "deps=" + str(len(ctx.attr.deps)))
    return [DefaultInfo(default_output = out)]


write_deps_rule = rule(
    implementation = _write_deps_impl,
    attrs = {
        "deps": attr.label_list(default = []),
    },
)
