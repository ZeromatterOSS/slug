use base64::Engine as _;
use sha2::Digest as _;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;

use super::*;

fn archive(payload: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(
        flate2::GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), flate2::Compression::default()),
    );
    let mut header = tar::Header::new_gnu();
    header.set_size(payload.len() as u64);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    tar.append_data(&mut header, "payload", payload).unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
#[ignore = "actual compiled CLI + original optional-checksum archive A/B/A; finite native supervisor required"]
fn native_original_optional_archive_is_fresh_across_a_b_a() {
    // Authored original Starlark uses the normal public repository API. The
    // archive/result/state below are produced by that invocation, not injected.
    // Bazel 9.2 download_and_extract returns the actual sha256 and integrity
    // when both expected checksums are omitted (upstream 8220c6198837).
    let input = tempfile::tempdir().unwrap();
    let input_path = input.path().join("source.tar.gz");
    let url = url::Url::from_file_path(&input_path).unwrap();
    let source = format!(
        r#"
def _repository(ctx):
    result = ctx.download_and_extract({url:?}, type="tar.gz")
    payload = ctx.read("payload")
    ctx.file("out", result.sha256 + "|" + result.integrity + "|" + payload, executable=False)
    ctx.file("BUILD.bazel", "exports_files(['out'])", executable=False)
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#,
        url = url.as_str(),
    );
    let (workspace, policy) = original_workspace(&source, "out");
    let original_module = std::fs::read(workspace.path().join("MODULE.bazel")).unwrap();
    let original_source = std::fs::read(workspace.path().join("ext.bzl")).unwrap();
    let a = archive(b"payload:A\n");
    let b = archive(b"payload:B\n");
    assert_ne!(a, b);
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut bindings = Vec::new();
    let mut identities = Vec::new();
    for (ordinal, (compressed, payload)) in [
        (a.as_slice(), b"payload:A\n".as_slice()),
        (b.as_slice(), b"payload:B\n".as_slice()),
        (a.as_slice(), b"payload:A\n".as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        // Same URL, same authored source and module, no distdir/cache input.
        std::fs::write(&input_path, compressed).unwrap();
        let digest: [u8; 32] = sha2::Sha256::digest(compressed).into();
        let sri = format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        );
        let expected = format!(
            "{}|{}|{}",
            hex::encode(digest),
            sri,
            std::str::from_utf8(payload).unwrap()
        );
        let accepted = runtime
            .cquery_command_with_bzlmod_inputs(
                "//:use",
                true,
                true,
                policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                CommandConfigurationOverlay::new(Arc::from([])),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "optional archive command {ordinal} failed at {}: {error}; bank={:?}",
                    runtime.native_demand_sessions.workspace_stage(),
                    resources.snapshot()
                )
            });
        assert!(
            accepted.terminal_for_test().as_ref().is_ok(),
            "{:#?}",
            accepted.terminal_for_test()
        );
        let state = runtime.native_demand_sessions.state.lock().unwrap();
        assert!(matches!(state.phase, NativeDemandLeasePhase::Idle));
        let requests = state.accepted.selected.repository_requests();
        let realizations = requests
            .iter()
            .filter_map(|request| match &request.kind {
                RepositoryMaterializationKind::GeneralWorkspace(realization) => Some(realization),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(realizations.len(), 1);
        let realization = realizations[0];
        let reservation = realization.reservation();
        bindings.push((
            reservation.lease(),
            reservation.instance(),
            reservation.display_root().clone(),
        ));
        assert!(!reservation.display_root().as_path().exists());
        let archives = realization
            .transcript()
            .records()
            .iter()
            .filter(|record| {
                matches!(
                    record.request().operation(),
                    RepositoryWorkspaceOperation::Archive(_)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(archives.len(), 1);
        let RepositoryWorkspaceOperation::Archive(effect) = archives[0].request().operation()
        else {
            unreachable!()
        };
        assert_eq!(effect.expected_sha256(), None);
        assert_eq!(effect.urls().len(), 1);
        assert_eq!(effect.urls()[0].as_str(), url.as_str());
        assert_eq!(
            archives[0].result(),
            &RepositoryWorkspaceResult::Archive(digest)
        );
        let RepositoryWorkspaceNode::File(extracted) =
            archives[0].next().entry("payload").unwrap().node()
        else {
            panic!("actual extracted regular payload")
        };
        assert_eq!(extracted.as_ref(), payload);
        let reads = realization
            .transcript()
            .records()
            .iter()
            .filter(|record| {
                matches!(
                    record.request().operation(),
                    RepositoryWorkspaceOperation::Read(_)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 1);
        assert_eq!(
            reads[0].request().predecessor(),
            archives[0].next(),
            "read belongs to this command's actual extracted state"
        );
        let RepositoryWorkspaceResult::Read(read) = reads[0].result() else {
            panic!("actual observed Read result")
        };
        assert_eq!(read.as_ref(), payload);
        let RepositoryWorkspaceNode::File(out) =
            realization.final_state().entry("out").unwrap().node()
        else {
            panic!("actual output file")
        };
        assert_eq!(out.as_ref(), expected.as_bytes());
        identities.push(realization.source_identity().clone());
        assert_eq!(
            std::fs::read(workspace.path().join("MODULE.bazel")).unwrap(),
            original_module
        );
        assert_eq!(
            std::fs::read(workspace.path().join("ext.bzl")).unwrap(),
            original_source
        );
        drop(state);
        drop(accepted);
    }
    for (index, binding) in bindings.iter().enumerate() {
        for other in &bindings[..index] {
            assert_ne!(binding.0, other.0);
            assert_ne!(binding.1, other.1);
            assert_eq!(binding.2, other.2);
        }
    }
    assert_ne!(identities[0], identities[1]);
    assert_eq!(identities[0], identities[2]);
    drop(runtime);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
}
