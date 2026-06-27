use slug_bzlmod_v2::ArchiveOverride;
use slug_bzlmod_v2::BazelDep;
use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::GitOverride;
use slug_bzlmod_v2::LocalPathOverride;
use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::MultipleVersionOverride;
use slug_bzlmod_v2::SingleVersionOverride;

#[test]
fn parses_module_directives_in_order() {
    let parsed = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0", repo_name = "root_alias", compatibility_level = 1, bazel_compatibility = [">=9.0.0"])
include("//:deps.MODULE.bazel")
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
    assert_eq!(module.repo_name.as_deref(), Some("root_alias"));
    assert_eq!(module.compatibility_level, Some(1));
    assert_eq!(module.bazel_compatibility, vec![">=9.0.0".to_owned()]);
    assert_eq!(parsed.directives.len(), 5);
    assert_eq!(
        parsed.directives[0],
        Directive::Include("//:deps.MODULE.bazel".to_owned())
    );
    assert_eq!(
        parsed.directives[1],
        Directive::BazelDep(BazelDep {
            name: "dep".to_owned(),
            version: "1.0.0".to_owned(),
            repo_name: Some("dep_alias".to_owned()),
            dev_dependency: false,
        })
    );
    assert_eq!(
        parsed.directives[2],
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
fn parses_override_directives_with_lists() {
    let parsed = ModuleFile::parse(
        r#"
single_version_override(module_name = "dep", version = "2.0.0", registry = "file:///registry", patches = ["//:dep.patch"], patch_cmds = ["echo patched"], patch_strip = 1)
multiple_version_override(module_name = "dep", versions = ["1.0.0", "2.0.0"], registry = "file:///registry")
archive_override(module_name = "archive", urls = ["file:///a.zip", "https://example.invalid/a.zip"], integrity = "sha256-abc", strip_prefix = "src", patches = ["//:a.patch"], patch_strip = 2)
git_override(module_name = "git", remote = "https://example.invalid/repo.git", commit = "0123456789abcdef0123456789abcdef01234567", shallow_since = "2026-06-01", patches = ["//:g.patch"], patch_strip = 3)
"#,
    )
    .unwrap();

    assert_eq!(
        parsed.directives[0],
        Directive::SingleVersionOverride(SingleVersionOverride {
            module_name: "dep".to_owned(),
            version: "2.0.0".to_owned(),
            registry: Some("file:///registry".to_owned()),
            patches: vec!["//:dep.patch".to_owned()],
            patch_cmds: vec!["echo patched".to_owned()],
            patch_strip: 1,
        })
    );
    assert_eq!(
        parsed.directives[1],
        Directive::MultipleVersionOverride(MultipleVersionOverride {
            module_name: "dep".to_owned(),
            versions: vec!["1.0.0".to_owned(), "2.0.0".to_owned()],
            registry: Some("file:///registry".to_owned()),
        })
    );
    assert_eq!(
        parsed.directives[2],
        Directive::ArchiveOverride(ArchiveOverride {
            module_name: "archive".to_owned(),
            urls: vec![
                "file:///a.zip".to_owned(),
                "https://example.invalid/a.zip".to_owned(),
            ],
            integrity: Some("sha256-abc".to_owned()),
            strip_prefix: Some("src".to_owned()),
            patches: vec!["//:a.patch".to_owned()],
            patch_strip: 2,
        })
    );
    assert_eq!(
        parsed.directives[3],
        Directive::GitOverride(GitOverride {
            module_name: "git".to_owned(),
            remote: "https://example.invalid/repo.git".to_owned(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            shallow_since: Some("2026-06-01".to_owned()),
            patches: vec!["//:g.patch".to_owned()],
            patch_strip: 3,
        })
    );
}

#[test]
fn rejects_unsupported_directives() {
    let err = ModuleFile::parse("workspace(name = \"old\")").unwrap_err();
    assert!(err.contains("unsupported"));
}
