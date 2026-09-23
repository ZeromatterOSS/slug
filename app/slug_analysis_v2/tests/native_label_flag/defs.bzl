# Authored consumers; native label_flag supplies forwarding, never a shim.
Proof = provider()
def _leaf(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value, is_executable = True)
    return [DefaultInfo(files = depset([out]), executable = out), Proof(value = ctx.attr.value)]
leaf = rule(implementation = _leaf, attrs = {"value": attr.string()}, executable = True)
def _probe(ctx):
    dep = ctx.attr.dep
    return [Proof(value = dep[Proof].value, label = dep.label, file = ctx.file.dep, files = ctx.files.dep)]
probe = rule(implementation = _probe, attrs = {"dep": attr.label(allow_single_file = True)})
def _select(ctx):
    return [Proof(value = ctx.attr.value)]
select_probe = rule(implementation = _select, attrs = {"value": attr.string()})
def _tool(ctx):
    return [Proof(file = ctx.executable.dep, label = ctx.attr.dep.label)]
tool_probe = rule(implementation = _tool, attrs = {"dep": attr.label(executable = True, cfg = "exec")})
