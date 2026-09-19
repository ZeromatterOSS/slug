//! Authored binary/repository fixture shared by REAPI and CLI publication proofs.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod workspace;

pub fn write(root: &Path) -> String {
    workspace::write(root);
    // Stabilize workspace entries before observation without priming actions/cache.
    fs::create_dir(root.join("bazel-out")).unwrap();
    let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    fs::write(
        root.join("repo.bzl"),
        r#"def _impl(ctx):
    ctx.file("BUILD.bazel", "exports_files(['data', 'executable_data'])\n")
    ctx.file("data", ctx.attr.content, executable = False)
    ctx.file("executable_data", ctx.attr.content, executable = True)
repo = repository_rule(implementation = _impl, attrs = {"content": attr.string()})
"#,
    )
    .unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), "load(':defs.bzl', 'binary')\nplatform(name='platform')\nexports_files(['input', 'tools/tool'])\nbinary(name='one', input='input', tool='tools/tool', data=['@generated//:data', '@generated//:executable_data'], mapping='binary.repo_mapping', manifest='binary.runfiles/MANIFEST')\n").unwrap();
    fs::create_dir_all(root.join("tools")).unwrap();
    fs::write(
        root.join("tools/tool"),
        "#!/bin/sh\nset -eu\n/bin/cat input > hidden.txt\n/bin/cat input > unused_cooutput\n",
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

const DEFS: &str = r##"def _impl(ctx):
    executable = ctx.actions.declare_file("binary")
    hidden = ctx.actions.declare_file("hidden.txt")
    unused = ctx.actions.declare_file("unused_cooutput")
    never = ctx.actions.declare_file("unused_action")
    ctx.actions.write(executable, '#!/bin/sh\nset -eu\n/bin/cat "$0.runfiles/_main/hidden.txt" "$0.runfiles/+repo+generated/data" "$0.runfiles/+repo+generated/executable_data"\n', is_executable = True)
    ctx.actions.run(executable = ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs = ctx.attr.input[DefaultInfo].files, outputs = [hidden, unused])
    ctx.actions.write(never, "must not execute")
    files = [hidden]
    for target in ctx.attr.data:
        files += target[DefaultInfo].files.to_list()
    return [DefaultInfo(files = depset([executable]), executable = executable, runfiles = ctx.runfiles(files = files))]
binary = rule(implementation = _impl, executable = True, attrs = {"input": attr.label(allow_single_file = True), "tool": attr.label(allow_single_file = True), "data": attr.label_list(allow_files = True), "mapping": attr.output(), "manifest": attr.output()})
"##;

pub fn check_tree(workspace: &Path, root: &Path, bytes: &[u8]) -> [PathBuf; 2] {
    let tree = root.join("binary.runfiles");
    assert_eq!(
        fs::read_link(tree.join("MANIFEST")).unwrap(),
        root.join("binary.runfiles_manifest")
    );
    assert_eq!(
        fs::read_link(tree.join("_repo_mapping")).unwrap(),
        root.join("binary.repo_mapping")
    );
    assert_eq!(
        fs::read_link(tree.join("_main/hidden.txt")).unwrap(),
        root.join("hidden.txt")
    );
    assert_eq!(fs::read(root.join("hidden.txt")).unwrap(), bytes);
    let paths = ["data", "executable_data"]
        .map(|name| fs::read_link(tree.join(format!("+repo+generated/{name}"))).unwrap());
    assert_ne!(
        paths[0], paths[1],
        "equal bytes with different modes require separate backing"
    );
    for (path, mode) in paths.iter().zip([0o444, 0o555]) {
        assert!(path.starts_with(workspace.join("bazel-out/.slug-runfiles-sources/v1")));
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            mode
        );
        assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
        let manifest = fs::read_to_string(root.join("binary.runfiles_manifest")).unwrap();
        assert!(
            manifest
                .lines()
                .any(|line| line.ends_with(&format!(" {}", path.display())))
        );
    }
    let output = std::process::Command::new(root.join("binary"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, bytes.repeat(3));
    for absent in ["unused_cooutput", "unused_action"] {
        assert!(!root.join(absent).exists());
        assert!(!tree.join(format!("_main/{absent}")).exists());
    }
    paths
}
