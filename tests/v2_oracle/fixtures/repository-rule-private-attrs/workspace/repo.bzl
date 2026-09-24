"""Small local repository rules for private-attribute oracle rows."""

def _string_impl(rctx):
    rctx.file("BUILD.bazel", "filegroup(name = %r)\n" % ("value_" + rctx.attr._rule_name))

string_repo = repository_rule(
    implementation = _string_impl,
    attrs = {"_rule_name": attr.string(default = "python_repository")},
)

def _label_impl(rctx):
    marker = rctx.read(rctx.attr._private_label).strip()
    rctx.file("BUILD.bazel", "filegroup(name = %r)\n" % ("label_" + marker))

label_repo = repository_rule(
    implementation = _label_impl,
    attrs = {"_private_label": attr.label(default = Label("//:marker.txt"))},
)
