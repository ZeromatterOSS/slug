P = provider()
Q = provider()

def _p(ctx):
    return [P()]

def _q(ctx):
    return [Q()]

def _pq(ctx):
    return [P(), Q()]

def _advertised_only(ctx):
    return [DefaultInfo()]

def _absent(ctx):
    return [DefaultInfo()]

def _consumer(ctx):
    return [DefaultInfo()]

def _generated(ctx):
    ctx.actions.write(ctx.outputs.out, "generated")
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

p_rule = rule(implementation = _p)
q_rule = rule(implementation = _q)
pq_rule = rule(implementation = _pq)
advertised_only = rule(implementation = _advertised_only, provides = [P])
absent = rule(implementation = _absent)
generated = rule(
    implementation = _generated,
    attrs = {"out": attr.output(mandatory = True)},
)
consumer = rule(
    implementation = _consumer,
    attrs = {
        "scalar": attr.label(providers = [[P], [Q]], allow_files = True),
        "sequence": attr.label_list(providers = [[P], [Q]], allow_files = True),
        "mapped": attr.string_keyed_label_dict(providers = [[P], [Q]], allow_files = True),
        "reverse": attr.label_keyed_string_dict(providers = [[P], [Q]], allow_files = True),
        "grouped": attr.label_list_dict(providers = [[P], [Q]], allow_files = True),
    },
)
flat_consumer = rule(
    implementation = _consumer,
    attrs = {"dep": attr.label(providers = [P])},
)
builtin_consumer = rule(
    implementation = _consumer,
    attrs = {"dep": attr.label(providers = [DefaultInfo])},
)
empty_consumer = rule(
    implementation = _consumer,
    attrs = {
        "outer": attr.label(providers = []),
        "conjunction": attr.label(providers = [[]]),
    },
)
conjunction_consumer = rule(
    implementation = _consumer,
    attrs = {"dep": attr.label(providers = [[P, Q]])},
)
