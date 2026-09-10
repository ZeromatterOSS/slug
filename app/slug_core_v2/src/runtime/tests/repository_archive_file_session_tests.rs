use std::io::Write;

use base64::Engine;
use flate2::Compression;
use flate2::write::GzEncoder;
use slug_bzlmod_v2::OverrideAttributeKey;

use super::super::repository_archive::ArchivePlan;
use super::super::repository_archive::materialize_selected_bcr_capture;
use super::super::repository_archive::parse_archive_plan;

struct SelectedBcrFileFixture {
    _directory: tempfile::TempDir,
    archive: std::path::PathBuf,
    overlay: std::path::PathBuf,
    patch: std::path::PathBuf,
    module: std::path::PathBuf,
    archive_bytes: Vec<u8>,
    module_bytes: Vec<u8>,
    patched_value: Vec<u8>,
    spec: RepoSpec,
}

fn selected_bcr_file_session_sri(bytes: &[u8]) -> CompactString {
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(bytes))
    )
    .into()
}

fn selected_bcr_file_session_url(path: &std::path::Path) -> CompactString {
    url::Url::from_file_path(path).unwrap().to_string().into()
}

fn selected_bcr_file_session_value(value: impl Into<CompactString>) -> OverrideAttributeValue {
    OverrideAttributeValue::String(value.into())
}

fn selected_bcr_file_session_fixture() -> SelectedBcrFileFixture {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("archive.tar.gz");
    let overlay = directory.path().join("overlay.txt");
    let patch = directory.path().join("change.patch");
    let module = directory.path().join("MODULE.bazel");
    let tar = ustar(
        &[
            TarEntry {
                name: b"pkg/value.txt",
                prefix: b"",
                typeflag: b'0',
                data: b"archive\n",
            },
            TarEntry {
                name: b"pkg/MODULE.bazel",
                prefix: b"",
                typeflag: b'0',
                data: b"archive module\n",
            },
        ],
        true,
    );
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar).unwrap();
    let archive_bytes = encoder.finish().unwrap();
    let overlay_bytes = b"overlay before patch\n".to_vec();
    let patched_value = b"patched from local file\n".to_vec();
    let patch_bytes =
        b"--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-overlay before patch\n+patched from local file\n";
    let module_bytes = b"module(name = 'local_file_registry')\n".to_vec();
    std::fs::write(&archive, &archive_bytes).unwrap();
    std::fs::write(&overlay, &overlay_bytes).unwrap();
    std::fs::write(&patch, patch_bytes).unwrap();
    std::fs::write(&module, &module_bytes).unwrap();

    let archive_url = selected_bcr_file_session_url(&archive);
    let overlay_url = selected_bcr_file_session_url(&overlay);
    let patch_url = selected_bcr_file_session_url(&patch);
    let module_url = selected_bcr_file_session_url(&module);
    let overlay_key = OverrideAttributeKey::String("value.txt".into());
    let attributes: [(CompactString, OverrideAttributeValue); 10] = [
        (
            "urls".into(),
            OverrideAttributeValue::Iterable(Arc::new([selected_bcr_file_session_value(
                archive_url,
            )])),
        ),
        (
            "integrity".into(),
            selected_bcr_file_session_value(selected_bcr_file_session_sri(&archive_bytes)),
        ),
        ("type".into(), selected_bcr_file_session_value("tar.gz")),
        (
            "strip_prefix".into(),
            selected_bcr_file_session_value("pkg"),
        ),
        (
            "remote_patches".into(),
            OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                OverrideAttributeKey::String(patch_url),
                selected_bcr_file_session_value(selected_bcr_file_session_sri(patch_bytes)),
            )]))),
        ),
        (
            "remote_file_urls".into(),
            OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                overlay_key.clone(),
                OverrideAttributeValue::Iterable(Arc::new([selected_bcr_file_session_value(
                    overlay_url,
                )])),
            )]))),
        ),
        (
            "remote_file_integrity".into(),
            OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                overlay_key,
                selected_bcr_file_session_value(selected_bcr_file_session_sri(&overlay_bytes)),
            )]))),
        ),
        ("remote_patch_strip".into(), OverrideAttributeValue::Int(1)),
        (
            "remote_module_file_urls".into(),
            OverrideAttributeValue::Iterable(Arc::new([selected_bcr_file_session_value(
                module_url,
            )])),
        ),
        (
            "remote_module_file_integrity".into(),
            selected_bcr_file_session_value(selected_bcr_file_session_sri(&module_bytes)),
        ),
    ];
    SelectedBcrFileFixture {
        _directory: directory,
        archive,
        overlay,
        patch,
        module,
        archive_bytes,
        module_bytes,
        patched_value,
        spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                    .unwrap(),
                rule_name: "http_archive".into(),
            },
            attributes: Arc::new(SmallMap::from_iter(attributes)),
        },
    }
}

