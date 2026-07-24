def _marker_repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"marker.txt\"])\n")
    ctx.file("marker.txt", ctx.attr.content + "\n")

marker_repo = repository_rule(
    implementation = _marker_repo_impl,
    attrs = {"content": attr.string(mandatory = True)},
)

def _valid_repo_impl(ctx):
    content = "{}|{}|{}|{}|{}".format(
        ctx.attr.value,
        ctx.attr.integer,
        ctx.attr.enabled,
        ",".join(ctx.attr.items),
        ",".join(["{}={}".format(key, ctx.attr.mapping[key]) for key in sorted(ctx.attr.mapping.keys())]),
    )
    ctx.file("BUILD.bazel", "exports_files([\"marker.txt\"])\n")
    ctx.file("marker.txt", content + "\n")

valid_repo = repository_rule(
    implementation = _valid_repo_impl,
    attrs = {
        "value": attr.string(mandatory = True),
        "integer": attr.int(mandatory = True),
        "enabled": attr.bool(mandatory = True),
        "items": attr.string_list(mandatory = True),
        "mapping": attr.string_dict(mandatory = True),
    },
)

string_repo = repository_rule(
    implementation = _marker_repo_impl,
    attrs = {
        "value": attr.string(mandatory = True),
        "content": attr.string(default = "innate"),
    },
)

def _list_repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"marker.txt\"])\n")
    ctx.file("marker.txt", ",".join(ctx.attr.value) + "\n")

list_repo = repository_rule(
    implementation = _list_repo_impl,
    attrs = {"value": attr.string_list(mandatory = True)},
)

valid_record = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "value": attr.string(mandatory = True),
    "integer": attr.int(mandatory = True),
    "enabled": attr.bool(mandatory = True),
    "items": attr.string_list(mandatory = True),
    "mapping": attr.string_dict(mandatory = True),
})

string_record = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "value": attr.string(mandatory = True),
})

list_record = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "value": attr.string_list(mandatory = True),
})

dict_record = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "value": attr.string_dict(mandatory = True),
})

alias_record = tag_class(attrs = {
    "name": attr.string(mandatory = True),
    "items": attr.string_list(mandatory = True),
    "mapping": attr.string_dict(mandatory = True),
})

def _valid_ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag in module.tags.valid_structural:
            marker_repo(
                name = tag.name,
                content = "{}|{}|{}|{}|{}".format(
                    tag.value,
                    tag.integer,
                    tag.enabled,
                    ",".join(tag.items),
                    ",".join(["{}={}".format(key, tag.mapping[key]) for key in sorted(tag.mapping.keys())]),
                ),
            )

def _string_ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag_class_name in [
            "float_value",
            "builtin_callable",
            "extension_proxy",
            "update_value",
        ]:
            for tag in getattr(module.tags, tag_class_name):
                marker_repo(name = tag.name, content = tag.value)

def _list_ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag_class_name in ["nested_list", "self_cycle"]:
            for tag in getattr(module.tags, tag_class_name):
                marker_repo(name = tag.name, content = ",".join(tag.value))

def _dict_ext_impl(module_ctx):
    for module in module_ctx.modules:
        for tag in module.tags.nested_dict:
            marker_repo(
                name = tag.name,
                content = ",".join(["{}={}".format(key, tag.value[key]) for key in sorted(tag.value.keys())]),
            )

def _alias_ext_impl(module_ctx):
    observed = []
    for module in module_ctx.modules:
        for tag in module.tags.alias_observation:
            observed.append("{}|{}".format(",".join(tag.items), tag.mapping["phase"]))
    marker_repo(name = "tag_alias_repo", content = ";".join(observed))

valid_ext = module_extension(
    implementation = _valid_ext_impl,
    tag_classes = {"valid_structural": valid_record},
)
string_ext = module_extension(
    implementation = _string_ext_impl,
    tag_classes = {
        "float_value": string_record,
        "builtin_callable": string_record,
        "extension_proxy": string_record,
        "update_value": string_record,
    },
)
list_ext = module_extension(
    implementation = _list_ext_impl,
    tag_classes = {
        "nested_list": list_record,
        "self_cycle": list_record,
    },
)
dict_ext = module_extension(
    implementation = _dict_ext_impl,
    tag_classes = {"nested_dict": dict_record},
)
alias_ext = module_extension(
    implementation = _alias_ext_impl,
    tag_classes = {"alias_observation": alias_record},
)
