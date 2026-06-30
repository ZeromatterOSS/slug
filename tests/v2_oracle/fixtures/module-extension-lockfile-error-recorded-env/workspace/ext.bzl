def _repo_impl(ctx):
    filename = ctx.attr.value + ".txt"
    ctx.file("BUILD.bazel", "exports_files([\"{}\"])\n".format(filename))
    ctx.file(filename, ctx.attr.value + "\n")

generated_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {"value": attr.string(mandatory = True)},
)

def _ext_impl(module_ctx):
    value = module_ctx.getenv("SLUG_STAGE5_ENV", "one")
    generated_repo(name = "generated", value = value)

ext = module_extension(implementation = _ext_impl)