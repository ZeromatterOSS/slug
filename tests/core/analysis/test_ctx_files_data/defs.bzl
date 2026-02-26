"""Tests for ctx.file, ctx.files, and expand_location functionality."""


def _source_lib_impl(ctx):
    """A rule that produces multiple output files."""
    out1 = ctx.actions.declare_file("lib_a.txt")
    out2 = ctx.actions.declare_file("lib_b.txt")
    ctx.actions.write(out1, "lib_a")
    ctx.actions.write(out2, "lib_b")
    return [DefaultInfo(files = depset([out1, out2]))]


source_lib = rule(
    implementation = _source_lib_impl,
    attrs = {},
)


def _single_file_rule_impl(ctx):
    """A rule that produces a single output file."""
    out = ctx.actions.declare_file("single.txt")
    ctx.actions.write(out, "single_content")
    return [DefaultInfo(default_output = out)]


single_file_rule = rule(
    implementation = _single_file_rule_impl,
    attrs = {},
)


def _collect_from_deps_impl(ctx):
    """Collects files from deps using ctx.files.deps and writes their names."""
    out = ctx.actions.declare_file("collected.txt")
    names = [f.basename for f in ctx.files.deps]
    ctx.actions.write(out, "\n".join(sorted(names)))
    return [DefaultInfo(default_output = out)]


collect_from_deps = rule(
    implementation = _collect_from_deps_impl,
    attrs = {
        "deps": attr.label_list(allow_files = True, default = []),
    },
)


def _single_file_from_dep_impl(ctx):
    """Accesses a single file from dep using ctx.file.dep."""
    out = ctx.actions.declare_file("single_from_dep.txt")
    f = ctx.file.dep
    ctx.actions.write(out, f.basename)
    return [DefaultInfo(default_output = out)]


single_file_from_dep = rule(
    implementation = _single_file_from_dep_impl,
    attrs = {
        "dep": attr.label(allow_single_file = True),
    },
)


def _expand_location_rule_impl(ctx):
    """Tests ctx.expand_location() for $(location :target) expansion."""
    out = ctx.actions.declare_file("expanded.txt")

    # Expand location template
    template = "$(location :single_dep)"
    expanded = ctx.expand_location(template, targets = [ctx.attr.dep])
    ctx.actions.write(out, expanded)
    return [DefaultInfo(default_output = out)]


expand_location_rule = rule(
    implementation = _expand_location_rule_impl,
    attrs = {
        "dep": attr.label(allow_single_file = True),
    },
)
