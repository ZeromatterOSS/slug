use std::collections::BTreeMap;

use slug_bzlmod_v2::ArchiveOverride;
use slug_bzlmod_v2::BazelDep;
use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::ExtensionTag;
use slug_bzlmod_v2::GitOverride;
use slug_bzlmod_v2::InjectRepo;
use slug_bzlmod_v2::LocalPathOverride;
use slug_bzlmod_v2::ModuleAttributeValue;
use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::MultipleVersionOverride;
use slug_bzlmod_v2::OverrideRepo;
use slug_bzlmod_v2::Registration;
use slug_bzlmod_v2::RepoImport;
use slug_bzlmod_v2::RepoRuleInvocation;
use slug_bzlmod_v2::SingleVersionOverride;
use slug_bzlmod_v2::UseExtension;
use slug_bzlmod_v2::UseRepo;
use slug_bzlmod_v2::UseRepoRule;

#[test]
fn parses_module_directives_in_order() {
    let parsed = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0", repo_name = "root_alias", compatibility_level = 1, bazel_compatibility = [">=9.0.0"])
include("//:deps.MODULE.bazel")
bazel_dep(name = "dep", version = "1.0.0", repo_name = "dep_alias")
local_path_override(module_name = "dep", path = "../dep")
register_toolchains("//:toolchain", "//:extra_toolchain", dev_dependency = True)
register_execution_platforms("//:platform", dev_dependency = False)
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
    assert_eq!(
        parsed.directives[3],
        Directive::RegisterToolchains(Registration {
            labels: vec!["//:toolchain".to_owned(), "//:extra_toolchain".to_owned()],
            dev_dependency: true,
        })
    );
    assert_eq!(
        parsed.directives[4],
        Directive::RegisterExecutionPlatforms(Registration {
            labels: vec!["//:platform".to_owned()],
            dev_dependency: false,
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
fn parses_extension_usage_directives() {
    let parsed = ModuleFile::parse(
        r#"
ext = use_extension("//:ext.bzl", "ext", dev_dependency = True, isolate = True)
ext.repo(name = "tagged", message = "hello")
use_repo(ext, "generated", tools = "tools_repo")
"#,
    )
    .unwrap();

    assert_eq!(
        parsed.directives[0],
        Directive::UseExtension(UseExtension {
            proxy_name: "ext".to_owned(),
            bzl_label: "//:ext.bzl".to_owned(),
            extension_name: "ext".to_owned(),
            dev_dependency: true,
            isolate: true,
        })
    );
    let mut tag_attrs = BTreeMap::new();
    tag_attrs.insert(
        "message".to_owned(),
        ModuleAttributeValue::String("hello".to_owned()),
    );
    tag_attrs.insert(
        "name".to_owned(),
        ModuleAttributeValue::String("tagged".to_owned()),
    );
    assert_eq!(
        parsed.directives[1],
        Directive::ExtensionTag(ExtensionTag {
            extension_proxy: "ext".to_owned(),
            tag_class: "repo".to_owned(),
            attrs: tag_attrs,
        })
    );
    assert_eq!(
        parsed.directives[2],
        Directive::UseRepo(UseRepo {
            extension_proxy: "ext".to_owned(),
            repos: vec![
                RepoImport {
                    apparent_name: "generated".to_owned(),
                    repo_name: "generated".to_owned(),
                },
                RepoImport {
                    apparent_name: "tools".to_owned(),
                    repo_name: "tools_repo".to_owned(),
                },
            ],
        })
    );
}

#[test]
fn parses_repo_rule_and_extension_repo_directives() {
    let parsed = ModuleFile::parse(
        r#"
repo = use_repo_rule("//:repo.bzl", "simple_repo", dev_dependency = True)
repo(name = "direct", filename = "direct.txt", patches = ["//:p.patch"], executable = False, strip = 1)
ext = use_extension("//:ext.bzl", "ext")
inject_repo(ext, "injected")
override_repo(ext, generated = "replacement")
"#,
    )
    .unwrap();

    assert_eq!(
        parsed.directives[0],
        Directive::UseRepoRule(UseRepoRule {
            proxy_name: "repo".to_owned(),
            bzl_label: "//:repo.bzl".to_owned(),
            rule_name: "simple_repo".to_owned(),
            dev_dependency: true,
        })
    );

    let mut attrs = BTreeMap::new();
    attrs.insert(
        "filename".to_owned(),
        ModuleAttributeValue::String("direct.txt".to_owned()),
    );
    attrs.insert(
        "patches".to_owned(),
        ModuleAttributeValue::StringList(vec!["//:p.patch".to_owned()]),
    );
    attrs.insert("executable".to_owned(), ModuleAttributeValue::Bool(false));
    attrs.insert("strip".to_owned(), ModuleAttributeValue::Integer(1));
    assert_eq!(
        parsed.directives[1],
        Directive::RepoRuleInvocation(RepoRuleInvocation {
            rule_proxy: "repo".to_owned(),
            repo_name: "direct".to_owned(),
            attrs,
        })
    );
    assert_eq!(
        parsed.directives[3],
        Directive::InjectRepo(InjectRepo {
            extension_proxy: "ext".to_owned(),
            repos: vec![RepoImport {
                apparent_name: "injected".to_owned(),
                repo_name: "injected".to_owned(),
            }],
        })
    );
    assert_eq!(
        parsed.directives[4],
        Directive::OverrideRepo(OverrideRepo {
            extension_proxy: "ext".to_owned(),
            repos: vec![RepoImport {
                apparent_name: "generated".to_owned(),
                repo_name: "replacement".to_owned(),
            }],
        })
    );
}

#[test]
fn rejects_unsupported_directives() {
    let err = ModuleFile::parse("workspace(name = \"old\")").unwrap_err();
    assert!(err.contains("unsupported"));
}
