//! Small authored FilesToRun consumers; independent of the WP750 publication fixture.
use std::fs;
use std::path::Path;

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod workspace;

pub fn write(root: &Path) -> String {
    workspace::write(root);
    fs::create_dir(root.join("bazel-out")).unwrap();
    let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    fs::write(
        root.join("repo.bzl"),
        r#"def _impl(ctx):
    ctx.file("BUILD.bazel", "exports_files(['data'])\n")
    ctx.file("data", ctx.attr.content, executable = False)
repo = repository_rule(implementation = _impl, attrs = {"content": attr.string()})
"#,
    )
    .unwrap();
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace("TOOL_BODY", &format!("{TOOL:?}")),
    )
    .unwrap();
    fs::write(root.join("BUILD.bazel"), r#"load(':defs.bzl', 'backing', 'binary', 'consume')
platform(name='platform')
exports_files(['input', 'tools/producer', 'tools/runner'])
backing(name='backing', input='input', tool='tools/producer')
binary(name='plain_tool', backing=':backing', host='input', materialized='@generated//:data')
binary(name='authored_tool', backing=':backing', host='input', materialized='@generated//:data', authored=True)
consume(name='implicit', tool=':plain_tool', runner='tools/runner')
consume(name='explicit', tool=':plain_tool', runner='tools/runner', explicit=True)
consume(name='authored', tool=':authored_tool', runner='tools/runner', explicit=True)
"#).unwrap();
    fs::create_dir(root.join("tools")).unwrap();
    fs::write(
        root.join("tools/producer"),
        r#"#!/bin/sh
set -eu
/bin/mkdir -p gen_tree/nested gen_tree/omitted_empty
/bin/cat input > gen_file
/bin/cat input > gen_tree/nested/value
/bin/cat input > unused_cooutput
/bin/chmod 0644 gen_file gen_tree/nested/value
"#,
    )
    .unwrap();
    fs::write(
        root.join("tools/runner"),
        "#!/bin/sh\nset -eu\nexec \"./$1\" \"$2\"\n",
    )
    .unwrap();
    content(root, &module, "aaa");
    module
}

pub fn content(root: &Path, module: &str, bytes: &str) {
    fs::write(root.join("input"), bytes).unwrap();
    fs::write(root.join("MODULE.bazel"), format!(
        "{module}\nrepo = use_repo_rule('//:repo.bzl', 'repo')\nrepo(name='generated', content='{bytes}')\n",
    )).unwrap();
}

const DEFS: &str = r#"def _backing(ctx):
    file = ctx.actions.declare_file('gen_file')
    tree = ctx.actions.declare_directory('gen_tree')
    empty = ctx.actions.declare_directory('empty_tree')
    unused = ctx.actions.declare_file('unused_cooutput')
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=ctx.attr.input[DefaultInfo].files, outputs=[file, tree, empty, unused])
    return [DefaultInfo(files=depset([file, tree, empty]))]
backing = rule(implementation=_backing, attrs={'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
def _binary(ctx):
    exe = ctx.actions.declare_file('tools/' + ctx.label.name)
    ctx.actions.write(exe, TOOL_BODY, is_executable=True)
    host = ctx.attr.host[DefaultInfo].files.to_list()[0]
    files = ctx.attr.backing[DefaultInfo].files.to_list() + [host] + ctx.attr.materialized[DefaultInfo].files.to_list()
    roots = {'MANIFEST': host} if ctx.attr.authored else {}
    return [DefaultInfo(files=depset([exe]), executable=exe, runfiles=ctx.runfiles(files=files, root_symlinks=roots))]
binary = rule(implementation=_binary, executable=True, attrs={'backing':attr.label(), 'host':attr.label(allow_single_file=True), 'materialized':attr.label(allow_single_file=True), 'authored':attr.bool()})
def _consume(ctx):
    out = ctx.actions.declare_file(ctx.label.name + '.out')
    if ctx.attr.explicit:
        tool = ctx.attr.tool[DefaultInfo]
        ctx.actions.run(executable=ctx.attr.runner[DefaultInfo].files.to_list()[0], tools=[tool.files_to_run], outputs=[out], arguments=[tool.files.to_list()[0].path, out.path])
    else:
        ctx.actions.run(executable=ctx.executable.tool, outputs=[out], arguments=[out.path])
    return [DefaultInfo(files=depset([out]))]
consume = rule(implementation=_consume, attrs={'tool':attr.label(executable=True, cfg='exec'), 'runner':attr.label(allow_single_file=True), 'explicit':attr.bool()})
"#;

const TOOL: &str = r#"#!/bin/sh
set -eu
rf="$0.runfiles"
[ -x "$rf/_main/gen_file" ]
[ -x "$rf/_main/gen_tree/nested/value" ]
[ -x "$rf/_main/input" ]
[ -x "$rf/+repo+generated/data" ]
[ -d "$rf/_main/empty_tree" ]
[ ! -e "$rf/_main/gen_tree/omitted_empty" ]
[ ! -L "$rf/_main/gen_file" ]
[ -s "$rf/_repo_mapping" ]
/bin/grep generated "$rf/_repo_mapping" > /dev/null
case "$0" in
  *authored_tool) /usr/bin/cmp "$rf/MANIFEST" "$rf/_main/input" ;;
  *) [ ! -e "$rf/MANIFEST" ] ;;
esac
/bin/cat "$rf/_main/gen_file" "$rf/_main/gen_tree/nested/value" "$rf/_main/input" "$rf/+repo+generated/data" > "$1"
"#;
