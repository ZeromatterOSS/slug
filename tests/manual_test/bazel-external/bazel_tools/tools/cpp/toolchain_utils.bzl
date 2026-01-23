# Minimal bazel_tools shim for tools/cpp/toolchain_utils.bzl
#
# In real Bazel, this file re-exports from @rules_cc.
# This shim provides stub implementations for testing.
#
# When Phase 5c is complete, this will be replaced with the real
# bazel_tools bundled from Bazel's repository.

def find_cpp_toolchain(ctx):
    """Stub implementation - returns None for now."""
    return None

def use_cpp_toolchain():
    """Stub implementation - returns empty list for now."""
    return []
