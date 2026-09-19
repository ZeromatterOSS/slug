//! Accepted manifest handles own native repository generations independently of DICE.

use slug_workspace_v2::PathObservationNamespace;

use super::runfiles_manifest_tests::check_frontier;
use super::runfiles_manifest_tests::check_no_effects;
use super::runfiles_manifest_tests::owner;
use super::runfiles_manifest_tests::prepare;
use super::runfiles_manifest_tests::write_binary;
use super::*;
use crate::runtime::PreparedRunfilesManifests;

fn check_source(value: &PreparedRunfilesManifests, expected: &[u8]) {
    assert_eq!(value.sources().len(), 1);
    let source = value.sources().next().unwrap();
    assert_eq!(
        source.label(),
        &CanonicalLabel::parse("@@+repo+generated//:input").unwrap()
    );
    assert!(matches!(
        source.namespace(),
        PathObservationNamespace::Materialization(_)
    ));
    assert_eq!(source.requested_path(), source.real_path());
    assert_eq!(
        fs::read(source.requested_path().as_path()).unwrap(),
        expected
    );
    assert_eq!(
        source.digest().sha256().as_slice(),
        Sha256::digest(expected).as_slice()
    );
    assert_eq!(source.digest().size_bytes(), expected.len() as u64);
    let manifest = String::from_utf8(value.source_manifest_bytes().unwrap()).unwrap();
    assert!(
        manifest.lines().any(|line| line
            == format!(
                "+repo+generated/input {}",
                source.requested_path().as_path().display()
            )),
        "{manifest}"
    );
    assert!(
        String::from_utf8(value.repo_mapping_manifest_bytes().unwrap())
            .unwrap()
            .lines()
            .any(|line| line == ",generated,+repo+generated")
    );
}

#[test]
fn native_materialized_runfiles_generations_survive_restore_and_runtime_shutdown() {
    let workspace = Workspace::new();
    fs::write(
        workspace.root.path().join("repo.bzl"),
        r#"def _impl(ctx):
    ctx.file("BUILD.bazel", "exports_files(['input'])\n")
    ctx.file("input", ctx.attr.content)
repo = repository_rule(implementation = _impl, attrs = {"content": attr.string()})
"#,
    )
    .unwrap();
    let module = fs::read_to_string(workspace.root.path().join("MODULE.bazel")).unwrap();
    let write_module = |content| {
        fs::write(
            workspace.root.path().join("MODULE.bazel"),
            format!("{module}\nrepo = use_repo_rule('//:repo.bzl', 'repo')\nrepo(name = 'generated', content = '{content}')\n"),
        )
        .unwrap();
    };
    write_module("A");
    write_binary(&workspace, "data_a", "@generated//:input");
    let key = owner(&workspace, Default::default());
    let mut held = Vec::new();
    let mut manifests = Vec::new();
    for (index, content) in ["A", "A", "B", "A"].into_iter().enumerate() {
        // The second request is unchanged, including MODULE metadata.
        if index >= 2 {
            write_module(content);
        }
        let accepted = prepare(&workspace, key.clone(), Default::default()).unwrap();
        let value = Arc::clone(accepted.terminal_for_test());
        drop(accepted);
        check_source(&value, content.as_bytes());
        check_frontier(&workspace, &value);
        check_no_effects(&workspace, &value);
        manifests.push(value.source_manifest_bytes().unwrap());
        held.push(value);
        for (old, bytes) in held.iter().zip(&manifests) {
            assert_eq!(old.source_manifest_bytes().unwrap(), *bytes);
        }
    }
    let source = |index: usize| held[index].sources().next().unwrap();
    assert_eq!(
        source(0),
        source(1),
        "warm preparation must reuse its generation"
    );
    assert_ne!(source(0).namespace(), source(2).namespace());
    assert_ne!(source(0).requested_path(), source(2).requested_path());
    assert_ne!(source(0).digest(), source(2).digest());
    assert_eq!(source(0).digest(), source(3).digest());
    // Content restoration need not reuse the original physical generation.
    let roots = held
        .iter()
        .map(|value| {
            value
                .sources()
                .next()
                .unwrap()
                .requested_path()
                .as_path()
                .parent()
                .unwrap()
                .to_path_buf()
        })
        .collect::<Vec<_>>();
    let survivors = held.clone();
    drop(held);
    let Workspace { root, runtime } = workspace;
    drop(runtime);
    for ((value, content), bytes) in survivors.iter().zip(["A", "A", "B", "A"]).zip(&manifests) {
        check_source(value, content.as_bytes());
        assert_eq!(value.source_manifest_bytes().unwrap(), *bytes);
    }
    drop(survivors);
    for generation in roots {
        assert_eq!(
            fs::symlink_metadata(&generation).unwrap_err().kind(),
            std::io::ErrorKind::NotFound,
            "unowned generation remains: {}",
            generation.display()
        );
    }
    // Keep the authored workspace alive until the generation cleanup assertion.
    assert!(root.path().is_dir());
}
