def _edge_rule_impl(ctx):
    return [DefaultInfo()]

edge_rule = rule(
    implementation = _edge_rule_impl,
    attrs = {"deps": attr.label_list()},
)
