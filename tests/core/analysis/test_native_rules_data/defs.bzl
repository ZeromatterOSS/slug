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


def _cc_command_line_test_impl(ctx):
    """Tests cc_common.get_tool_for_action() and get_memory_inefficient_command_line()."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Get the compiler tool
    compiler_path = cc_common.get_tool_for_action(
        feature_configuration = fc,
        action_name = "c++-compile",
    )

    # Get command line for a compile action
    compile_cmdline = cc_common.get_memory_inefficient_command_line(
        feature_configuration = fc,
        action_name = "c++-compile",
        variables = cc_common.create_compile_variables(
            cc_toolchain = None,
            feature_configuration = fc,
            source_file = "test.cc",
            output_file = "test.o",
        ),
    )

    # Get the linker tool
    linker_path = cc_common.get_tool_for_action(
        feature_configuration = fc,
        action_name = "c++-link-executable",
    )

    # Get command line for a link action (with output_file)
    link_cmdline = cc_common.get_memory_inefficient_command_line(
        feature_configuration = fc,
        action_name = "c++-link-executable",
        variables = cc_common.create_link_variables(
            cc_toolchain = None,
            feature_configuration = fc,
            output_file = "my_binary.exe",
        ),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "compiler_path=" + compiler_path,
        "compile_cmdline_len=" + str(len(compile_cmdline)),
        "has_source_in_compile=" + str(any(["test.cc" in str(a) for a in compile_cmdline])),
        "has_output_in_compile=" + str(any(["test.o" in str(a) for a in compile_cmdline])),
        "linker_path=" + linker_path,
        "link_cmdline_len=" + str(len(link_cmdline)),
        "has_output_in_link=" + str(any(["my_binary" in str(a) for a in link_cmdline])),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


cc_command_line_test = rule(
    implementation = _cc_command_line_test_impl,
    attrs = {},
)


def _java_common_test_impl(ctx):
    """Tests that java_common module is available and has expected attributes."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    # Test JavaInfo is callable
    java_info = JavaInfo(compile_jar = None, output_jar = None)
    has_compile_jar = hasattr(java_info, "compile_jar")
    has_output_jar = hasattr(java_info, "output_jar")

    # Test java_common.compile() returns a JavaInfo instance
    compile_result = java_common.compile(ctx = ctx)
    compile_has_compile_jar = hasattr(compile_result, "compile_jar")
    compile_has_transitive = hasattr(compile_result, "transitive_compile_time_jars")

    # Test java_common.merge() returns a JavaInfo instance
    merge_result = java_common.merge([])
    merge_has_compile_jar = hasattr(merge_result, "compile_jar")

    # Test JavaPluginInfo is callable
    plugin_info = JavaPluginInfo(runtime_deps = [], processor_class = "com.example.Proc")
    has_processor_class = hasattr(plugin_info, "processor_class")

    # Test java_common.JavaRuntimeInfo and JavaToolchainInfo attributes
    has_runtime_info = hasattr(java_common, "JavaRuntimeInfo")
    has_toolchain_info = hasattr(java_common, "JavaToolchainInfo")

    lines = [
        "type=" + type(java_common),
        "has_compile=" + str(hasattr(java_common, "compile")),
        "has_merge=" + str(hasattr(java_common, "merge")),
        "has_boot_class_path=" + str(hasattr(java_common, "boot_class_path")),
        "java_info_type=" + type(JavaInfo),
        "java_plugin_info_type=" + type(JavaPluginInfo),
        "java_info_callable=" + str(has_compile_jar and has_output_jar),
        "compile_returns_java_info=" + str(compile_has_compile_jar and compile_has_transitive),
        "merge_returns_java_info=" + str(merge_has_compile_jar),
        "plugin_info_callable=" + str(has_processor_class),
        "has_runtime_info=" + str(has_runtime_info),
        "has_toolchain_info=" + str(has_toolchain_info),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


java_common_test = rule(
    implementation = _java_common_test_impl,
    attrs = {},
)


def _int_values_test_impl(ctx):
    """Tests that attr.int(values=[...]) constraint works."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "stamp=" + str(ctx.attr.stamp) + "\n")
    return [DefaultInfo(default_output = out)]

int_values_test = rule(
    implementation = _int_values_test_impl,
    attrs = {
        "stamp": attr.int(default = 0, values = [-1, 0, 1]),
    },
)


MyInfo = provider(fields = ["value"])


def _provides_valid_impl(ctx):
    """A rule that declares and returns the required provider."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "ok\n")
    return [DefaultInfo(default_output = out), MyInfo(value = "hello")]


provides_valid_rule = rule(
    implementation = _provides_valid_impl,
    provides = [MyInfo],
    attrs = {},
)


def _provides_missing_impl(ctx):
    """A rule that declares MyInfo in provides but does NOT return it."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "missing provider\n")
    return [DefaultInfo(default_output = out)]


provides_missing_rule = rule(
    implementation = _provides_missing_impl,
    provides = [MyInfo],
    attrs = {},
)


def _executable_rule_impl(ctx):
    """Tests that rule(executable=True) provides ctx.outputs.executable."""
    exe = ctx.outputs.executable
    ctx.actions.write(exe, "#!/bin/sh\necho hello\n")
    return [DefaultInfo(
        default_output = exe,
        executable = exe,
    )]


executable_rule = rule(
    implementation = _executable_rule_impl,
    executable = True,
    attrs = {},
)


def _non_executable_rule_impl(ctx):
    """A rule without executable=True for comparison."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "not executable\n")
    return [DefaultInfo(default_output = out)]


non_executable_rule = rule(
    implementation = _non_executable_rule_impl,
    attrs = {},
)


def _exec_groups_test_impl(ctx):
    """Tests that rule(exec_groups={...}) is accepted and ctx.exec_groups works."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    # Access exec_groups dict
    eg = ctx.exec_groups
    has_compile = "compile" in eg
    has_link = "link" in eg
    # Access toolchains within an exec group
    compile_group = eg["compile"]
    has_toolchains = hasattr(compile_group, "toolchains")
    lines = [
        "type=" + type(eg),
        "has_compile=" + str(has_compile),
        "has_link=" + str(has_link),
        "has_toolchains=" + str(has_toolchains),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


exec_groups_test = rule(
    implementation = _exec_groups_test_impl,
    exec_groups = {
        "compile": exec_group(
            toolchains = [],
        ),
        "link": exec_group(
            toolchains = [],
        ),
    },
    attrs = {},
)


def _fragments_test_impl(ctx):
    """Tests that rule(fragments=[...]) is accepted and ctx.fragments works."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    frags = ctx.fragments
    has_cpp = hasattr(frags, "cpp")
    cpp_mode = frags.cpp.compilation_mode() if has_cpp else "none"
    lines = [
        "has_cpp=" + str(has_cpp),
        "compilation_mode=" + cpp_mode,
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


fragments_test = rule(
    implementation = _fragments_test_impl,
    fragments = ["cpp"],
    attrs = {},
)


def _nonempty_deps_test_impl(ctx):
    """Tests that allow_empty=False on label_list is enforced."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    names = [dep.label.name for dep in ctx.attr.deps]
    ctx.actions.write(out, ",".join(names) + "\n")
    return [DefaultInfo(default_output = out)]


nonempty_deps_test = rule(
    implementation = _nonempty_deps_test_impl,
    attrs = {
        "deps": attr.label_list(allow_empty = False),
    },
)


def _nonempty_strings_test_impl(ctx):
    """Tests that allow_empty=False on string_list is enforced."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ",".join(ctx.attr.items) + "\n")
    return [DefaultInfo(default_output = out)]


nonempty_strings_test = rule(
    implementation = _nonempty_strings_test_impl,
    attrs = {
        "items": attr.string_list(allow_empty = False),
    },
)


# === rule(initializer=...) test ===

def _initializer_test_initializer(**kwargs):
    """Transforms attributes before target creation."""
    # Convert stamp bool -> int (like rules_cc cc_test does)
    if "stamp" in kwargs and type(kwargs["stamp"]) == type(True):
        kwargs["stamp"] = 1 if kwargs["stamp"] else 0
    # Add a prefix to the message if not already present
    if "message" in kwargs and not kwargs["message"].startswith("INIT:"):
        kwargs["message"] = "INIT:" + kwargs["message"]
    return kwargs

def _initializer_test_impl(ctx):
    """Implementation that writes the transformed attribute values."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "stamp=" + str(ctx.attr.stamp),
        "message=" + ctx.attr.message,
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]

initializer_test = rule(
    implementation = _initializer_test_impl,
    initializer = _initializer_test_initializer,
    attrs = {
        "stamp": attr.int(default = 0),
        "message": attr.string(default = "default"),
    },
)


def _build_config_test_impl(ctx):
    """Writes build configuration values to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "stamp_binaries=" + str(ctx.configuration.stamp_binaries),
        "coverage_enabled=" + str(ctx.configuration.coverage_enabled),
        "test_env=" + str(ctx.configuration.test_env),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]

build_config_test = rule(
    implementation = _build_config_test_impl,
    attrs = {},
)


def _instrumented_files_test_impl(ctx):
    """Tests coverage_common.instrumented_files_info() with source files."""
    info = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["deps"],
        extensions = ["c", "h", "cc"],
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    inst_files = info.instrumented_files.to_list() if hasattr(info.instrumented_files, "to_list") else []
    meta_files = info.metadata_files.to_list() if hasattr(info.metadata_files, "to_list") else []

    lines = [
        "type=" + type(info),
        "has_instrumented_files=" + str(hasattr(info, "instrumented_files")),
        "has_metadata_files=" + str(hasattr(info, "metadata_files")),
        "instrumented_count=" + str(len(inst_files)),
        "metadata_count=" + str(len(meta_files)),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


instrumented_files_test = rule(
    implementation = _instrumented_files_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
    },
)


def _instrumented_files_empty_test_impl(ctx):
    """Tests coverage_common.instrumented_files_info() with no args."""
    info = coverage_common.instrumented_files_info(ctx)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(info),
        "has_instrumented_files=" + str(hasattr(info, "instrumented_files")),
        "has_metadata_files=" + str(hasattr(info, "metadata_files")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


instrumented_files_empty_test = rule(
    implementation = _instrumented_files_empty_test_impl,
    attrs = {},
)


# === is_tool_configuration test ===

def _is_tool_configuration_test_impl(ctx):
    """Tests ctx.configuration.is_tool_configuration() returns a bool."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    config = ctx.configuration
    is_tool = config.is_tool_configuration()
    lines = [
        "is_tool=" + str(is_tool),
        "type=" + type(is_tool),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


is_tool_configuration_test = rule(
    implementation = _is_tool_configuration_test_impl,
    attrs = {},
)


# === ctx.split_attr test ===

def _split_attr_test_impl(ctx):
    """Tests ctx.split_attr wraps attribute values in config dicts."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    # Access split_attr - should be available
    has_split_attr = hasattr(ctx, "split_attr")

    # Access a specific attribute through split_attr
    split_message = ctx.split_attr.message
    is_dict = type(split_message) == "dict"

    # The dict should have "//conditions:default" as key
    keys = list(split_message.keys())
    has_default_key = "//conditions:default" in keys

    # The value should be the original attribute value
    value = split_message.get("//conditions:default", None)

    lines = [
        "has_split_attr=" + str(has_split_attr),
        "is_dict=" + str(is_dict),
        "has_default_key=" + str(has_default_key),
        "value=" + str(value),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


split_attr_test = rule(
    implementation = _split_attr_test_impl,
    attrs = {
        "message": attr.string(default = "hello_split"),
    },
)


def _resolve_command_test_impl(ctx):
    """Tests ctx.resolve_command() returns a 3-tuple (inputs, command, input_manifests)."""
    result = ctx.resolve_command(command = "echo hello")
    if type(result) != "tuple":
        fail("resolve_command should return tuple, got %s" % type(result))
    if len(result) != 3:
        fail("resolve_command tuple should have 3 elements, got %d" % len(result))
    inputs, command, manifests = result
    if type(inputs) != "list":
        fail("inputs should be list, got %s" % type(inputs))
    if type(command) != "list":
        fail("command should be list, got %s" % type(command))
    if len(command) != 1:
        fail("command should have 1 element, got %d" % len(command))
    if command[0] != "echo hello":
        fail("command[0] should be 'echo hello', got '%s'" % command[0])
    if type(manifests) != "list":
        fail("manifests should be list, got %s" % type(manifests))

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "resolve_command_ok")
    return [DefaultInfo(default_output = out)]


resolve_command_test = rule(
    implementation = _resolve_command_test_impl,
    attrs = {},
)


def _new_file_test_impl(ctx):
    """Tests ctx.new_file() creates a declared artifact."""
    f = ctx.new_file(ctx.label.name + "_output.txt")
    # f should be a declared artifact that we can write to
    ctx.actions.write(f, "new_file_ok")
    return [DefaultInfo(default_output = f)]


new_file_test = rule(
    implementation = _new_file_test_impl,
    attrs = {},
)


def _java_toolchain_test_impl(ctx):
    """Tests that Java toolchain stubs provide expected attributes."""
    # Test Java toolchain lookup
    java_tc_wrapper = ctx.toolchains["@rules_java//java:toolchain_type"]
    results = []

    # The wrapper should have a .java attribute
    results.append("has_java=" + str(hasattr(java_tc_wrapper, "java")))
    java_tc = java_tc_wrapper.java

    # JavaToolchainInfo should have source_version, target_version, java_runtime
    results.append("source_version=" + str(java_tc.source_version))
    results.append("target_version=" + str(java_tc.target_version))
    results.append("has_java_runtime=" + str(java_tc.java_runtime != None))
    results.append("has_bootclasspath=" + str(java_tc.bootclasspath != None))
    results.append("has_jvm_opt=" + str(java_tc.jvm_opt != None))
    results.append("worker_support=" + str(java_tc._javac_supports_workers))

    # Test Java runtime toolchain lookup
    runtime_wrapper = ctx.toolchains["@rules_java//java:runtime_toolchain_type"]
    results.append("has_java_runtime_attr=" + str(hasattr(runtime_wrapper, "java_runtime")))
    java_runtime = runtime_wrapper.java_runtime
    results.append("has_java_home=" + str(java_runtime.java_home != None))
    results.append("has_java_exe=" + str(java_runtime.java_executable_exec_path != None))
    results.append("version=" + str(java_runtime.version))

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(results) + "\n")
    return [DefaultInfo(default_output = out)]


java_toolchain_test = rule(
    implementation = _java_toolchain_test_impl,
    attrs = {},
)


def _constraint_provider_test_impl(ctx):
    """Tests that ConstraintSettingInfo and ConstraintValueInfo are callable."""
    results = []

    # Test ConstraintSettingInfo is callable
    cs_info = platform_common.ConstraintSettingInfo(label = "//test:setting")
    results.append("cs_callable=True")
    results.append("cs_has_label=" + str(hasattr(cs_info, "label")))
    results.append("cs_label=" + str(cs_info.label))

    # Test ConstraintValueInfo is callable
    cv_info = platform_common.ConstraintValueInfo(
        label = "//test:value",
        constraint_setting = "//test:setting",
    )
    results.append("cv_callable=True")

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(results) + "\n")
    return [DefaultInfo(default_output = out)]


