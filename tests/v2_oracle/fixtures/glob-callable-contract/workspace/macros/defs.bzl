def declare_macro_glob_targets():
    for path in native.glob(("macro/*.txt",)):
        native.filegroup(
            name = "macro_" + path.replace("/", "_"),
            srcs = [path],
        )
