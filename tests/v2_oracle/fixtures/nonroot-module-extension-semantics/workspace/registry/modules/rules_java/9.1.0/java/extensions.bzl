def _empty_repo_impl(ctx):
    ctx.file("BUILD.bazel", "")

_empty_repo = repository_rule(implementation = _empty_repo_impl)

def _toolchains_impl(ctx):
    _empty_repo(name = "local_jdk")
    _empty_repo(name = "remote_java_tools")

toolchains = module_extension(implementation = _toolchains_impl)
