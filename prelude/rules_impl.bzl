# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# This file provides the core Buck2/Kuro prelude rule implementations.
# Buck2-specific language rules (android, apple, cxx, erlang, haskell, java,
# kotlin, python, rust, etc.) have been removed. Use the corresponding Bazel
# rules_* packages instead (rules_cc, rules_rust, rules_python, etc.).

load("@prelude//:alias.bzl", "alias_impl", "configured_alias_impl", "versioned_alias_impl")
load("@prelude//:command_alias.bzl", "command_alias_impl")
load("@prelude//:export_file.bzl", "export_file_impl")
load("@prelude//:filegroup.bzl", "filegroup_impl")
load("@prelude//:genrule.bzl", "genrule_attributes", "genrule_impl")
load("@prelude//:http_file.bzl", "http_file_impl")
load("@prelude//:remote_file.bzl", "remote_file_impl")
load("@prelude//:sh_binary.bzl", "sh_binary_impl")
load("@prelude//:sh_test.bzl", "sh_test_impl")
load("@prelude//:test_suite.bzl", "test_suite_impl")
load("@prelude//configurations:rules.bzl", _config_extra_attributes = "extra_attributes", _config_implemented_rules = "implemented_rules")
load("@prelude//decls:common.bzl", "buck")
load("@prelude//decls:core_rules.bzl", "core_rules")
load("@prelude//decls:shell_rules.bzl", "shell_rules")
load("@prelude//git:git_fetch.bzl", "git_fetch_impl")
load("@prelude//http_archive:http_archive.bzl", "http_archive_impl")
load("@prelude//transitions:constraint_overrides.bzl", "constraint_overrides")
load("@prelude//zip_file:zip_file.bzl", _zip_file_extra_attributes = "extra_attributes", _zip_file_implemented_rules = "implemented_rules")

_CORE_RULES_KEY = "core"
_SHELL_RULES_KEY = "shell"

categorized_rule_decl_records = {
    _CORE_RULES_KEY: core_rules,
    _SHELL_RULES_KEY: shell_rules,
}

def _merge_dicts(dicts):
    result = {}
    for d in dicts:
        for key, value in d.items():
            result[key] = value
    return result

_extra_impl_rules = _merge_dicts([
    {
        # Core rules
        "alias": alias_impl,
        "command_alias": command_alias_impl,
        "configured_alias": configured_alias_impl,
        "export_file": export_file_impl,
        "filegroup": filegroup_impl,
        "genrule": genrule_impl,
        "git_fetch": git_fetch_impl,
        "http_archive": http_archive_impl,
        "http_file": http_file_impl,
        "remote_file": remote_file_impl,
        "sh_binary": sh_binary_impl,
        "sh_test": sh_test_impl,
        "test_suite": test_suite_impl,
        "toolchain_alias": alias_impl,
        "versioned_alias": versioned_alias_impl,
    },
    _config_implemented_rules,
    _zip_file_implemented_rules,
])

extra_implemented_rules = struct(**_extra_impl_rules)

_core_extra_attributes = {
    "export_file": constraint_overrides.attributes,
    "filegroup": constraint_overrides.attributes,
    "genrule": genrule_attributes() | constraint_overrides.attributes,
    "remote_file": {
        "sha1": attrs.option(attrs.string(), default = None),
        "sha256": attrs.option(attrs.string(), default = None),
        "_unzip_tool": attrs.default_only(attrs.exec_dep(providers = [RunInfo], default = "prelude//zip_file/tools:unzip")),
    },
}

categorized_extra_attributes = {
    _CORE_RULES_KEY: _core_extra_attributes | _config_extra_attributes | _zip_file_extra_attributes,
    _SHELL_RULES_KEY: {
        "sh_test": constraint_overrides.attributes,
    },
}

toolchain_rule_names = []
