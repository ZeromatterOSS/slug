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


def _sibling_file_impl(ctx):
    """Declares a file relative to a sibling artifact's directory."""
    # First declare a file in a subdirectory
    original = ctx.actions.declare_file("subdir/" + ctx.label.name + "_original.txt")
    ctx.actions.write(original, "original\n")
    # Now declare a sibling file in the same directory
    sibling_out = ctx.actions.declare_file(ctx.label.name + "_sibling.txt", sibling = original)
    ctx.actions.write(sibling_out, "sibling\n")
    return [DefaultInfo(
        default_output = sibling_out,
        files = depset([original, sibling_out]),
    )]


sibling_file = rule(
    implementation = _sibling_file_impl,
    attrs = {},
)


def _stamp_file_info_impl(ctx):
    """Tests that ctx.info_file and ctx.version_file return File-like objects."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    info = ctx.info_file
    version = ctx.version_file
    lines = [
        "info_type=" + type(info),
        "info_path=" + info.path,
        "info_short_path=" + info.short_path,
        "info_basename=" + info.basename,
        "info_extension=" + info.extension,
        "version_type=" + type(version),
        "version_path=" + version.path,
        "version_short_path=" + version.short_path,
        "version_basename=" + version.basename,
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


stamp_file_info = rule(
    implementation = _stamp_file_info_impl,
    attrs = {},
)


def _run_env_info_impl(ctx):
    """Tests that RunEnvironmentInfo returns a proper provider."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    env_info = RunEnvironmentInfo(
        environment = {"MY_VAR": "hello", "OTHER_VAR": "world"},
        inherited_environment = ["PATH", "HOME"],
    )
    lines = [
        "type=" + type(env_info),
        "env_type=" + type(env_info.environment),
        "inherited_type=" + type(env_info.inherited_environment),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out), env_info]


run_env_info = rule(
    implementation = _run_env_info_impl,
    attrs = {},
)


def _cc_link_test_impl(ctx):
    """Tests that cc_common.link() is callable and returns CcLinkingOutputs."""
    feature_config = cc_common.configure_features(cc_toolchain = None, ctx = ctx)
    result = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name,
        feature_configuration = feature_config,
        cc_toolchain = None,
        output_type = "executable",
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(result),
        "has_library_to_link=" + str(hasattr(result, "library_to_link")),
        "has_executable=" + str(hasattr(result, "executable")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_link_test = rule(
    implementation = _cc_link_test_impl,
    attrs = {},
)


def _cc_compilation_context_test_impl(ctx):
    """Tests that cc_common.create_compilation_context() creates proper contexts."""
    headers_depset = depset([])
    includes_depset = depset(["include/"])
    defines_depset = depset(["FOO=1"])
    comp_ctx = cc_common.create_compilation_context(
        headers = headers_depset,
        includes = includes_depset,
        defines = defines_depset,
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(comp_ctx),
        "has_headers=" + str(hasattr(comp_ctx, "headers")),
        "has_includes=" + str(hasattr(comp_ctx, "includes")),
        "has_defines=" + str(hasattr(comp_ctx, "defines")),
        "has_system_includes=" + str(hasattr(comp_ctx, "system_includes")),
        "has_direct_headers=" + str(hasattr(comp_ctx, "direct_headers")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_compilation_context_test = rule(
    implementation = _cc_compilation_context_test_impl,
    attrs = {},
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
