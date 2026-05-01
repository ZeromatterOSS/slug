# Plan 28.4 Stage 7 acceptance fixture for the `ctx.tokenize`
# migration. The Rust impl + `shell_tokenize` helper were deleted;
# `_kuro_tokenize` in `@kuro_builtins//:exports.bzl` now serves the
# call.
#
# The pre-existing `tokenize_test` in
# `tests/core/analysis/test_native_rules_data/defs.bzl` already
# covers the basic cases (unquoted, single-quoted, double-quoted,
# empty, multi-whitespace). This fixture pins the edge cases the
# Rust impl handled that the basic test does not exercise:
#
#   - Backslash escape outside quotes.
#   - All four escapable chars inside double quotes (`"`, `\`, `$`,
#     `` ` ``).
#   - Non-escapable char after backslash inside double quotes:
#     keep literal `\`, do not consume next char.
#   - Trailing `\` at end of input (outside and inside double
#     quotes): silently dropped.
#   - Multiple ASCII whitespace forms as separators (space, tab,
#     newline, CR, form feed).

def _check(label, actual, expected):
    if actual != expected:
        fail("Plan 28.4 Stage 7 (%s): got %r, want %r" % (label, actual, expected))

def _tokenize_proof_impl(ctx):
    # Backslash escape outside quotes: `\X` becomes literal `X`,
    # joining what would otherwise be split tokens.
    _check("backslash-escapes-space", ctx.tokenize("foo\\ bar baz"), ["foo bar", "baz"])
    _check("backslash-escapes-quote", ctx.tokenize("a\\\"b"), ["a\"b"])

    # Trailing `\` outside quotes is dropped.
    _check("trailing-backslash", ctx.tokenize("hello\\"), ["hello"])

    # Double-quoted: each escapable char survives unchanged.
    _check("dq-escape-quote", ctx.tokenize("\"a\\\"b\""), ["a\"b"])
    _check("dq-escape-backslash", ctx.tokenize("\"a\\\\b\""), ["a\\b"])
    _check("dq-escape-dollar", ctx.tokenize("\"a\\$b\""), ["a$b"])
    _check("dq-escape-backtick", ctx.tokenize("\"a\\`b\""), ["a`b"])

    # Double-quoted: backslash before non-escapable char keeps the
    # literal `\` and does not consume the next char (Rust impl
    # quirk we are preserving).
    _check("dq-non-escapable", ctx.tokenize("\"a\\nb\""), ["a\\nb"])

    # Trailing `\` inside double-quoted string: dropped silently.
    _check("dq-trailing-backslash", ctx.tokenize("\"hello\\"), ["hello"])

    # Whitespace mix: tab, newline, CR all split tokens.
    _check("tab-separator", ctx.tokenize("a\tb"), ["a", "b"])
    _check("newline-separator", ctx.tokenize("a\nb"), ["a", "b"])
    _check("cr-separator", ctx.tokenize("a\rb"), ["a", "b"])
    _check("formfeed-separator", ctx.tokenize("a\x0cb"), ["a", "b"])

    # Mixed: quoted strings concatenate with adjacent unquoted text
    # because no whitespace separates them.
    _check("adjacent-quoted", ctx.tokenize("foo'bar baz'qux"), ["foobar bazqux"])

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "tokenize-proof-ok\n")
    return [DefaultInfo(default_output = out)]

tokenize_proof = rule(
    implementation = _tokenize_proof_impl,
    attrs = {},
)
