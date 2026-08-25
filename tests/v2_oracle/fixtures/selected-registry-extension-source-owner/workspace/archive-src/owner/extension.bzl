load("//shared:local.bzl", "LOCAL")
load("@mapped_dep//:mapped.bzl", "MAPPED")

print("SELECTED_REGISTRY_MARKER:%s:%s" % (LOCAL, MAPPED))

def _probe_impl(module_ctx):
    pass

probe = module_extension(implementation = _probe_impl)
