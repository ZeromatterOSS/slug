# Plan 28.4 Stage 9 acceptance fixture for the
# `ctx.expand_make_variables` migration. The Rust impl was deleted;
# the bundled `_slug_expand_make_variables` (sharing
# `_slug_make_substitutions` with `_slug_var`) now serves the call.
#
# Pins:
#   - User-provided substitutions win over builtins.
#   - Builtins resolve when not overridden.
#   - Unresolved `$(VAR)` patterns are left in place verbatim.
#   - Unbalanced `$(` (no closing `)`) is left in place verbatim
#     and the scan continues.
#   - Multiple substitutions in one string all expand.
#   - Whitespace inside `$(...)` is stripped (Rust used `.trim()`).

def _expand_make_variables_proof_impl(ctx):
    # User subs win over builtins: override BINDIR.
    user_wins = ctx.expand_make_variables(
        "cmd",
        "$(BINDIR)",
        {"BINDIR": "user-override"},
    )
    if user_wins != "user-override":
        fail("Plan 28.4 Stage 9: user_wins = %r, want 'user-override'" % user_wins)

    # Builtin resolves with no user subs.
    abi = ctx.expand_make_variables("cmd", "abi=$(ABI)", {})
    if abi != "abi=local":
        fail("Plan 28.4 Stage 9: ABI builtin: got %r, want 'abi=local'" % abi)

    # Unresolved $(VAR) survives verbatim.
    unresolved = ctx.expand_make_variables("cmd", "before $(NOT_DEFINED) after", {})
    if unresolved != "before $(NOT_DEFINED) after":
        fail("Plan 28.4 Stage 9: unresolved: got %r" % unresolved)

    # Unbalanced $( survives verbatim.
    unbalanced = ctx.expand_make_variables("cmd", "x $( y", {})
    if unbalanced != "x $( y":
        fail("Plan 28.4 Stage 9: unbalanced: got %r" % unbalanced)

    # Multiple substitutions in one string.
    multi = ctx.expand_make_variables(
        "cmd",
        "$(A)/$(B)/$(C)",
        {"A": "1", "B": "2", "C": "3"},
    )
    if multi != "1/2/3":
        fail("Plan 28.4 Stage 9: multi: got %r, want '1/2/3'" % multi)

    # Whitespace inside $(...) is stripped (Rust used `.trim()`).
    spaced = ctx.expand_make_variables("cmd", "$( SPACED )", {"SPACED": "ok"})
    if spaced != "ok":
        fail("Plan 28.4 Stage 9: spaced: got %r, want 'ok'" % spaced)

    # No-op string with no `$(` survives unchanged.
    plain = ctx.expand_make_variables("cmd", "no markers here", {})
    if plain != "no markers here":
        fail("Plan 28.4 Stage 9: plain: got %r" % plain)

    # Empty substitutions dict OR None both work.
    empty = ctx.expand_make_variables("cmd", "$(ABI)", None)
    if empty != "local":
        fail("Plan 28.4 Stage 9: None subs: got %r, want 'local'" % empty)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "expand-make-variables-proof-ok\n")
    return [DefaultInfo(default_output = out)]

expand_make_variables_proof = rule(
    implementation = _expand_make_variables_proof_impl,
    attrs = {},
)
