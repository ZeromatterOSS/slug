def _lock_printer_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.name + ".bat")
    ctx.actions.write(
        out,
        "@echo off\r\nfindstr /C:\"moduleExtensions\" /C:\"//:ext.bzl%%ext\" /C:\"bzlTransitiveDigest\" /C:\"usagesDigest\" /C:\"generatedRepoSpecs\" /C:\"tagged\" /C:\"repoRuleId\" /C:\"message\" /C:\"hello from tag\" \"%BUILD_WORKSPACE_DIRECTORY%\\MODULE.bazel.lock\"\r\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

lock_printer = rule(
    implementation = _lock_printer_impl,
    executable = True,
    attrs = {"data": attr.label_list(allow_files = True)},
)