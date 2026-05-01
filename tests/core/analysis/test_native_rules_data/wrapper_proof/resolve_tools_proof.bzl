# Plan 28.4 Stage 11 acceptance fixture for the `ctx.resolve_tools`
# migration. The Rust impl was deleted; `_kuro_resolve_tools` in
# `@kuro_builtins//:exports.bzl` now serves the call by iterating
# `tools`, collecting each dep's `DefaultInfo.default_outputs`, and
# returning `(files_list, [])`.
#
# Fixture layout:
#   - `resolve_tools_leaf`: produces two output files via DefaultInfo.
#   - `resolve_tools_proof`: receives a single `resolve_tools_leaf`
#     dep via `tools`, calls `ctx.resolve_tools(tools=ctx.attr.tools)`,
#     and asserts:
#       1. The first element is a list.
#       2. The second element is an empty list (manifests).
#       3. The file count equals the number of outputs from the leaf.
#     Sentinel output: `"resolve-tools-proof-ok\n"`.

def _resolve_tools_leaf_impl(ctx):
    out_a = ctx.actions.declare_file(ctx.label.name + "_a.txt")
    out_b = ctx.actions.declare_file(ctx.label.name + "_b.txt")
    ctx.actions.write(out_a, "leaf-a\n")
    ctx.actions.write(out_b, "leaf-b\n")
    return [DefaultInfo(default_outputs = [out_a, out_b])]

resolve_tools_leaf = rule(
    implementation = _resolve_tools_leaf_impl,
    attrs = {},
)

def _resolve_tools_proof_impl(ctx):
    result = ctx.resolve_tools(tools = ctx.attr.tools)

    if type(result) != "tuple":
        fail("Plan 28.4 Stage 11: resolve_tools returned %r, want tuple" % type(result))
    if len(result) != 2:
        fail("Plan 28.4 Stage 11: resolve_tools tuple len = %d, want 2" % len(result))

    files = result[0]
    manifests = result[1]

    if type(files) != "list":
        fail("Plan 28.4 Stage 11: resolve_tools files type = %r, want list" % type(files))
    if type(manifests) != "list":
        fail("Plan 28.4 Stage 11: resolve_tools manifests type = %r, want list" % type(manifests))
    if len(manifests) != 0:
        fail("Plan 28.4 Stage 11: resolve_tools manifests len = %d, want 0" % len(manifests))

    # One dep with two outputs → two files.
    if len(files) != 2:
        fail("Plan 28.4 Stage 11: resolve_tools files len = %d, want 2" % len(files))

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "resolve-tools-proof-ok\n")
    return [DefaultInfo(default_output = out)]

resolve_tools_proof = rule(
    implementation = _resolve_tools_proof_impl,
    attrs = {
        "tools": attr.label_list(),
    },
)
