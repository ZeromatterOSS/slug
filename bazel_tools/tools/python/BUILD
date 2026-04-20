load("//tools/build_rules:filegroup.bzl", "filegroup")

package(default_visibility = ["PUBLIC"])

# Python toolchain type - referenced by rules_python
toolchain_type(
    name = "toolchain_type",
    visibility = ["PUBLIC"],
)

filegroup(
    name = "srcs",
    srcs = [],
    visibility = ["PUBLIC"],
)

# Bootstrap template for Python executable scripts.
#
# The source file `python_bootstrap_template.txt` exists in this package,
# so referencing `:python_bootstrap_template.txt` resolves directly to
# that file — no filegroup wrapper is needed, and adding one with the
# same name would create a self-referential cycle under kuro.
exports_files(
    ["python_bootstrap_template.txt"],
    visibility = ["PUBLIC"],
)
