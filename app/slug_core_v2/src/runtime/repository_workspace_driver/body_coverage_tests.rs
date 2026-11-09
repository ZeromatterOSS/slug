//! A complete original repository command at a scaled native admission boundary.
//! This is a retained-credit discriminator, never a whole-command RSS bound.
use sha2::Digest as _;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;

use super::*;

const BODY_BYTES: usize = 48 * 1024 * 1024;
const BANK_BYTES: usize = 64 * 1024 * 1024;

fn archive() -> Vec<u8> {
    let mut tar = tar::Builder::new(
        flate2::GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), flate2::Compression::fast()),
    );
    for (path, body) in [
        ("large", vec![0x6d; BODY_BYTES]),
        ("small", b"actual small body\n".to_vec()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        tar.append_data(&mut header, path, body.as_slice()).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
#[ignore = "actual compiled CLI and complete original large-archive ownership path; finite native supervisor required"]
fn native_original_archive_above_half_bank_keeps_full_history_through_publication_and_seal() {
    assert!(BODY_BYTES > BANK_BYTES / 2 && BODY_BYTES < BANK_BYTES);
    let input = tempfile::tempdir().unwrap();
    let path = input.path().join("observed.tar.gz");
    let bytes = archive();
    let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
    std::fs::write(&path, &bytes).unwrap();
    let url = url::Url::from_file_path(&path).unwrap();
    let source = format!(
        r#"
def _repository(ctx):
    result = ctx.download_and_extract({url:?}, type="tar.gz")
    value = ctx.read("small")
    ctx.file("out", result.sha256 + "|" + value, executable=False)
    ctx.file("BUILD.bazel", "exports_files(['out'])", executable=False)
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#,
        url = url.as_str(),
    );
    let (workspace, policy) = original_workspace(&source, "out");
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(BANK_BYTES, 512, 8 * 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    let baseline = resources.snapshot().retained_bytes;
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let accepted = runtime
        .cquery_command_with_bzlmod_inputs(
            "//:use",
            true,
            true,
            policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            CommandConfigurationOverlay::new(Arc::from([])),
        )
        .unwrap_or_else(|error| {
            panic!(
                "full archive command failed at {}: {error}; bank={:?}",
                runtime.native_demand_sessions.workspace_stage(),
                resources.snapshot()
            )
        });
    assert!(accepted.terminal_for_test().as_ref().is_ok());
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
    let records = realization.transcript().records();
    assert_eq!(records.len(), 4);
    assert!(
        matches!(records[0].request().operation(), RepositoryWorkspaceOperation::Archive(effect) if effect.expected_sha256().is_none())
    );
    assert_eq!(
        records[0].result(),
        &RepositoryWorkspaceResult::Archive(digest)
    );
    assert!(
        matches!(records[1].request().operation(), RepositoryWorkspaceOperation::Read(path) if path == "small")
    );
    let RepositoryWorkspaceNode::File(large) =
        realization.final_state().entry("large").unwrap().node()
    else {
        panic!("complete final state must retain the large file")
    };
    assert_eq!(large.len(), BODY_BYTES);
    for index in [0, BODY_BYTES / 2, BODY_BYTES - 1] {
        assert_eq!(large[index], 0x6d);
    }
    for record in records {
        let RepositoryWorkspaceNode::File(historical) =
            record.next().entry("large").unwrap().node()
        else {
            panic!("every historical state must retain its complete large body")
        };
        assert!(Arc::ptr_eq(large, historical));
    }
    let RepositoryWorkspaceNode::File(out) = realization.final_state().entry("out").unwrap().node()
    else {
        panic!("actual Starlark output must be retained")
    };
    assert_eq!(
        out.as_ref(),
        format!("{}|actual small body\n", hex::encode(digest)).as_bytes()
    );
    let peak = resources.snapshot().peak_retained_bytes;
    assert!(peak >= BODY_BYTES && peak <= BANK_BYTES);
    eprintln!(
        "NativeBodyCoverageOriginal body={BODY_BYTES} bank={BANK_BYTES} peak={peak} records={}",
        records.len()
    );
    drop(state);
    drop(accepted);
    drop(runtime);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
}
