# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# `native` is fine to use in the prelude for v2
# @lint-ignore-every BUCKLINT

# This is kuro's shim import. Any public symbols here will be available within
# **all** interpreted files.

load("@prelude//user:all.bzl", _user_rules = "rules")
load("@prelude//utils:buckconfig.bzl", _read_config = "read_config_with_logging", _read_root_config = "read_root_config_with_logging", log_buckconfigs = "LOG_BUCKCONFIGS")

def __struct_to_dict(s):
    vals = {}
    for name in dir(s):
        vals[name] = getattr(s, name)
    return vals

# When buckconfig logging is enabled (Meta-internal), override read_config/read_root_config
# with versions that log their usage. In OSS this is always empty.
__overridden_builtins__ = {
    "read_config": _read_config,
    "read_root_config": _read_root_config,
} if log_buckconfigs else {}

__shimmed_native__ = __struct_to_dict(__kuro_builtins__)
__shimmed_native__.update(__overridden_builtins__)
__shimmed_native__.update(_user_rules)

native = struct(**__shimmed_native__)
