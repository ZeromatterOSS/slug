# Plan 28.4 Stage 9 acceptance fixture for the `ctx.var` migration.
# The Rust impl was deleted; `_kuro_var` in
# `@kuro_builtins//:exports.bzl` (sharing the substitution table
# with `_kuro_expand_make_variables` via `_kuro_make_substitutions`)
# now serves the field. The 13 builtin keys must remain present and
# string-typed; values that derive from the cfg / host are pinned
# loosely (string non-empty, or matches a known set) because exact
# values shift per host.

_BUILTIN_KEYS = [
    "BINDIR",
    "GENDIR",
    "TARGET_CPU",
    "COMPILATION_MODE",
    "WORKSPACE_ROOT",
    "CC",
    "CC_FLAGS",
    "JAVA",
    "JAVA_RUNFILES",
    "JAVABASE",
    "ABI_GLIBC_VERSION",
    "ABI",
    "STACK_FRAME_UNLIMITED",
]

def _var_proof_impl(ctx):
    v = ctx.var

    # Every builtin must be present and string-typed (matches Rust impl).
    for k in _BUILTIN_KEYS:
        if k not in v:
            fail("Plan 28.4 Stage 9: ctx.var missing builtin key %r" % k)
        val = v[k]
        if type(val) != "string":
            fail("Plan 28.4 Stage 9: ctx.var[%r] type = %r, want string" % (k, type(val)))

    # BINDIR / GENDIR must reflect the target's bin_dir path. The
    # facade reads `raw_ctx.bin_dir.path` directly, so they must
    # equal `ctx.bin_dir.path`.
    expected_bin = ctx.bin_dir.path
    if v["BINDIR"] != expected_bin:
        fail("Plan 28.4 Stage 9: BINDIR = %r, want %r" % (v["BINDIR"], expected_bin))
    if v["GENDIR"] != expected_bin:
        fail("Plan 28.4 Stage 9: GENDIR = %r, want %r" % (v["GENDIR"], expected_bin))

    # WORKSPACE_ROOT comes from `raw_ctx.label.workspace_root` which
    # for the root cell is the empty string.
    if v["WORKSPACE_ROOT"] != ctx.label.workspace_root:
        fail("Plan 28.4 Stage 9: WORKSPACE_ROOT = %r, want %r" % (
            v["WORKSPACE_ROOT"],
            ctx.label.workspace_root,
        ))

    # Pinned-string fields from the previous Rust table.
    if v["ABI_GLIBC_VERSION"] != "2.17":
        fail("Plan 28.4 Stage 9: ABI_GLIBC_VERSION = %r, want '2.17'" % v["ABI_GLIBC_VERSION"])
    if v["ABI"] != "local":
        fail("Plan 28.4 Stage 9: ABI = %r, want 'local'" % v["ABI"])
    if v["CC_FLAGS"] != "":
        fail("Plan 28.4 Stage 9: CC_FLAGS = %r, want ''" % v["CC_FLAGS"])
    if v["STACK_FRAME_UNLIMITED"] != "":
        fail("Plan 28.4 Stage 9: STACK_FRAME_UNLIMITED = %r, want ''" % v["STACK_FRAME_UNLIMITED"])

    # ctx.var must be iterable as a dict (the previous Rust impl
    # returned an actual `Dict`; the Starlark replacement also does).
    items = v.items()
    if len(items) < len(_BUILTIN_KEYS):
        fail("Plan 28.4 Stage 9: ctx.var.items() len = %d, want >= %d" % (
            len(items),
            len(_BUILTIN_KEYS),
        ))

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "var-proof-ok\n")
    return [DefaultInfo(default_output = out)]

var_proof = rule(
    implementation = _var_proof_impl,
    attrs = {},
)
