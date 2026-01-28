# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Starlark cc_common module for rules_cc compatibility.

This module provides the complete cc_common implementation in Starlark.
It is injected as a global in BUILD files via the bootstrap mechanism.

The cc_common module has two types of methods:

1. ACTION PRIMITIVES - Create/manipulate build actions. Since Kuro only supports
   Bazel 9.0 and these are all stubs, they return None.
   - create_cc_compile_action, create_cc_compile_action_template
   - wrap_link_actions, declare_compile_output_file, declare_other_output_file
   - actions2ctx_cheat, create_compile_action

2. STUB METHODS - Return hardcoded values, empty collections, or pass-through.

Usage:
    # cc_common is available as a global in BUILD files
    tool_path = cc_common.get_tool_for_action(feature_configuration=fc, action_name="c++-compile")

Reference: https://bazel.build/rules/lib/cc_common
"""

# ============================================================================
# Tool path helpers
# ============================================================================

_DEFAULT_TOOLS = {
    "c-compile": "/usr/bin/gcc",
    "c++-compile": "/usr/bin/g++",
    "c++-link-executable": "/usr/bin/g++",
    "c++-link-dynamic-library": "/usr/bin/g++",
    "c++-link-nodeps-dynamic-library": "/usr/bin/g++",
    "c++-link-static-library": "/usr/bin/ar",
    "strip": "/usr/bin/strip",
    "objcopy": "/usr/bin/objcopy",
    "objcopy_embed_data": "/usr/bin/objcopy",
    "ld": "/usr/bin/ld",
    "gcc": "/usr/bin/gcc",
    "ar": "/usr/bin/ar",
}

def _get_tool_for_action(feature_configuration, action_name):
    """Gets the tool path for a given action.

    Args:
        feature_configuration: The feature configuration (unused in stub).
        action_name: The name of the action to get the tool for.

    Returns:
        A string path to the tool.
    """
    _ = feature_configuration  # unused
    return _DEFAULT_TOOLS.get(action_name, "/usr/bin/gcc")

# ============================================================================
# Artifact naming helpers
# ============================================================================

def _get_artifact_name_for_category(cc_toolchain, category, output_name = ""):
    """Gets the artifact name for a given category.

    This implements Bazel's artifact naming conventions for C++ compilation.

    Args:
        cc_toolchain: The C++ toolchain (unused in stub).
        category: The artifact category (object_file, executable, etc.).
        output_name: Optional output name base.

    Returns:
        The artifact name with appropriate extension.
    """
    _ = cc_toolchain  # unused
    name = output_name if output_name else "output"

    if category == "object_file":
        return name + ".o"
    elif category == "pic_object_file":
        return name + ".pic.o"
    elif category == "executable":
        return name
    elif category == "static_library":
        return "lib" + name + ".a"
    elif category == "dynamic_library":
        return "lib" + name + ".so"
    elif category == "interface_library":
        return "lib" + name + ".so"
    elif category == "pic_file":
        return name + ".pic"
    elif category == "preprocessed_c_source":
        return name + ".i"
    elif category == "preprocessed_cpp_source":
        return name + ".ii"
    elif category == "coverage_data_file":
        return name + ".gcno"
    elif category == "coverage_notes_file":
        return name + ".gcda"
    else:
        return name + "." + category

# ============================================================================
# Variables helpers
# ============================================================================

def _empty_variables():
    """Creates an empty CcToolchainVariables-like struct.

    Returns:
        An empty struct representing toolchain variables.
    """
    return struct()

def _combine_cc_toolchain_variables(parent = None, child = None):
    """Combines toolchain variables from parent and child.

    Args:
        parent: Parent variables to merge (optional).
        child: Child variables to merge (optional).

    Returns:
        A combined struct of variables.
    """
    _ = parent, child  # unused in stub
    return struct()

def _cc_toolchain_variables(vars = None):
    """Creates CcToolchainVariables from a dictionary.

    Args:
        vars: Dictionary of variables to create.

    Returns:
        A struct representing the variables.
    """
    _ = vars  # unused in stub
    return struct()

# ============================================================================
# Feature configuration helpers
# ============================================================================

def _action_is_enabled(feature_configuration, action_name):
    """Checks if an action is enabled in the feature configuration.

    Args:
        feature_configuration: The feature configuration.
        action_name: The action name to check.

    Returns:
        True if the action is enabled (always True in stub).
    """
    _ = feature_configuration, action_name
    return True

def _get_execution_requirements(feature_configuration, action_name):
    """Gets execution requirements for an action.

    Args:
        feature_configuration: The feature configuration.
        action_name: The action name.

    Returns:
        A dict of execution requirements (empty in stub).
    """
    _ = feature_configuration, action_name
    return {}

def _get_memory_inefficient_command_line(feature_configuration, action_name, variables):
    """Gets the command line for an action.

    Args:
        feature_configuration: The feature configuration.
        action_name: The action name.
        variables: The toolchain variables.

    Returns:
        A list of command line arguments (empty in stub).
    """
    _ = feature_configuration, action_name, variables
    return []

def _get_environment_variables(feature_configuration, action_name, variables):
    """Gets environment variables for an action.

    Args:
        feature_configuration: The feature configuration.
        action_name: The action name.
        variables: The toolchain variables.

    Returns:
        A dict of environment variables (empty in stub).
    """
    _ = feature_configuration, action_name, variables
    return {}

# ============================================================================
# Misc helpers
# ============================================================================

def _legacy_cc_flags_make_variable_do_not_use(cc_toolchain):
    """Gets legacy CC_FLAGS make variable value.

    Args:
        cc_toolchain: The C++ toolchain.

    Returns:
        The CC_FLAGS value (empty string in stub).
    """
    _ = cc_toolchain
    return ""

def _check_experimental_cc_shared_library():
    """Checks if experimental cc_shared_library is enabled.

    Returns:
        True (always enabled in stub).
    """
    return True

def _incompatible_disable_objc_library_transition():
    """Checks if objc_library transition is disabled.

    Returns:
        False (not disabled in stub).
    """
    return False

def _add_go_exec_groups_to_binary_rules():
    """Checks if Go exec groups should be added to binary rules.

    Returns:
        False (not enabled in stub).
    """
    return False

def _implementation_deps_allowed_by_allowlist(ctx):
    """Checks if implementation_deps is allowed by allowlist.

    Args:
        ctx: The rule context.

    Returns:
        True (always allowed in stub).
    """
    _ = ctx
    return True

def _freeze(value):
    """Freezes a list to an immutable tuple.

    Args:
        value: The value to freeze.

    Returns:
        The value unchanged (no actual freezing in stub).
    """
    return value

def _is_tree_artifact(artifact):
    """Checks if an artifact is a tree artifact.

    Args:
        artifact: The artifact to check.

    Returns:
        False (always false in stub).
    """
    _ = artifact
    return False

def _compute_output_name_prefix_dir(output_name):
    """Computes the output name prefix directory.

    Args:
        output_name: The output name.

    Returns:
        Empty string in stub.
    """
    _ = output_name
    return ""

def _intern_string_sequence_variable_value(value):
    """Interns a string sequence variable value.

    Args:
        value: The value to intern.

    Returns:
        The value unchanged.
    """
    return value

def _per_file_copts(ctx):
    """Gets per-file compile options.

    Args:
        ctx: The rule context.

    Returns:
        Empty list in stub.
    """
    _ = ctx
    return []

def _check_private_api(allowlist):
    """Checks access to private API.

    Args:
        allowlist: The allowlist to check against.

    Returns:
        True (always allowed in stub).
    """
    _ = allowlist
    return True

def _get_link_args(linking_context, expand_to_linker_flags = False):
    """Gets link arguments from a linking context.

    Args:
        linking_context: The linking context.
        expand_to_linker_flags: Whether to expand to linker flags.

    Returns:
        Empty list in stub.
    """
    _ = linking_context, expand_to_linker_flags
    return []

def _create_header_info(headers = None, modular_headers = None, textual_headers = None):
    """Creates a HeaderInfo struct.

    Args:
        headers: Direct headers.
        modular_headers: Modular headers.
        textual_headers: Textual headers.

    Returns:
        A struct with header information.
    """
    return struct(
        headers = headers if headers else depset(),
        modular_headers = modular_headers if modular_headers else depset(),
        textual_headers = textual_headers if textual_headers else depset(),
    )

def _create_header_info_with_deps(headers = None, modular_headers = None, textual_headers = None, deps = None):
    """Creates a HeaderInfo struct with dependency tracking.

    Args:
        headers: Direct headers.
        modular_headers: Modular headers.
        textual_headers: Textual headers.
        deps: Header dependencies.

    Returns:
        A struct with header information.
    """
    return struct(
        headers = headers if headers else depset(),
        modular_headers = modular_headers if modular_headers else depset(),
        textual_headers = textual_headers if textual_headers else depset(),
        deps = deps if deps else [],
    )

# ============================================================================
# Action Primitives (stubs that return None)
# ============================================================================

def _create_cc_compile_action(**kwargs):
    """Creates a C++ compile action.

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = kwargs
    return None

