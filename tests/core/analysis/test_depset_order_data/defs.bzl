def _depset_order_impl(ctx):
    order = ctx.attr.order
    a = depset(["a"], order = order)
    b = depset(["b"], order = order)
    c = depset(["c"], transitive = [a, b], order = order)
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = "\n".join([str(x) for x in c.to_list()]))
    return [DefaultInfo(files = depset([out]))]


def _depset_default_infer_impl(ctx):
    a = depset(["a"], order = "preorder")
    b = depset(["b"], order = "preorder")
    c = depset(["c"], transitive = [a, b])
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = "\n".join([str(x) for x in c.to_list()]))
    return [DefaultInfo(files = depset([out]))]


def _depset_mismatch_impl(ctx):
    a = depset(["a"], order = "preorder")
    b = depset(["b"], order = "postorder")
    depset(["c"], transitive = [a, b], order = "preorder")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = "unused")
    return [DefaultInfo(files = depset([out]))]


depset_order_rule = rule(
    implementation = _depset_order_impl,
    attrs = {
        "order": attr.string(default = "default"),
    },
)

depset_default_infer_rule = rule(
    implementation = _depset_default_infer_impl,
    attrs = {},
)

depset_mismatch_rule = rule(
    implementation = _depset_mismatch_impl,
    attrs = {},
)
