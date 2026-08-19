def _crate_info(target):
    available = providers(target)
    keys = sorted([
        key
        for key in available
        if key.endswith("//rust/private:providers.bzl%CrateInfo")
    ])
    if len(keys) != 1:
        fail("expected exactly one rules_rust CrateInfo, got %s" % keys)
    return available[keys[0]]

def format(target):
    crate = _crate_info(target)
    deps = sorted([
        str(dep.crate_info.owner)
        for dep in crate.deps.to_list()
        if dep.crate_info != None
    ])
    data = sorted([file.short_path for file in crate.data.to_list()])
    return "label=%s owner=%s name=%s type=%s edition=%s root=%s output=%s deps=%s data=%s" % (
        target.label,
        crate.owner,
        crate.name,
        crate.type,
        crate.edition,
        crate.root.short_path,
        crate.output.short_path,
        deps,
        data,
    )
