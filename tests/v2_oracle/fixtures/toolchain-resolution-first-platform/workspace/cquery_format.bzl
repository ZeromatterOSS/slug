def format(target):
    provider_key = "//:defs.bzl%ProbeInfo"
    return "label=%s provider=ProbeInfo marker=%s" % (
        target.label,
        providers(target)[provider_key].marker,
    )
