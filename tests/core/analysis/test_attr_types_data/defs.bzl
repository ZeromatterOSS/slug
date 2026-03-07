"""Test rules for attr type tests."""


def _string_dict_impl(ctx):
    """Writes key=value pairs from a string_dict attribute."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    for k, v in ctx.attr.data.items():
        lines.append(k + "=" + v)
    ctx.actions.write(out, "\n".join(sorted(lines)) + "\n")
    return [DefaultInfo(default_output = out)]


string_dict_rule = rule(
    implementation = _string_dict_impl,
    attrs = {
        "data": attr.string_dict(),
    },
)


def _string_dict_iter_impl(ctx):
    """Writes keys of a string_dict one per line."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    keys = sorted(ctx.attr.data.keys())
    ctx.actions.write(out, "\n".join(keys) + "\n")
    return [DefaultInfo(default_output = out)]


string_dict_iter_rule = rule(
    implementation = _string_dict_iter_impl,
    attrs = {
        "data": attr.string_dict(),
    },
)


def _string_list_dict_impl(ctx):
    """Writes key:val1,val2 lines from a string_list_dict attribute."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    for k, v_list in ctx.attr.data.items():
        lines.append(k + ":" + ",".join(v_list))
    ctx.actions.write(out, "\n".join(sorted(lines)) + "\n")
    return [DefaultInfo(default_output = out)]


string_list_dict_rule = rule(
    implementation = _string_list_dict_impl,
    attrs = {
        "data": attr.string_list_dict(),
    },
)


def _label_keyed_dict_impl(ctx):
    """Writes basename:value lines from a label_keyed_string_dict attribute."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    for label, val in ctx.attr.deps.items():
        # label is a Target; get its output file basenames
        for f in label.files.to_list():
            lines.append(f.basename + ":" + val)
    ctx.actions.write(out, "\n".join(sorted(lines)) + "\n")
    return [DefaultInfo(default_output = out)]


label_keyed_dict_rule = rule(
    implementation = _label_keyed_dict_impl,
    attrs = {
        "deps": attr.label_keyed_string_dict(allow_files = True),
    },
)


def _output_attr_impl(ctx):
    """Writes to the named output file declared via attr.output."""
    ctx.actions.write(ctx.outputs.out, "output_written\n")
    return [DefaultInfo(default_output = ctx.outputs.out)]


output_attr_rule = rule(
    implementation = _output_attr_impl,
    attrs = {
        "out": attr.output(),
    },
)


def _output_list_impl(ctx):
    """Writes to multiple output files declared via attr.output_list."""
    for out in ctx.outputs.outs:
        name = out.basename.replace(".txt", "")
        ctx.actions.write(out, name + "\n")
    return [DefaultInfo(files = depset(ctx.outputs.outs))]


output_list_rule = rule(
    implementation = _output_list_impl,
    attrs = {
        "outs": attr.output_list(),
    },
)


def _int_attr_impl(ctx):
    """Writes integer value to output."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, str(ctx.attr.count) + "\n")
    return [DefaultInfo(default_output = out)]


int_attr_rule = rule(
    implementation = _int_attr_impl,
    attrs = {
        "count": attr.int(default = 0),
    },
)


def _bool_attr_impl(ctx):
    """Writes bool value to output."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, str(ctx.attr.flag) + "\n")
    return [DefaultInfo(default_output = out)]


bool_attr_rule = rule(
    implementation = _bool_attr_impl,
    attrs = {
        "flag": attr.bool(default = False),
    },
)
