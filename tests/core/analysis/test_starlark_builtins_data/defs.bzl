"""Tests for Bazel-compatible Starlark builtins: struct, json, dir, type, hasattr, getattr."""


# ============================================================================
# struct tests
# ============================================================================

def _struct_fields_impl(ctx):
    """Tests struct creation and field access."""
    s = struct(name = "test", count = 42, items = ["a", "b", "c"])
    out = ctx.actions.declare_file("struct_fields.txt")
    lines = [
        "name=" + s.name,
        "count=" + str(s.count),
        "items=" + ",".join(s.items),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


struct_fields_rule = rule(
    implementation = _struct_fields_impl,
    attrs = {},
)


def _struct_hasattr_impl(ctx):
    """Tests hasattr/getattr on structs."""
    s = struct(x = 1, y = 2)
    out = ctx.actions.declare_file("struct_hasattr.txt")
    lines = [
        "has_x=" + str(hasattr(s, "x")),
        "has_z=" + str(hasattr(s, "z")),
        "getattr_x=" + str(getattr(s, "x")),
        "getattr_z=" + str(getattr(s, "z", 99)),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


struct_hasattr_rule = rule(
    implementation = _struct_hasattr_impl,
    attrs = {},
)


def _struct_nested_impl(ctx):
    """Tests nested structs."""
    inner = struct(value = "inner_value")
    outer = struct(child = inner, label = "outer")
    out = ctx.actions.declare_file("struct_nested.txt")
    lines = [
        "outer_label=" + outer.label,
        "inner_value=" + outer.child.value,
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


struct_nested_rule = rule(
    implementation = _struct_nested_impl,
    attrs = {},
)


# ============================================================================
# json tests
# ============================================================================

def _json_encode_impl(ctx):
    """Tests json.encode() with various types."""
    out = ctx.actions.declare_file("json_encode.txt")
    # Encode a dict with string/int/bool/list
    data = {"name": "kuro", "version": 9, "stable": True, "tags": ["fast", "hermetic"]}
    encoded = json.encode(data)
    ctx.actions.write(out, encoded)
    return [DefaultInfo(default_output = out)]


json_encode_rule = rule(
    implementation = _json_encode_impl,
    attrs = {},
)


def _json_decode_impl(ctx):
    """Tests json.decode() to parse a JSON string."""
    out = ctx.actions.declare_file("json_decode.txt")
    parsed = json.decode('{"key": "hello", "num": 42}')
    lines = [
        "key=" + parsed["key"],
        "num=" + str(parsed["num"]),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


json_decode_rule = rule(
    implementation = _json_decode_impl,
    attrs = {},
)


# ============================================================================
# type() and dir() tests
# ============================================================================

def _type_and_dir_impl(ctx):
    """Tests type() and dir() builtin functions."""
    out = ctx.actions.declare_file("type_dir.txt")
    s = struct(a = 1)
    lines = [
        "type_string=" + type("hello"),
        "type_int=" + type(42),
        "type_list=" + type([1, 2]),
        "type_dict=" + type({"k": "v"}),
        "type_bool=" + type(True),
        "type_none=" + type(None),
        "type_struct=" + type(s),
        "has_a_in_dir=" + str("a" in dir(s)),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


type_and_dir_rule = rule(
    implementation = _type_and_dir_impl,
    attrs = {},
)
