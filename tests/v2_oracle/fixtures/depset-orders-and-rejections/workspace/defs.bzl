def _probe_impl(ctx):
    pre_a = depset(["a"], order = "preorder")
    pre_b = depset(["b"], order = "preorder")
    pre_c = depset(["c"], transitive = [pre_a, pre_b], order = "preorder")

    post_a = depset(["a"], order = "postorder")
    post_b = depset(["b"], order = "postorder")
    post_c = depset(["c"], transitive = [post_a, post_b], order = "postorder")

    default_c = depset(["c"], transitive = [pre_a])

    top_a = depset(["a"], order = "topological")
    top_b = depset(["b"], transitive = [top_a], order = "topological")
    top_c = depset(["c"], transitive = [top_a], order = "topological")
    top_d = depset(["d"], transitive = [top_b, top_c], order = "topological")

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    content = "\n".join([
        "preorder=" + ",".join(pre_c.to_list()),
        "postorder=" + ",".join(post_c.to_list()),
        "default=" + ",".join(default_c.to_list()),
        "topological=" + ",".join(top_d.to_list()),
    ]) + "\n"
    ctx.actions.write(out, content)
    return [DefaultInfo(files = depset([out]))]


def _bad_impl(ctx):
    depset(["x"], order = "preorder", transitive = [depset(["y"], order = "postorder")])
    return [DefaultInfo()]


depset_probe = rule(implementation = _probe_impl)
depset_bad = rule(implementation = _bad_impl)