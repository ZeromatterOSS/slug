"""Test rules for cc_common.link() functionality."""

def _link_executable_test_impl(ctx):
    """Tests that cc_common.link() with output_type='executable' works."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)
    result = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name,
        feature_configuration = fc,
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


link_executable_test = rule(
    implementation = _link_executable_test_impl,
    attrs = {},
)


def _link_dynamic_library_test_impl(ctx):
    """Tests that cc_common.link() with output_type='dynamic_library' works."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)
    result = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name,
        feature_configuration = fc,
        cc_toolchain = None,
        output_type = "dynamic_library",
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(result),
        "has_library_to_link=" + str(hasattr(result, "library_to_link")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


link_dynamic_library_test = rule(
    implementation = _link_dynamic_library_test_impl,
    attrs = {},
)


def _link_with_user_flags_test_impl(ctx):
    """Tests that cc_common.link() passes user_link_flags."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)
    result = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name,
        feature_configuration = fc,
        cc_toolchain = None,
        output_type = "executable",
        user_link_flags = ["-lm", "-lpthread"],
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(result),
        "has_library_to_link=" + str(hasattr(result, "library_to_link")),
        "has_executable=" + str(hasattr(result, "executable")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


link_with_user_flags_test = rule(
    implementation = _link_with_user_flags_test_impl,
    attrs = {},
)


def _link_with_linking_contexts_test_impl(ctx):
    """Tests that cc_common.link() accepts linking_contexts from deps."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Create a linking context via linker_input
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        user_link_flags = depset(["-lz"]),
    )
    linking_ctx = cc_common.create_linking_context(
        linker_inputs = depset([linker_input]),
    )

    result = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name,
        feature_configuration = fc,
        cc_toolchain = None,
        output_type = "executable",
        linking_contexts = [linking_ctx],
    )
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(result),
        "has_library_to_link=" + str(hasattr(result, "library_to_link")),
        "has_executable=" + str(hasattr(result, "executable")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


link_with_linking_contexts_test = rule(
    implementation = _link_with_linking_contexts_test_impl,
    attrs = {},
)


def _create_library_to_link_test_impl(ctx):
    """Tests cc_common.create_library_to_link() with different library types."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Create a static library_to_link
    static_lib = ctx.actions.declare_file("libtest.a")
    ctx.actions.write(static_lib, "")
    lib_to_link = cc_common.create_library_to_link(
        actions = ctx.actions,
        cc_toolchain = None,
        feature_configuration = fc,
        static_library = static_lib,
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(lib_to_link),
        "has_static_library=" + str(hasattr(lib_to_link, "static_library")),
        "has_dynamic_library=" + str(hasattr(lib_to_link, "dynamic_library")),
        "has_pic_static_library=" + str(hasattr(lib_to_link, "pic_static_library")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


create_library_to_link_test = rule(
    implementation = _create_library_to_link_test_impl,
    attrs = {},
)


def _link_deps_statically_test_impl(ctx):
    """Tests that cc_common.link() respects link_deps_statically parameter."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Create a library_to_link with both static and dynamic libraries
    static_lib = ctx.actions.declare_file("libdep.a")
    ctx.actions.write(static_lib, "")
    dynamic_lib = ctx.actions.declare_file("libdep.so")
    ctx.actions.write(dynamic_lib, "")

    lib_to_link = cc_common.create_library_to_link(
        actions = ctx.actions,
        cc_toolchain = None,
        feature_configuration = fc,
        static_library = static_lib,
        dynamic_library = dynamic_lib,
    )
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        libraries = depset([lib_to_link]),
    )
    linking_ctx = cc_common.create_linking_context(
        linker_inputs = depset([linker_input]),
    )

    # Link statically - should prefer static_library
    result_static = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name + "_static",
        feature_configuration = fc,
        cc_toolchain = None,
        output_type = "executable",
        link_deps_statically = True,
        linking_contexts = [linking_ctx],
    )

    # Link dynamically - should prefer dynamic_library
    result_dynamic = cc_common.link(
        actions = ctx.actions,
        name = ctx.label.name + "_dynamic",
        feature_configuration = fc,
        cc_toolchain = None,
        output_type = "executable",
        link_deps_statically = False,
        linking_contexts = [linking_ctx],
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "static_type=" + type(result_static),
        "static_has_executable=" + str(hasattr(result_static, "executable")),
        "dynamic_type=" + type(result_dynamic),
        "dynamic_has_executable=" + str(hasattr(result_dynamic, "executable")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


link_deps_statically_test = rule(
    implementation = _link_deps_statically_test_impl,
    attrs = {},
)


def _linker_input_test_impl(ctx):
    """Tests cc_common.create_linker_input() with various inputs."""
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        user_link_flags = depset(["-L/usr/local/lib", "-lssl", "-lcrypto"]),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    flags = linker_input.user_link_flags
    flags_list = sorted(flags.to_list())
    lines = [
        "type=" + type(linker_input),
        "has_user_link_flags=" + str(hasattr(linker_input, "user_link_flags")),
        "flags_count=" + str(len(flags_list)),
        "flags=" + ",".join(flags_list),
        "has_owner=" + str(hasattr(linker_input, "owner")),
    ]
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


linker_input_test = rule(
    implementation = _linker_input_test_impl,
    attrs = {},
)
