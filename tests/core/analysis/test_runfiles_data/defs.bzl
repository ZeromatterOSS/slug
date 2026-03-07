def _leaf_impl(ctx):
    src = ctx.file.src
    return [
        DefaultInfo(
            files = depset([src]),
            runfiles = ctx.runfiles(files = [src]),
        ),
    ]


leaf = rule(
    implementation = _leaf_impl,
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
    implementation = _collector_impl,
    attrs = {
        "deps": attr.label_list(),
        "runtime_deps": attr.label_list(),
        "data": attr.label_list(),
        "files": attr.label_list(allow_files = True),
    },
)


def _runfiles_merge_impl(ctx):
    """Tests runfiles.merge() combining two runfiles objects."""
    rf1 = ctx.runfiles(files = ctx.files.srcs1)
    rf2 = ctx.runfiles(files = ctx.files.srcs2)
    merged = rf1.merge(rf2)

    basenames = sorted([f.basename for f in merged.files.to_list()])
    out = ctx.actions.write("merged.txt", "\n".join(basenames))
    return [DefaultInfo(default_output = out)]


runfiles_merge_rule = rule(
    implementation = _runfiles_merge_impl,
    attrs = {
        "srcs1": attr.label_list(allow_files = True),
        "srcs2": attr.label_list(allow_files = True),
    },
)


def _runfiles_merge_all_impl(ctx):
    """Tests runfiles.merge_all() combining a list of runfiles objects."""
    rf_list = [ctx.runfiles(files = [f]) for f in ctx.files.srcs]
    base = ctx.runfiles()
    merged = base.merge_all(rf_list)

    basenames = sorted([f.basename for f in merged.files.to_list()])
    out = ctx.actions.write("merged_all.txt", "\n".join(basenames))
    return [DefaultInfo(default_output = out)]


runfiles_merge_all_rule = rule(
    implementation = _runfiles_merge_all_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _runfiles_transitive_impl(ctx):
    """Tests ctx.runfiles(transitive_files=depset([...])) includes files."""
    files_depset = depset(ctx.files.srcs)
    rf = ctx.runfiles(transitive_files = files_depset)

    basenames = sorted([f.basename for f in rf.files.to_list()])
    out = ctx.actions.write("transitive.txt", "\n".join(basenames))
    return [DefaultInfo(default_output = out)]


runfiles_transitive_rule = rule(
    implementation = _runfiles_transitive_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _runfiles_files_attr_impl(ctx):
    """Tests that runfiles.files returns a depset."""
    rf = ctx.runfiles(files = ctx.files.srcs)

    # runfiles.files is a depset
    files_list = rf.files.to_list()
    basenames = sorted([f.basename for f in files_list])
    out = ctx.actions.write("files_attr.txt", "\n".join(basenames))
    return [DefaultInfo(default_output = out)]


runfiles_files_attr_rule = rule(
    implementation = _runfiles_files_attr_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)
