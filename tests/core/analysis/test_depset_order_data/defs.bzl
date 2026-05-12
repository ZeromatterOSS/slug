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


def _depset_topological_diamond_impl(ctx):
    a = depset(["a"], order = "topological")
    b = depset(["b"], transitive = [a], order = "topological")
    c = depset(["c"], transitive = [a], order = "topological")
    d = depset(["d"], transitive = [b, c], order = "topological")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = "\n".join([str(x) for x in d.to_list()]))
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

depset_topological_diamond_rule = rule(
    implementation = _depset_topological_diamond_impl,
    attrs = {},
)

depset_mismatch_rule = rule(
    implementation = _depset_mismatch_impl,
    attrs = {},
)


# === Cross-rule depset traversal tests ===
# Tests that depsets can be passed through providers and traversed in other rules.
# This exercises frozen depset traversal without public .direct/.transitive attributes.

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


# === depset(direct=...) keyword form ===

def _depset_keyword_impl(ctx):
    """Tests depset() with keyword 'direct=' and 'transitive=' arguments."""
    a = depset(direct = ["x", "y"])
    b = depset(direct = ["z"], transitive = [a])
    out = ctx.actions.declare_file("keyword_depset.txt")
    ctx.actions.write(out, "\n".join(b.to_list()))
    return [DefaultInfo(default_output = out)]


depset_keyword_rule = rule(
    implementation = _depset_keyword_impl,
    attrs = {},
)


# === depset | operator (union) ===

def _depset_union_impl(ctx):
    """Bazel 9 rejects the old Slug prototype depset | depset operator."""
    a = depset(["x", "y"])
    b = depset(["z"])
    c = a | b
    out = ctx.actions.declare_file("union_depset.txt")
    ctx.actions.write(out, "\n".join(sorted(c.to_list())))
    return [DefaultInfo(default_output = out)]


depset_union_rule = rule(
    implementation = _depset_union_impl,
    attrs = {},
)


# === depset .order attribute ===

def _depset_order_attr_impl(ctx):
    """Bazel 9 rejects public depset.order attribute access."""
    a = depset(["x"], order = "preorder")
    b = depset(["y"])  # default order
    out = ctx.actions.declare_file("order_attr.txt")
    ctx.actions.write(out, a.order + "\n" + b.order)
    return [DefaultInfo(default_output = out)]


depset_order_attr_rule = rule(
    implementation = _depset_order_attr_impl,
    attrs = {},
)


# === len(depset) ===

def _depset_len_impl(ctx):
    """Bazel 9 rejects len(depset)."""
    a = depset(["x", "y"])
    b = depset(["z"], transitive = [a])
    out = ctx.actions.declare_file("len_depset.txt")
    ctx.actions.write(out, str(len(b)))
    return [DefaultInfo(default_output = out)]


depset_len_rule = rule(
    implementation = _depset_len_impl,
    attrs = {},
)


# === depset <-> transitive_set bridge ===

def _depset_bridge_impl(ctx):
    """Tests Slug's internal depset/transitive_set bridge shape and roundtrip."""
    leaf = depset(["a", "b"])
    parent = depset(["c", "d"], transitive = [leaf])
    tset = native.transitive_set_from_depset(parent, actions = ctx.actions)

    node_sizes = [str(len(node)) for node in tset.traverse(ordering = "preorder")]
    roundtrip_default = native.depset_from_transitive_set(tset).to_list()
    roundtrip_preorder = native.depset_from_transitive_set(tset, order = "preorder").to_list()

    out = ctx.actions.declare_file("depset_bridge.txt")
    ctx.actions.write(
        out,
        "\n".join([
            "nodes=" + ",".join(node_sizes),
            "default=" + ",".join(roundtrip_default),
            "preorder=" + ",".join(roundtrip_preorder),
        ]),
    )
    return [DefaultInfo(default_output = out)]


depset_bridge_rule = rule(
    implementation = _depset_bridge_impl,
    attrs = {},
)
