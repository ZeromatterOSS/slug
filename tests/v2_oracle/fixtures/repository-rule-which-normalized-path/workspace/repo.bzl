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

def _dirname_impl(ctx):
    shell = ctx.which("sh")
    if shell == None:
        fail("sh was not found through /usr/../bin")

    shell_parent = ctx.path(shell).dirname
    if str(shell_parent) != "/bin":
        fail("which path has an unexpected parent: %s" % shell_parent)

    bin_path = ctx.path("/bin")
    if shell_parent != bin_path:
        fail("which path parent differs from the direct Host path")
    if str(bin_path) != "/bin":
        fail("direct Host path has unexpected text: %s" % bin_path)
    if str(bin_path.dirname) != "/":
        fail("direct Host path has an unexpected parent: %s" % bin_path.dirname)

    root = ctx.path("/")
    if str(root) != "/":
        fail("root path has unexpected text: %s" % root)
    if root.dirname != None:
        fail("root path unexpectedly has a parent: %s" % root.dirname)

    ctx.file("BUILD.bazel", "filegroup(name = 'dirname_paths')\n")

dirname_repo = repository_rule(implementation = _dirname_impl, environ = ["PATH"])
