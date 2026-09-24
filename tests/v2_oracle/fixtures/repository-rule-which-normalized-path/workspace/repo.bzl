def _impl(ctx):
    shell = ctx.which("sh")
    if shell == None:
        fail("sh was not found through /usr/../bin")
    if str(shell) != "/bin/sh":
        fail("which returned an unnormalized shell path: %s" % shell)
    ctx.file("BUILD.bazel", "filegroup(name = 'normalized_sh')\n")

which_repo = repository_rule(implementation = _impl, environ = ["PATH"])
