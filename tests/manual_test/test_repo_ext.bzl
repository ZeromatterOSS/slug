load(":test_repo_ctx_simple.bzl", "simple_repo")

def _repo_ext_impl(mctx):
    simple_repo(
        name = "my_test_repo",
        test_attr = "world",
    )

repo_ext = module_extension(
    implementation = _repo_ext_impl,
)
