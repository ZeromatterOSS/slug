"""Test rules for cc_common.compile() functionality."""

def _compile_basic_test_impl(ctx):
    """Tests that cc_common.compile() creates compilation outputs from source files."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        public_hdrs = ctx.files.hdrs,
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "comp_ctx_type=" + type(compilation_ctx),
        "outputs_type=" + type(compilation_outputs),
        "has_objects=" + str(hasattr(compilation_outputs, "objects")),
        "has_pic_objects=" + str(hasattr(compilation_outputs, "pic_objects")),
    ]

    # Check if objects were produced
    if hasattr(compilation_outputs, "objects"):
        objs = compilation_outputs.objects
        if hasattr(objs, "to_list"):
            lines.append("objects_count=" + str(len(objs.to_list())))
        else:
            lines.append("objects_count=0")

    # Check compilation context attributes
    lines.append("has_headers=" + str(hasattr(compilation_ctx, "headers")))
    lines.append("has_includes=" + str(hasattr(compilation_ctx, "includes")))
    lines.append("has_defines=" + str(hasattr(compilation_ctx, "defines")))
    lines.append("has_direct_headers=" + str(hasattr(compilation_ctx, "direct_headers")))

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_basic_test = rule(
    implementation = _compile_basic_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "hdrs": attr.label_list(allow_files = True),
    },
)


def _compile_with_defines_test_impl(ctx):
    """Tests that cc_common.compile() passes defines to the compilation context."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        defines = depset(["MY_DEFINE=1", "ANOTHER_DEFINE"]),
        local_defines = depset(["LOCAL_DEF=2"]),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    # Check that defines appear in the compilation context
    ctx_defines = []
    if hasattr(compilation_ctx, "defines") and hasattr(compilation_ctx.defines, "to_list"):
        ctx_defines = compilation_ctx.defines.to_list()

    lines = [
        "comp_ctx_type=" + type(compilation_ctx),
        "defines_count=" + str(len(ctx_defines)),
        "defines=" + ",".join(sorted(ctx_defines)),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_with_defines_test = rule(
    implementation = _compile_with_defines_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _compile_with_includes_test_impl(ctx):
    """Tests that cc_common.compile() passes include directories."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        includes = depset(["myinc/"]),
        quote_includes = depset(["myquote/"]),
        system_includes = depset(["mysystem/"]),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    # Check include directories in compilation context
    includes_list = []
    quote_list = []
    system_list = []
    if hasattr(compilation_ctx, "includes") and hasattr(compilation_ctx.includes, "to_list"):
        includes_list = compilation_ctx.includes.to_list()
    if hasattr(compilation_ctx, "quote_includes") and hasattr(compilation_ctx.quote_includes, "to_list"):
        quote_list = compilation_ctx.quote_includes.to_list()
    if hasattr(compilation_ctx, "system_includes") and hasattr(compilation_ctx.system_includes, "to_list"):
        system_list = compilation_ctx.system_includes.to_list()

    lines = [
        "includes=" + ",".join(sorted(includes_list)),
        "quote_includes=" + ",".join(sorted(quote_list)),
        "system_includes=" + ",".join(sorted(system_list)),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_with_includes_test = rule(
    implementation = _compile_with_includes_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _compile_with_flags_test_impl(ctx):
    """Tests that cc_common.compile() passes user_compile_flags."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        user_compile_flags = ["-Wall", "-Wextra", "-O2"],
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "comp_ctx_type=" + type(compilation_ctx),
        "outputs_type=" + type(compilation_outputs),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_with_flags_test = rule(
    implementation = _compile_with_flags_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _compile_multiple_srcs_test_impl(ctx):
    """Tests that cc_common.compile() handles multiple source files."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        public_hdrs = ctx.files.hdrs,
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    obj_count = 0
    if hasattr(compilation_outputs, "objects") and hasattr(compilation_outputs.objects, "to_list"):
        obj_count = len(compilation_outputs.objects.to_list())

    lines = [
        "objects_count=" + str(obj_count),
        "comp_ctx_type=" + type(compilation_ctx),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_multiple_srcs_test = rule(
    implementation = _compile_multiple_srcs_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "hdrs": attr.label_list(allow_files = True),
    },
)


def _compile_dep_contexts_test_impl(ctx):
    """Tests that cc_common.compile() accepts compilation_contexts from deps."""
    fc = cc_common.configure_features(cc_toolchain = None, ctx = ctx)

    # Create a dep compilation context with some defines
    dep_ctx = cc_common.create_compilation_context(
        headers = depset([]),
        defines = depset(["FROM_DEP=1"]),
        includes = depset(["dep_include/"]),
    )

    compilation_ctx, compilation_outputs = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = fc,
        srcs = ctx.files.srcs,
        compilation_contexts = [dep_ctx],
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "comp_ctx_type=" + type(compilation_ctx),
        "outputs_type=" + type(compilation_outputs),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compile_dep_contexts_test = rule(
    implementation = _compile_dep_contexts_test_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)
