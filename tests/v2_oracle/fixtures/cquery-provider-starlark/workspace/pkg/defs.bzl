ThingInfo = provider(fields = ["message"])

def _thing_impl(ctx):
    return [ThingInfo(message = "configured provider")]

thing = rule(implementation = _thing_impl)