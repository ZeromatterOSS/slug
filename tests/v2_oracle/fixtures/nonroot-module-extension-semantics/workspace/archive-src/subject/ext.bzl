def _marker_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"{}\"])\n".format(ctx.attr.filename))
    ctx.file(ctx.attr.filename, ctx.attr.message + "\n")

marker_repo = repository_rule(
    implementation = _marker_impl,
    attrs = {
        "filename": attr.string(mandatory = True),
        "message": attr.string(mandatory = True),
    },
)

marker = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "filename": attr.string(mandatory = True),
    "message": attr.string(mandatory = True),
})

def _ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag in module.tags.marker:
            marker_repo(name = tag.name, filename = tag.filename, message = tag.message)

ext = module_extension(
    implementation = _ext_impl,
    tag_classes = {"marker": marker},
)
