# Minimal bazel_tools shim for tools/build_defs/repo/http.bzl
#
# In real Bazel, this provides http_archive, http_file, http_jar.
# This shim provides stub implementations for testing.
#
# When Phase 5c is complete, this will be replaced with the real
# bazel_tools bundled from Bazel's repository.

def _http_archive_impl(ctx):
    """Stub implementation - not functional yet."""
    fail("http_archive is not yet implemented in Kuro. See Phase 5c.")

http_archive = rule(
    implementation = _http_archive_impl,
    attrs = {
        "url": attr.string(),
        "urls": attr.string_list(),
        "sha256": attr.string(),
        "strip_prefix": attr.string(),
        "build_file": attr.label(),
        "build_file_content": attr.string(),
    },
)

def _http_file_impl(ctx):
    """Stub implementation - not functional yet."""
    fail("http_file is not yet implemented in Kuro. See Phase 5c.")

http_file = rule(
    implementation = _http_file_impl,
    attrs = {
        "url": attr.string(),
        "urls": attr.string_list(),
        "sha256": attr.string(),
        "downloaded_file_path": attr.string(),
        "executable": attr.bool(default = False),
    },
)
