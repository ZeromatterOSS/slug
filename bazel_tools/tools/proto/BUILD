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

load("//tools/proto:cc_proto_toolchain.bzl", "cc_proto_toolchain")

cc_proto_toolchain(
    name = "cc_toolchain",
    runtime = "@protobuf//src/google/protobuf",
)
