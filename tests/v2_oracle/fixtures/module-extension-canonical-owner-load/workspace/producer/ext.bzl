"""Generate BUILD files with a direct canonical load of the owner module."""

def _repo_impl(ctx):
    ctx.file(
        "BUILD.bazel",
        "load(\"@@producer+//:%s.bzl\", \"define\")\ndefine(\"loaded\")\n" % ctx.attr.source,
    )

_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"source": attr.string(mandatory = True)},
)

def _ext_impl(ctx):
    _repo(name = "hub", source = "suite")
    _repo(name = "broken", source = "missing")

ext = module_extension(implementation = _ext_impl)
