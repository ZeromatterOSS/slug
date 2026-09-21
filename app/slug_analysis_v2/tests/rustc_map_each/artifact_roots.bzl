# Authored rules invoking unchanged public path helpers on genuine Files/ctx.
load("@rules_cc//cc/common:cc_helper_internal.bzl", "root_relative_path")
load("//rust/private:utils.bzl", "is_exec_configuration")

Proof = provider(fields = ["file", "pre", "root", "snapshot"])

def _producer(ctx):
    file = ctx.actions.declare_file(ctx.label.name + ".out")
    ctx.actions.write(file, "rooted output")
    pre = ctx.outputs.pre
    ctx.actions.write(pre, "predeclared output")
    args = ctx.actions.args()
    args.add(file)
    # Ordinary provider publication retains the extracted root separately.
    return [DefaultInfo(files = depset([file, pre])), Proof(
        file = file,
        pre = pre,
        root = file.root,
        snapshot = struct(path = file.path, short = file.short_path,
            relative = root_relative_path(file), pre = root_relative_path(pre),
            owner = file.owner, bin = ctx.bin_dir, genfiles = ctx.genfiles_dir,
            exec = is_exec_configuration(ctx), rooted_dict = {file.root: "root"}))]

producer = rule(implementation = _producer, outputs = {"pre": "%{name}.pre"})

def _consumer(ctx):
    target = ctx.attr.target[Proof]
    execution = ctx.attr.execution[Proof]
    if target.root != target.file.root or execution.root != execution.file.root:
        fail("published extracted roots changed identity")
    if target.snapshot.bin != target.root or target.snapshot.genfiles != target.root:
        fail("context and File root diverged")
    sources = [source[DefaultInfo].files.to_list()[0]
               for source in ctx.attr.sources]
    # External generated Files are admitted through their generating rule;
    # direct external generated-label requests remain a separate boundary.
    generated = target.pre
    return [Proof(file = target.file, root = target.root, snapshot = struct(
        target = target.snapshot, execution = execution.snapshot,
        generated = generated.path, generated_root = generated.root,
        source_paths = [source.path for source in sources],
        source_relative = [root_relative_path(source) for source in sources],
        source_roots = [source.root for source in sources],
        source_owners = [source.owner for source in sources],
        repeated_root_lookup = target.snapshot.rooted_dict[target.root]))]

consumer = rule(implementation = _consumer, attrs = {
    "target": attr.label(mandatory = True),
    "execution": attr.label(mandatory = True, cfg = "exec"),
    "sources": attr.label_list(allow_files = True),
})
