# Test module_extension() and tag_class() globals
# These are used to define module extensions in .bzl files

def _test_extension_impl(module_ctx):
    """Test implementation - just a stub for now."""
    # In a real extension, we would iterate over module_ctx.modules
    # and create repositories based on tags
    pass

# Test that tag_class() can be called with attrs
_test_tag = tag_class(
    attrs = {
        "name": attr.string(mandatory = True),
        "version": attr.string(default = "1.0"),
    },
    doc = "A test tag class",
)

# Test that module_extension() can be called with tag_classes
test_extension = module_extension(
    implementation = _test_extension_impl,
    tag_classes = {
        "test": _test_tag,
    },
    doc = "A test module extension",
)

def test_module_extension_available():
    """Returns True if module_extension and tag_class are available."""
    # If we got here without errors, the globals are available
    return True
