# Plan 28.4 Stage 10 acceptance fixture for the `ctx.new_file` migration.
# The Rust impl was deleted; the bundled `_kuro_new_file` in
# `@kuro_builtins//:exports.bzl` now serves both call shapes by
# delegating to `ctx.actions.declare_file(name)`.
#
# Two call shapes are exercised:
#   1. `ctx.new_file(filename)` — the common single-arg form.
#   2. `ctx.new_file(sibling, filename)` — the sibling form; the sibling
#      is ignored (matching the deleted Rust impl byte-for-byte).
#
# We verify that both returned Files have the expected basename, then
# write a sentinel "new-file-proof-ok\n" to the declared output so the
# Python test can confirm the build succeeded.

def _new_file_proof_impl(ctx):
    # Shape 1: ctx.new_file(filename)
    f1 = ctx.new_file("foo.txt")
    if f1.basename != "foo.txt":
        fail("Plan 28.4 Stage 10: new_file('foo.txt').basename = %r, want 'foo.txt'" % f1.basename)
    ctx.actions.write(f1, "f1\n")

    # Shape 2: ctx.new_file(sibling, filename) — sibling is ignored.
    f2 = ctx.new_file(f1, "bar.txt")
    if f2.basename != "bar.txt":
        fail("Plan 28.4 Stage 10: new_file(sibling, 'bar.txt').basename = %r, want 'bar.txt'" % f2.basename)
    ctx.actions.write(f2, "f2\n")

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "new-file-proof-ok\n")
    return [DefaultInfo(default_output = out)]

new_file_proof = rule(
    implementation = _new_file_proof_impl,
    attrs = {},
)
