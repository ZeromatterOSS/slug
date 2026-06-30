_OUTPUT_NAME = "impl_one"

def _repo_impl(ctx):
    filename = _OUTPUT_NAME + ".txt"
    ctx.file("BUILD.bazel", "exports_files([\"%s\"])\n" % filename)
    ctx.file(filename, _OUTPUT_NAME + "\n")

generated_repo = repository_rule(implementation = _repo_impl)

def _ext_impl(module_ctx):
    generated_repo(name = "generated")

ext = module_extension(implementation = _ext_impl)