"""Test that Bazel compatibility modules are available as globals in .bzl files.

This verifies that REGISTER_BUCK2_BUILD_API_GLOBALS registration works correctly,
making these modules available in ALL Starlark contexts (not just BUILD files).
"""

def test_bazel_modules_available():
    """Test that bazel compatibility modules are globals in .bzl context."""
    # These should all be available as globals (not via native.*)
    results = {
        "config_common": hasattr(config_common, "toolchain_type"),
        "platform_common": hasattr(platform_common, "TemplateVariableInfo"),
        "apple_common": hasattr(apple_common, "platform_type"),
        "coverage_common": hasattr(coverage_common, "instrumented_files_info"),
    }
    return results