fn selected_bcr_file_session_request(
    workspace: &NormalizedAbsolutePath,
    repo: &str,
    spec: RepoSpec,
) -> Arc<RepositoryMaterializationRequest> {
    native_request(
        workspace,
        repo,
        spec,
        RepositoryMaterializationKind::Immutable,
    )
}

fn selected_bcr_file_session_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn selected_bcr_file_session_success(
    materializer: &RepositoryMaterializer,
    repo: &str,
) -> (std::path::PathBuf, PathObservationInstanceId) {
    immutable_result(materializer, repo)
}

#[test]
fn selected_bcr_file_session_materializes_transforms_and_reuses_deleted_sources() {
    let fixture = selected_bcr_file_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let request = selected_bcr_file_session_request(&workspace, "local_file", fixture.spec.clone());
    let runtime = selected_bcr_file_session_runtime();
    let token = begin_empty(&materializer);
    materializer
        .materialize_native_with_runtime(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(1),
            &runtime,
        )
        .unwrap();
    let (root, instance) = selected_bcr_file_session_success(&materializer, "local_file");
    assert_eq!(
        std::fs::read(root.join("value.txt")).unwrap(),
        fixture.patched_value
    );
    assert_eq!(
        std::fs::read(root.join("MODULE.bazel")).unwrap(),
        fixture.module_bytes
    );
    let module_demand = PathObservationDemand::new(
        PathObservationNamespace::Materialization(instance),
        NormalizedAbsolutePath::new(root.join("MODULE.bazel")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let observed = materializer
        .observe_native(token, [module_demand.clone()])
        .unwrap();
    materializer
        .accept(
            token,
            std::slice::from_ref(&request),
            vec![RepositoryValidation::new(
                request.clone(),
                vec![(
                    module_demand.clone(),
                    epoch_result(&observed, &module_demand),
                )],
            )],
        )
        .unwrap();
    for path in [
        &fixture.archive,
        &fixture.overlay,
        &fixture.patch,
        &fixture.module,
    ] {
        std::fs::remove_file(path).unwrap();
    }
    let reuse = materializer.begin().unwrap();
    let preflight = materializer.preflight_native(reuse, []).unwrap();
    assert!(matches!(
        epoch_result(preflight.path_observations(), &module_demand),
        PathObservationResult::FileBytes(PathOperationResult::Present(_))
    ));
    materializer
        .materialize_native_with_runtime(
            reuse,
            request,
            RepositoryMaterializationGeneration(2),
            &runtime,
        )
        .unwrap();
    assert_eq!(
        selected_bcr_file_session_success(&materializer, "local_file"),
        (root, instance)
    );
    materializer.discard(reuse).unwrap();
}

#[test]
fn selected_bcr_file_session_sri_a_b_a_reuses_the_original_root() {
    let fixture = selected_bcr_file_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let runtime = selected_bcr_file_session_runtime();
    let a = selected_bcr_file_session_request(&workspace, "revision", fixture.spec.clone());
    let token = begin_empty(&materializer);
    materializer
        .materialize_native_with_runtime(
            token,
            a.clone(),
            RepositoryMaterializationGeneration(1),
            &runtime,
        )
        .unwrap();
    let first = selected_bcr_file_session_success(&materializer, "revision");
    materializer
        .accept(token, std::slice::from_ref(&a), vec![])
        .unwrap();
    let changed_module = b"module(name = 'local_file_registry_b')\n";
    std::fs::write(&fixture.module, changed_module).unwrap();
    let mut changed = fixture.spec.clone();
    Arc::make_mut(&mut changed.attributes).insert(
        "remote_module_file_integrity".into(),
        selected_bcr_file_session_value(selected_bcr_file_session_sri(changed_module)),
    );
    let b = selected_bcr_file_session_request(&workspace, "revision", changed);
    let token = begin_empty(&materializer);
    materializer
        .materialize_native_with_runtime(token, b, RepositoryMaterializationGeneration(2), &runtime)
        .unwrap();
    let second = selected_bcr_file_session_success(&materializer, "revision");
    assert_ne!(second.0, first.0);
    assert_ne!(second.1, first.1);
    assert_eq!(
        std::fs::read(second.0.join("MODULE.bazel")).unwrap(),
        changed_module
    );
    materializer.discard(token).unwrap();
    for path in [
        &fixture.archive,
        &fixture.overlay,
        &fixture.patch,
        &fixture.module,
    ] {
        std::fs::remove_file(path).unwrap();
    }
    let token = begin_empty(&materializer);
    materializer
        .materialize_native_with_runtime(token, a, RepositoryMaterializationGeneration(3), &runtime)
        .unwrap();
    assert_eq!(
        selected_bcr_file_session_success(&materializer, "revision"),
        first
    );
    materializer.discard(token).unwrap();
}

#[test]
fn selected_bcr_file_session_missing_create_mismatch_repair_retry_newer_generation() {
    let fixture = selected_bcr_file_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let runtime = selected_bcr_file_session_runtime();
    let request = selected_bcr_file_session_request(&workspace, "repair", fixture.spec.clone());
    let materialize = |token, generation| {
        materializer
            .materialize_native_with_runtime(token, request.clone(), generation, &runtime)
            .unwrap();
    };
    std::fs::remove_file(&fixture.archive).unwrap();
    let token = begin_empty(&materializer);
    materialize(token, RepositoryMaterializationGeneration(1));
    assert!(matches!(
        active_result(&materializer, "repair"),
        RepositoryMaterializationResult::TransportError {
            generation: RepositoryMaterializationGeneration(1),
            ..
        }
    ));
    materializer.discard(token).unwrap();
    std::fs::write(&fixture.archive, &fixture.archive_bytes).unwrap();
    let token = begin_empty(&materializer);
    materialize(token, RepositoryMaterializationGeneration(2));
    let created = selected_bcr_file_session_success(&materializer, "repair");
    materializer.discard(token).unwrap();
    assert!(!created.0.exists());
    std::fs::write(&fixture.archive, b"mismatched local archive").unwrap();
    let token = begin_empty(&materializer);
    materialize(token, RepositoryMaterializationGeneration(3));
    assert!(matches!(
        active_result(&materializer, "repair"),
        RepositoryMaterializationResult::TransportError {
            generation: RepositoryMaterializationGeneration(3),
            ..
        }
    ));
    materializer.discard(token).unwrap();
    std::fs::write(&fixture.archive, &fixture.archive_bytes).unwrap();
    let token = begin_empty(&materializer);
    materialize(token, RepositoryMaterializationGeneration(4));
    let repaired = selected_bcr_file_session_success(&materializer, "repair");
    assert!(repaired.0.exists());
    assert_ne!(repaired.1.value(), 0);
    materializer.discard(token).unwrap();
}

#[test]
fn selected_bcr_file_session_stale_real_capture_drops_root_and_allows_replacement() {
    let fixture = selected_bcr_file_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let request = selected_bcr_file_session_request(&workspace, "stale", fixture.spec.clone());
    let runtime = selected_bcr_file_session_runtime();
    let token = begin_empty(&materializer);
    let mut captured_root = None;
    let mut replacement = None;
    let error = materializer
        .materialize_selected_bcr_capture_for_test(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(4),
            |active| {
                let ArchivePlan::SelectedBcrTarGz(plan) = parse_archive_plan(&request.repo_spec)
                    .expect("fixture must parse as selected BCR")
                else {
                    unreachable!("fixture must remain on selected BCR path")
                };
                let captured = materialize_selected_bcr_capture(&plan, &runtime, active)?;
                let Materialized::AssociatedImmutable { root, .. } = &captured else {
                    unreachable!("native selected BCR capture is associated immutable")
                };
                captured_root = Some(root.path().to_path_buf());
                materializer.discard(token).unwrap();
                replacement = Some(begin_empty(&materializer));
                Ok(captured)
            },
        )
        .unwrap_err();
    let replacement = replacement.unwrap();
    assert_eq!(
        error,
        RepositorySessionError::StaleToken {
            active: Some(replacement),
            supplied: token,
        }
    );
    assert!(!captured_root.unwrap().exists());
    materializer
        .materialize_native_with_runtime(
            replacement,
            selected_bcr_file_session_request(&workspace, "replacement", fixture.spec),
            RepositoryMaterializationGeneration(5),
            &runtime,
        )
        .unwrap();
    let (root, instance) = selected_bcr_file_session_success(&materializer, "replacement");
    assert!(root.exists());
    assert_ne!(instance.value(), 0);
    materializer.discard(replacement).unwrap();
}
