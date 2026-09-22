# Authored inputs only; complete upstream initializer/public compile bodies run
# unchanged. Tool/FDO inputs remain the explicit inert constructor fixture.
load("//cc/common:cc_common.bzl", "cc_common")
load("//cc/common:cc_info.bzl", "CcInfo")
load("//cc/private/compile:cc_compilation_helper.bzl", "cc_compilation_helper")
load("//cc/private/rules_impl:cc_toolchain_info.bzl", "CcToolchainInfo")

Proof = provider(fields = ["context", "implementation", "info", "alias", "snapshot", "rows"])

def _files(targets):
    return [f for target in targets for f in target[DefaultInfo].files.to_list()]

def _impl(ctx):
    if ctx.attr.rows:
        rows = [dep[Proof] for dep in ctx.attr.rows]
        for row in rows:
            if row.context._header_info != row.alias:
                fail("HeaderInfo alias changed across publication")
            if row.info.compilation_context != row.context:
                fail("full CcInfo context changed across publication")
        return [Proof(rows = rows)]
    if ctx.attr.leaf:
        context = cc_common.create_compilation_context(
            headers = depset(_files(ctx.attr.public)),
            direct_public_headers = _files(ctx.attr.public),
            direct_private_headers = _files(ctx.attr.private),
            includes = depset([ctx.attr.leaf + "-include"]),
            quote_includes = depset([ctx.attr.leaf + "-quote"]),
            system_includes = depset([ctx.attr.leaf + "-system"]),
            defines = depset([ctx.attr.leaf + "-define"]),
            local_defines = depset([ctx.attr.leaf + "-local"]),
        )
        return [Proof(context = context, info = CcInfo(compilation_context = context), alias = context._header_info)]
    tc = ctx.attr.toolchain[CcToolchainInfo]
    features = tc._toolchain_features.configure_features(ctx.attr.requested)
    deps = [dep[Proof].context for dep in ctx.attr.deps]
    implementation_deps = [dep[Proof].context for dep in ctx.attr.implementation_deps]
    public = _files(ctx.attr.public)
    private = _files(ctx.attr.private)
    textual = _files(ctx.attr.textual)
    if ctx.attr.public_compile:
        context, outputs = cc_common.compile(
            actions = ctx.actions, feature_configuration = features, cc_toolchain = tc,
            name = ctx.label.name, srcs = _files(ctx.attr.sources),
            public_hdrs = public, private_hdrs = private, textual_hdrs = textual,
            includes = ["own-include", "own-include"], quote_includes = ["own-quote"],
            system_includes = ["own-system"], framework_includes = ["own-framework"],
            defines = ["own-define"], local_defines = ["own-local"],
            compilation_contexts = deps, implementation_compilation_contexts = implementation_deps,
            purpose = "cc_library-compile",
        )
        # The whole outputs provider contains a generic temps closure. Inspect
        # it here; do not expand callable-publication admission for this proof.
        output_observation = (outputs.objects, outputs.pic_objects, outputs.temps().to_list())
        implementation = None
    else:
        context, implementation = cc_compilation_helper.init_cc_compilation_context(
            ctx = ctx, binfiles_dir = ctx.bin_dir.path, genfiles_dir = ctx.genfiles_dir.path,
            label = ctx.label, config = ctx.configuration,
            quote_include_dirs = ["own-quote"], framework_include_dirs = ["own-framework"],
            system_include_dirs = ["own-system"], include_dirs = ["own-include", "own-include"],
            feature_configuration = features, public_headers_artifacts = public,
            include_prefix = "", strip_include_prefix = "", non_module_map_headers = [],
            cc_toolchain_compilation_context = tc._cc_info.compilation_context,
            defines = ["own-define"], local_defines = ["own-local"],
            public_textual_headers = textual, private_headers_artifacts = private,
            additional_inputs = [], separate_module_headers = [], generate_module_map = False,
            generate_pic_action = False, generate_no_pic_action = True,
            module_map = None, additional_exported_headers = [], deps = deps,
            implementation_deps = implementation_deps, additional_cpp_module_maps = [],
        )
        output_observation = None
    return [Proof(context = context, implementation = implementation,
        info = CcInfo(compilation_context = context), alias = context._header_info,
        snapshot = struct(root = ctx.bin_dir, owner = ctx.label, outputs = output_observation))]

subject = rule(implementation = _impl, fragments = ["cpp"], attrs = {
    "leaf": attr.string(), "rows": attr.label_list(),
    "toolchain": attr.label(), "requested": attr.string_list(),
    "public": attr.label_list(allow_files = True), "private": attr.label_list(allow_files = True),
    "textual": attr.label_list(allow_files = True), "sources": attr.label_list(allow_files = True),
    "deps": attr.label_list(), "implementation_deps": attr.label_list(),
    "public_compile": attr.bool(),
})

def _exec_impl(ctx):
    return [ctx.attr.dep[Proof]]

exec_subject = rule(implementation = _exec_impl, attrs = {"dep": attr.label(cfg = "exec")})
