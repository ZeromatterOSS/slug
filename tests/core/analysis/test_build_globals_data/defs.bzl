"""Helper macros and rules for testing BUILD-level globals."""

# === write_value_rule: writes a single value to a file ===
def _write_value_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.value)
    return [DefaultInfo(files = depset([out]))]

write_value_rule = rule(
    implementation = _write_value_impl,
    attrs = {
        "value": attr.string(default = ""),
    },
)

# === write_list_rule: writes a list of values as newlines ===
def _write_list_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(ctx.attr.values))
    return [DefaultInfo(files = depset([out]))]

write_list_rule = rule(
    implementation = _write_list_impl,
    attrs = {
        "values": attr.string_list(default = []),
    },
)

# === Macro that uses native.existing_rules() ===
def count_rules_macro():
    """Returns the number of rules defined so far as a string."""
    rules = native.existing_rules()
    return str(len(rules))

# === Macro that uses native.package_name() ===
def package_name_macro():
    """Returns native.package_name() from within a macro."""
    return native.package_name()

# === Macro that uses native.repository_name() ===
def repo_name_macro():
    """Returns native.repository_name() from within a macro."""
    return native.repository_name()

# === Macro that inspects an existing rule ===
def get_rule_kind_macro(name):
    """Returns the 'kind' of a previously-defined rule."""
    rule_info = native.existing_rule(name)
    if rule_info == None:
        return "NOT_FOUND"
    return rule_info.get("kind", "UNKNOWN")

# === Macro that creates a genrule using native.glob() ===
def glob_count_macro(pattern):
    """Returns the count of files matching a glob pattern."""
    files = native.glob(pattern)
    return str(len(files))

# === Macro that uses package_relative_label ===
def resolve_label_macro(label_string):
    """Resolves a label string relative to the current package."""
    label = native.package_relative_label(label_string)
    return str(label)
