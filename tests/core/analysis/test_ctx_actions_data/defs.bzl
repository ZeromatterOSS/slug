"""Rules testing ctx.actions methods comprehensively."""

# === Test ctx.actions.write with is_executable ===
def _write_exec_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/bash\necho executable_output", is_executable = True)
    return [DefaultInfo(files = depset([out]))]

write_exec_rule = rule(
    implementation = _write_exec_impl,
    attrs = {},
)

# === Test ctx.actions.expand_template ===
def _expand_template_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.expand_template(
        template = ctx.file.template,
        output = out,
        substitutions = {
            "{NAME}": ctx.attr.name_val,
            "{PROJECT}": "kuro",
            "{VERSION}": "9.0",
        },
    )
    return [DefaultInfo(files = depset([out]))]

expand_template_rule = rule(
    implementation = _expand_template_impl,
    attrs = {
        "template": attr.label(allow_single_file = True),
        "name_val": attr.string(default = "world"),
    },
)

# === Test ctx.actions.run_shell with string command ===
def _run_shell_string_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        command = "echo shell_string_output > " + out.path,
    )
    return [DefaultInfo(files = depset([out]))]

run_shell_string_rule = rule(
    implementation = _run_shell_string_impl,
    attrs = {},
)

# === Test ctx.actions.run_shell with inputs ===
def _run_shell_inputs_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    srcs = ctx.files.srcs
    # Concatenate all input files
    cmd = "cat"
    for src in srcs:
        cmd += " " + src.path
    cmd += " > " + out.path
    ctx.actions.run_shell(
        outputs = [out],
        inputs = srcs,
        command = cmd,
    )
    return [DefaultInfo(files = depset([out]))]

run_shell_inputs_rule = rule(
    implementation = _run_shell_inputs_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
    },
)

# === Test ctx.actions.declare_directory ===
def _declare_dir_impl(ctx):
    out_dir = ctx.actions.declare_directory(ctx.label.name + "_dir")
    marker = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out_dir, marker],
        command = "mkdir -p " + out_dir.path + " && echo file1 > " + out_dir.path + "/a.txt && echo file2 > " + out_dir.path + "/b.txt && echo ok > " + marker.path,
    )
    return [DefaultInfo(files = depset([marker]))]

declare_dir_rule = rule(
    implementation = _declare_dir_impl,
    attrs = {},
)

# === Test ctx.actions.declare_file Bazel-shaped paths ===
def _declare_file_path_shape_impl(ctx):
    selected = ctx.actions.declare_file("build/c.s")
    marker = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(selected, "selected")
    ctx.actions.write(marker, selected.path)
    return [DefaultInfo(files = depset([marker]))]

declare_file_path_shape_rule = rule(
    implementation = _declare_file_path_shape_impl,
    attrs = {},
)

# === Test ctx.actions.declare_directory Bazel-shaped paths ===
def _declare_directory_path_shape_impl(ctx):
    selected = ctx.actions.declare_directory("build/tree")
    marker = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [selected, marker],
        command = "mkdir -p {tree} && echo ok > {tree}/marker.txt && printf '%s' {tree} > {marker}".format(
            marker = marker.path,
            tree = selected.path,
        ),
    )
    return [DefaultInfo(files = depset([marker]))]

declare_directory_path_shape_rule = rule(
    implementation = _declare_directory_path_shape_impl,
    attrs = {},
)

# === Test ctx.actions.args() with add, add_all, add_joined ===
def _args_test_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    args = ctx.actions.args()
    args.add("--flag")
    args.add("--name", ctx.attr.name_val)
    args.add_all("--items", ctx.attr.items)
    args.add_joined("--joined", ctx.attr.items, join_with = ",")

    # Write the args to a file via run_shell
    # We'll use printf to output each arg on a line
    ctx.actions.run_shell(
        outputs = [out],
        command = "echo " + " ".join(["--flag", "--name", ctx.attr.name_val] + ["--items"] + list(ctx.attr.items) + ["--joined", ",".join(ctx.attr.items)]) + " > " + out.path,
    )
    return [DefaultInfo(files = depset([out]))]

args_test_rule = rule(
    implementation = _args_test_impl,
    attrs = {
        "name_val": attr.string(default = "test"),
        "items": attr.string_list(default = []),
    },
)

# === Test ctx.actions.write with multiple lines ===
def _write_multiline_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    content = "\n".join(ctx.attr.lines)
    ctx.actions.write(out, content)
    return [DefaultInfo(files = depset([out]))]

write_multiline_rule = rule(
    implementation = _write_multiline_impl,
    attrs = {
        "lines": attr.string_list(default = []),
    },
)

# === Test ctx.label attributes ===
def _label_info_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    lines = []
    lines.append("name=" + ctx.label.name)
    lines.append("package=" + ctx.label.package)
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(files = depset([out]))]

label_info_rule = rule(
    implementation = _label_info_impl,
    attrs = {},
)

# === Test ctx.bin_dir ===
def _bin_dir_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    bin_path = ctx.bin_dir.path
    ctx.actions.write(out, bin_path)
    return [DefaultInfo(files = depset([out]))]

bin_dir_rule = rule(
    implementation = _bin_dir_impl,
    attrs = {},
)

# === Test ctx.runfiles ===
def _runfiles_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    rf = ctx.runfiles(files = ctx.files.data)
    lines = []
    lines.append("count=" + str(len(ctx.files.data)))
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(files = depset([out]), runfiles = rf)]

runfiles_rule = rule(
    implementation = _runfiles_impl,
    attrs = {
        "data": attr.label_list(allow_files = True),
    },
)

# === Test provider() and DefaultInfo ===
MyInfo = provider(fields = ["val", "count"])

def _provider_test_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value)
    return [
        DefaultInfo(files = depset([out])),
        MyInfo(val = ctx.attr.value, count = 42),
    ]

provider_test_rule = rule(
    implementation = _provider_test_impl,
    attrs = {
        "value": attr.string(default = "provider_value"),
    },
)

def _provider_consumer_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    dep = ctx.attr.dep
    info = dep[MyInfo]
    lines = []
    lines.append("val=" + info.val)
    lines.append("count=" + str(info.count))
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(files = depset([out]))]

provider_consumer_rule = rule(
    implementation = _provider_consumer_impl,
    attrs = {
        "dep": attr.label(providers = [MyInfo]),
    },
)

# === Test ctx.actions.run_shell with env parameter ===
def _run_shell_env_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        command = 'echo "${MY_VAR}|${OTHER_VAR}" > ' + out.path,
        env = {
            "MY_VAR": "hello_env",
            "OTHER_VAR": "world_env",
        },
    )
    return [DefaultInfo(files = depset([out]))]

run_shell_env_rule = rule(
    implementation = _run_shell_env_impl,
    attrs = {},
)

# === Test actions.run with progress_message ===
def _progress_message_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        command = "echo progress_works > " + out.path,
        mnemonic = "TestProgress",
        progress_message = "Testing progress message for %{output}",
    )
    return [DefaultInfo(files = depset([out]))]

progress_message_rule = rule(
    implementation = _progress_message_impl,
    attrs = {},
)
