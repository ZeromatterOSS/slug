# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file. You may select, at
# your option, one of the above-listed licenses.

def python_pytest(srcs = [], visibility = ["PUBLIC"], **kwargs):
    # @lint-ignore BUCKLINT: avoid "Direct usage of native rules is not allowed."
    native.python_test(srcs = srcs, visibility = visibility, **kwargs)
