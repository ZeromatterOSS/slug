def emit():
    native.filegroup(
        name = "macro_%s_%s" % (
            native.module_name(),
            native.module_version().replace(".", "_") or "empty",
        ),
    )
