"""Exercise a concrete private label override from a .bzl context."""

load(":repo.bzl", "label_repo")

def _impl(_ctx):
    label_repo(name = "label_default")
    label_repo(name = "label_override", _private_label = Label("//:other.txt"))

labels = module_extension(implementation = _impl)
