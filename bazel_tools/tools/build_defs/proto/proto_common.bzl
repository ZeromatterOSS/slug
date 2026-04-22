# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Starlark proto_common module for protobuf rules compatibility.

This module provides the complete proto_common_do_not_use implementation in Starlark.
It is injected as a global in BUILD files via the bootstrap mechanism.

The proto_common module has two types of methods:

1. ACTION PRIMITIVES - Create proto compilation actions. Since Kuro only supports
   Bazel 9.0 and these are all stubs, they return None.
   - compile()

2. STUB METHODS - Return hardcoded values.

Usage:
    # proto_common_do_not_use is available as a global in BUILD files
    flag = proto_common_do_not_use.proto_path_flag(proto_lang_toolchain_info)

Reference: https://bazel.build/rules/lib/proto_common
"""

# ============================================================================
# Proto path helpers
# ============================================================================

def _proto_path_flag(proto_lang_toolchain_info):
    """Returns the proto path flag.

    Args:
        proto_lang_toolchain_info: The proto language toolchain info.

    Returns:
        The proto path flag string.
    """
    _ = proto_lang_toolchain_info
    return "--proto_path="

def _descriptor_set_flag(proto_lang_toolchain_info):
    """Returns the descriptor set output flag.

    Args:
        proto_lang_toolchain_info: The proto language toolchain info.

    Returns:
        The descriptor set flag string.
    """
    _ = proto_lang_toolchain_info
    return "--descriptor_set_out="

# ============================================================================
# Toolchain helpers
# ============================================================================

def _get_tool_path(proto_lang_toolchain_info):
    """Gets the protoc tool path.

    Args:
        proto_lang_toolchain_info: The proto language toolchain info.

    Returns:
        The path to protoc.
    """
    _ = proto_lang_toolchain_info
    return "/usr/bin/protoc"

def _has_plugin(proto_lang_toolchain_info):
    """Checks if a proto toolchain has a plugin.

    Args:
        proto_lang_toolchain_info: The proto language toolchain info.

    Returns:
        False (no plugin in stub).
    """
    _ = proto_lang_toolchain_info
    return False

# ============================================================================
# Configuration helpers
# ============================================================================

def _experimental_use_proto_source_order():
    """Checks if experimental_use_proto_source_order is enabled.

    Returns:
        False (not enabled in stub).
    """
    return False

# ============================================================================
# Action Primitives (stubs that return None)
# ============================================================================

def _compile(
        actions,
        proto_info,
        proto_lang_toolchain_info = None,
        generated_files = None,
        plugin_output = None,
        additional_args = None,
        additional_inputs = None,
        additional_tools = None,
        resource_set = "",
        experimental_progress_message = False):
    """Compiles proto files using the proto toolchain.

    ACTION PRIMITIVE: Returns None in stub.

    Args:
        actions: The actions object.
        proto_info: Proto info for the sources.
        proto_lang_toolchain_info: Language toolchain info.
        generated_files: Output files to generate.
        plugin_output: Plugin output directory.
        additional_args: Additional arguments.
        additional_inputs: Additional input files.
        additional_tools: Additional tools.
        resource_set: Resource set name.
        experimental_progress_message: Whether to show progress.

    Returns:
        None (stub implementation).
    """
    _ = (
        actions,
        proto_info,
        proto_lang_toolchain_info,
        generated_files,
        plugin_output,
        additional_args,
        additional_inputs,
        additional_tools,
        resource_set,
        experimental_progress_message,
    )
    return None

# ============================================================================
# Main proto_common_do_not_use module
# ============================================================================

# This is the complete proto_common_do_not_use module that will be injected as a global.
# It matches the Bazel proto_common API (internal, do_not_use).

proto_common_do_not_use = struct(
    # Attributes
    INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION = True,
    # Public methods
    proto_path_flag = _proto_path_flag,
    descriptor_set_flag = _descriptor_set_flag,
    get_tool_path = _get_tool_path,
    has_plugin = _has_plugin,
    experimental_use_proto_source_order = _experimental_use_proto_source_order,
    # Action primitive (returns None)
    compile = _compile,
)
