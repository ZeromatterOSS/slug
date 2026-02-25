package(default_visibility = ["//visibility:public"])

filegroup(
    name = "srcs",
    srcs = glob(["**"] + ["**/*"]),
)

exports_files(["BUILD"])

# Those aliases are needed to resolve the repository name correctly in both
# bzlmod and WORKSPACE mode. They are resolved in the namespace of MODULE.tools

alias(
    name = "protoc",
    actual = "@com_google_protobuf//:protoc",
)

alias(
    name = "javalite_toolchain",
    actual = "@com_google_protobuf//:javalite_toolchain",
)

alias(
    name = "java_toolchain",
    actual = "@com_google_protobuf//:java_toolchain",
)

# System protoc wrapper - copies the host-installed /usr/bin/protoc (includes Python generator).
# Used as the default proto compiler for toolchains that need full protoc support (py, java, etc.)
# cc_toolchain overrides this via protoc_minimal_do_not_use, so it's unaffected.
genrule(
    name = "system_protoc",
    outs = ["system_protoc_bin"],
    cmd = "cp /usr/bin/protoc $@",
    visibility = ["//visibility:public"],
)

load("//tools/proto:cc_proto_toolchain.bzl", "cc_proto_toolchain")

cc_proto_toolchain(
    name = "cc_toolchain",
    runtime = "@protobuf//src/google/protobuf",
)
