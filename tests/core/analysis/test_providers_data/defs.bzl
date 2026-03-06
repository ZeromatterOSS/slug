"""Tests for provider() API and built-in providers."""

# ============================================================================
# User-defined provider via provider()
# ============================================================================

# Define a custom provider
MyInfo = provider(
    doc = "Custom provider for testing.",
    fields = {
        "message": "A string message",
        "count": "An integer count",
    },
)

TagInfo = provider(
    fields = ["value"],
)


def _my_rule_impl(ctx):
    """A rule that returns a custom provider."""
    out = ctx.actions.declare_file("my_rule_out.txt")
    ctx.actions.write(out, ctx.attr.message)
    return [
        DefaultInfo(default_output = out),
        MyInfo(message = ctx.attr.message, count = ctx.attr.count),
    ]


my_rule = rule(
    implementation = _my_rule_impl,
    attrs = {
        "message": attr.string(default = "hello"),
        "count": attr.int(default = 0),
    },
)


def _read_provider_impl(ctx):
    """Reads MyInfo from a dep and writes its fields to output."""
    dep = ctx.attr.dep
    info = dep[MyInfo]
    out = ctx.actions.declare_file("provider_read.txt")
    ctx.actions.write(out, "message={}\ncount={}".format(info.message, info.count))
    return [DefaultInfo(default_output = out)]


read_provider_rule = rule(
    implementation = _read_provider_impl,
    attrs = {
        "dep": attr.label(),
    },
)


def _check_provider_impl(ctx):
    """Checks if a provider exists using 'in' operator."""
    dep = ctx.attr.dep
    has_my_info = MyInfo in dep
    has_tag_info = TagInfo in dep
    out = ctx.actions.declare_file("provider_check.txt")
    ctx.actions.write(out, "has_my_info={}\nhas_tag_info={}".format(has_my_info, has_tag_info))
    return [DefaultInfo(default_output = out)]


check_provider_rule = rule(
    implementation = _check_provider_impl,
    attrs = {
        "dep": attr.label(),
    },
)


# ============================================================================
# OutputGroupInfo provider
# ============================================================================

def _multi_output_group_impl(ctx):
    """Returns multiple output groups via OutputGroupInfo."""
    out_a = ctx.actions.declare_file("group_a.txt")
    out_b = ctx.actions.declare_file("group_b.txt")
    ctx.actions.write(out_a, "group_a_content")
    ctx.actions.write(out_b, "group_b_content")
    return [
        DefaultInfo(files = depset([out_a])),
        OutputGroupInfo(
            group_a = depset([out_a]),
            group_b = depset([out_b]),
        ),
    ]


multi_output_group = rule(
    implementation = _multi_output_group_impl,
    attrs = {},
)


# ============================================================================
# provider with init function
# ============================================================================

# Provider created with struct-style (no fields= parameter)
FlatInfo = provider(
    fields = ["name", "srcs"],
)


def _flat_provider_impl(ctx):
    """Returns a provider with list fields."""
    info = FlatInfo(name = ctx.attr.name, srcs = ctx.files.srcs)
    out = ctx.actions.declare_file("flat_info.txt")
    ctx.actions.write(out, "name={}\nsrc_count={}".format(info.name, len(info.srcs)))
    return [DefaultInfo(default_output = out), info]


flat_provider_rule = rule(
    implementation = _flat_provider_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
    },
)


# ============================================================================
# DefaultInfo with executable
# ============================================================================

def _executable_rule_impl(ctx):
    """Returns DefaultInfo with executable set."""
    script = ctx.actions.declare_file("run_me.sh")
    ctx.actions.write(script, "#!/bin/bash\necho 'executed'", is_executable = True)
    return [DefaultInfo(executable = script)]


executable_rule = rule(
    implementation = _executable_rule_impl,
    attrs = {},
    executable = True,
)
