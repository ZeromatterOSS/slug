# Debug wrapper for cc_library to trace the analysis flow
# Simplified version without toolchain dependencies

load("@rules_cc//cc/common:cc_helper.bzl", "cc_helper")

def _debug_cc_library_impl(ctx):
    """Debug implementation to trace cc_helper.get_srcs and cc_common.compile."""
    print("=== debug_cc_library START ===")
    print("ctx.attr.srcs = %s" % ctx.attr.srcs)
    print("ctx.attr.srcs type = %s" % type(ctx.attr.srcs))

    # Check each src in ctx.attr.srcs
    for i, src in enumerate(ctx.attr.srcs):
        print("  ctx.attr.srcs[%d] = %s (type=%s)" % (i, src, type(src)))
        # Check if DefaultInfo in src works
        if DefaultInfo in src:
            print("    DefaultInfo in src = True")
            di = src[DefaultInfo]
            print("    src[DefaultInfo] = %s" % di)
            files = di.files.to_list()
            print("    di.files.to_list() = %s" % files)
            for f in files:
                print("      file: %s, extension: %s" % (f, f.extension))
        else:
            print("    DefaultInfo in src = False")

    # Trace cc_helper.get_srcs
    print("Calling cc_helper.get_srcs(ctx)...")
    srcs = cc_helper.get_srcs(ctx)
    print("cc_helper.get_srcs returned: %s" % srcs)
    print("srcs length: %s" % len(srcs))

    for i, item in enumerate(srcs):
        print("  srcs[%d] = %s (type=%s)" % (i, item, type(item)))

    # Now try calling cc_common.compile
    print("Calling cc_common.compile with srcs=%s" % srcs)

    # Create a stub feature configuration
    feature_configuration = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = ctx.toolchains["@rules_cc//cc/toolchains:toolchain_type"] if "@rules_cc//cc/toolchains:toolchain_type" in ctx.toolchains else None,
    )
    print("feature_configuration = %s" % feature_configuration)

    compilation_result = cc_common.compile(
        actions = ctx.actions,
        name = ctx.label.name,
        cc_toolchain = None,
        feature_configuration = feature_configuration,
        srcs = srcs,
    )
    print("cc_common.compile returned: %s" % str(compilation_result))

    compilation_context, compilation_outputs = compilation_result
    print("compilation_context = %s" % compilation_context)
    print("compilation_outputs = %s" % compilation_outputs)
    print("compilation_outputs.objects = %s (len=%d)" % (compilation_outputs.objects, len(compilation_outputs.objects)))
    print("compilation_outputs.pic_objects = %s (len=%d)" % (compilation_outputs.pic_objects, len(compilation_outputs.pic_objects)))

    print("=== debug_cc_library END ===")

    # Return the object files as outputs
    all_outputs = list(compilation_outputs.objects) + list(compilation_outputs.pic_objects)
    if all_outputs:
        return [DefaultInfo(files = depset(all_outputs))]
    else:
        out = ctx.actions.declare_file(ctx.label.name + ".marker")
        ctx.actions.write(out, "debug marker")
        return [DefaultInfo(files = depset([out]))]

debug_cc_library = rule(
    implementation = _debug_cc_library_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)
