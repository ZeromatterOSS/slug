//! Authored tiny source-staging rule; no compiler action or copied ruleset.
use std::path::Path;

pub fn write(root: &Path) {
    std::fs::create_dir_all(root.join("platforms/host")).unwrap();
    for (path, bytes) in [
        (
            "MODULE.bazel",
            "module(name='stage')\nbazel_dep(name='platforms', version='1.0.0')\nlocal_path_override(module_name='platforms', path='platforms')\nregister_execution_platforms('//:platform')\n",
        ),
        (
            "platforms/MODULE.bazel",
            "module(name='platforms', version='1.0.0')\n",
        ),
        (
            "platforms/host/BUILD.bazel",
            "exports_files(['constraints.bzl'])\nplatform(name='host')\n",
        ),
        ("platforms/host/constraints.bzl", "HOST_CONSTRAINTS = []\n"),
        (
            "BUILD.bazel",
            "load(':defs.bzl', 'stage')\nplatform(name='platform')\nexports_files(['tool', 'input'])\nstage(name='one', input='input', tool='tool')\nstage(name='two', input='input', tool='tool')\n",
        ),
        ("defs.bzl", DEFS),
        ("tool", "stage tool bytes"),
        ("input", "aaa"),
    ] {
        std::fs::write(root.join(path), bytes).unwrap();
    }
    let mut module = std::fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    for (name, version) in BUILTIN_DEPENDENCIES {
        if *name == "platforms" {
            continue;
        }
        std::fs::create_dir_all(root.join(name)).unwrap();
        std::fs::write(
            root.join(name).join("MODULE.bazel"),
            format!("module(name='{name}', version='{version}')\n"),
        )
        .unwrap();
        module.push_str(&format!(
            "local_path_override(module_name='{name}', path='{name}')\n"
        ));
        if matches!(
            *name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            module.push_str(&format!("bazel_dep(name='{name}', version='{version}')\n"));
        }
    }
    std::fs::write(root.join("MODULE.bazel"), module).unwrap();
}

pub const DEFS: &str = r#"def _impl(ctx):
    out = ctx.actions.declare_file('shared.out')
    args = ctx.actions.args()
    args.add('parameter')
    args.use_param_file('@%s', use_always = True)
    args.set_param_file_format('multiline')
    ctx.actions.run(executable = ctx.attr.tool, inputs = depset([ctx.attr.input, ctx.attr.tool]), tools = [ctx.attr.tool], outputs = [out], arguments = [args])
    return [DefaultInfo(files = depset([out]))]
stage = rule(implementation = _impl, attrs = {'input': attr.label(allow_single_file = True), 'tool': attr.label(allow_single_file = True)})
"#;

// Unrelated built-in MODULE dependencies are local declarations only, as in WP-7-30.
const BUILTIN_DEPENDENCIES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];
