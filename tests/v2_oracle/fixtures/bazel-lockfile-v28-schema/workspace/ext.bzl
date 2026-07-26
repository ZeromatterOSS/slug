def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"value.txt\"])\n")
    ctx.file("value.txt", ctx.attr.message + "\n")

typed_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {
        "bool_value": attr.bool(),
        "dict_value": attr.string_dict(),
        "int_value": attr.int(),
        "label_value": attr.label(),
        "list_value": attr.string_list(),
        "message": attr.string(),
    },
)

config = tag_class(attrs = {"message": attr.string()})

def _schema_impl(ctx):
    print("LOCKFILE_SCHEMA_EXTENSION_EVALUATED")
    env_value = ctx.getenv("LOCKFILE_SCHEMA_ENV")
    file_value = ctx.read(Label("//:input.txt"), watch = "yes").strip()
    dir_entries = ctx.path(Label("//:input_dir/entry.txt")).dirname.readdir(watch = "yes")
    mapped_label = Label("@subject//:probe")
    tagged = ctx.modules[0].tags.config[0].message

    typed_repo(
        name = "alpha",
        bool_value = True,
        dict_value = {"z": "last", "a": "first"},
        int_value = 17,
        label_value = mapped_label,
        list_value = ["z", "a"],
        message = "%s|%s|%s|%s" % (tagged, env_value, file_value, dir_entries[0].basename),
    )
    typed_repo(
        name = "beta",
        bool_value = False,
        int_value = -3,
        label_value = Label("//:input.txt"),
        message = "second",
    )
    return ctx.extension_metadata(
        root_module_direct_deps = ["alpha", "beta"],
        root_module_direct_dev_deps = [],
        facts = {
            "z": {"nested": [{"b": 2, "a": 1}, True, None]},
            "a": "first",
        },
    )

schema = module_extension(
    implementation = _schema_impl,
    tag_classes = {"config": config},
    environ = ["LOCKFILE_SCHEMA_ENV"],
    os_dependent = True,
    arch_dependent = True,
    facts_version = 7,
)
