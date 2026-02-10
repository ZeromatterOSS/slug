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

# Bootstrap template for Python executable scripts
filegroup(
    name = "python_bootstrap_template.txt",
    srcs = ["python_bootstrap_template.txt"],
    visibility = ["PUBLIC"],
)