constraint_provider_test = rule(
    implementation = _constraint_provider_test_impl,
    attrs = {},
)


def _actions_fail_test_impl(ctx):
    """Tests ctx.actions.fail() raises an error at analysis time."""
    ctx.actions.fail("unsupported platform")
    # Should not reach here
    return [DefaultInfo()]


actions_fail_test = rule(
    implementation = _actions_fail_test_impl,
    attrs = {},
)


def _provider_callable_test_impl(ctx):
    """Tests that DebugPackageInfo and CcSharedLibraryInfo are callable providers."""
    results = []

    # DebugPackageInfo should not be None
    results.append("debug_is_not_none=" + str(DebugPackageInfo != None))
    results.append("debug_type=" + str(type(DebugPackageInfo)))

    # Create an instance
    dpi = DebugPackageInfo(target_label = ctx.label, stripped_file = None)
    results.append("debug_instance_ok=" + str(dpi != None))
    results.append("debug_target_label=" + str(dpi.target_label == ctx.label))

    # CcSharedLibraryInfo should not be None
    results.append("shared_is_not_none=" + str(CcSharedLibraryInfo != None))
    results.append("shared_type=" + str(type(CcSharedLibraryInfo)))

    # Create an instance
    sli = CcSharedLibraryInfo(dynamic_library = None, linker_input = None)
    results.append("shared_instance_ok=" + str(sli != None))

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(results) + "\n")
    return [DefaultInfo(default_output = out)]


