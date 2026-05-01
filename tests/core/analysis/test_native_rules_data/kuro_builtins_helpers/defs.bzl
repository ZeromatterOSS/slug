# Plan 28.6 PR 2 acceptance fixture: import helper modules from the
# bundled `@kuro_builtins//` cell and exercise a few methods. Proves
# the migration in PR 2 made these modules loadable from a real BUILD
# file using the new load namespace, without `@prelude//`.

load("@kuro_builtins//:paths.bzl", "paths")
load("@kuro_builtins//utils:expect.bzl", "expect")
load("@kuro_builtins//utils:type_defs.bzl", "is_list", "is_string")
load("@kuro_builtins//utils:utils.bzl", "flatten", "value_or")

_BASENAME = paths.basename("/foo/bar/baz.txt")
_DIRNAME = paths.dirname("/foo/bar/baz.txt")
_JOIN = paths.join("a", "b", "c")
_FLAT = flatten([[1, 2], [3]])
_VOR = value_or(None, "fallback")

# Module-eval-time assertions (fail loudly if a helper regressed).
expect(_BASENAME == "baz.txt", "paths.basename: %r" % _BASENAME)
expect(_DIRNAME == "/foo/bar", "paths.dirname: %r" % _DIRNAME)
expect(_JOIN == "a/b/c", "paths.join: %r" % _JOIN)
expect(_FLAT == [1, 2, 3], "flatten: %r" % _FLAT)
expect(_VOR == "fallback", "value_or: %r" % _VOR)
expect(is_string("x"), "is_string('x') must be True")
expect(is_list([1]), "is_list([1]) must be True")
expect(not is_string([]), "is_string([]) must be False")

def _kuro_builtins_helpers_proof_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "kuro-builtins-helpers-proof-ok\n")
    return [DefaultInfo(default_output = out)]

kuro_builtins_helpers_proof = rule(
    implementation = _kuro_builtins_helpers_proof_impl,
    attrs = {},
)
