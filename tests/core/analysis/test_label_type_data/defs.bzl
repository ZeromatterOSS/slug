"""Tests for the Label() constructor and Label type attributes."""

# ============================================================================
# Label() constructor - attributes
# ============================================================================

def _label_attrs_rule_impl(ctx):
    """Tests Label() constructor attributes: name, package, workspace_name."""
    # Label() creates a BazelLabel with .name, .package, .workspace_name attrs
    lbl = Label("//pkg/sub:my_target")
    out = ctx.actions.declare_file("label_attrs.txt")
    ctx.actions.write(
        out,
        "name={}\npackage={}\nworkspace_name={}".format(
            lbl.name, lbl.package, lbl.workspace_name,
        ),
    )
    return [DefaultInfo(default_output = out)]


label_attrs_rule = rule(
    implementation = _label_attrs_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - root package
# ============================================================================

def _label_root_rule_impl(ctx):
    """Tests Label() at root package: //target => package is empty."""
    lbl = Label("//:root_target")
    out = ctx.actions.declare_file("label_root.txt")
    ctx.actions.write(
        out,
        "name={}\npackage={}".format(lbl.name, lbl.package),
    )
    return [DefaultInfo(default_output = out)]


label_root_rule = rule(
    implementation = _label_root_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - external repo
# ============================================================================

def _label_external_rule_impl(ctx):
    """Tests Label() with external repo: @repo//pkg:target."""
    lbl = Label("@my_repo//some/path:my_lib")
    out = ctx.actions.declare_file("label_external.txt")
    ctx.actions.write(
        out,
        "name={}\npackage={}\nworkspace_name={}".format(
            lbl.name, lbl.package, lbl.workspace_name,
        ),
    )
    return [DefaultInfo(default_output = out)]


label_external_rule = rule(
    implementation = _label_external_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - relative() method
# ============================================================================

def _label_relative_rule_impl(ctx):
    """Tests Label.relative() to resolve relative labels."""
    lbl = Label("//pkg:target")
    # :other resolves to //pkg:other (same package)
    sibling = lbl.relative(":other_target")
    out = ctx.actions.declare_file("label_relative.txt")
    ctx.actions.write(out, str(sibling))
    return [DefaultInfo(default_output = out)]


label_relative_rule = rule(
    implementation = _label_relative_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - equality
# ============================================================================

def _label_eq_rule_impl(ctx):
    """Tests Label equality comparisons."""
    lbl1 = Label("//pkg:target")
    lbl2 = Label("//pkg:target")
    lbl3 = Label("//pkg:other")
    out = ctx.actions.declare_file("label_eq.txt")
    ctx.actions.write(
        out,
        "same_eq={}\ndiff_eq={}".format(
            lbl1 == lbl2,
            lbl1 == lbl3,
        ),
    )
    return [DefaultInfo(default_output = out)]


label_eq_rule = rule(
    implementation = _label_eq_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - string conversion
# ============================================================================

def _label_str_rule_impl(ctx):
    """Tests str(Label()) returns the full label string."""
    lbl = Label("//my/package:my_target")
    out = ctx.actions.declare_file("label_str.txt")
    ctx.actions.write(out, str(lbl))
    return [DefaultInfo(default_output = out)]


label_str_rule = rule(
    implementation = _label_str_rule_impl,
    attrs = {},
)


# ============================================================================
# Label() - same_package_label() method
# ============================================================================

def _label_same_package_rule_impl(ctx):
    """Tests Label.same_package_label() to create sibling labels."""
    lbl = Label("//my/pkg:original")
    sibling = lbl.same_package_label("sibling_target")
    out = ctx.actions.declare_file("label_same_pkg.txt")
    ctx.actions.write(
        out,
        "name={}\npackage={}".format(sibling.name, sibling.package),
    )
    return [DefaultInfo(default_output = out)]


label_same_package_rule = rule(
    implementation = _label_same_package_rule_impl,
    attrs = {},
)
