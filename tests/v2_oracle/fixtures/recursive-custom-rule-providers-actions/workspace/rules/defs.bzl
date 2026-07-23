LeafInfo = provider(fields = {"value": "leaf target name"})
ParentInfo = provider(fields = {"value": "dependency leaf names in declaration order"})

def _leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.label.name + "\n")
    return [
        DefaultInfo(files = depset([out])),
        LeafInfo(value = ctx.label.name),
    ]

leaf_rule = rule(implementation = _leaf_impl)

def _parent_impl(ctx):
    values = [dep[LeafInfo].value for dep in ctx.attr.deps]
    out = ctx.actions.declare_file("parent.txt")
    ctx.actions.write(out, ",".join(values) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        ParentInfo(value = ",".join(values)),
    ]

parent_rule = rule(
    implementation = _parent_impl,
    attrs = {
        "deps": attr.label_list(),
    },
)
