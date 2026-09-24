def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files(['marker.txt'])\n")
    ctx.file("marker.txt", "same repository call\n")

_probe = repository_rule(implementation = _repo_impl)

def emit(ctx):
    _probe(name = "probe")
