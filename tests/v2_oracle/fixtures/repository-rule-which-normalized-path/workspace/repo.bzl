def _impl(ctx):
    shell = ctx.which("sh")
    if shell == None:
        fail("sh was not found through /usr/../bin")
    if str(shell) != "/bin/sh":
        fail("which returned an unnormalized shell path: %s" % shell)
    ctx.file("BUILD.bazel", "filegroup(name = 'normalized_sh')\n")

which_repo = repository_rule(implementation = _impl, environ = ["PATH"])

def _aggregate_impl(ctx):
    for i in range(65):
        program = "slug_v2_which_absent_" + str(i)
        found = ctx.which(program)
        if found != None:
            fail("reserved absent program unexpectedly found: %s at %s" % (program, found))

    shell = ctx.which("sh")
    if shell == None:
        fail("sh was not found after 65 absent programs")
    if str(shell) != "/bin/sh":
        fail("which returned an unnormalized shell path after 65 misses: %s" % shell)
    ctx.file("BUILD.bazel", "filegroup(name = 'aggregate_which')\n")

aggregate_which_repo = repository_rule(implementation = _aggregate_impl, environ = ["PATH"])
