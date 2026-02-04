# Debug rule to trace cc_binary all_deps issue
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cc//cc/common:semantics.bzl", "semantics")

def _debug_cc_binary_impl(ctx):
    """Debug implementation to trace all_deps list contents."""
    print("=== debug_cc_binary START ===")

    # Print ctx.attr.deps
    print("ctx.attr.deps = %s" % ctx.attr.deps)
    print("ctx.attr.deps type = %s" % type(ctx.attr.deps))
    for i, dep in enumerate(ctx.attr.deps):
        print("  deps[%d] = %s (type=%s)" % (i, dep, type(dep)))
        if dep == None:
            print("    WARNING: dep is None!")

    # Print ctx.attr.link_extra_lib
    print("ctx.attr.link_extra_lib = %s" % ctx.attr.link_extra_lib)
    print("ctx.attr.link_extra_lib type = %s" % type(ctx.attr.link_extra_lib))

    # Print ctx.attr.malloc
    print("ctx.attr.malloc = %s" % ctx.attr.malloc)
    print("ctx.attr.malloc type = %s" % type(ctx.attr.malloc))

    # Print _default_malloc
    print("ctx.attr._default_malloc = %s" % ctx.attr._default_malloc)
    print("ctx.attr._default_malloc type = %s" % type(ctx.attr._default_malloc))

    # Check custom_malloc fragment value
    print("ctx.fragments.cpp.custom_malloc() = %s" % ctx.fragments.cpp.custom_malloc())
    print("ctx.fragments.cpp.custom_malloc() type = %s" % type(ctx.fragments.cpp.custom_malloc()))
    print("ctx.fragments.cpp.custom_malloc() == None: %s" % (ctx.fragments.cpp.custom_malloc() == None))
    print("ctx.fragments.cpp.custom_malloc() != None: %s" % (ctx.fragments.cpp.custom_malloc() != None))

    # Call get_cc_runtimes like cc_binary does
    print("Calling semantics.get_cc_runtimes(ctx, False)...")
    is_link_shared = False  # For regular binary
    cc_runtimes = semantics.get_cc_runtimes(ctx, is_link_shared)
    print("cc_runtimes = %s" % cc_runtimes)
    print("cc_runtimes type = %s" % type(cc_runtimes))
    for i, runtime in enumerate(cc_runtimes):
        print("  cc_runtimes[%d] = %s (type=%s)" % (i, runtime, type(runtime)))
        if runtime == None:
            print("    WARNING: runtime is None!")

    # Build all_deps like cc_binary does
    print("Building all_deps = ctx.attr.deps + cc_runtimes...")
    all_deps = ctx.attr.deps + cc_runtimes
    print("all_deps = %s" % all_deps)
    print("all_deps type = %s" % type(all_deps))
    print("all_deps len = %d" % len(all_deps))

    for i, dep in enumerate(all_deps):
        print("  all_deps[%d] = %s (type=%s)" % (i, dep, type(dep)))
        if dep == None:
            print("    WARNING: dep is None!")
        else:
            # Try the CcInfo in dep check (this is where cc_binary fails)
            result = CcInfo in dep
            print("    CcInfo in dep = %s" % result)

    print("=== debug_cc_binary END ===")

    # Return empty default info
    out = ctx.actions.declare_file(ctx.label.name + ".marker")
    ctx.actions.write(out, "debug marker")
    return [DefaultInfo(files = depset([out]))]

debug_cc_binary = rule(
    implementation = _debug_cc_binary_impl,
    attrs = {
        "deps": attr.label_list(
            allow_files = [".ld", ".lds", ".ldscript"],
            providers = [CcInfo],
        ),
        "malloc": attr.label(
            default = "@bazel_tools//tools/cpp:malloc",
            allow_files = False,
            providers = [CcInfo],
        ),
        "link_extra_lib": attr.label(
            default = "@bazel_tools//tools/cpp:link_extra_lib",
            providers = [CcInfo],
        ),
        "_default_malloc": attr.label(
            default = configuration_field(fragment = "cpp", name = "custom_malloc"),
        ),
    },
    fragments = ["cpp"],
)
