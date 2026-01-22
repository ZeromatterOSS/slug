# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Avoid some copy-paste
def _app(s):
    return "//kuro/app/" + s + ":" + s

# These crates should only implement late bindings and not be depended on
# directly
LATE_BINDING_ONLY_CRATES = [
    _app("kuro_anon_target"),
    _app("kuro_cmd_audit_server"),
    _app("kuro_cmd_query_server"),
    _app("kuro_cmd_targets_server"),
    _app("kuro_bxl"),
    _app("kuro_query_impls"),
]

# These crates may only be depended on from `app/kuro`
TOP_LEVEL_ONLY_CRATES = [
    _app("kuro_cmd_debug_client"),
    _app("kuro_cmd_log_client"),
]

# Unordered pairs where neither crate may depend on the other
BANNED_DEP_PATHS = [
    (_app("kuro_common"), _app("kuro_directory")),
    (_app("kuro_common"), "//kuro/starlark-rust/starlark:starlark"),
    (_app("kuro_build_api"), _app("kuro_execute_impl")),
    (_app("kuro_build_api"), _app("kuro_interpreter_for_build")),
    (_app("kuro_server"), _app("kuro_server_commands")),
    (_app("kuro_bxl"), _app("kuro_configured")),
]
