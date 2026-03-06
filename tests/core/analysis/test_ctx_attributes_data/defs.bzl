"""Tests for ctx attributes and ctx.actions methods."""


# ============================================================================
# ctx.bin_dir / ctx.genfiles_dir
# ============================================================================

def _bin_dir_rule_impl(ctx):
    """Writes ctx.bin_dir.path to an output file."""
    out = ctx.actions.declare_file("bin_dir_path.txt")
    ctx.actions.write(out, ctx.bin_dir.path)
    return [DefaultInfo(default_output = out)]


bin_dir_rule = rule(
    implementation = _bin_dir_rule_impl,
    attrs = {},
)


def _genfiles_dir_rule_impl(ctx):
    """Writes ctx.genfiles_dir.path to an output file."""
    out = ctx.actions.declare_file("genfiles_dir_path.txt")
    ctx.actions.write(out, ctx.genfiles_dir.path)
    return [DefaultInfo(default_output = out)]


genfiles_dir_rule = rule(
    implementation = _genfiles_dir_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.features / ctx.disabled_features
# ============================================================================

def _features_rule_impl(ctx):
    """Writes enabled features (from ctx.features) to output."""
    out = ctx.actions.declare_file("features.txt")
    ctx.actions.write(out, "\n".join(sorted(ctx.features)))
    return [DefaultInfo(default_output = out)]


features_rule = rule(
    implementation = _features_rule_impl,
    attrs = {},
)


def _disabled_features_rule_impl(ctx):
    """Writes disabled features (from ctx.disabled_features) to output."""
    out = ctx.actions.declare_file("disabled_features.txt")
    ctx.actions.write(out, "\n".join(sorted(ctx.disabled_features)))
    return [DefaultInfo(default_output = out)]


disabled_features_rule = rule(
    implementation = _disabled_features_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.workspace_name
# ============================================================================

def _workspace_name_rule_impl(ctx):
    """Writes ctx.workspace_name to output."""
    out = ctx.actions.declare_file("workspace_name.txt")
    ctx.actions.write(out, ctx.workspace_name)
    return [DefaultInfo(default_output = out)]


workspace_name_rule = rule(
    implementation = _workspace_name_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.label
# ============================================================================

def _label_rule_impl(ctx):
    """Writes ctx.label info to output."""
    out = ctx.actions.declare_file("label_info.txt")
    lines = [
        "package=" + ctx.label.package,
        "name=" + ctx.label.name,
        "workspace=" + ctx.label.workspace_name,
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


label_rule = rule(
    implementation = _label_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.expand_make_variables
# ============================================================================

def _expand_make_vars_rule_impl(ctx):
    """Tests ctx.expand_make_variables with standard Make vars."""
    out = ctx.actions.declare_file("make_vars.txt")

    # Test BINDIR and GENDIR expansions
    bindir = ctx.expand_make_variables("attr", "$(BINDIR)", {})
    gendir = ctx.expand_make_variables("attr", "$(GENDIR)", {})

    # Test custom substitution
    custom = ctx.expand_make_variables("attr", "$(MY_VAR)", {"MY_VAR": "custom_value"})

    lines = [
        "bindir_starts_with_buck_out=" + str(bindir.startswith("buck-out")),
        "gendir_starts_with_buck_out=" + str(gendir.startswith("buck-out")),
        "custom=" + custom,
    ]
    ctx.actions.write(out, "\n".join(lines))
    return [DefaultInfo(default_output = out)]


expand_make_vars_rule = rule(
    implementation = _expand_make_vars_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.actions.symlink
# ============================================================================

def _symlink_rule_impl(ctx):
    """Tests ctx.actions.symlink to create a file symlink."""
    source = ctx.actions.declare_file("source.txt")
    ctx.actions.write(source, "symlink_source_content")

    link = ctx.actions.declare_file("link.txt")
    ctx.actions.symlink(output = link, target_file = source)
    return [DefaultInfo(default_output = link)]


symlink_rule = rule(
    implementation = _symlink_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.actions.expand_template
# ============================================================================

def _expand_template_rule_impl(ctx):
    """Tests ctx.actions.expand_template with substitutions."""
    template = ctx.file.template
    out = ctx.actions.declare_file("expanded.txt")
    ctx.actions.expand_template(
        template = template,
        output = out,
        substitutions = {
            "{NAME}": "Kuro",
            "{VERSION}": "9.0.0",
        },
    )
    return [DefaultInfo(default_output = out)]


expand_template_rule = rule(
    implementation = _expand_template_rule_impl,
    attrs = {
        "template": attr.label(allow_single_file = True),
    },
)


# ============================================================================
# ctx.actions.do_nothing
# ============================================================================

def _do_nothing_rule_impl(ctx):
    """Tests that ctx.actions.do_nothing() doesn't error."""
    ctx.actions.do_nothing(mnemonic = "NoOp")
    out = ctx.actions.declare_file("do_nothing_ok.txt")
    ctx.actions.write(out, "do_nothing_ran")
    return [DefaultInfo(default_output = out)]


do_nothing_rule = rule(
    implementation = _do_nothing_rule_impl,
    attrs = {},
)


# ============================================================================
# ctx.resolve_tools
# ============================================================================

def _resolve_tools_rule_impl(ctx):
    """Tests ctx.resolve_tools() collects files from tool deps."""
    tool_files, _manifests = ctx.resolve_tools(tools = ctx.attr.tools)
    out = ctx.actions.declare_file("tool_names.txt")
    names = sorted([f.basename for f in tool_files])
    ctx.actions.write(out, "\n".join(names))
    return [DefaultInfo(default_output = out)]


resolve_tools_rule = rule(
    implementation = _resolve_tools_rule_impl,
    attrs = {
        "tools": attr.label_list(default = []),
    },
)


# ============================================================================
# ctx.actions.template_dict (computed_substitutions for expand_template)
# ============================================================================

def _template_dict_rule_impl(ctx):
    """Tests ctx.actions.template_dict() for computed substitutions."""
    template = ctx.file.template

    # Build a template_dict with computed substitutions
    subs = ctx.actions.template_dict()
    subs.add("{GREETING}", "Hi")
    subs.add_joined("{ITEMS}", ["a", "b", "c"], join_with = ",")

    out = ctx.actions.declare_file("computed_template.txt")
    ctx.actions.expand_template(
        template = template,
        output = out,
        substitutions = {},
        computed_substitutions = subs,
    )
    return [DefaultInfo(default_output = out)]


template_dict_rule = rule(
    implementation = _template_dict_rule_impl,
    attrs = {
        "template": attr.label(allow_single_file = True),
    },
)
