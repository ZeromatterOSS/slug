//! Authored artifact aliases shared by native REAPI and CLI process-exit proofs.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod workspace;

pub fn write(root: &Path) -> String {
    workspace::write(root);
    fs::create_dir_all(root.join("bazel-out")).unwrap();
    fs::create_dir_all(root.join("tools")).unwrap();
    let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    fs::write(
        root.join("repo.bzl"),
        r#"def _impl(ctx):
    ctx.file("BUILD.bazel", "exports_files(['data'])\n")
    ctx.file("data", ctx.attr.content, executable=False)
repo = repository_rule(implementation=_impl, attrs={'content':attr.string()})
"#,
    )
    .unwrap();
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace("SCRIPT_BODY", &format!("{SCRIPT:?}")),
    )
    .unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    fs::write(root.join("tools/source_script"), SCRIPT).unwrap();
    fs::set_permissions(
        root.join("tools/source_script"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::write(root.join("tools/producer"), "#!/bin/sh\nset -eu\n/bin/cat input > generated\n/bin/cat input > unused\n/bin/chmod 0644 generated unused\n").unwrap();
    symlink("input", root.join("input-link")).unwrap();
    content(root, &module, "aaa");
    module
}

pub fn content(root: &Path, module: &str, bytes: &str) {
    fs::write(root.join("input"), bytes).unwrap();
    fs::write(root.join("MODULE.bazel"), format!("{module}\nrepo = use_repo_rule('//:repo.bzl', 'repo')\nrepo(name='generated', content='{bytes}')\n")).unwrap();
}

pub fn replace_host_alias(root: &Path, regular: bool) {
    fs::write(
        root.join("BUILD.bazel"),
        BUILD.replace(
            "regular=False",
            if regular {
                "regular=True"
            } else {
                "regular=False"
            },
        ),
    )
    .unwrap();
}

pub fn check_aliases(workspace: &Path, bin: &Path, bytes: &[u8]) -> PathBuf {
    assert_eq!(
        fs::read_link(bin.join("host.alias")).unwrap(),
        workspace.join("input-link")
    );
    assert_eq!(
        fs::read_link(workspace.join("input-link")).unwrap(),
        PathBuf::from("input")
    );
    let durable = fs::read_link(bin.join("material.alias")).unwrap();
    assert!(durable.starts_with(workspace.join("bazel-out/.slug-runfiles-sources/v1")));
    assert!(
        !fs::symlink_metadata(&durable)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::metadata(&durable).unwrap().permissions().mode() & 0o777,
        0o444
    );
    assert_eq!(
        fs::read_link(bin.join("generated.alias")).unwrap(),
        bin.join("generated.first")
    );
    assert_eq!(
        fs::read_link(bin.join("generated.first")).unwrap(),
        bin.join("generated")
    );
    for name in [
        "host.alias",
        "material.alias",
        "generated.alias",
        "generated.first",
        "generated",
    ] {
        assert_eq!(fs::read(bin.join(name)).unwrap(), bytes, "{name}");
    }
    assert!(!bin.join("unused").exists());
    durable
}

pub fn check_manifest_alias(workspace: &Path, bin: &Path, bytes: &[u8]) {
    check_aliases(workspace, bin, bytes);
    let tree = bin.join("tools/source_binary.runfiles");
    assert_eq!(
        fs::read_link(bin.join("manifest.alias")).unwrap(),
        tree.join("MANIFEST")
    );
    assert_eq!(
        fs::read_link(tree.join("MANIFEST")).unwrap(),
        bin.join("tools/source_binary.runfiles_manifest")
    );
    assert!(fs::read(bin.join("manifest.alias")).unwrap().len() > 0);
    for name in ["host.alias", "material.alias", "generated.alias"] {
        assert_eq!(fs::read(tree.join("_main").join(name)).unwrap(), bytes);
    }
    assert_eq!(
        fs::read_link(bin.join("tools/source_binary")).unwrap(),
        bin.join("tools/source_binary.first")
    );
    assert_eq!(
        fs::read_link(bin.join("tools/source_binary.first")).unwrap(),
        workspace.join("tools/source_script")
    );
    let output = workspace.join("after-exit.out");
    let status = std::process::Command::new(bin.join("tools/source_binary"))
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read(&output).unwrap(), bytes.repeat(3));
    fs::remove_file(output).unwrap();
}