def _create_cc_compile_action_template(**kwargs):
    """Creates a tree artifact compile action template.

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = kwargs
    return None

def _wrap_link_actions(actions = None, linking_outputs = None, **kwargs):
    """Wraps link actions for platform compatibility.

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = actions, linking_outputs, kwargs
    return None

def _declare_compile_output_file(actions = None, cc_toolchain = None, source_file = None, output_name = "", pic = False, **kwargs):
    """Declares a compile output file.

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = actions, cc_toolchain, source_file, output_name, pic, kwargs
    return None

def _declare_other_output_file(actions = None, cc_toolchain = None, source_file = None, extension = "", **kwargs):
    """Declares an auxiliary output file (dwo, gcno, etc.).

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = actions, cc_toolchain, source_file, extension, kwargs
    return None

def _actions2ctx_cheat(actions = None, **kwargs):
    """Gets the rule context from an actions object.

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = actions, kwargs
    return None

def _create_compile_action(**kwargs):
    """Creates a compilation action (allowlisted).

    ACTION PRIMITIVE: Returns None in stub.
    """
    _ = kwargs
    return None

# ============================================================================
# Internal API struct for cc_common.internal_DO_NOT_USE compatibility
# ============================================================================

# This struct contains internal methods used by rules_cc.
# All methods are either stubs or action primitives that return None.

_cc_common_internal = struct(
    # Action Primitives (return None)
    create_cc_compile_action = _create_cc_compile_action,
    create_cc_compile_action_template = _create_cc_compile_action_template,
    wrap_link_actions = _wrap_link_actions,
    declare_compile_output_file = _declare_compile_output_file,
    declare_other_output_file = _declare_other_output_file,
    actions2ctx_cheat = _actions2ctx_cheat,
    # Stub methods
    get_artifact_name_for_category = _get_artifact_name_for_category,
    combine_cc_toolchain_variables = _combine_cc_toolchain_variables,
    cc_toolchain_variables = _cc_toolchain_variables,
    freeze = _freeze,
    get_link_args = _get_link_args,
    is_tree_artifact = _is_tree_artifact,
    compute_output_name_prefix_dir = _compute_output_name_prefix_dir,
    intern_string_sequence_variable_value = _intern_string_sequence_variable_value,
    per_file_copts = _per_file_copts,
    check_private_api = _check_private_api,
    create_header_info = _create_header_info,
    create_header_info_with_deps = _create_header_info_with_deps,
)

# ============================================================================
# Main cc_common module
# ============================================================================

# This is the complete cc_common module that will be injected as a global.
# It matches the Bazel cc_common API.

cc_common = struct(
    # Attributes
    internal_DO_NOT_USE = _cc_common_internal,
    CcToolchainInfo = None,  # Placeholder - will be defined by rules_cc
    do_not_use_tools_cpp_compiler_present = True,
    # Public methods
    get_tool_for_action = _get_tool_for_action,
    get_execution_requirements = _get_execution_requirements,
    action_is_enabled = _action_is_enabled,
    get_memory_inefficient_command_line = _get_memory_inefficient_command_line,
    get_environment_variables = _get_environment_variables,
    empty_variables = _empty_variables,
    legacy_cc_flags_make_variable_do_not_use = _legacy_cc_flags_make_variable_do_not_use,
    check_experimental_cc_shared_library = _check_experimental_cc_shared_library,
    incompatible_disable_objc_library_transition = _incompatible_disable_objc_library_transition,
    add_go_exec_groups_to_binary_rules = _add_go_exec_groups_to_binary_rules,
    implementation_deps_allowed_by_allowlist = _implementation_deps_allowed_by_allowlist,
    # Action primitives (return None)
    create_compile_action = _create_compile_action,
)

# ============================================================================
# Legacy exports for compatibility
# ============================================================================

# These are exported for backward compatibility with code that loads
# individual helpers from this file.

cc_common_helpers = struct(
    get_tool_for_action = _get_tool_for_action,
    get_execution_requirements = _get_execution_requirements,
    action_is_enabled = _action_is_enabled,
    get_memory_inefficient_command_line = _get_memory_inefficient_command_line,
    get_environment_variables = _get_environment_variables,
    empty_variables = _empty_variables,
    legacy_cc_flags_make_variable_do_not_use = _legacy_cc_flags_make_variable_do_not_use,
    check_experimental_cc_shared_library = _check_experimental_cc_shared_library,
    incompatible_disable_objc_library_transition = _incompatible_disable_objc_library_transition,
    add_go_exec_groups_to_binary_rules = _add_go_exec_groups_to_binary_rules,
    implementation_deps_allowed_by_allowlist = _implementation_deps_allowed_by_allowlist,
    # Internal helpers
    internal = _cc_common_internal,
)

cc_common_internal_helpers = _cc_common_internal
