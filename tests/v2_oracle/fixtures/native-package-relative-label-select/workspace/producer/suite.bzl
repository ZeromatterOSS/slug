"""Macros invoked by generated nonroot BUILD files."""

def probe():
    relative = native.package_relative_label(":marker.txt")
    bare = native.package_relative_label("marker.txt")
    same_repo = native.package_relative_label("//:marker.txt")
    defining = Label(":marker.txt")
    apparent = native.package_relative_label("@producer//:marker.txt")
    canonical = native.package_relative_label("@@producer+//:marker.txt")
    preserved = native.package_relative_label(defining)
    print("WP802 relative=%s bare=%s same_repo=%s defining=%s apparent=%s canonical=%s preserved=%s preserved_type=%s" % (
        relative,
        bare,
        same_repo,
        defining,
        apparent,
        canonical,
        preserved,
        type(preserved),
    ))

    native.config_setting(name = "condition", values = {"compilation_mode": "dbg"})
    native.filegroup(name = "value")
    native.filegroup(name = "fallback")
    native.filegroup(
        name = "selected",
        srcs = select({
            native.package_relative_label(":condition"): [":value"],
            "//conditions:default": [":fallback"],
        }),
    )

def bad():
    native.package_relative_label(7)
