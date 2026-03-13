# Symbolic macro definitions for testing.

def _simple_macro_impl(name, visibility = None, greeting = "hello"):
    """A simple macro that creates a genrule."""
    native.genrule(
        name = name,
        outs = [name + ".txt"],
        cmd = "echo {} > $@".format(greeting),
        visibility = visibility,
    )

simple_macro = macro(
    implementation = _simple_macro_impl,
    attrs = {
        "greeting": attr.string(default = "hello"),
    },
)

def _multi_target_macro_impl(name, visibility = None, content = "default"):
    """A macro that creates multiple targets."""
    native.genrule(
        name = name + "_gen",
        outs = [name + "_gen.txt"],
        cmd = "echo {} > $@".format(content),
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_gen"],
        visibility = visibility,
    )

multi_target_macro = macro(
    implementation = _multi_target_macro_impl,
    attrs = {
        "content": attr.string(default = "default"),
    },
)

def _no_attrs_macro_impl(name, visibility = None):
    """A macro with no custom attributes."""
    native.genrule(
        name = name,
        outs = [name + ".txt"],
        cmd = "echo no_attrs > $@",
        visibility = visibility,
    )

no_attrs_macro = macro(
    implementation = _no_attrs_macro_impl,
)
