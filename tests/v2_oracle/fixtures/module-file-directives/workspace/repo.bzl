def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"{}\"])\n".format(ctx.attr.filename))
    ctx.file(ctx.attr.filename, ctx.attr.message + "\n")

simple_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {
        "filename": attr.string(mandatory = True),
        "message": attr.string(mandatory = True),
    },
)