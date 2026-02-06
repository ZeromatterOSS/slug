def _leaf_impl(ctx):
    src = ctx.file.src
    return [
        DefaultInfo(
            files = depset([src]),
            runfiles = ctx.runfiles(files = [src]),
        ),
    ]


leaf = rule(
    impl = _leaf_impl,
    attrs = {
        "src": attr.label(allow_single_file = True),
    },
)


def _collector_impl(ctx):
    runfiles = ctx.runfiles(
        files = ctx.files.files,
        collect_default = True,
        collect_data = True,
    )

    basenames = [f.basename for f in runfiles.files.to_list()]
    basenames = sorted(basenames)
    out = ctx.actions.write("runfiles.txt", "\n".join(basenames))
    return [DefaultInfo(default_output = out)]


collector = rule(
    impl = _collector_impl,
    attrs = {
        "deps": attr.label_list(),
        "runtime_deps": attr.label_list(),
        "data": attr.label_list(),
        "files": attr.label_list(allow_files = True),
    },
)
