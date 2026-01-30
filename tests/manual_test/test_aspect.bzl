"""Test that aspect() built-in function works.

Phase 8a: aspect() can be declared and attached to attributes
Phase 8b: aspect implementation functions are called with AspectContext
"""

def _test_aspect_impl(target, ctx):
    """Aspect implementation function - Phase 8b test."""
    print("Phase 8b test: aspect invoked!")
    print("  ctx.label:", ctx.label)
    print("  ctx.rule.kind:", ctx.rule.kind)
    print("  ctx.rule.attr:", ctx.rule.attr)
    if DefaultInfo in target:
        print("  target has DefaultInfo")
    return []  # Return empty list (aspects cannot return DefaultInfo)

# Test 1: Simple aspect definition
test_aspect = aspect(
    implementation = _test_aspect_impl,
    attr_aspects = ["deps"],
    doc = "Test aspect for Phase 8a verification",
)

# Test 2: Aspect with required_providers (flat list)
test_aspect_with_providers = aspect(
    implementation = _test_aspect_impl,
    attr_aspects = ["deps", "srcs"],
    required_providers = [DefaultInfo],
    doc = "Test aspect with providers filter",
)

# Test 3: Aspect with nested required_providers (any-of syntax)
# Note: In Phase 8a, this is parsed but not executed
# test_aspect_nested_providers = aspect(
#     implementation = _test_aspect_impl,
#     attr_aspects = ["*"],
#     required_providers = [[DefaultInfo], [OutputGroupInfo]],
#     doc = "Test aspect with any-of providers filter",
# )

# Test 4: Aspect with custom attributes
test_aspect_with_attrs = aspect(
    implementation = _test_aspect_impl,
    attr_aspects = ["deps"],
    attrs = {
        "_tool": attr.label(default = "//:BUILD.bazel"),
    },
)

# Test 5: Aspect with fragments and toolchains (accepted but ignored in Phase 8a)
test_aspect_with_fragments = aspect(
    implementation = _test_aspect_impl,
    attr_aspects = ["deps"],
    fragments = ["cpp", "java"],
    toolchains = ["@bazel_tools//tools/cpp:toolchain_type"],
)

def test_aspects_available():
    """Returns test results for aspect definitions."""
    return {
        "simple_aspect": test_aspect != None,
        "aspect_with_providers": test_aspect_with_providers != None,
        "aspect_with_attrs": test_aspect_with_attrs != None,
        "aspect_with_fragments": test_aspect_with_fragments != None,
        "aspect_type": type(test_aspect),
    }

# Test rule that uses aspects on its deps attribute
def _test_rule_with_aspect_impl(ctx):
    """Simple rule implementation that triggers aspect execution."""
    print("Test rule analyzing:", ctx.label)
    return [DefaultInfo()]

test_rule_with_aspect = rule(
    implementation = _test_rule_with_aspect_impl,
    attrs = {
        "deps": attr.label_list(
            aspects = [test_aspect],
            doc = "Dependencies with test_aspect attached",
        ),
    },
)
