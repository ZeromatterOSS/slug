load(":repo.bzl", "emit")

def _impl(ctx):
    emit(ctx)

os_only = module_extension(
    implementation = _impl,
    os_dependent = True,
)

arch_only = module_extension(
    implementation = _impl,
    arch_dependent = True,
)

general = module_extension(implementation = _impl)
