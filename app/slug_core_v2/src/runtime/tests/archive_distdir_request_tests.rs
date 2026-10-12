use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use base64::Engine;
use sha2::Digest;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::SourcePreparationNeeds;

use super::*;
use crate::runtime::ArchiveDistdirPolicy;
use crate::runtime::ProcessHostOwner;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;
use crate::runtime::dice::source_service::WorkspaceSourceServices;

fn request_fixture(root: &Path) -> Arc<RepositoryMaterializationRequest> {
    let module = b"module(name = 'distdir_probe')\n";
    fs::write(root.join("MODULE.bazel"), module).unwrap();
    fs::create_dir(root.join("a")).unwrap();
    fs::create_dir(root.join("b")).unwrap();
    let mut tar = tar::Builder::new(Vec::new());
    let payload = b"verified request-local bytes\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(payload.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "payload.txt", &payload[..])
        .unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&tar.into_inner().unwrap()).unwrap();
    let archive = gzip.finish().unwrap();
    fs::write(root.join("a/probe.tar.gz"), &archive).unwrap();
    fs::write(root.join("b/probe.tar.gz"), b"wrong source bytes").unwrap();
    let string = |s: String| OverrideAttributeValue::String(s.into());
    let sri = |b: &[u8]| {
        string(format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(b))
        ))
    };
    let list = |s: String| OverrideAttributeValue::Iterable(Arc::new([string(s)]));
    let empty = || OverrideAttributeValue::Map(Arc::new(SmallMap::new()));
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new(root).unwrap(),
            canonical_repo: slug_identity_v2::CanonicalRepoName::new("probe+").unwrap(),
        },
        kind: RepositoryMaterializationKind::Immutable,
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                    .unwrap(),
                rule_name: "http_archive".into(),
            },
            attributes: Arc::new(SmallMap::from_iter([
                (
                    compact_str::CompactString::new("urls"),
                    list("https://archive.invalid/probe.tar.gz".into()),
                ),
                ("integrity".into(), sri(&archive)),
                ("strip_prefix".into(), string(String::new())),
                ("remote_patches".into(), empty()),
                ("remote_file_urls".into(), empty()),
                ("remote_file_integrity".into(), empty()),
                ("remote_patch_strip".into(), OverrideAttributeValue::Int(0)),
                (
                    "remote_module_file_urls".into(),
                    list(
                        url::Url::from_file_path(root.join("MODULE.bazel"))
                            .unwrap()
                            .to_string(),
                    ),
                ),
                ("remote_module_file_integrity".into(), sri(module)),
            ])),
        },
    })
}

fn inputs(root: &Path, paths: &[&str]) -> NativeDemandRequestInputBundle {
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.archive_distdirs = ArchiveDistdirPolicy::from_request(
        root,
        &paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>(),
    )
    .unwrap();
    inputs
}

fn materialized_root(
    command: &NativeDemandAbortGuard<'_>,
    request: &RepositoryMaterializationRequest,
) -> PathBuf {
    let command = command.command();
    match command
        .runtime
        .repository_materializer
        .active_result_for_test(
            command.repository_session,
            request.id.canonical_repo.as_str(),
        ) {
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            generation_root,
            ..
        }) => generation_root,
        result => panic!("expected real extracted repository: {result:?}"),
    }
}

