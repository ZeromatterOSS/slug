# Plan 28.4 Stage 6 acceptance fixture for the
# `ctx.package_relative_label` migration. The Rust impl was deleted;
# the bundled `_kuro_package_relative_label` in
# `@kuro_builtins//:exports.bzl` now serves the method via a
# closure on the rule facade.
#
# Three input shapes are exercised, mirroring the branches in the
# previous Rust impl:
#
#   1. A bare relative target (no leading `:`) → `@<cell>//<pkg>:<target>`.
#   2. A `:target` form → same as above.
#   3. A `//pkg:target` form (absolute within current cell) →
#      `@<cell>//pkg:target`.
#   4. A `@cell//...` form (already fully qualified) → passes
#      through unchanged.
#
# We compare the canonical strings on the returned `Label` values
# (Label stores them in `@@<cell>//<pkg>:<target>` form per
# `BazelLabel::parse`).

def _package_relative_label_proof_impl(ctx):
    # The canonical Label string elides the cell name for the root
    # workspace (see `BazelLabel::parse` — "@@//pkg:tgt" instead of
    # "@@native_rules_test//pkg:tgt"). Match the actual form by
    # round-tripping through `Label()` ourselves to derive expected
    # strings, rather than hardcoding cell names.
    pkg = ctx.label.package

    bare = ctx.package_relative_label("bare_target")
    expected_bare = str(Label("//" + pkg + ":bare_target"))
    if str(bare) != expected_bare:
        fail("Plan 28.4 Stage 6: bare-target form: got %r, want %r" % (str(bare), expected_bare))

    colon = ctx.package_relative_label(":colon_target")
    expected_colon = str(Label("//" + pkg + ":colon_target"))
    if str(colon) != expected_colon:
        fail("Plan 28.4 Stage 6: :target form: got %r, want %r" % (str(colon), expected_colon))

    abs_pkg = ctx.package_relative_label("//other:abs_target")
    expected_abs = str(Label("//other:abs_target"))
    if str(abs_pkg) != expected_abs:
        fail("Plan 28.4 Stage 6: //pkg:target form: got %r, want %r" % (str(abs_pkg), expected_abs))

    # Fully-qualified labels pass through to `Label()` unchanged. The
    # facade can't predict the canonical form for an arbitrary cell
    # (Label's canonicalisation lives in BazelLabel::parse), but the
    # workspace component must survive intact.
    qualified = ctx.package_relative_label("@some_other_cell//pkg:tgt")
    if qualified.workspace_name != "some_other_cell":
        fail("Plan 28.4 Stage 6: @cell form lost workspace: got %r" % qualified.workspace_name)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "package-relative-label-proof-ok\n")
    return [DefaultInfo(default_output = out)]

package_relative_label_proof = rule(
    implementation = _package_relative_label_proof_impl,
    attrs = {},
)
