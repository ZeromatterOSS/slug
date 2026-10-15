use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::ModuleFile;

use super::*;

#[test]
fn complete_original_source_batch_keeps_bodies_and_dependency_aliases() {
    let root = super::super::super::scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    let repositories = super::super::prepare(&root);
    for &(repo, relative, source) in members::SOURCES {
        assert_eq!(
            fs::read(root.join(repo).join(relative)).unwrap(),
            source.as_bytes(),
            "{repo}/{relative}"
        );
        assert!(
            repositories
                .iter()
                .any(|(name, path)| name == &format!("{repo}+") && path == repo)
        );
    }
    for (repo, dependency, alias) in [
        ("rules_cc", "protobuf", "com_google_protobuf"),
        ("protobuf", "bazel_features", "proto_bazel_features"),
        (
            "bazel_features",
            "bazel_features_globals",
            "bazel_features_globals",
        ),
        (
            "bazel_features",
            "bazel_features_version",
            "bazel_features_version",
        ),
        ("cc_compatibility_proxy", "rules_cc", "rules_cc"),
        ("bazel_skylib", "rules_license", "rules_license"),
    ] {
        let text = fs::read_to_string(root.join(repo).join("MODULE.bazel")).unwrap();
        let module = ModuleFile::parse(&text).unwrap();
        assert!(module.directives.iter().any(|d| matches!(d, Directive::BazelDep(dep) if dep.name == dependency && dep.repo_name.as_deref().unwrap_or(dependency) == alias)), "{repo}: {text}");
    }
    assert_eq!(
        fs::read_to_string(root.join("bazel_features_version/version.bzl")).unwrap(),
        "version = '9.2.0'"
    );
    assert!(
        fs::read_to_string(root.join("cc_compatibility_proxy/proxy.bzl"))
            .unwrap()
            .contains("@rules_cc//cc/private/rules_impl:cc_binary.bzl")
    );
}

#[test]
fn authored_local_sources_versions_and_nonlocal_overrides_stay_authoritative() {
    let root = super::super::super::scratch();
    let authored = "module(name='custom')\nbazel_dep(name='rules_cc',version='99')\nlocal_path_override(module_name='rules_cc',path='custom_cc')\nbazel_dep(name='protobuf',version='88')\narchive_override(module_name='protobuf',urls=['https://invalid.example/proto.tar.gz'])\n";
    fs::write(root.join("MODULE.bazel"), authored).unwrap();
    // An existing feature MODULE may intentionally have no helper-owned aux
    // edges. Root-owned fixture dependencies must keep their overrides used.
    fs::create_dir_all(root.join("bazel_features")).unwrap();
    let features = "module(name='bazel_features', version='1.42.1')\n# authored minimal fixture\n";
    fs::write(root.join("bazel_features/MODULE.bazel"), features).unwrap();
    fs::create_dir_all(root.join("custom_cc/cc")).unwrap();
    let module =
        "module(name='rules_cc',version='99')\n# caller owns all dependency declarations\n";
    fs::write(root.join("custom_cc/MODULE.bazel"), module).unwrap();
    let program = "# caller authored source\n";
    fs::write(root.join("custom_cc/cc/cc_library.bzl"), program).unwrap();
    let repositories = super::super::prepare(&root);
    assert!(
        fs::read_to_string(root.join("MODULE.bazel"))
            .unwrap()
            .starts_with(authored)
    );
    assert_eq!(
        fs::read_to_string(root.join("custom_cc/MODULE.bazel")).unwrap(),
        module
    );
    assert_eq!(
        fs::read_to_string(root.join("custom_cc/cc/cc_library.bzl")).unwrap(),
        program
    );
    assert_eq!(
        fs::read_to_string(root.join("bazel_features/MODULE.bazel")).unwrap(),
        features
    );
    let root_module =
        ModuleFile::parse(&fs::read_to_string(root.join("MODULE.bazel")).unwrap()).unwrap();
    for (name, version) in EXTRA_MODULES {
        assert!(root_module.directives.iter().any(|d| matches!(d, Directive::BazelDep(dep) if dep.name == *name && dep.version == *version)), "auxiliary fixture module {name} must be root-reachable");
    }
    assert!(!root.join("protobuf").exists());
    assert!(!repositories.iter().any(|(name, _)| name == "protobuf+"));
    assert!(
        repositories
            .iter()
            .any(|(name, path)| name == "rules_cc+" && path == "custom_cc")
    );
}
