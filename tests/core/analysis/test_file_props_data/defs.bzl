"""Test rules for File object property tests."""


def _file_props_impl(ctx):
    """Writes properties of source and generated files to an output file."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []

    # Check source file properties
    src = ctx.file.src
    lines.append("src.basename=" + src.basename)
    lines.append("src.extension=" + src.extension)
    lines.append("src.dirname=" + src.dirname)
    lines.append("src.short_path=" + src.short_path)
    lines.append("src.is_source=" + str(src.is_source))
    lines.append("src.is_directory=" + str(src.is_directory))

    # Check path ends correctly
    lines.append("src.path.endswith_basename=" + str(src.path.endswith(src.basename)))

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


file_props_rule = rule(
    implementation = _file_props_impl,
    attrs = {
        "src": attr.label(allow_single_file = True),
    },
)


def _generated_file_props_impl(ctx):
    """Writes properties of a generated file to output."""
    # First generate a file
    gen_out = ctx.actions.declare_file("generated_for_props.txt")
    ctx.actions.write(gen_out, "generated_content\n")

    # Now inspect its properties
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    lines.append("gen.basename=" + gen_out.basename)
    lines.append("gen.extension=" + gen_out.extension)
    lines.append("gen.short_path=" + gen_out.short_path)
    lines.append("gen.is_source=" + str(gen_out.is_source))
    lines.append("gen.is_directory=" + str(gen_out.is_directory))
    # root.path should be the buck-out prefix
    lines.append("gen.root.path.contains_buck_out=" + str("buck-out" in gen_out.root.path))
    # short_path should NOT contain buck-out
    lines.append("gen.short_path.no_buck_out=" + str("buck-out" not in gen_out.short_path))
    # path = root.path + "/" + short_path
    expected_path = gen_out.root.path + "/" + gen_out.short_path
    lines.append("gen.path.equals_root_plus_short=" + str(gen_out.path == expected_path))

    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


generated_file_props_rule = rule(
    implementation = _generated_file_props_impl,
    attrs = {},
)


def _file_dirname_impl(ctx):
    """Tests file dirname for files in subdirectories."""
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    for src in ctx.files.srcs:
        lines.append(src.basename + ":" + src.dirname)
    ctx.actions.write(out, "\n".join(sorted(lines)) + "\n")
    return [DefaultInfo(default_output = out)]


file_dirname_rule = rule(
    implementation = _file_dirname_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)


def _dir_artifact_impl(ctx):
    """Tests directory artifact properties."""
    out_dir = ctx.actions.declare_directory(ctx.label.name + "_dir")
    ctx.actions.run_shell(
        outputs = [out_dir],
        command = "mkdir -p {} && echo 'file' > {}/content.txt".format(
            out_dir.path, out_dir.path
        ),
    )

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    lines.append("dir.basename=" + out_dir.basename)
    lines.append("dir.is_directory=" + str(out_dir.is_directory))
    lines.append("dir.is_source=" + str(out_dir.is_source))
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(default_output = out)]


dir_artifact_rule = rule(
    implementation = _dir_artifact_impl,
    attrs = {},
)
