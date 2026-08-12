load("//:repo.bzl", "simple_repo")

def _extension_impl(module_ctx):
    for module in module_ctx.modules:
        for tag in module.tags.repo:
            simple_repo(name = tag.name, marker = tag.name)

extension = module_extension(
    implementation = _extension_impl,
    tag_classes = {
        "repo": tag_class(attrs = {"name": attr.string(mandatory = True)}),
    },
)
