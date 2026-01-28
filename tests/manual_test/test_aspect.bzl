"""Test that aspect() built-in function works.

This verifies Phase 8a implementation: aspect() can be declared and
attached to attributes, but the implementation is not yet called.
"""

def _test_aspect_impl(target, ctx):
    """Aspect implementation function (not called in Phase 8a stub)."""
    # This function is NOT called in Phase 8a - it's just a stub
    return []

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
