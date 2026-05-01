# Plan 28.4 Stage 14 acceptance fixture for the `ctx.runfiles`
# migration. The Rust impl was deleted; `_kuro_runfiles` (bound as
# `_runfiles_bound` in `_make_rule_facade`) in
# `@kuro_builtins//:exports.bzl` now serves the call via two
# kuro_runtime globals: `kuro_create_runfiles` and
# `kuro_collect_runfiles_into`.
#
# Fixture layout:
#   - `runfiles_leaf`: rule that declares a file and returns it in
#     both DefaultInfo.default_outputs and default_runfiles via
#     `ctx.runfiles(files=[out])`.
#   - `runfiles_proof`: receives a single `runfiles_leaf` dep, then
#     exercises four `ctx.runfiles` call shapes:
#       1. `ctx.runfiles()` — no args; returns a Runfiles value.
#       2. `ctx.runfiles(files=[f])` — explicit files list.
#       3. `ctx.runfiles(transitive_files=depset([f]))` — transitive.
#       4. `ctx.runfiles(files=[f], collect_default=True)` — collects
#          from deps.
#     All four must return a non-None Runfiles. Sentinel: "runfiles-proof-ok\n".

def _runfiles_leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "leaf\n")
    rf = ctx.runfiles(files = [out])
    return [DefaultInfo(default_outputs = [out], default_runfiles = rf)]

runfiles_leaf = rule(
    implementation = _runfiles_leaf_impl,
    attrs = {},
)

def _runfiles_proof_impl(ctx):
    f = ctx.actions.declare_file(ctx.label.name + "_src.txt")
    ctx.actions.write(f, "src\n")

    # Shape 1: no args.
    rf0 = ctx.runfiles()
    if rf0 == None:
        fail("Plan 28.4 Stage 14: ctx.runfiles() returned None")

    # Shape 2: explicit files list.
    rf1 = ctx.runfiles(files = [f])
    if rf1 == None:
        fail("Plan 28.4 Stage 14: ctx.runfiles(files=[f]) returned None")

    # Shape 3: transitive_files depset.
    rf2 = ctx.runfiles(transitive_files = depset([f]))
    if rf2 == None:
        fail("Plan 28.4 Stage 14: ctx.runfiles(transitive_files=...) returned None")

    # Shape 4: collect_default collects from deps.
    rf3 = ctx.runfiles(files = [f], collect_default = True)
    if rf3 == None:
        fail("Plan 28.4 Stage 14: ctx.runfiles(collect_default=True) returned None")

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "runfiles-proof-ok\n")
    return [DefaultInfo(default_output = out)]

runfiles_proof = rule(
    implementation = _runfiles_proof_impl,
    attrs = {
        "deps": attr.label_list(),
    },
)
