# Test file for Phase 8e: required_aspect_providers filtering
#
# This tests that required_aspect_providers correctly filters which targets
# the aspect applies to, based on the target's providers.

# Provider that the aspect requires on targets
TestInfo = provider(fields=["data"])

# Provider that the aspect returns
CollectedInfo = provider(fields=["visited"])

def _filtered_aspect_impl(target, ctx):
    """Aspect that only applies to targets with TestInfo."""
    print("Aspect visiting:", ctx.label)
    print("  Rule kind:", ctx.rule.kind)

    # Collect visited targets
    visited = [str(ctx.label)]

    # Collect from deps that have CollectedInfo (from aspect)
    if hasattr(ctx.rule.attr, "deps"):
        for dep in ctx.rule.attr.deps:
            if CollectedInfo in dep:
                visited.extend(dep[CollectedInfo].visited)

    return [CollectedInfo(visited=visited)]

# Aspect that only applies to targets with TestInfo provider
filtered_aspect = aspect(
    implementation = _filtered_aspect_impl,
    attr_aspects = ["deps"],
    required_aspect_providers = [[TestInfo]],
)

def _rule_with_test_info_impl(ctx):
    """Rule that provides TestInfo - aspect should apply to this."""
    return [
        DefaultInfo(),
        TestInfo(data = str(ctx.label) + " has TestInfo"),
    ]

rule_with_test_info = rule(
    implementation = _rule_with_test_info_impl,
    attrs = {
        "deps": attr.label_list(default = []),
    },
)

def _rule_without_test_info_impl(ctx):
    """Rule that does NOT provide TestInfo - aspect should NOT apply to this."""
    return [DefaultInfo()]

rule_without_test_info = rule(
    implementation = _rule_without_test_info_impl,
    attrs = {
        "deps": attr.label_list(default = []),
    },
)

def _top_rule_impl(ctx):
    """Top-level rule that has the aspect attached to its deps."""
    return [DefaultInfo()]

top_rule = rule(
    implementation = _top_rule_impl,
    attrs = {
        "deps": attr.label_list(aspects=[filtered_aspect]),
    },
)