#[cfg(target_os = "linux")]
#[test]
fn native_distdir_inputs_are_command_local_busy_and_restore() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let request = request_fixture(&root);
    let deadline = Instant::now() + Duration::from_secs(20);
    let scope = SourceServiceScope::new_for_test(
        request.id.workspace.clone(),
        deadline,
        deadline + Duration::from_secs(2),
        SourceServiceLimits {
            allow_network: false,
            ..Default::default()
        },
    )
    .unwrap();
    let runtime = WorkspaceRuntime::new_with_source_services(
        &root,
        ProcessHostOwner::native(),
        WorkspaceSourceServices::Diagnostic(scope.clone()),
    )
    .unwrap();
    let prior = accepted_native_snapshot(&runtime);
    let preflight = runtime
        .begin_native_demand_command_with_inputs(inputs(&root, &["a"]))
        .unwrap();
    let counters = (
        runtime.next_revision.load(Ordering::Relaxed),
        runtime.next_registry_generation.load(Ordering::Relaxed),
        runtime
            .next_repository_materialization_generation
            .load(Ordering::Relaxed),
    );
    assert!(matches!(
        runtime.begin_native_demand_command_with_inputs(inputs(&root, &["b"])),
        Err(NativeDemandSessionError::Busy)
    ));
    assert_eq!(
        counters,
        (
            runtime.next_revision.load(Ordering::Relaxed),
            runtime.next_registry_generation.load(Ordering::Relaxed),
            runtime
                .next_repository_materialization_generation
                .load(Ordering::Relaxed)
        )
    );
    let mut command = NativeDemandAbortGuard::new(preflight.into_command());
    let need = SourcePreparationNeeds::repository(request.as_ref().clone());
    assert_eq!(
        command.progress(&need).unwrap(),
        NativeDemandProgress::Repositories
    );
    let provisional = materialized_root(&command, &request);
    assert_eq!(
        fs::read(provisional.join("payload.txt")).unwrap(),
        b"verified request-local bytes\n"
    );
    command.discard().unwrap();
    assert!(!provisional.exists());
    assert_current_native_snapshot(&runtime, &prior);

    let mut command = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(inputs(&root, &["a"]))
            .unwrap()
            .into_command(),
    );
    command.progress(&need).unwrap();
    let retained = materialized_root(&command, &request);
    command
        .accept_selected_for_test(SelectedWorkspaceDemands::for_test(
            Arc::from([request.clone()]),
            Arc::from([]),
        ))
        .unwrap();
    let accepted = accepted_native_snapshot(&runtime);
    // Different physical source policy cannot invalidate equal semantic bytes.
    let warm = runtime
        .begin_native_demand_command_with_inputs(inputs(&root, &["b"]))
        .unwrap();
    assert_eq!(warm.repository_results(), &accepted.repository_results);
    let mut warm = NativeDemandAbortGuard::new(warm.into_command());
    assert_eq!(materialized_root(&warm, &request), retained);
    warm.accept_selected_for_test(SelectedWorkspaceDemands::for_test(
        Arc::from([request.clone()]),
        Arc::from([]),
    ))
    .unwrap();
    let empty = runtime.begin_native_demand_command().unwrap();
    assert!(
        empty
            .command
            .inputs
            .request
            .archive_distdirs
            .paths()
            .is_empty()
    );
    assert_eq!(empty.repository_results(), &accepted.repository_results);
    NativeDemandAbortGuard::new(empty.into_command())
        .discard()
        .unwrap();

    // A real local MODULE overlay failure after a verified archive hit stays
    // recoverable; NetworkDisabled remains armed against any HTTPS fallback.
    let accepted = accepted_native_snapshot(&runtime);
    let mut fresh = request.as_ref().clone();
    fresh.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("fresh+").unwrap();
    let fresh = Arc::new(fresh);
    fs::remove_file(root.join("MODULE.bazel")).unwrap();
    let mut failed = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(inputs(&root, &["a"]))
            .unwrap()
            .into_command(),
    );
    failed
        .progress(&SourcePreparationNeeds::repository(fresh.as_ref().clone()))
        .unwrap();
    assert!(matches!(
        runtime
            .repository_materializer
            .active_result_for_test(failed.command().repository_session, "fresh+"),
        RepositoryMaterializationResult::TransportError { .. }
    ));
    assert!(
        failed
            .accept_selected_for_test(SelectedWorkspaceDemands::for_test(
                Arc::from([fresh.clone()]),
                Arc::from([])
            ))
            .is_err()
    );
    assert_current_native_snapshot(&runtime, &accepted);
    assert!(scope.check().is_ok());
    fs::write(
        root.join("MODULE.bazel"),
        b"module(name = 'distdir_probe')\n",
    )
    .unwrap();
    let mut restored = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(inputs(&root, &["a"]))
            .unwrap()
            .into_command(),
    );
    restored
        .progress(&SourcePreparationNeeds::repository(fresh.as_ref().clone()))
        .unwrap();
    assert_eq!(
        fs::read(materialized_root(&restored, &fresh).join("payload.txt")).unwrap(),
        b"verified request-local bytes\n"
    );
    restored
        .accept_selected_for_test(SelectedWorkspaceDemands::for_test(
            Arc::from([request, fresh]),
            Arc::from([]),
        ))
        .unwrap();
    assert!(scope.check().is_ok());
}
