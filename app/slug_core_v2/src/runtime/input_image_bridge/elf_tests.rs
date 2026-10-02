//! Tiny sectionless ELF64 fixtures follow gABI header offsets, not Host binaries.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use slug_workspace_v2::ElfMetadataOutcome;

use super::*;
use crate::runtime::ProcessHostOwner;

fn elf(entry: u64) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[7] = 3; // Linux OSABI, ABI version0.
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&entry.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes
}
fn observe(runtime: &WorkspaceRuntime) -> ObservedElfMetadata {
    runtime
        .observe_declared_elf_metadata(
            DeclaredImageSource::WorkspaceSubtree("input".into()),
            InputImagePolicy::plain(),
            "tool".into(),
            ElfMetadataPolicy::default(),
        )
        .unwrap()
}
fn entry(metadata: &ObservedElfMetadata) -> u64 {
    match metadata.verified().outcome() {
        ElfMetadataOutcome::Parsed(facts) => facts.header().entry,
        other => panic!("expected parsed ELF, got {other:?}"),
    }
}
fn reopen(runtime: &WorkspaceRuntime) {
    let preflight = runtime.begin_native_demand_command().unwrap();
    assert!(preflight.path_observations().observations().is_empty());
    NativeDemandAbortGuard::new(preflight.into_command())
        .discard()
        .unwrap();
}

#[test]
fn declared_elf_normal_need_workspace_content_mode_a_b_a_and_restore() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("tool"), elf(7)).unwrap();
    fs::set_permissions(input.join("tool"), fs::Permissions::from_mode(0o751)).unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    let first = observe(&runtime);
    let warm = observe(&runtime);
    assert_eq!(first.content(), warm.content());
    assert_eq!(entry(&first), 7);
    fs::write(input.join("tool"), elf(9)).unwrap();
    let second = observe(&runtime);
    assert_eq!(entry(&second), 9);
    assert_ne!(first.content(), second.content());
    assert_eq!(entry(&first), 7, "historical facts stay immutable");
    fs::write(input.join("tool"), elf(7)).unwrap();
    let restored = observe(&runtime);
    assert_eq!(first.content(), restored.content());
    fs::set_permissions(input.join("tool"), fs::Permissions::from_mode(0o750)).unwrap();
    assert_ne!(first.content(), observe(&runtime).content());
    assert_eq!(
        runtime.image_resources.logical_bytes_used(),
        0,
        "metadata observation does not stage tool payloads"
    );
    reopen(&runtime);
}

#[test]
fn declared_elf_genuine_immutable_writer_and_exact_namespace() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    let workspace = tempfile::tempdir().unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    let (old_request, _) = super::tests::generated_request(&runtime);
    let mut request = old_request.as_ref().clone();
    request.kind = slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(
        GeneratedRepositoryFileEffectPlan::build([(
            "tool".into(),
            Arc::<[u8]>::from(elf(31)),
            true,
        )])
        .unwrap(),
    );
    let metadata = runtime
        .observe_declared_elf_metadata(
            DeclaredImageSource::ImmutableRepository {
                request: Arc::new(request),
                subtree: ".".into(),
            },
            InputImagePolicy::plain(),
            "tool".into(),
            ElfMetadataPolicy::default(),
        )
        .unwrap();
    assert_eq!(entry(&metadata), 31);
    assert!(matches!(
        metadata.image().namespace(),
        PathObservationNamespace::Materialization(_)
    ));
    assert!(
        metadata
            .file()
            .as_path()
            .starts_with(metadata.image().anchor_root().as_path())
    );
    reopen(&runtime);
}

