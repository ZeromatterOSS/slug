"""Tests for provider(init=...) Bazel API."""

# ============================================================================
# provider(init=fn) - basic init function
# ============================================================================

def _my_info_init(value):
    """Init function that normalizes the 'value' field."""
    return {"value": value.upper() if type(value) == "string" else str(value)}

MyInfo, _my_info_new = provider(
    doc = "A provider with init function.",
    fields = ["value"],
    init = _my_info_init,
)


def _init_provider_rule_impl(ctx):
    """Creates a provider via init function."""
    info = MyInfo(ctx.attr.input_value)
    out = ctx.actions.declare_file("init_result.txt")
    ctx.actions.write(out, info.value)
    return [DefaultInfo(default_output = out)]


init_provider_rule = rule(
    implementation = _init_provider_rule_impl,
    attrs = {
        "input_value": attr.string(default = "hello"),
    },
)


# ============================================================================
# provider(init=fn) - raw constructor bypass
# ============================================================================

def _raw_constructor_rule_impl(ctx):
    """Uses the raw constructor to bypass init."""
    # _my_info_new is the raw constructor, bypasses init
    info = _my_info_new(value = ctx.attr.input_value)
    out = ctx.actions.declare_file("raw_result.txt")
    ctx.actions.write(out, info.value)
    return [DefaultInfo(default_output = out)]


raw_constructor_rule = rule(
    implementation = _raw_constructor_rule_impl,
    attrs = {
        "input_value": attr.string(default = "hello"),
    },
)


# ============================================================================
# provider(init=fn) - init with multiple fields
# ============================================================================

def _multi_field_init(label_name, count = 0):
    """Init function that validates and transforms multiple fields."""
    return {
        "label_name": label_name.strip(),
        "count": count + 1,  # init increments count by 1
        "display": "{} (count={})".format(label_name.strip(), count + 1),
    }

MultiInfo, _multi_info_new = provider(
    fields = ["label_name", "count", "display"],
    init = _multi_field_init,
)


def _multi_init_rule_impl(ctx):
    """Tests provider init with multiple fields."""
    info = MultiInfo(label_name = ctx.attr.label_name, count = ctx.attr.count)
    out = ctx.actions.declare_file("multi_result.txt")
    ctx.actions.write(
        out,
        "label_name={}\ncount={}\ndisplay={}".format(
            info.label_name, info.count, info.display,
        ),
    )
    return [DefaultInfo(default_output = out)]


multi_init_rule = rule(
    implementation = _multi_init_rule_impl,
    attrs = {
        "label_name": attr.string(default = "test"),
        "count": attr.int(default = 0),
    },
)


# ============================================================================
# provider(init=fn) - provider type checking with init
# ============================================================================

def _validator_init(files, headers = None):
    """Init function for a CC-like provider with validation."""
    if headers == None:
        headers = []
    return {
        "files": files,
        "headers": headers,
        "count": len(files),
    }

ValidatedInfo, _validated_new = provider(
    fields = ["files", "headers", "count"],
    init = _validator_init,
)


def _validated_provider_rule_impl(ctx):
    """Returns a ValidatedInfo via init function."""
    info = ValidatedInfo(files = ctx.files.srcs, headers = ctx.files.hdrs)
    out = ctx.actions.declare_file("validated_result.txt")
    ctx.actions.write(
        out,
        "file_count={}\nheader_count={}".format(info.count, len(info.headers)),
    )
    return [DefaultInfo(default_output = out)]


validated_provider_rule = rule(
    implementation = _validated_provider_rule_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "hdrs": attr.label_list(allow_files = True),
    },
)
