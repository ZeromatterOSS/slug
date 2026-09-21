# Authored native-boundary callers, not cc_common.compile or a compiler action.
# Keep the main-origin external load same-package: cross-package descendants
# remain a separate loader boundary. Public toolchain naming is protected by
# cc_context_naming; this proof supplies an authored compile output name.
load("//cc/private:cc_internal.bzl", "cc_internal")

Proof = provider(fields = ["snapshot"])

def _configuration_escape(ctx):
    return [Proof(snapshot = ctx.configuration)]

configuration_escape = rule(implementation = _configuration_escape)

# Label construction is currently admitted at module initialization only.
_DESCENDANTS = {
    "main": Label("@@//pkg/child:located"),
    "external": Label("@@rules_rust+//pkg/child:located"),
}

def _producer(ctx):
    prefix = cc_internal.compute_output_name_prefix_dir(
        configuration = ctx.configuration,
        purpose = ctx.attr.purpose,
    )
    name = "dir/unit.o"
    if prefix:
        name = prefix + "/" + name
    location = ctx.label.same_package_label("location")
    descendant = _DESCENDANTS[ctx.label.name]
    file = cc_internal.declare_compile_output_file(
        ctx = ctx, label = location, output_name = name,
        configuration = ctx.configuration,
    )
    repeated = cc_internal.declare_compile_output_file(
        ctx = ctx, label = location, output_name = name,
        configuration = ctx.configuration,
    )
    if repeated != file:
        fail("identical native declaration changed File identity")
    child = cc_internal.declare_compile_output_file(
        ctx = ctx, label = descendant, output_name = "child.o",
        configuration = ctx.configuration,
    )
    normalized = cc_internal.declare_compile_output_file(
        ctx = ctx, label = location, output_name = "scratch/../normalized.o",
        configuration = ctx.configuration,
    )
    files = [file, child, normalized]
    for output in files:
        if output.owner != ctx.label or output.root != ctx.bin_dir or output.is_directory:
            fail("compile declaration did not preserve active owner/bin/regular kind")
        ctx.actions.write(output, "authored declaration marker")
    return [DefaultInfo(files = depset(files)), Proof(snapshot = struct(
        files = files, root = file.root, owner = ctx.label,
        location = location, descendant = descendant, prefix = prefix,
        named = name, root_dict = {file.root: "published-root"},
    ))]

producer = rule(implementation = _producer, attrs = {
    "purpose": attr.string(default = "cc_library-compile"),
})

def _consumer(ctx):
    rows = []
    for dep in [ctx.attr.external, ctx.attr.external_exec, ctx.attr.main, ctx.attr.main_exec]:
        row = dep[Proof].snapshot
        if row.root_dict[row.files[0].root] != "published-root":
            fail("root identity changed across configured provider publication")
        if row.location == row.owner:
            fail("fixture must distinguish location label from active owner")
        rows.append(row)
    if rows[0].root != rows[2].root or rows[1].root != rows[3].root:
        fail("repository placement incorrectly changed the shared bin root")
    if rows[0].root == rows[1].root:
        fail("target and exec roots must differ")
    return [Proof(snapshot = rows)]

subject = rule(implementation = _consumer, attrs = {
    "external": attr.label(mandatory = True),
    "external_exec": attr.label(mandatory = True, cfg = "exec"),
    "main": attr.label(mandatory = True),
    "main_exec": attr.label(mandatory = True, cfg = "exec"),
})
