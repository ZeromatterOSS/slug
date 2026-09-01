SettingInfo = provider(fields = {"value": "configured string value"})
ToolInfo = provider(fields = {"name": "tool target name", "platform": "selected target-platform marker", "setting": "target-scoped setting value"})
def _setting(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, attrs = {"scope": attr.string()}, build_setting = config.string(flag = True))
def _tool(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/sh\n", is_executable = True)
    return [DefaultInfo(executable = out), ToolInfo(
        name = ctx.label.name,
        platform = ctx.attr.platform,
        setting = ctx.attr._switch[SettingInfo].value,
    )]
tool = rule(implementation = _tool, executable = True, attrs = {
    "platform": attr.string(),
    "_switch": attr.label(default = "//:switch"),
})
def _tool_names(values):
    return [value[ToolInfo].name for value in values]
def _probe(ctx):
    scalar = ctx.attr.scalar[ToolInfo]
    target_tool = ctx.attr.target_tool[ToolInfo]
    if scalar.name != "chosen":
        fail("scalar select was not resolved before the exec transition")
    if scalar.setting != "default" or scalar.platform != "exec":
        fail("exec dependency did not remove target scope or select exec platform: %s/%s" % (scalar.setting, scalar.platform))
    if target_tool.setting != "selected" or target_tool.platform != "target":
        fail("explicit target executable changed configuration: %s/%s" % (target_tool.setting, target_tool.platform))
    if _tool_names(ctx.attr.vector) != ["chosen", "other"]:
        fail("label_list order changed")
    if [ctx.attr.string_keyed[key][ToolInfo].name for key in ["first", "second"]] != ["chosen", "other"]:
        fail("string_keyed_label_dict orientation changed")
    if [value[ToolInfo].name + ":" + text for value, text in ctx.attr.label_keyed.items()] != ["chosen:first", "other:second"]:
        fail("label_keyed_string_dict orientation changed")
    if _tool_names(ctx.attr.list_dict["group"]) != ["other", "chosen"]:
        fail("label_list_dict orientation changed")
    if type(ctx.executable.scalar) != "File" or ctx.executable.scalar.basename != "chosen.sh":
        fail("exec executable projection changed")
    if type(ctx.executable.target_tool) != "File" or ctx.executable.target_tool.basename != "other.sh":
        fail("target executable projection changed")
    if ctx.executable.optional_tool != None or ctx.attr.optional_tool != None:
        fail("omitted optional executable did not remain None")
    if hasattr(ctx.executable, "ordinary"):
        fail("nonexecutable attribute leaked into ctx.executable")
    if [file.basename for file in ctx.files.source] != ["data.txt"]:
        fail("direct source dependency did not materialize")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "exec-configured-label-attributes\n")
    return [DefaultInfo(files = depset([out]))]

probe = rule(implementation = _probe, attrs = {
    "scalar": attr.label(cfg = "exec", executable = True),
    "target_tool": attr.label(cfg = "target", executable = True),
    "optional_tool": attr.label(cfg = "exec", executable = True),
    "ordinary": attr.label(),
    "vector": attr.label_list(cfg = "exec"),
    "string_keyed": attr.string_keyed_label_dict(cfg = "exec"),
    "label_keyed": attr.label_keyed_string_dict(cfg = "exec"),
    "list_dict": attr.label_list_dict(cfg = "exec"),
    "source": attr.label(cfg = "exec", allow_files = True),
})
