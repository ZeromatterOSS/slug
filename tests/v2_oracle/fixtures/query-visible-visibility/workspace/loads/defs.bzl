def make_consumer(name):
    native.filegroup(
        name = name,
        srcs = ["defs.bzl"],
    )
