# Simplified BUILD file for Kuro compatibility
# Only contains targets needed by rules_cc cc_binary/cc_test

load("@rules_cc//cc:defs.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

licenses(["notice"])

exports_files(glob(["*.bzl"]))

# Default malloc implementation - cc_binary uses this if no custom malloc is specified
# In Bazel, this is typically an empty cc_library (uses system malloc)
cc_library(
    name = "malloc",
    visibility = ["//visibility:public"],
)

# Extra libraries to link into cc_binary
# In Bazel, this allows injecting additional libraries via --@bazel_tools//tools/cpp:link_extra_libs
cc_library(
    name = "link_extra_lib",
    visibility = ["//visibility:public"],
)
