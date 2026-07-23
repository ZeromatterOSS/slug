def _edge_impl(ctx):
    return []

edge = rule(
    implementation = _edge_impl,
    attrs = {"deps": attr.label_list()},
)
