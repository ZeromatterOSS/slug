"""Test aspect propagation through dependency graph (Phase 8c).

This test verifies that:
- Aspects propagate through the dependency graph
- Aspect executes on all targets in the dependency chain
- ctx.rule.kind returns the correct rule type
- ctx.label returns the correct target label
- ctx.rule.attr.deps contains aspect results (shadow graph)
"""

# Provider to collect names from dependency chain
CollectNamesInfo = provider(fields=["names"])

def _collect_aspect_impl(target, ctx):
    """Aspect that collects target names through the dependency graph."""
    print("Aspect visiting:", ctx.label)
    print("  Rule kind:", ctx.rule.kind)

    # Start with current target's name
    names = [str(ctx.label)]

    # Collect names from dependencies if they have the aspect's provider
    if hasattr(ctx.rule.attr, "deps"):
        for dep in ctx.rule.attr.deps:
            if CollectNamesInfo in dep:
                names.extend(dep[CollectNamesInfo].names)

    return [CollectNamesInfo(names=names)]

collect_aspect = aspect(
    implementation = _collect_aspect_impl,
    attr_aspects = ["deps"],  # Propagate through deps attribute
)

def _test_rule_impl(ctx):
    """Simple rule that just returns DefaultInfo."""
    return [DefaultInfo()]

test_rule = rule(
    implementation = _test_rule_impl,
    attrs = {
        "deps": attr.label_list(aspects=[collect_aspect], default=[]),
    },
)
