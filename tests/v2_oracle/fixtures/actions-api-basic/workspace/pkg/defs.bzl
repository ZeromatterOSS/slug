def _impl(ctx):
    write_out = ctx.actions.declare_file(ctx.label.name + ".write.txt")
    json_out = ctx.actions.declare_file(ctx.label.name + ".json")
    template_out = ctx.actions.declare_file(ctx.label.name + ".template.txt")
    run_out = ctx.actions.declare_file(ctx.label.name + ".run.txt")
    shell_out = ctx.actions.declare_file(ctx.label.name + ".shell.txt")
    link_out = ctx.actions.declare_file(ctx.label.name + ".link.txt")

    ctx.actions.write(write_out, "write\n")
    ctx.actions.write(json_out, "{\"ok\": true}\n")
    ctx.actions.expand_template(
        template = ctx.file.template,
        output = template_out,
        substitutions = {"{NAME}": "Slug"},
    )
    ctx.actions.run(
        outputs = [run_out],
        inputs = [write_out],
        executable = "/bin/sh",
        arguments = ["-c", "cp %s %s" % (write_out.path, run_out.path)],
        mnemonic = "ActionRunProbe",
    )
    ctx.actions.run_shell(
        outputs = [shell_out],
        inputs = [write_out],
        command = "cp $1 $2",
        arguments = [write_out.path, shell_out.path],
        mnemonic = "ActionShellProbe",
    )
    ctx.actions.symlink(output = link_out, target_file = write_out)
    return [DefaultInfo(files = depset([write_out, json_out, template_out, run_out, shell_out, link_out]))]


actions_rule = rule(
    implementation = _impl,
    attrs = {
        "template": attr.label(allow_single_file = True),
    },
)