//! Authentic cc_autoconf through the original route. Source fixture assembly
//! reuses the verified archive/registry corpus; no repository effects are authored.
use std::io::Read;
use std::path::Path;
use std::process::Command;

use sha2::Digest;
use slug_bzlmod_v2::RepositoryEnvironmentEntry;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositorySourceSnapshotKind;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::*;

fn verified_source_fixture(parent: &Path) -> std::path::PathBuf {
    // Same closed, independently pinned fixture assembler as CLI's configured
    // tests. Python authors test inputs only; Slug acquisition/evaluation/setup
    // and arbitrary compiler execution remain the production Rust path.
    let destination = parent.join("configured-inputs");
    let output = Command::new("python3")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/v2_oracle/configured_cli_fixture.py"),
        )
        .arg("assemble")
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "verified fixture assembly failed: {output:?}"
    );
    destination
}

// Reuse the reviewed archive/member inventory used by the coupled authentic
// callback fixture. MODULE/BUILD metadata below is authored fixture topology;
// only the two unchanged provider source bodies are copied from the release.
fn prepare_protobuf_sources(workspace: &Path, inputs: &Path) {
    const ROWS: &[[&str; 5]] = &include!("../tests/rust_toolchain_callback_sources.json");
    let archive_row = ROWS
        .iter()
        .find(|row| row[0] == "archive" && row[1] == "protobuf")
        .unwrap();
    let relative = archive_row[2]
        .strip_prefix("tests/v2_oracle/fixtures/configured-cli-authentic/")
        .unwrap();
    let source = std::fs::File::open(inputs.join(relative)).unwrap();
    let size = archive_row[4].parse::<usize>().unwrap();
    assert_eq!(source.metadata().unwrap().len(), size as u64);
    let mut bytes = Vec::with_capacity(size);
    source
        .take(size as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), size);
    assert_eq!(hex::encode(sha2::Sha256::digest(&bytes)), archive_row[3]);
    let expected = ROWS
        .iter()
        .filter(|row| row[0] == "protobuf")
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 2, "existing reviewed provider member pair");
    let mut found = [false; 2];
    let root = workspace.join(".general_test_modules/protobuf");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().into_owned();
        let Some(index) = expected.iter().position(|row| name == Path::new(row[1])) else {
            continue;
        };
        assert!(!found[index], "duplicate authenticated archive member");
        let row = expected[index];
        let size = row[4].parse::<usize>().unwrap();
        assert!(entry.header().entry_type().is_file());
        assert_eq!(entry.header().size().unwrap(), size as u64);
        let mut body = Vec::with_capacity(size);
        entry
            .by_ref()
            .take(size as u64 + 1)
            .read_to_end(&mut body)
            .unwrap();
        assert_eq!(body.len(), size);
        assert_eq!(hex::encode(sha2::Sha256::digest(&body)), row[3]);
        let relative = Path::new(row[2].strip_prefix("protobuf/").unwrap());
        assert!(
            relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        );
        let destination = root.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, body).unwrap();
        std::fs::write(destination.parent().unwrap().join("BUILD.bazel"), "").unwrap();
        found[index] = true;
    }
    assert!(found.into_iter().all(|found| found));
    std::fs::write(root.join("MODULE.bazel"),"module(name='protobuf',version='33.4')\nbazel_dep(name='bazel_features',version='1.42.1',repo_name='proto_bazel_features')\n").unwrap();
}

