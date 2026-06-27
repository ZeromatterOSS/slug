use slug_bzlmod_v2::BazelDep;
use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::LocalPathOverride;
use slug_bzlmod_v2::ModuleFile;

#[test]
fn parses_module_directives_in_order() {
    let parsed = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0", compatibility_level = 1)
bazel_dep(name = "dep", version = "1.0.0", repo_name = "dep_alias")
local_path_override(module_name = "dep", path = "../dep")
register_toolchains("//:toolchain")
register_execution_platforms("//:platform")
"#,
    )
    .unwrap();

    let module = parsed.module.unwrap();
    assert_eq!(module.name, "root");
    assert_eq!(module.version.as_deref(), Some("0.0.0"));
    assert_eq!(module.compatibility_level, Some(1));
    assert_eq!(parsed.directives.len(), 4);
    assert_eq!(
        parsed.directives[0],
        Directive::BazelDep(BazelDep {
            name: "dep".to_owned(),
            version: "1.0.0".to_owned(),
            repo_name: Some("dep_alias".to_owned()),
            dev_dependency: false,
        })
    );
    assert_eq!(
        parsed.directives[1],
        Directive::LocalPathOverride(LocalPathOverride {
            module_name: "dep".to_owned(),
            path: "../dep".to_owned(),
        })
    );
}

#[test]
fn parses_dev_dependency_flag() {
    let parsed =
        ModuleFile::parse(r#"bazel_dep(name = "dev", version = "1.0.0", dev_dependency = True)"#)
            .unwrap();
    let Directive::BazelDep(dep) = &parsed.directives[0] else {
        panic!("expected bazel_dep");
    };
    assert!(dep.dev_dependency);
}

#[test]
fn rejects_unsupported_directives() {
    let err = ModuleFile::parse("archive_override(module_name = \"dep\")").unwrap_err();
    assert!(err.contains("unsupported"));
}
