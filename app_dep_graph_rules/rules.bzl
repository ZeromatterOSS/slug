# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Avoid some copy-paste
def _app(s):
    return "//slug/app/" + s + ":" + s

# These crates should only implement late bindings and not be depended on
# directly
LATE_BINDING_ONLY_CRATES = [
    _app("slug_anon_target"),
    _app("slug_cmd_audit_server"),
    _app("slug_cmd_query_server"),
    _app("slug_cmd_targets_server"),
    _app("slug_bxl"),
    _app("slug_query_impls"),
]

# These crates may only be depended on from `app/slug`
TOP_LEVEL_ONLY_CRATES = [
    _app("slug_cmd_debug_client"),
    _app("slug_cmd_log_client"),
]

# Unordered pairs where neither crate may depend on the other
BANNED_DEP_PATHS = [
    (_app("slug_common"), _app("slug_directory")),
    (_app("slug_common"), "//slug/starlark-rust/starlark:starlark"),
    (_app("slug_build_api"), _app("slug_execute_impl")),
    (_app("slug_build_api"), _app("slug_interpreter_for_build")),
    (_app("slug_server"), _app("slug_server_commands")),
    (_app("slug_bxl"), _app("slug_configured")),
]
