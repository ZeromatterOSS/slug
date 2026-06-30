load("//:repo.bzl", "simple_repo")

def _ext_impl(module_ctx):
    simple_repo(name = "generated", filename = "extension_only.txt")

ext = module_extension(implementation = _ext_impl)
