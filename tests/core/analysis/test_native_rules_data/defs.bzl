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


def _cc_configure_features_test_impl(ctx):
    """Tests that cc_common.configure_features() respects requested/unsupported features."""
    # Default configuration
    default_fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Configuration with requested features
    with_features = cc_common.configure_features(
        cc_toolchain = None,
        ctx = ctx,
        requested_features = ["my_custom_feature", "c++17"],
    )

    # Configuration with unsupported features (disable pic)
    without_pic = cc_common.configure_features(
        cc_toolchain = None,
        ctx = ctx,
        unsupported_features = ["pic", "supports_pic"],
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "default_type=" + type(default_fc),
        # Check some default features
        "default_supports_dynamic_linker=" + str(cc_common.is_enabled(feature_configuration = default_fc, feature_name = "supports_dynamic_linker")),
        "default_compiler_param_file=" + str(cc_common.is_enabled(feature_configuration = default_fc, feature_name = "compiler_param_file")),
        "default_my_custom=" + str(cc_common.is_enabled(feature_configuration = default_fc, feature_name = "my_custom_feature")),
        # Check requested features are enabled
        "with_custom=" + str(cc_common.is_enabled(feature_configuration = with_features, feature_name = "my_custom_feature")),
        "with_c++17=" + str(cc_common.is_enabled(feature_configuration = with_features, feature_name = "c++17")),
        # Check unsupported features are disabled
        "without_pic=" + str(cc_common.is_enabled(feature_configuration = without_pic, feature_name = "pic")),
        "without_supports_pic=" + str(cc_common.is_enabled(feature_configuration = without_pic, feature_name = "supports_pic")),
        # Unsupported doesn't affect other features
        "without_pic_dynamic_linker=" + str(cc_common.is_enabled(feature_configuration = without_pic, feature_name = "supports_dynamic_linker")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_configure_features_test = rule(
    implementation = _cc_configure_features_test_impl,
    attrs = {},
)


def _cc_linker_input_test_impl(ctx):
    """Tests that cc_common.create_linker_input() preserves user_link_flags."""
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        user_link_flags = depset(["-lpthread", "-lm"]),
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    flags = linker_input.user_link_flags
    lines = [
        "type=" + type(linker_input),
        "has_user_link_flags=" + str(hasattr(linker_input, "user_link_flags")),
        "flags_type=" + type(flags),
        "flags_list=" + str(sorted(flags.to_list())),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_linker_input_test = rule(
    implementation = _cc_linker_input_test_impl,
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


def _cc_merge_infos_test_impl(ctx):
    """Tests that cc_common.merge_cc_infos() merges CcInfo providers."""
    # Create two CcInfo instances with compilation contexts
    comp_ctx1 = cc_common.create_compilation_context(
        headers = depset([]),
        defines = depset(["DEF1=1"]),
        includes = depset(["inc1/"]),
    )
    comp_ctx2 = cc_common.create_compilation_context(
        headers = depset([]),
        defines = depset(["DEF2=2"]),
        includes = depset(["inc2/"]),
    )
    info1 = CcInfo(compilation_context = comp_ctx1)
    info2 = CcInfo(compilation_context = comp_ctx2)

    merged = cc_common.merge_cc_infos(cc_infos = [info1, info2])

    # Verify merged compilation context has defines from BOTH inputs
    merged_defines = merged.compilation_context.defines.to_list()
    merged_includes = merged.compilation_context.includes.to_list()

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(merged),
        "has_compilation_context=" + str(hasattr(merged, "compilation_context")),
        "has_linking_context=" + str(hasattr(merged, "linking_context")),
        "comp_ctx_type=" + type(merged.compilation_context),
        "defines_count=" + str(len(merged_defines)),
        "defines=" + ",".join(sorted(merged_defines)),
        "includes_count=" + str(len(merged_includes)),
        "includes=" + ",".join(sorted(merged_includes)),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_merge_infos_test = rule(
    implementation = _cc_merge_infos_test_impl,
    attrs = {},
)


def _existing_rules_test_impl(ctx):
    """Writes existing_rules info captured at load time."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(ctx.attr.items) + "\n")
    return [DefaultInfo(default_output = out)]


existing_rules_test = rule(
    implementation = _existing_rules_test_impl,
    attrs = {
        "items": attr.string_list(default = []),
    },
)


def capture_existing_rules(name):
    """Macro that captures native.existing_rules() and writes the info."""
    rules = native.existing_rules()
    items = []
    for rule_name, rule_info in rules.items():
        kind = rule_info.get("kind", "MISSING")
        items.append("{}={}".format(rule_name, kind))

    # Also verify that attributes are returned for a known target
    original_rule = rules.get("original")
    if original_rule:
        items.append("original_has_items=" + str("items" in original_rule))
        original_items = original_rule.get("items", [])
        items.append("original_items=" + ",".join(original_items))

    # Verify existing_rule() returns attributes too
    single = native.existing_rule("source_files")
    if single:
        items.append("single_kind=" + single.get("kind", "MISSING"))
        items.append("single_has_srcs=" + str("srcs" in single))

    items.append("repo=" + native.repository_name())
    existing_rules_test(
        name = name,
        items = items,
    )


# Verify that hasattr(native, "starlark_doc_extract") returns True
# This is critical for rules_python IS_BAZEL_7_OR_HIGHER detection.
HAS_STARLARK_DOC_EXTRACT = hasattr(native, "starlark_doc_extract")
if not HAS_STARLARK_DOC_EXTRACT:
    fail("hasattr(native, 'starlark_doc_extract') must be True for Bazel 7+ compat")


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