provider_callable_test = rule(
    implementation = _provider_callable_test_impl,
    attrs = {},
)


def _write_file_test_impl(ctx):
    """Tests ctx.actions.write_file() Bazel-compatible alias."""
    out = ctx.actions.write_file(
        ctx.actions.declare_file(ctx.label.name + ".txt"),
        "hello from write_file\n",
    )
    return [DefaultInfo(default_output = out)]


write_file_test = rule(
    implementation = _write_file_test_impl,
    attrs = {},
)


def _write_file_executable_test_impl(ctx):
    """Tests ctx.actions.write_file() with is_executable=True."""
    out = ctx.actions.write_file(
        ctx.actions.declare_file(ctx.label.name + ".sh"),
        "#!/bin/bash\necho write_file_exec\n",
        True,
    )
    return [DefaultInfo(default_output = out)]


write_file_executable_test = rule(
    implementation = _write_file_executable_test_impl,
    attrs = {},
)


def _do_nothing_test_impl(ctx):
    """Tests ctx.actions.do_nothing() binds outputs correctly."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    # Write real content first (since do_nothing writes empty)
    ctx.actions.write(out, "do_nothing_output\n")

    return [DefaultInfo(default_output = out)]


do_nothing_test = rule(
    implementation = _do_nothing_test_impl,
    attrs = {},
)


def _do_nothing_binds_test_impl(ctx):
    """Tests ctx.actions.do_nothing() actually binds the output artifact."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    # Use do_nothing to bind the output (writes empty content)
    ctx.actions.do_nothing(
        mnemonic = "DoNothing",
        outputs = [out],
    )

    return [DefaultInfo(default_output = out)]