const BUILD: &str = r#"load(':defs.bzl', 'producer', 'aliases', 'binary', 'generated_script', 'consumer', 'source_alias', 'manifest_alias')
platform(name='platform')
exports_files(['input', 'input-link', 'tools/source_script', 'tools/producer'])
producer(name='producer', source='input', tool='tools/producer')
aliases(name='aliases', host='input-link', material='@generated//:data', generated=':producer', regular=False)
binary(name='source_binary', script='tools/source_script', data=':aliases')
generated_script(name='generated_script')
binary(name='generated_binary', script=':generated_script', data=':aliases')
consumer(name='source_consumer', tool=':source_binary')
consumer(name='generated_consumer', tool=':generated_binary')
source_alias(name='direct_exec', source='tools/source_script')
source_alias(name='nested_exec', source='tools/source_script', nested=True)
manifest_alias(name='manifest_alias', tool=':source_binary')
"#;

const DEFS: &str = r#"def _producer(ctx):
    out = ctx.actions.declare_file('generated')
    unused = ctx.actions.declare_file('unused')
    ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=ctx.attr.source[DefaultInfo].files, outputs=[out, unused])
    return [DefaultInfo(files=depset([out]))]
producer = rule(implementation=_producer, attrs={'source':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
def _aliases(ctx):
    host = ctx.actions.declare_file('host.alias')
    material = ctx.actions.declare_file('material.alias')
    first = ctx.actions.declare_file('generated.first')
    last = ctx.actions.declare_file('generated.alias')
    if ctx.attr.regular:
        ctx.actions.write(host, 'replacement')
    else:
        ctx.actions.symlink(output=host, target_file=ctx.attr.host[DefaultInfo].files.to_list()[0])
    ctx.actions.symlink(output=material, target_file=ctx.attr.material[DefaultInfo].files.to_list()[0])
    ctx.actions.symlink(output=first, target_file=ctx.attr.generated[DefaultInfo].files.to_list()[0])
    ctx.actions.symlink(output=last, target_file=first)
    return [DefaultInfo(files=depset([host, material, last]))]
aliases = rule(implementation=_aliases, attrs={'host':attr.label(allow_single_file=True), 'material':attr.label(allow_single_file=True), 'generated':attr.label(), 'regular':attr.bool()})
def _binary(ctx):
    first = ctx.actions.declare_file('tools/' + ctx.label.name + '.first')
    exe = ctx.actions.declare_file('tools/' + ctx.label.name)
    ctx.actions.symlink(output=first, target_file=ctx.attr.script[DefaultInfo].files.to_list()[0])
    ctx.actions.symlink(output=exe, target_file=first, is_executable=True)
    return [DefaultInfo(files=depset([exe]), executable=exe, runfiles=ctx.runfiles(files=ctx.attr.data[DefaultInfo].files.to_list()))]
binary = rule(implementation=_binary, executable=True, attrs={'script':attr.label(allow_single_file=True), 'data':attr.label()})
def _generated_script(ctx):
    script = ctx.actions.declare_file('tools/generated_script')
    ctx.actions.write(script, SCRIPT_BODY, is_executable=False)
    return [DefaultInfo(files=depset([script]))]
generated_script = rule(implementation=_generated_script)
def _consumer(ctx):
    out = ctx.actions.declare_file(ctx.label.name + '.out')
    ctx.actions.run(executable=ctx.executable.tool, outputs=[out], arguments=[out.path])
    return [DefaultInfo(files=depset([out]))]
consumer = rule(implementation=_consumer, attrs={'tool':attr.label(executable=True, cfg='exec')})
def _source_alias(ctx):
    source = ctx.attr.source[DefaultInfo].files.to_list()[0]
    if ctx.attr.nested:
        first = ctx.actions.declare_file(ctx.label.name + '.first')
        ctx.actions.symlink(output=first, target_file=source)
        source = first
    out = ctx.actions.declare_file(ctx.label.name + '.alias')
    ctx.actions.symlink(output=out, target_file=source, is_executable=True)
    return [DefaultInfo(files=depset([out]))]
source_alias = rule(implementation=_source_alias, attrs={'source':attr.label(allow_single_file=True), 'nested':attr.bool()})
def _manifest_alias(ctx):
    out = ctx.actions.declare_file('manifest.alias')
    ctx.actions.symlink(output=out, target_file=ctx.attr.tool[DefaultInfo].files_to_run.runfiles_manifest)
    return [DefaultInfo(files=depset([out]))]
manifest_alias = rule(implementation=_manifest_alias, attrs={'tool':attr.label()})
"#;

const SCRIPT: &str = r#"#!/bin/sh
set -eu
rf="$0.runfiles/_main"
/bin/cat "$rf/host.alias" "$rf/material.alias" "$rf/generated.alias" > "$1"
"#;
