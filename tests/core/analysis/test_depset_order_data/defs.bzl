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


# === Cross-rule depset traversal tests ===
# Tests that depsets can be passed through providers and traversed in other rules.
# This exercises the critical fix for FrozenLiveDepset.direct/transitive attributes.

ItemsInfo = provider(fields = ["items"])


def _depset_producer_impl(ctx):
    a = depset(["item_a1", "item_a2"])
    b = depset(["item_b1"], transitive = [a])
    c = depset(["item_c1"], transitive = [b])
    return [
        DefaultInfo(),
        ItemsInfo(items = c),
    ]


depset_producer = rule(
    implementation = _depset_producer_impl,
    attrs = {},
)


def _depset_consumer_impl(ctx):
    # Traverse the depset passed through a provider from another rule
    dep = ctx.attr.dep
    info = dep[ItemsInfo]
    items = info.items.to_list()
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = "\n".join(sorted(items)))
    return [DefaultInfo(files = depset([out]))]


depset_consumer = rule(
    implementation = _depset_consumer_impl,
    attrs = {
        "dep": attr.label(),
    },
)
