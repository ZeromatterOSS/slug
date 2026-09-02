def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"generated.txt\"])\n")
    ctx.file("generated.txt", "hello from extension\n")
    ctx.file("value.bzl", "SIBLING_VALUE = 'loaded'\n")
    ctx.file("defs.bzl", "load('@sibling//:value.bzl', 'SIBLING_VALUE')\nGENERATED_VALUE = SIBLING_VALUE\n")

generated_repo = repository_rule(implementation = _repo_impl)

def _ext_impl(module_ctx):
    generated_repo(name = "generated")
    generated_repo(name = "sibling")

ext = module_extension(implementation = _ext_impl)
