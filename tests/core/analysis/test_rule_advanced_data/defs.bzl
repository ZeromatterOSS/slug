"""Tests for advanced rule() features: provides, initializer, doc, private attrs."""

# === Custom provider with fields and doc ===

CompileInfo = provider(
    doc = "Information about compilation outputs.",
    fields = {
        "objects": "List of compiled object files",
        "defines": "List of preprocessor defines",
    },
)

LinkInfo = provider(
    doc = "Information about linking outputs.",
    fields = ["library", "shared"],
)


# === Rule with provides ===

def _compile_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".o")
    ctx.actions.write(out, "object_data")
    info = CompileInfo(
        objects = [out],
        defines = ctx.attr.defines,
    )
    return [DefaultInfo(default_output = out), info]


compile_rule = rule(
    implementation = _compile_impl,
    attrs = {
        "defines": attr.string_list(default = []),
    },
    provides = [CompileInfo],
    doc = "A rule that produces compilation outputs.",
)


# === Rule that reads providers from deps ===

def _link_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".lib")
    all_defines = []
    all_objects = []
    for dep in ctx.attr.deps:
        if CompileInfo in dep:
            ci = dep[CompileInfo]
            all_defines.extend(ci.defines)
            all_objects.extend(ci.objects)
    lines = [
        "objects=" + str(len(all_objects)),
        "defines=" + ",".join(all_defines),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


link_rule = rule(
    implementation = _link_impl,
    attrs = {
        "deps": attr.label_list(default = []),
    },
)


# === Rule with initializer ===

def _processed_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "prefix=" + ctx.attr.prefix + "\nvalue=" + ctx.attr.value)
    return [DefaultInfo(default_output = out)]


def _processed_init(name, value = "", **kwargs):
    # Initializer adds a prefix based on the name
    kwargs["prefix"] = "init_" + name
    kwargs["value"] = value.upper()
    return kwargs


processed_rule = rule(
    implementation = _processed_impl,
    attrs = {
        "prefix": attr.string(default = ""),
        "value": attr.string(default = ""),
    },
    initializer = _processed_init,
)


# === Rule with private attrs (default only, not user-settable) ===

def _with_private_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "public_val=" + ctx.attr.public_val,
        "has_tool=" + str(ctx.attr._tool != None),
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


with_private_rule = rule(
    implementation = _with_private_impl,
    attrs = {
        "public_val": attr.string(default = ""),
        "_tool": attr.string(default = "default_tool"),
    },
)


# === Rule with executable=True ===

def _executable_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/sh\necho hello", is_executable = True)
    return [DefaultInfo(
        default_output = out,
        executable = out,
    )]


executable_rule = rule(
    implementation = _executable_impl,
    attrs = {},
    executable = True,
)


# === Rule with test=True ===

def _test_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    script = "#!/bin/sh\nexit " + str(ctx.attr.exit_code)
    ctx.actions.write(out, script, is_executable = True)
    return [DefaultInfo(
        default_output = out,
        executable = out,
    )]


my_test_rule = rule(
    implementation = _test_impl,
    attrs = {
        "exit_code": attr.int(default = 0),
    },
    test = True,
)


# === Macro that exercises existing_rules() ===

def count_rules():
    """Returns the count of rules defined so far in this BUILD file."""
    return len(native.existing_rules())
