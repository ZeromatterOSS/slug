def _impl(rctx):
    source = rctx.path(Label("@@source+//:linked.BUILD"))
    rctx.symlink(source, "BUILD.bazel")

linked_repo = repository_rule(implementation = _impl)