#[test]
#[ignore = "actual compiled CLI + unchanged authentic cc_autoconf + native compiler; supervised Linux gate required"]
fn native_authentic_cc_autoconf_source_links_load_bzl_and_execute_original_probes() {
    let (workspace, policy) = original_workspace("", "empty");
    let inputs = verified_source_fixture(workspace.path());
    prepare_protobuf_sources(workspace.path(), &inputs);
    let module = std::fs::read_to_string(workspace.path().join("MODULE.bazel")).unwrap();
    // Only the fixture's authored header overrides are removed. Ordinary BCR
    // selection, actual source integrity/patches and original rule loading run.
    let mut module = module
        .lines()
        .filter(|line| {
            *line != "e=use_extension('//:ext.bzl','ext')"
                && *line != "use_repo(e, proof='proof')"
                && !["rules_cc", "bazel_features", "bazel_skylib"]
                    .iter()
                    .any(|name| {
                        line.starts_with(&format!("local_path_override(module_name='{name}',"))
                    })
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Select the authenticated Skylib release whose source is present in the
    // existing fixture, through ordinary MVS rather than an authored stub.
    module.push_str("\nbazel_dep(name='rules_cc',version='0.2.17')\nbazel_dep(name='bazel_skylib',version='1.8.2')\nrepo=use_repo_rule('@rules_cc//cc/private/toolchain:cc_configure.bzl','cc_autoconf')\nrepo(name='proof')\n");
    std::fs::write(workspace.path().join("MODULE.bazel"), module).unwrap();
    std::fs::remove_file(workspace.path().join("ext.bzl")).unwrap();
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let environment = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
        RepositoryEnvironmentEntry::new("CC", "/usr/bin/gcc"),
        RepositoryEnvironmentEntry::new("PATH", "/usr/bin:/bin"),
    ]))
    .unwrap();
    let archive = inputs.join("mirror/github.com/bazelbuild/rules_cc/releases/download/0.2.17");
    let distdirs = crate::runtime::ArchiveDistdirPolicy::from_request(
        workspace.path(),
        &[archive.to_str().unwrap().to_owned()],
    )
    .unwrap();
    let registries = [url::Url::from_directory_path(inputs.join("registry"))
        .unwrap()
        .to_string()];
    let accepted = runtime
        .cquery_command_with_repository_environment_and_distdirs(
            "//:use",
            true,
            true,
            policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &registries,
            environment,
            CommandConfigurationOverlay::new(Arc::from([])),
            distdirs,
        )
        .unwrap_or_else(|error| {
            panic!(
                "authentic cc_autoconf stopped at {}: {error}; resources={:?}",
                runtime.native_demand_sessions.workspace_stage(),
                resources.snapshot()
            )
        });
    assert!(
        accepted.terminal_for_test().as_ref().is_ok(),
        "{:#?}",
        accepted.terminal_for_test()
    );
    {
        let state = runtime.native_demand_sessions.state.lock().unwrap();
        let request = state
            .accepted
            .selected
            .repository_requests()
            .iter()
            .find(|request| {
                matches!(
                    &request.kind,
                    RepositoryMaterializationKind::GeneralWorkspace(_)
                )
            })
            .expect("original cc repository receipt");
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
            unreachable!()
        };
        let mut source_links = 0;
        let mut actual_processes = 0;
        for record in realization.transcript().records() {
            if let RepositoryWorkspaceOperation::Symlink {
                path,
                target,
                source: Some(source),
            } = record.request().operation()
            {
                if matches!(
                    path.as_str(),
                    "cc_toolchain_config.bzl" | "armeabi_cc_toolchain_config.bzl"
                ) {
                    source_links += 1;
                    assert_eq!(
                        source
                            .source_state()
                            .unwrap()
                            .reservation()
                            .request()
                            .kind(),
                        RepositorySourceSnapshotKind::Immutable
                    );
                    let member = if path == "cc_toolchain_config.bzl" {
                        "unix_cc_toolchain_config.bzl"
                    } else {
                        "armeabi_cc_toolchain_config.bzl"
                    };
                    assert!(target.as_path().ends_with(member));
                    assert!(
                        matches!(realization.final_state().entry(path).unwrap().node(),RepositoryWorkspaceNode::Link(text) if Some(text.as_str())==target.as_path().to_str())
                    );
                }
            }
            if let RepositoryWorkspaceResult::Execute(result) = record.result() {
                actual_processes += 1;
                assert!((0..=255).contains(&result.return_code()));
            }
        }
        assert_eq!(source_links, 2, "both unchanged upstream symlink calls");
        assert!(
            actual_processes > 0,
            "original compiler probes must actually execute"
        );
        let bindings = realization.final_state().source_bindings();
        assert!(!bindings.is_empty());
        // The requested generated package loads both linked .bzl files from its
        // unchanged BUILD template. Its certificate must retain terminal Source
        // bytes, with logical generated-repository ownership kept separately.
        for member in [
            "unix_cc_toolchain_config.bzl",
            "armeabi_cc_toolchain_config.bzl",
        ] {
            assert!(
                state
                    .accepted
                    .path_observations
                    .observations()
                    .iter()
                    .any(
                        |(demand, _)| demand.operation() == PathObservationOperation::FileBytes
                            && demand.path().as_path().ends_with(member)
                            && bindings.iter().any(|view| demand.namespace()
                                == PathObservationNamespace::Materialization(view.instance()))
                    ),
                "normal linked .bzl loading must observe Source bytes: {member}"
            );
        }
    }
    drop(accepted);
    drop(runtime);
    let snapshot = resources.snapshot();
    assert_eq!(snapshot.native_fds, 0);
    assert_eq!(snapshot.backing_bytes, 0);
    assert_eq!(snapshot.retained_bytes, baseline);
}
