def format(target):
    available = providers(target)
    default_files = sorted([
        file.short_path
        for file in available["DefaultInfo"].files.to_list()
    ])
    parent_key = "//rules:defs.bzl%ParentInfo"
    leaf_key = "//rules:defs.bzl%LeafInfo"
    provider_key = parent_key if parent_key in available else leaf_key
    return "label=%s files=%s provider=%s value=%s" % (
        target.label,
        ",".join(default_files),
        provider_key,
        available[provider_key].value,
    )
