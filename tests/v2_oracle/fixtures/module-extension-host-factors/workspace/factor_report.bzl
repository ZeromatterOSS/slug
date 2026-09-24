def _factor_report_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".py")
    ctx.actions.write(
        output,
        """#!/usr/bin/env python3
import json
import os
from pathlib import Path

lockfile = Path(os.environ["BUILD_WORKSPACE_DIRECTORY"]) / "MODULE.bazel.lock"
extensions = json.loads(lockfile.read_text(encoding="utf-8"))["moduleExtensions"]
ids = {
    "both": "//:both.bzl%both",
    "os_only": "//:other.bzl%os_only",
    "arch_only": "//:other.bzl%arch_only",
    "general": "//:other.bzl%general",
}
keys = {}
for name, extension_id in ids.items():
    entries = extensions[extension_id]
    assert len(entries) == 1, (name, entries)
    key, entry = next(iter(entries.items()))
    assert set(entry["generatedRepoSpecs"]) == {"probe"}, (name, entry)
    keys[name] = key

os_component, separator, arch_component = keys["both"].partition(",arch:")
assert separator and os_component.startswith("os:") and len(os_component) > 3
assert arch_component and "," not in arch_component
assert keys["os_only"] == os_component, keys
assert keys["arch_only"] == "arch:" + arch_component, keys
assert keys["general"] == "general", keys

for name in ("both", "os_only", "arch_only", "general"):
    print(name + "=" + keys[name])
""",
        is_executable = True,
    )
    return [DefaultInfo(executable = output)]

factor_report = rule(
    implementation = _factor_report_impl,
    executable = True,
)
