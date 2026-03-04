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

def __struct_to_dict(s):
    vals = {}
    for name in dir(s):
        vals[name] = getattr(s, name)
    return vals

__shimmed_native__ = __struct_to_dict(__kuro_builtins__)

native = struct(**__shimmed_native__)
