# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//http_archive:extract_archive.bzl", _extract_archive_spec = "registration_spec")
load(":write_file.bzl", _write_file_spec = "registration_spec")

_all_specs = [
    _extract_archive_spec,
    _write_file_spec,
]

rules = {
    s.name: rule(
        impl = s.impl,
        attrs = s.attrs,
        doc = s.doc,
        is_toolchain_rule = s.is_toolchain_rule,
        **{k: v for k, v in {"cfg": s.cfg}.items() if v != None}
    )
    for s in _all_specs
}

# The rules are accessed by doing module.name, so we have to put them on the correct module.
load_symbols(rules)
