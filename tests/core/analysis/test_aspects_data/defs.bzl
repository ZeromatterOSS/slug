# Test rules for aspect propagation.

FilesInfo = provider(fields = ["files"])
TagInfo = provider(fields = ["tag"])
CountInfo = provider(fields = ["count"])
BaseInfo = provider(fields = ["name"])
ShadowInfo = provider(fields = ["names"])

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


# ── ctx.label in aspect and ctx.rule.attr with string/int attrs ──────────────

LabelInfo = provider(fields = ["label_str"])
StringAttrInfo = provider(fields = ["value", "count"])


def _label_aspect_impl(target, ctx):
    """Record the canonical label of each visited target."""
    return [LabelInfo(label_str = str(ctx.label))]


label_aspect = aspect(
    implementation = _label_aspect_impl,
    attr_aspects = ["deps"],
)


def _attr_aspect_impl(target, ctx):
    """Record string/int attrs from the visited rule."""
    value = ctx.rule.attr.value if hasattr(ctx.rule.attr, "value") else "N/A"
    count = ctx.rule.attr.count if hasattr(ctx.rule.attr, "count") else -1
    return [StringAttrInfo(value = str(value), count = count)]


attr_value_aspect = aspect(
    implementation = _attr_aspect_impl,
    attr_aspects = ["deps"],
)


def _configurable_lib_impl(ctx):
    return [DefaultInfo()]


configurable_lib = rule(
    impl = _configurable_lib_impl,
    attrs = {
        "value": attr.string(default = "default_value"),
        "count": attr.int(default = 42),
        "deps": attr.label_list(aspects = [label_aspect, attr_value_aspect]),
    },
)


def _label_reporter_impl(ctx):
    """Report label strings collected by label_aspect."""
    labels = []
    for dep in ctx.attrs.deps:
        if LabelInfo in dep:
            labels.append(dep[LabelInfo].label_str)
    out = ctx.actions.write("labels.txt", "\n".join(sorted(labels)))
    return [DefaultInfo(default_outputs = [out])]


label_reporter = rule(
    impl = _label_reporter_impl,
    attrs = {
        "deps": attr.label_list(aspects = [label_aspect]),
    },
)


def _attr_reporter_impl(ctx):
    """Report string/int attr values collected by attr_value_aspect."""
    lines = []
    for dep in ctx.attrs.deps:
        if StringAttrInfo in dep:
            lines.append("{}:{}".format(dep[StringAttrInfo].value, dep[StringAttrInfo].count))
    out = ctx.actions.write("attr_values.txt", "\n".join(sorted(lines)))
    return [DefaultInfo(default_outputs = [out])]


attr_reporter = rule(
    impl = _attr_reporter_impl,
    attrs = {
        "deps": attr.label_list(aspects = [attr_value_aspect]),
    },
)

# ── aspect provider overlay keeps base providers ─────────────────────────────

def _shadow_aspect_impl(target, ctx):
    names = []
    if BaseInfo in target:
        names.append(target[BaseInfo].name)
    for dep in ctx.rule.attr.deps:
        if ShadowInfo in dep:
            names.extend(dep[ShadowInfo].names)
    return [ShadowInfo(names = names)]

shadow_aspect = aspect(
    implementation = _shadow_aspect_impl,
    attr_aspects = ["deps"],
)

def _base_lib_impl(ctx):
    return [DefaultInfo(), BaseInfo(name = ctx.label.name)]

base_lib = rule(
    impl = _base_lib_impl,
    attrs = {
        "deps": attr.label_list(
            aspects = [shadow_aspect],
            providers = [BaseInfo],
        ),
    },
)

def _shadow_reporter_impl(ctx):
    names = []
    for dep in ctx.attrs.deps:
        if ShadowInfo in dep:
            names.extend(dep[ShadowInfo].names)
    out = ctx.actions.write("shadow.txt", "\n".join(sorted(names)))
    return [DefaultInfo(default_outputs = [out])]

shadow_reporter = rule(
    impl = _shadow_reporter_impl,
    attrs = {
        "deps": attr.label_list(aspects = [shadow_aspect]),
    },
)
