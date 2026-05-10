def _declare_file_path_shape_impl(ctx):
    selected = ctx.actions.declare_file("build/c.s")
    marker = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(selected, "selected")
    ctx.actions.write(marker, selected.path)
    return [DefaultInfo(files = depset([marker]))]

declare_file_path_shape_rule = rule(
    implementation = _declare_file_path_shape_impl,
    attrs = {},
)

def _declare_directory_path_shape_impl(ctx):
    selected = ctx.actions.declare_directory("build/tree")
    marker = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [selected, marker],
        command = "mkdir -p {tree} && echo ok > {tree}/marker.txt && printf '%s' {tree} > {marker}".format(
            marker = marker.path,
            tree = selected.path,
        ),
    )
    return [DefaultInfo(files = depset([marker]))]

declare_directory_path_shape_rule = rule(
    implementation = _declare_directory_path_shape_impl,
    attrs = {},
)
