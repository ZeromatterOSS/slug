# Authored input rule; compilation-context production and merging use complete,
# unchanged upstream public cc_common and private cc_info implementations.
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

HeaderProof = provider(fields = ["context", "alias", "received", "received_alias"])

def _files(targets):
    return [f for target in targets for f in target[DefaultInfo].files.to_list()]

def _impl(ctx):
    received = None
    received_alias = None
    if ctx.attr.forwarded:
        proof = ctx.attr.forwarded[HeaderProof]
        received = proof.context
        received_alias = proof.alias
        if received._header_info != received_alias:
            fail("HeaderInfo alias changed across configured dependency publication")
        if {received_alias: "same"}[received._header_info] != "same":
            fail("HeaderInfo identity/hash changed across publication")
        context = cc_common.create_compilation_context(
            exported_dependent_cc_compilation_contexts = [received],
        )
    else:
        public = _files(ctx.attr.public)
        private = _files(ctx.attr.private)
        textual = _files(ctx.attr.textual)
        context = cc_common.create_compilation_context(
            headers = depset(public),
            direct_public_headers = public + public,
            direct_private_headers = private + private,
            direct_textual_headers = textual + textual,
            separate_module_headers = _files(ctx.attr.separate) * 2,
            dependent_cc_compilation_contexts = [dep[HeaderProof].context for dep in ctx.attr.ordinary],
            exported_dependent_cc_compilation_contexts = [dep[HeaderProof].context for dep in ctx.attr.exported],
        )
    return [HeaderProof(context = context, alias = context._header_info,
        received = received, received_alias = received_alias)]

subject = rule(implementation = _impl, attrs = {
    "public": attr.label_list(allow_files = True),
    "private": attr.label_list(allow_files = True),
    "textual": attr.label_list(allow_files = True),
    "separate": attr.label_list(allow_files = True),
    "ordinary": attr.label_list(), "exported": attr.label_list(),
    "forwarded": attr.label(),
})
