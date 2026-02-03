# Test DefaultInfo files parameter
# This tests that DefaultInfo(files = depset([...])) properly populates default_outputs

def _test_files_param_impl(ctx):
    """Test that DefaultInfo(files=depset([out])) works correctly."""
    # Create an output file
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "test content")

    # Return using Bazel-style files parameter
    return [DefaultInfo(files = depset([out]))]

test_files_param = rule(
    implementation = _test_files_param_impl,
    attrs = {},
)

def _test_nested_depset_impl(ctx):
    """Test that DefaultInfo(files=depset(..., transitive=[...])) works correctly."""
    # Create output files
    out1 = ctx.actions.declare_file(ctx.label.name + "_a.txt")
    out2 = ctx.actions.declare_file(ctx.label.name + "_b.txt")
    ctx.actions.write(out1, "content a")
    ctx.actions.write(out2, "content b")

    # Create nested depsets
    inner = depset([out1])
    outer = depset([out2], transitive = [inner])

    # Return using Bazel-style files parameter with nested depset
    return [DefaultInfo(files = outer)]

test_nested_depset = rule(
    implementation = _test_nested_depset_impl,
    attrs = {},
)

def _test_outputs_param_impl(ctx):
    """Test that DefaultInfo(default_output=out) works correctly."""
    # Create an output file
    out = ctx.actions.declare_output(ctx.label.name + ".txt")
    ctx.actions.write(out, "test content")

    # Return using Buck-style default_output parameter
    return [DefaultInfo(default_output = out)]

test_outputs_param = rule(
    implementation = _test_outputs_param_impl,
    attrs = {},
)
