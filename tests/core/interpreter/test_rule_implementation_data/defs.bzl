# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Rule definitions for testing implementation parameter handling."""

def _simple_impl(ctx):
    """Simple rule implementation that does nothing."""
    return [DefaultInfo()]

# Bazel-style: using `implementation` parameter (preferred)
bazel_style_rule = rule(
    implementation = _simple_impl,
    attrs = {},
)

# Kuro-style: using `impl` parameter (also supported)
kuro_style_rule = rule(
    impl = _simple_impl,
    attrs = {},
)

# Error case: both parameters specified (should fail)
# Uncomment to test error handling:
# both_params_rule = rule(
#     impl = _simple_impl,
#     implementation = _simple_impl,
#     attrs = {},
# )

# Error case: neither parameter specified (should fail)
# Uncomment to test error handling:
# no_impl_rule = rule(
#     attrs = {},
# )
