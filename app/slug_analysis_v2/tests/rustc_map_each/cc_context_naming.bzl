# Authored consumer of the unchanged public wrapper and complete toolchain.
load("//cc/common:cc_common.bzl", "cc_common")
load("//cc/private:cc_internal.bzl", "cc_internal")
load("//cc/private/rules_impl:cc_toolchain_info.bzl", "CcToolchainInfo")

Proof = provider(fields = ["toolchain", "snapshot", "shape"])
Shape = provider(fields = ["value"])

def _impl(ctx):
    recovered = cc_internal.actions2ctx_cheat(ctx.actions)
    if recovered != ctx or recovered != cc_internal.actions2ctx_cheat(ctx.actions):
        fail("actions did not recover original ctx")
    tc = recovered.attr.toolchain[CcToolchainInfo]
    names = [cc_common.get_artifact_name_for_category(cc_toolchain = tc, category = category, output_name = "dir/./discard/../file")
             for category in ["OBJECT_FILE", "PIC_OBJECT_FILE", "INCLUDED_FILE_LIST", "STATIC_LIBRARY"]]
    return [Proof(toolchain = tc, snapshot = struct(names = names, label = str(recovered.label)), shape = Shape(value = "same"))]

subject = rule(implementation = _impl, attrs = {"toolchain": attr.label(mandatory = True)})
