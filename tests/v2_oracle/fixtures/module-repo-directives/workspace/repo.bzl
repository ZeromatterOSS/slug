def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"{}\"])\n".format(ctx.attr.filename))
    ctx.file(ctx.attr.filename, ctx.name + "\n")

simple_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"filename": attr.string(mandatory = True)},
)
