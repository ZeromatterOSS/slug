"""Platform configuration helpers."""

def platform_specific_copts():
    """Returns platform-specific compiler flags."""
    return select({
        "@platforms//os:windows": ["/W4"],
        "@platforms//os:linux": ["-Wall", "-Wextra"],
        "@platforms//os:macos": ["-Wall", "-Wextra"],
    })
