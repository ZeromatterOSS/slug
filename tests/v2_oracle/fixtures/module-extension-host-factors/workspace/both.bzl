load(":repo.bzl", "emit")

def _impl(ctx):
    emit(ctx)

both = module_extension(
    implementation = _impl,
    os_dependent = True,
    arch_dependent = True,
)