#[test]
fn declared_elf_refuses_absent_nonregular_escape_and_restores_session() {
    let workspace = tempfile::tempdir().unwrap();
    fs::create_dir(workspace.path().join("input")).unwrap();
    fs::create_dir(workspace.path().join("input/not_file")).unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    for file in ["missing", "not_file", "../outside", "/outside"] {
        assert!(
            runtime
                .observe_declared_elf_metadata(
                    DeclaredImageSource::WorkspaceSubtree("input".into()),
                    InputImagePolicy::plain(),
                    file.into(),
                    ElfMetadataPolicy::default(),
                )
                .is_err()
        );
        reopen(&runtime);
    }
    fs::write(workspace.path().join("input/tool"), elf(12)).unwrap();
    assert_eq!(entry(&observe(&runtime)), 12);
    reopen(&runtime);
}

#[test]
fn declared_elf_allowance_is_exact_and_generic_image_cannot_authorize_it() {
    let workspace = tempfile::tempdir().unwrap();
    fs::create_dir(workspace.path().join("input")).unwrap();
    fs::write(workspace.path().join("input/tool"), elf(1)).unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    let metadata = observe(&runtime);
    let certificate = metadata.image().clone();
    let file = metadata.file().clone();
    let policy = ElfMetadataPolicy::default();
    let exact = DeclaredElfRequest {
        certificate: certificate.clone(),
        file: file.clone(),
        policy,
    };
    let anchor = ImageChargedPath::pin_existing(
        certificate.anchor_root().clone(),
        runtime.image_resources.descriptor_budget(),
    )
    .unwrap();
    let make = |expected_digest, policy| {
        PathObservationDemand::input_image(
            certificate.namespace(),
            file.clone(),
            slug_workspace_v2::ImageObservationRequest::new(
                anchor.clone(),
                ImageObservationOperation::BoundedElfMetadata {
                    expected_digest,
                    policy,
                },
            ),
        )
        .unwrap()
    };
    let digest = certificate
        .entries()
        .iter()
        .find(|entry| entry.path() == &file)
        .unwrap()
        .file_digest()
        .unwrap();
    let demand = make(ElfSourceDigestBinding::from_digest(digest), policy);
    assert!(exact.allows(&demand));
    let wrong_digest =
        slug_workspace_v2::FileContentDigest::new([0; 32], digest.size_bytes()).unwrap();
    assert!(!exact.allows(&make(
        ElfSourceDigestBinding::from_digest(wrong_digest),
        policy
    )));
    let tighter = ElfMetadataPolicy::new(
        policy.profile(),
        slug_workspace_v2::ElfMetadataLimits {
            max_proof_spans: 1,
            ..*policy.limits()
        },
    )
    .unwrap();
    assert!(!exact.allows(&make(ElfSourceDigestBinding::from_digest(digest), tighter)));
    let key = ObservedInputImageKey::new(
        certificate.namespace(),
        certificate.anchor_root().clone(),
        certificate.source_root().clone(),
        InputImagePolicy::plain(),
        runtime.image_resources.descriptor_budget(),
    )
    .unwrap();
    let generic = DeclaredImageRequest {
        key,
        elf: None,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
        repository: None,
    };
    assert!(!generic.allows(&demand));
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard.command.as_mut().unwrap().image_request = Some(generic);
    assert!(observe_command_paths(guard.command(), vec![demand]).is_err());
    guard.discard().unwrap();
    reopen(&runtime);
}

#[test]
fn declared_elf_parallel_workspaces_keep_origins_and_credits_separate() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    for (workspace, value) in [(&first, 2), (&second, 3)] {
        fs::create_dir(workspace.path().join("input")).unwrap();
        fs::write(workspace.path().join("input/tool"), elf(value)).unwrap();
    }
    std::thread::scope(|scope| {
        let first_job = scope.spawn(|| {
            let runtime = WorkspaceRuntime::new(first.path(), ProcessHostOwner::native()).unwrap();
            observe(&runtime)
        });
        let second_job = scope.spawn(|| {
            let runtime = WorkspaceRuntime::new(second.path(), ProcessHostOwner::native()).unwrap();
            observe(&runtime)
        });
        let a = first_job.join().unwrap();
        let b = second_job.join().unwrap();
        assert_eq!((entry(&a), entry(&b)), (2, 3));
        assert_ne!(a.content(), b.content());
    });
}
