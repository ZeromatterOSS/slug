"""Test rules for cc_common.create_compilation_context() functionality."""

def _basic_context_test_impl(ctx):
    """Tests basic creation of CcCompilationContext with all field types."""
    comp_ctx = cc_common.create_compilation_context(
        headers = depset([]),
        includes = depset(["include/", "src/"]),
        quote_includes = depset(["."]),
        system_includes = depset(["/usr/include"]),
        defines = depset(["VERSION=1", "DEBUG"]),
        local_defines = depset(["LOCAL_ONLY=1"]),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    # Extract all fields
    includes_list = comp_ctx.includes.to_list() if hasattr(comp_ctx.includes, "to_list") else []
    defines_list = comp_ctx.defines.to_list() if hasattr(comp_ctx.defines, "to_list") else []
    quote_list = comp_ctx.quote_includes.to_list() if hasattr(comp_ctx.quote_includes, "to_list") else []
    system_list = comp_ctx.system_includes.to_list() if hasattr(comp_ctx.system_includes, "to_list") else []

    lines = [
        "type=" + type(comp_ctx),
        "includes_count=" + str(len(includes_list)),
        "includes=" + ",".join(sorted(includes_list)),
        "defines_count=" + str(len(defines_list)),
        "defines=" + ",".join(sorted(defines_list)),
        "quote_includes_count=" + str(len(quote_list)),
        "quote_includes=" + ",".join(sorted(quote_list)),
        "system_includes_count=" + str(len(system_list)),
        "system_includes=" + ",".join(sorted(system_list)),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


basic_context_test = rule(
    implementation = _basic_context_test_impl,
    attrs = {},
)


def _empty_context_test_impl(ctx):
    """Tests that creating an empty CcCompilationContext works."""
    comp_ctx = cc_common.create_compilation_context()

    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    has_headers = hasattr(comp_ctx, "headers")
    has_includes = hasattr(comp_ctx, "includes")
    has_defines = hasattr(comp_ctx, "defines")

    lines = [
        "type=" + type(comp_ctx),
        "has_headers=" + str(has_headers),
        "has_includes=" + str(has_includes),
        "has_defines=" + str(has_defines),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


empty_context_test = rule(
    implementation = _empty_context_test_impl,
    attrs = {},
)


def _merge_contexts_test_impl(ctx):
    """Tests merging multiple CcCompilationContexts via CcInfo merge."""
    ctx1 = cc_common.create_compilation_context(
        defines = depset(["A=1"]),
        includes = depset(["inc_a/"]),
    )
    ctx2 = cc_common.create_compilation_context(
        defines = depset(["B=2"]),
        includes = depset(["inc_b/"]),
    )
    ctx3 = cc_common.create_compilation_context(
        defines = depset(["C=3"]),
        includes = depset(["inc_c/"]),
        system_includes = depset(["/opt/include"]),
    )

    info1 = CcInfo(compilation_context = ctx1)
    info2 = CcInfo(compilation_context = ctx2)
    info3 = CcInfo(compilation_context = ctx3)

    merged = cc_common.merge_cc_infos(cc_infos = [info1, info2, info3])
    merged_ctx = merged.compilation_context

    defines = merged_ctx.defines.to_list() if hasattr(merged_ctx.defines, "to_list") else []
    includes = merged_ctx.includes.to_list() if hasattr(merged_ctx.includes, "to_list") else []

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "defines_count=" + str(len(defines)),
        "defines=" + ",".join(sorted(defines)),
        "includes_count=" + str(len(includes)),
        "includes=" + ",".join(sorted(includes)),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


merge_contexts_test = rule(
    implementation = _merge_contexts_test_impl,
    attrs = {},
)


def _compilation_outputs_test_impl(ctx):
    """Tests cc_common.create_compilation_outputs() creates proper outputs struct."""
    obj1 = ctx.actions.declare_file("test1.o")
    obj2 = ctx.actions.declare_file("test2.o")
    ctx.actions.write(obj1, "")
    ctx.actions.write(obj2, "")

    outputs = cc_common.create_compilation_outputs(
        objects = depset([obj1, obj2]),
        pic_objects = depset([obj1]),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")

    has_objects = hasattr(outputs, "objects")
    has_pic_objects = hasattr(outputs, "pic_objects")
    obj_count = 0
    pic_count = 0
    if has_objects and hasattr(outputs.objects, "to_list"):
        obj_count = len(outputs.objects.to_list())
    if has_pic_objects and hasattr(outputs.pic_objects, "to_list"):
        pic_count = len(outputs.pic_objects.to_list())

    lines = [
        "type=" + type(outputs),
        "has_objects=" + str(has_objects),
        "has_pic_objects=" + str(has_pic_objects),
        "objects_count=" + str(obj_count),
        "pic_objects_count=" + str(pic_count),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


compilation_outputs_test = rule(
    implementation = _compilation_outputs_test_impl,
    attrs = {},
)


def _ccinfo_provider_test_impl(ctx):
    """Tests that CcInfo provider can be created with both compilation and linking contexts."""
    comp_ctx = cc_common.create_compilation_context(
        defines = depset(["TEST=1"]),
        includes = depset(["inc/"]),
    )

    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        user_link_flags = depset(["-ltest"]),
    )
    linking_ctx = cc_common.create_linking_context(
        linker_inputs = depset([linker_input]),
    )

    info = CcInfo(
        compilation_context = comp_ctx,
        linking_context = linking_ctx,
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = [
        "type=" + type(info),
        "has_compilation_context=" + str(hasattr(info, "compilation_context")),
        "has_linking_context=" + str(hasattr(info, "linking_context")),
        "comp_type=" + type(info.compilation_context),
        "link_type=" + type(info.linking_context),
    ]

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


ccinfo_provider_test = rule(
    implementation = _ccinfo_provider_test_impl,
    attrs = {},
)
