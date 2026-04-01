# BUILD file for @bazel_tools//tools/cpp
# Contains targets referenced by rules_cc cc_toolchain, cc_binary, cc_library

load("@rules_cc//cc:cc_library.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

licenses(["notice"])

exports_files(glob(["*.bzl"]))

# The toolchain type used to distinguish cc toolchains.
toolchain_type(name = "toolchain_type")

# The toolchain type used for cc runtimes.
toolchain_type(name = "cc_runtimes_toolchain_type")

# The toolchain_type used for cc test runners.
toolchain_type(name = "test_runner_toolchain_type")

# Aliases to @rules_cc targets (needed by cc_toolchain implicit deps)
[
    alias(
        name = target,
        actual = actual,
    )
    for target, actual in {
        "cc_compiler": "@rules_cc//cc/private/toolchain:cc_compiler",
        "clang": "@rules_cc//cc/private/toolchain:clang",
        "gcc": "@rules_cc//cc/private/toolchain:gcc",
        "msvc": "@rules_cc//cc/private/toolchain:msvc",
        "clang-cl": "@rules_cc//cc/private/toolchain:clang-cl",
        "mingw": "@rules_cc//cc/private/toolchain:mingw",
        "msys": "@rules_cc//cc/private/toolchain:msys",
        "grep-includes": "@rules_cc//cc/private/toolchain:grep-includes",
        "interface_library_builder": "@rules_cc//cc/private/toolchain:interface_library_builder",
        "link_dynamic_library": "@rules_cc//cc/private/toolchain:link_dynamic_library",
        "lib_cc_configure": "@rules_cc//cc/private/toolchain:lib_cc_configure",
        "compiler": "@rules_cc//cc/private/toolchain:compiler",
        "cc_flags": "@rules_cc//cc/private/toolchain:cc_flags",
        "current_cc_toolchain": "@rules_cc//cc:current_cc_toolchain",
        "optional_current_cc_toolchain": "@rules_cc//cc:optional_current_cc_toolchain",
        "current_libc_top": "@rules_cc//cc:current_libc_top",
    }.items()
]

cc_library(
    name = "empty_lib",
)

# Label flag for extra libraries to be linked into every binary.
label_flag(
    name = "link_extra_libs",
    build_setting_default = ":empty_lib",
)

# The final extra library to be linked into every binary target.
cc_library(
    name = "link_extra_lib",
    deps = [
        ":link_extra_libs",
    ],
)

cc_library(
    name = "malloc",
)

filegroup(
    name = "empty",
    srcs = [],
)

# Module tools (stubs - C++ modules not supported yet)
filegroup(
    name = "aggregate-ddi",
    srcs = [],
)

filegroup(
    name = "generate-modmap",
    srcs = [],
)

filegroup(
    name = "bzl_srcs",
    srcs = glob(["*.bzl"]),
)

config_setting(
    name = "disable_static_cc_toolchains",
    values = {
        "incompatible_disable_static_cc_toolchains": "true",
    },
)

# Points to @local_config_cc//:toolchain for --crosstool_top
alias(
    name = "toolchain",
    actual = "@local_config_cc//:toolchain",
)
