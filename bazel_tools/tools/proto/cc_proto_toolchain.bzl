"""Minimal proto_lang_toolchain for C++ code generation.

This provides the ProtoLangToolchainInfo needed by cc_proto_library's aspect
without requiring the full protobuf root BUILD.bazel to be loadable.
"""

load("@protobuf//bazel/common:proto_lang_toolchain_info.bzl", "ProtoLangToolchainInfo")

def _cc_proto_toolchain_impl(ctx):
    # Get protoc executable from proto_compiler dependency
    proto_compiler = None
    if ctx.attr.proto_compiler:
        dep = ctx.attr.proto_compiler
        default_info = dep[DefaultInfo]
        if hasattr(default_info, "executable") and default_info.executable:
            proto_compiler = default_info.executable
        else:
            outputs = default_info.default_outputs
            if outputs:
                proto_compiler = outputs[0]

    # Get protoc-gen-cpp plugin executable
    plugin = None
    if ctx.attr.plugin:
        dep = ctx.attr.plugin
        default_info = dep[DefaultInfo]
        if hasattr(default_info, "executable") and default_info.executable:
            plugin = default_info.executable
        else:
            outputs = default_info.default_outputs
            if outputs:
                plugin = outputs[0]

    proto_lang_toolchain_info = ProtoLangToolchainInfo(
        out_replacement_format_flag = "--cpp_out=%s",
        output_files = "multiple",
        plugin_format_flag = "--plugin=protoc-gen-cpp=%s" if plugin else None,
        plugin = plugin,
        runtime = ctx.attr.runtime,
        provided_proto_sources = [],
        proto_compiler = proto_compiler,
        protoc_opts = [],
        progress_message = "Generating proto_library %{label}",
        mnemonic = "GenProto",
        allowlist_different_package = None,
        toolchain_type = None,
    )
    return [
        DefaultInfo(files = depset()),
        proto_lang_toolchain_info,
    ]

cc_proto_toolchain = rule(
    implementation = _cc_proto_toolchain_impl,
    attrs = {
        "runtime": attr.label(),
        "proto_compiler": attr.label(
            default = "@protobuf//src/google/protobuf/compiler:protoc_minimal",
        ),
        "plugin": attr.label(
            default = "@protobuf//src/google/protobuf/compiler/cpp:protoc-gen-cpp",
        ),
    },
    provides = [ProtoLangToolchainInfo],
)
