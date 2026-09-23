# Authored File/runfiles origins and observers. Every forwarding group is native.
Extra = provider(fields = ["value"])
Proof = provider(fields = ["target", "files", "file", "executable", "exec_file", "exec_executable", "instrumented", "nested"])

def _check_instrumented(info, other):
    if type(info) != "InstrumentedFilesInfo":
        fail("native instrumentation type changed")
    if dir(info) != ["instrumented_files", "metadata_files"]:
        fail("native instrumentation fields changed")
    if not hasattr(info, "instrumented_files") or hasattr(info, "unknown"):
        fail("native instrumentation reflection changed")
    if repr(info) != "struct(instrumented_files = depset([]), metadata_files = depset([]))":
        fail("native instrumentation repr changed")
    if type(info.instrumented_files) != "depset" or type(info.metadata_files) != "depset":
        fail("native instrumentation fields are not depsets")
    if info.instrumented_files.to_list() != [] or info.metadata_files.to_list() != []:
        fail("disabled coverage contains instrumentation")
    lookup = {info: "first"}
    lookup[other] = "second"
    if info != other or len(lookup) != 1 or lookup[info] != "second":
        fail("native empty Info equality/hash changed")

def _origin(ctx):
    direct = [f for dep in ctx.attr.srcs for f in dep[DefaultInfo].files.to_list()]
    if ctx.attr.generated:
        out = ctx.actions.declare_file(ctx.label.name + ".txt")
        ctx.actions.write(out, ctx.attr.content)
        direct.append(out)
    files = depset(direct, transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps], order = ctx.attr.order)
    default_files = [f for dep in ctx.attr.default_files for f in dep[DefaultInfo].files.to_list()]
    data_files = [f for dep in ctx.attr.data_files for f in dep[DefaultInfo].files.to_list()]
    default = ctx.runfiles(files = default_files,
        symlinks = {"logical-default": default_files[0]} if default_files else {},
        root_symlinks = {"root-default": default_files[-1]} if default_files else {})
    data = ctx.runfiles(files = data_files,
        symlinks = {"logical-data": data_files[0]} if data_files else {},
        root_symlinks = {"root-data": data_files[-1]} if data_files else {})
    return [DefaultInfo(files = files, default_runfiles = default, data_runfiles = data),
        Extra(value = ctx.attr.content), OutputGroupInfo(_validation = depset(ctx.files.validation))]

origin = rule(implementation = _origin, attrs = {
    "srcs": attr.label_list(allow_files = True), "deps": attr.label_list(),
    "default_files": attr.label_list(allow_files = True), "data_files": attr.label_list(allow_files = True),
    "validation": attr.label_list(allow_files = True), "generated": attr.bool(),
    "content": attr.string(default = "A"), "order": attr.string(default = "default"),
})

def _predeclared(ctx):
    ctx.actions.write(ctx.outputs.out, "testonly producer")
    return [DefaultInfo(files = depset([ctx.outputs.out]))]
predeclared = rule(implementation = _predeclared, attrs = {"out": attr.output(mandatory = True)})

def _observe(ctx):
    dep = ctx.attr.dep
    if Extra in dep:
        fail("native filegroup forwarded arbitrary child provider")
    if ctx.file.single != ctx.executable.tool or ctx.file.exec_single != ctx.executable.exec_tool:
        fail("singleton executable differs from File projection")
    info = dep[InstrumentedFilesInfo]
    _check_instrumented(info, ctx.attr.single[InstrumentedFilesInfo])
    return [info, Proof(target = dep, files = ctx.files.dep, file = ctx.file.single,
        executable = ctx.executable.tool, exec_file = ctx.file.exec_single, exec_executable = ctx.executable.exec_tool,
        instrumented = info, nested = struct(info = info, values = [info], keys = {info: "kept"}))]

observe = rule(implementation = _observe, attrs = {
    "dep": attr.label(), "single": attr.label(allow_single_file = True),
    "tool": attr.label(executable = True, cfg = "target"),
    "exec_single": attr.label(allow_single_file = True, cfg = "exec"),
    "exec_tool": attr.label(executable = True, cfg = "exec"),
})

def _consume(ctx):
    proof = ctx.attr.dep[Proof]
    if proof.file != proof.executable or proof.exec_file != proof.exec_executable:
        fail("File identity changed after configured provider publication")
    if proof.files != proof.target[DefaultInfo].files.to_list():
        fail("captured files changed after publication")
    info = ctx.attr.dep[InstrumentedFilesInfo]
    for nested in [proof.instrumented, proof.nested.info, proof.nested.values[0], proof.nested.keys.keys()[0]]:
        _check_instrumented(info, nested)
    if proof.nested.keys[info] != "kept":
        fail("native provider dictionary key changed after publication")
    return [info, proof]
consume = rule(implementation = _consume, attrs = {"dep": attr.label()})

def _macro(name, visibility):
    origin(name = name + "_private", srcs = ["a.txt"], visibility = ["//visibility:private"])
    native.filegroup(name = name, srcs = [name + "_private"], visibility = visibility)
private_macro = macro(implementation = _macro)
