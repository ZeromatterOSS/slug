def _empty_repo_impl(ctx):
    ctx.file("BUILD.bazel", "")

_empty_repo = repository_rule(implementation = _empty_repo_impl)

def _buildozer_binary_impl(ctx):
    _empty_repo(name = "buildozer_binary")

buildozer_binary = module_extension(implementation = _buildozer_binary_impl)
