def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"tag.txt\"])\n")
    ctx.file("tag.txt", ctx.attr.message + "\n")

tagged_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"message": attr.string(mandatory = True)},
)

repo_tag = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "message": attr.string(mandatory = True),
})

def _ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag in module.tags.repo:
            tagged_repo(name = tag.name, message = tag.message)

ext = module_extension(
    implementation = _ext_impl,
    tag_classes = {"repo": repo_tag},
)
