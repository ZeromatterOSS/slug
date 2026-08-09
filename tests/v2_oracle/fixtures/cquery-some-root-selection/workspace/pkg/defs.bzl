def _impl(ctx):
    return [DefaultInfo(files = depset([]))]

probe = rule(implementation = _impl)
