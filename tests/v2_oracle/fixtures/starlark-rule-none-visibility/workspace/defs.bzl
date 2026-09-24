def _impl(ctx):
    return []

_rule = rule(implementation = _impl)

def wrapped_rule(name, visibility = None):
    _rule(name = name, visibility = visibility)
