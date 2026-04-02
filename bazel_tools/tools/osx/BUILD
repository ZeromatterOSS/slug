# Stub BUILD file for bazel_tools//tools/osx on non-Apple platforms.
#
# The real Bazel osx BUILD has complex genrules, select() conditions, and
# xcode_version_flag rules that require Apple SDKs. On non-Apple platforms,
# we only need the xcode_config stub so cc_toolchain_config.bzl can resolve
# its _xcode_config dependency.

package(default_visibility = ["//visibility:public"])

# Stub xcode_config for non-Apple platforms.
# cc_toolchain_config.bzl unconditionally accesses this via
# configuration_field(fragment="apple", name="xcode_config_label").
# Provides XcodeVersionConfig with dummy values.
xcode_config(
    name = "current_xcode_config",
)
