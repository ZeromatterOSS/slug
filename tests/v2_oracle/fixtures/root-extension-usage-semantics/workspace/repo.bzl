def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"marker.txt\"])\n")
    ctx.file("marker.txt", ctx.attr.marker + "\n")

simple_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"marker": attr.string(mandatory = True)},
)
