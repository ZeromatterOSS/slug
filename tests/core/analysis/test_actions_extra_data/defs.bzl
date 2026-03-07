# Test rules for ctx.actions.write_json, copy_file, and run with env.

# === write_json ===

def _write_json_dict_impl(ctx):
    """Write a struct (dict-like) to JSON."""
    data = struct(key = "value", num = 42)
    out = ctx.actions.declare_file("data.json")
    ctx.actions.write_json(out, data)
    return [DefaultInfo(default_outputs = [out])]

write_json_dict_rule = rule(
    implementation = _write_json_dict_impl,
    attrs = {},
)

def _write_json_struct_impl(ctx):
    data = struct(name = "kuro", version = 1, active = True)
    out = ctx.actions.declare_file("struct.json")
    ctx.actions.write_json(out, data)
    return [DefaultInfo(default_outputs = [out])]

write_json_struct_rule = rule(
    implementation = _write_json_struct_impl,
    attrs = {},
)

def _write_json_list_impl(ctx):
    out = ctx.actions.declare_file("list.json")
    ctx.actions.write_json(out, ["a", "b", "c"])
    return [DefaultInfo(default_outputs = [out])]

write_json_list_rule = rule(
    implementation = _write_json_list_impl,
    attrs = {},
)

# === copy_file ===

def _copy_file_impl(ctx):
    out = ctx.actions.copy_file("copied.txt", ctx.file.src)
    return [DefaultInfo(default_outputs = [out])]

copy_file_rule = rule(
    implementation = _copy_file_impl,
    attrs = {
        "src": attr.label(allow_single_file = True),
    },
)
