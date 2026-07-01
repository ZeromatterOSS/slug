def _repo_impl(ctx):
    filename = ctx.attr.message + ".txt"
    ctx.file("BUILD.bazel", "exports_files([\"{}\"])\n".format(filename))
    ctx.file(filename, ctx.attr.message + "\n")

tagged_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"message": attr.string(mandatory = True)},
)

repo_tag = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "message": attr.string(mandatory = True),
})

def _ext_impl(module_ctx):
    print("SLUG_STAGE5_EXTENSION_EXECUTED")
    for module in module_ctx.modules:
        for tag in module.tags.repo:
            tagged_repo(name = tag.name, message = tag.message)

ext = module_extension(
    implementation = _ext_impl,
    tag_classes = {"repo": repo_tag},
)