do_nothing_binds_test = rule(
    implementation = _do_nothing_binds_test_impl,
    attrs = {},
)


def _cc_toolchain_config_info_test_impl(ctx):
    """Tests cc_common.create_cc_toolchain_config_info() creates a provider."""
    config = cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = "test_toolchain",
        host_system_name = "local",
        target_system_name = "local",
        target_cpu = "x86_64",
        target_libc = "local",
        compiler = "gcc",
        abi_version = "local",
        abi_libc_version = "local",
        tool_paths = [],
    )

    # Verify we got a CcToolchainConfigInfo instance
    if type(config) != "CcToolchainConfigInfo":
        fail("Expected CcToolchainConfigInfo, got %s" % type(config))

    # Verify attributes are accessible
    if config.toolchain_identifier != "test_toolchain":
        fail("Expected toolchain_identifier='test_toolchain', got '%s'" % config.toolchain_identifier)
    if config.target_cpu != "x86_64":
        fail("Expected target_cpu='x86_64', got '%s'" % config.target_cpu)
    if config.compiler != "gcc":
        fail("Expected compiler='gcc', got '%s'" % config.compiler)

    # Write a success marker
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "cc_toolchain_config_info: ok\n")
    return [DefaultInfo(default_output = out)]


cc_toolchain_config_info_test = rule(
    implementation = _cc_toolchain_config_info_test_impl,
    attrs = {},
)
