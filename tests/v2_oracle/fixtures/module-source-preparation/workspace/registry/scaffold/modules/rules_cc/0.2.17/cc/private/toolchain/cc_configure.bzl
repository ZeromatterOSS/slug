def _cc_configure_impl(ctx):
    ctx.file("BUILD.bazel", "")

cc_configure = repository_rule(implementation = _cc_configure_impl)
