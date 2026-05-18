def _shared_ext_impl(module_ctx):
    pass

shared_ext = module_extension(
    implementation = _shared_ext_impl,
)
