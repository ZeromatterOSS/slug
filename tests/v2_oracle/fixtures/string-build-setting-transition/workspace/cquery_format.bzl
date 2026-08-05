def format(target):
    available = providers(target)
    consumer_key = "//:defs.bzl%ConsumerInfo"
    parent_key = "//:defs.bzl%ParentInfo"
    if consumer_key in available:
        return "label=%s provider=ConsumerInfo value=%s" % (
            target.label,
            available[consumer_key].value,
        )
    return "label=%s provider=ParentInfo value=%s" % (
        target.label,
        available[parent_key].value,
    )
