# Test provider access on artifacts
# Phase 6: Provider Access Semantics

def _test_provider_access_impl(ctx):
    """Test that DefaultInfo in artifact and artifact[DefaultInfo] work."""
    for src in ctx.attr.srcs:
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
