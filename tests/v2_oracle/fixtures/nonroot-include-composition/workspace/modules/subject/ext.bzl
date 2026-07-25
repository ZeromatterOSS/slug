def _marker_repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"marker.txt\"])\n")
    ctx.file("marker.txt", "|".join(ctx.attr.values) + "\n")

marker_repo = repository_rule(
    implementation = _marker_repo_impl,
    attrs = {"values": attr.string_list(mandatory = True)},
)

tag = tag_class(attrs = {"value": attr.string(mandatory = True)})

def _markers_impl(module_ctx):
    values = []
    for module in module_ctx.modules:
        for tag_instance in module.tags.tag:
            values.append(tag_instance.value)
    marker_repo(name = "include_marker", values = values)

markers = module_extension(
    implementation = _markers_impl,
    tag_classes = {"tag": tag},
)
