# Test rules for aspect propagation.

FilesInfo = provider(fields = ["files"])
TagInfo = provider(fields = ["tag"])
CountInfo = provider(fields = ["count"])

# ── Basic aspect: collect source paths transitively ───────────────────────────

def _files_aspect_impl(target, ctx):
    """Collect source file paths transitively via aspect shadow graph."""
    files = []
    # Collect srcs from the current rule
    for src in ctx.rule.attr.srcs:
        files.append(src.path)
    # Collect files from dependencies (shadow graph: deps already have aspect applied)
    for dep in ctx.rule.attr.deps:
        if FilesInfo in dep:
            files.extend(dep[FilesInfo].files)
    return [FilesInfo(files = files)]

files_aspect = aspect(
    implementation = _files_aspect_impl,
    attr_aspects = ["deps"],
)

def _my_lib_impl(ctx):
    return [DefaultInfo()]

my_lib = rule(
    impl = _my_lib_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(aspects = [files_aspect]),
    },
)

def _collector_impl(ctx):
    """Collect all files from deps (which have aspect results) and write to a file."""
    all_files = []
    for dep in ctx.attrs.deps:
        if FilesInfo in dep:
            all_files.extend(dep[FilesInfo].files)
    out = ctx.actions.write("files.txt", "\n".join(sorted(all_files)))
    return [DefaultInfo(default_outputs = [out])]

collector = rule(
    impl = _collector_impl,
    attrs = {
        "deps": attr.label_list(aspects = [files_aspect]),
    },
)

# ── required_providers aspect: only applies to tagged targets ─────────────────

def _count_aspect_impl(target, ctx):
    count = 1
    for dep in ctx.rule.attr.deps:
        if CountInfo in dep:
            count += dep[CountInfo].count
    return [CountInfo(count = count)]

# Only applies to targets that provide TagInfo
count_aspect = aspect(
    implementation = _count_aspect_impl,
    attr_aspects = ["deps"],
    required_providers = [[TagInfo]],
)

def _tagged_lib_impl(ctx):
    return [DefaultInfo(), TagInfo(tag = "tagged")]

tagged_lib = rule(
    impl = _tagged_lib_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(aspects = [count_aspect]),
    },
)

def _plain_lib_impl(ctx):
    return [DefaultInfo()]

plain_lib = rule(
    impl = _plain_lib_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(aspects = [count_aspect]),
    },
)

def _counter_impl(ctx):
    """Report which deps got CountInfo from the aspect."""
    lines = []
    for dep in ctx.attrs.deps:
        if CountInfo in dep:
            lines.append("tagged:" + str(dep[CountInfo].count))
        else:
            lines.append("untagged")
    out = ctx.actions.write("counts.txt", "\n".join(lines))
    return [DefaultInfo(default_outputs = [out])]

counter = rule(
    impl = _counter_impl,
    attrs = {
        "deps": attr.label_list(aspects = [count_aspect]),
    },
)

# ── ctx.rule.kind aspect ──────────────────────────────────────────────────────

KindInfo = provider(fields = ["kind"])

def _kind_aspect_impl(target, ctx):
    """Record the rule kind of each target."""
    return [KindInfo(kind = ctx.rule.kind)]

kind_aspect = aspect(
    implementation = _kind_aspect_impl,
    attr_aspects = ["deps"],
)

def _kind_reporter_impl(ctx):
    kinds = []
    for dep in ctx.attrs.deps:
        if KindInfo in dep:
            kinds.append(dep[KindInfo].kind)
    out = ctx.actions.write("kinds.txt", "\n".join(sorted(kinds)))
    return [DefaultInfo(default_outputs = [out])]

kind_reporter = rule(
    impl = _kind_reporter_impl,
    attrs = {
        "deps": attr.label_list(aspects = [kind_aspect]),
    },
)
