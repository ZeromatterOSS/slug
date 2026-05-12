# Test repository_rule() and repository_ctx built-in functions
# Plan Reference: thoughts/shared/plans/slug-bazel-subplans/02-bzlmod.md Phase 5

def _test_repo_impl(ctx):
    """Test implementation - just a stub for now."""
    # In a real repository rule, we would:
    # - Download files using ctx.download()
    # - Create BUILD files using ctx.file()
    # - etc.
    print("Repository rule implementation called for:", ctx.name)
    ctx.file("BUILD", "# Generated BUILD file")

# Test that repository_rule() can be called with attrs
test_repo = repository_rule(
    implementation = _test_repo_impl,
    attrs = {
        "url": attr.string(mandatory = True),
        "sha256": attr.string(default = ""),
        "strip_prefix": attr.string(default = ""),
    },
    environ = ["HOME"],
    doc = "A test repository rule",
)

def test_repository_rule_available():
    """Returns True if repository_rule is available."""
    # If we got here without errors, repository_rule is available
    return True
