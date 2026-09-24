"""Local module extensions for Bazel's dynamic and raw environment reads."""

def _repo_impl(rctx):
    rctx.file("BUILD.bazel", "filegroup(name = %r)\n" % rctx.attr.marker)

_probe = repository_rule(
    implementation = _repo_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)

def _dynamic_impl(ctx):
    value = ctx.getenv("SLUG_WP800_DYNAMIC")
    defaulted = ctx.getenv("SLUG_WP800_DYNAMIC", "fallback")
    first = "none" if value == None else ("empty" if value == "" else value)
    second = "empty" if defaulted == "" else defaulted
    _probe(name = "dynamic_repo", marker = "dyn_" + first + "_" + second)

def _raw_impl(ctx):
    value = ctx.os.environ.get("SLUG_WP800_RAW", "fallback")
    _probe(name = "raw_repo", marker = "raw_" + value)

dynamic = module_extension(
    implementation = _dynamic_impl,
    environ = ["SLUG_WP800_DECLARED"],
)
raw = module_extension(implementation = _raw_impl)
