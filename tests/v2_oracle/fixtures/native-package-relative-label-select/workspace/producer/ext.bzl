"""Generate BUILD files that invoke the owner module's macros."""

def _repo_impl(ctx):
    macro = ctx.attr.macro
    ctx.file("BUILD.bazel", "load(\"@@producer+//:suite.bzl\", \"%s\")\n%s()\n" % (macro, macro))

_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"macro": attr.string(mandatory = True)},
)

def _ext_impl(ctx):
    _repo(name = "hub", macro = "probe")
    _repo(name = "bad", macro = "bad")

ext = module_extension(implementation = _ext_impl)
