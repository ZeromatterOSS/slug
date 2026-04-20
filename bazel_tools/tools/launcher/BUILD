package(default_visibility = ["//visibility:public"])

filegroup(
    name = "srcs",
    srcs = glob(["**"]),
)

alias(
    name = "launcher",
    actual = "//src/tools/launcher:launcher",
)

alias(
    name = "launcher_maker",
    actual = "//src/tools/launcher:launcher_maker",
)

# Toolchain types for launcher toolchains. rules_python 1.9 requires
# launcher_maker_toolchain_type as a mandatory toolchain on py_binary when
# bazel_9_or_later is true, even on non-Windows targets where the launcher
# is never invoked. See @local_config_python for the stub registration.
toolchain_type(name = "launcher_toolchain_type")

toolchain_type(name = "launcher_maker_toolchain_type")
