# Simple rule definitions for testing bzlmod .bzl loading
# Compatible with Slug's Starlark API

def _simple_rule_impl(ctx):
    # Create a simple output file with the message
    # Note: Slug uses declare_output() instead of Bazel's declare_file()
    # Note: Slug uses ctx.attrs instead of Bazel's ctx.attr
    out = ctx.actions.declare_output(ctx.label.name + ".txt")
    # Note: Slug's write() takes positional args, Bazel uses named args
    ctx.actions.write(out, ctx.attrs.message + "\n")
    # Use list instead of depset for simplicity - works in both Bazel and Slug
    return [DefaultInfo(default_outputs = [out])]

simple_rule = rule(
    implementation = _simple_rule_impl,
    attrs = {
        "message": attr.string(
            default = "Hello from simple_rule!",
            doc = "Message to write to the output file",
        ),
    },
    doc = "A simple rule that writes a message to a file",
)

# A simple function that can be called from BUILD files
def greeting(name):
    """Returns a greeting string."""
    return "Hello, " + name + "!"
