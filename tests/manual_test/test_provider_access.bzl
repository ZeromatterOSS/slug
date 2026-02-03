# Test provider access on artifacts
# Phase 6: Provider Access Semantics

def _test_provider_access_impl(ctx):
    """Test that DefaultInfo in artifact and artifact[DefaultInfo] work."""
    for src in ctx.attr.srcs:
        print("DEBUG: src type = %s, src = %s" % (type(src), src))

        # Test 1: DefaultInfo in artifact should return True
        if DefaultInfo in src:
            print("Test 1 PASSED: DefaultInfo in artifact = True")
        else:
            fail("Test 1 FAILED: DefaultInfo in artifact should be True")

        # Test 2: artifact[DefaultInfo] should return DefaultInfo provider
        default_info = src[DefaultInfo]
        print("Test 2 PASSED: artifact[DefaultInfo] = %s" % default_info)

        # Test 3: Check that default_outputs can be accessed
        outputs = default_info.default_outputs
        print("Test 3 PASSED: default_outputs = %s" % outputs)

    # Create a simple output file
    out = ctx.actions.declare_output(ctx.label.name + ".txt")
    ctx.actions.write(out, "test")
    return [DefaultInfo(default_output = out)]

test_provider_access = rule(
    implementation = _test_provider_access_impl,
    attrs = {
        # Use attrs.list(attrs.source()) to accept source files
        "srcs": attrs.list(attrs.source()),
    },
)

# Test with Bazel-style attr.label_list
def _test_bazel_label_list_impl(ctx):
    """Test provider access with Bazel's attr.label_list."""
    print("DEBUG: ctx.attr.srcs = %s" % ctx.attr.srcs)
    for src in ctx.attr.srcs:
        print("DEBUG (label_list): src type = %s, src = %s" % (type(src), src))

        # Check provider access
        if DefaultInfo in src:
            print("Bazel label_list: DefaultInfo in src = True")
            default_info = src[DefaultInfo]
            print("Bazel label_list: src[DefaultInfo] = %s" % default_info)
            files = default_info.files.to_list()
            print("Bazel label_list: files depset = %s" % default_info.files)
            print("Bazel label_list: files = %s" % files)
            # Check what cc_helper.get_srcs needs: artifact.extension and src.label
            for artifact in files:
                print("  artifact.extension = %s" % artifact.extension)
            # Check if src has a label attribute (needed by cc_helper)
            if hasattr(src, "label"):
                print("  src.label = %s" % src.label)
            else:
                print("  src has no 'label' attribute (type=%s)" % type(src))
        else:
            print("Bazel label_list: DefaultInfo in src = False (type=%s)" % type(src))

    # Create a simple output file
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "test")
    return [DefaultInfo(files = depset([out]))]

test_bazel_label_list = rule(
    implementation = _test_bazel_label_list_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)
