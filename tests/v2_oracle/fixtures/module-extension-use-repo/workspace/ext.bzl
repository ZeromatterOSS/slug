def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"generated.txt\"])\n")
    ctx.file("generated.txt", "hello from extension\n")

generated_repo = repository_rule(implementation = _repo_impl)

def _ext_impl(module_ctx):
    generated_repo(name = "generated")

ext = module_extension(implementation = _ext_impl)