"""Simple test for Phase 8c - Aspect execution without shadow graph.

This test verifies that:
- Aspect implementation function is called during builds
- ctx.rule.kind returns the correct rule type
- ctx.label returns the correct target label
"""

# Provider to mark that the aspect executed
AspectExecutedInfo = provider(fields=["label", "rule_kind"])

def _simple_aspect_impl(target, ctx):
    """Simple aspect that prints and returns without accessing deps."""
    print("Aspect visiting:", ctx.label)
    print("  Rule kind:", ctx.rule.kind)

    # Return a provider to confirm aspect executed
    return [AspectExecutedInfo(
        label = str(ctx.label),
        rule_kind = ctx.rule.kind,
    )]

simple_aspect = aspect(
    implementation = _simple_aspect_impl,
    attr_aspects = ["deps"],  # Propagate through deps attribute
)

def _simple_rule_impl(ctx):
    """Simple rule that just returns DefaultInfo."""
    return [DefaultInfo()]

simple_rule = rule(
    implementation = _simple_rule_impl,
    attrs = {
        "deps": attr.label_list(aspects=[simple_aspect], default=[]),
    },
)
