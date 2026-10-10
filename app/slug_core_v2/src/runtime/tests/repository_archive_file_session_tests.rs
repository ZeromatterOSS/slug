use std::io::Write;

use base64::Engine;
use flate2::Compression;
use flate2::write::GzEncoder;
use slug_bzlmod_v2::OverrideAttributeKey;

use super::super::repository_archive::ArchivePlan;
use super::super::repository_archive::materialize_selected_bcr_capture;
use super::super::repository_archive::parse_archive_plan;
use crate::runtime::dice::source_service::OfflineArchiveBacking;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceServiceLimits;

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
            None,
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
            None,
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
            None,
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
        .materialize_native_with_runtime(
            token,
            b,
            RepositoryMaterializationGeneration(2),
            &runtime,
            None,
        )
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
        .materialize_native_with_runtime(
            token,
            a,
            RepositoryMaterializationGeneration(3),
            &runtime,
            None,
        )
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
            .materialize_native_with_runtime(token, request.clone(), generation, &runtime, None)
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
                let captured = materialize_selected_bcr_capture(&plan, &runtime, active, None)?;
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
            None,
        )
        .unwrap();
    let (root, instance) = selected_bcr_file_session_success(&materializer, "replacement");
    assert!(root.exists());
    assert_ne!(instance.value(), 0);
    materializer.discard(replacement).unwrap();
}

const OFFLINE_ARCHIVE_TEST_URL: &str = "https://example.invalid/original/archive.tar.gz";

fn selected_bcr_offline_session_fixture() -> (SelectedBcrFileFixture, Arc<OfflineArchiveBacking>) {
    let mut fixture = selected_bcr_file_session_fixture();
    let attributes = Arc::make_mut(&mut fixture.spec.attributes);
    attributes.shift_remove("type");
    attributes.insert(
        "urls".into(),
        OverrideAttributeValue::Iterable(Arc::new([selected_bcr_file_session_value(
            OFFLINE_ARCHIVE_TEST_URL,
        )])),
    );
    for key in [
        "remote_patches",
        "remote_file_urls",
        "remote_file_integrity",
    ] {
        attributes.insert(
            key.into(),
            OverrideAttributeValue::Map(Arc::new(SmallMap::new())),
        );
    }
    attributes.insert("remote_patch_strip".into(), OverrideAttributeValue::Int(0));
    let backing = OfflineArchiveBacking::new(
        OFFLINE_ARCHIVE_TEST_URL,
        sha2::Sha256::digest(&fixture.archive_bytes).into(),
        NormalizedAbsolutePath::new(fixture.archive.clone()).unwrap(),
    );
    (fixture, backing)
}

fn selected_bcr_offline_session_scope(
    workspace: &NormalizedAbsolutePath,
    backing: Option<Arc<OfflineArchiveBacking>>,
    limits: SourceServiceLimits,
) -> Arc<SourceServiceScope> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    SourceServiceScope::new_for_test_with_offline_archive(
        workspace.clone(),
        deadline,
        deadline + std::time::Duration::from_secs(1),
        limits,
        None,
        backing,
        false,
    )
    .unwrap()
}

#[test]
fn selected_bcr_offline_session_materializes_original_https_and_reuses_deleted_sources() {
    let (fixture, backing) = selected_bcr_offline_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let scope = selected_bcr_offline_session_scope(
        &workspace,
        Some(backing.clone()),
        SourceServiceLimits {
            allow_network: false,
            ..SourceServiceLimits::default()
        },
    );
    assert_eq!(Arc::strong_count(&backing), 2);
    assert!(std::ptr::eq(
        scope
            .offline_archive_path_for_test(backing.original_url(), backing.integrity())
            .unwrap(),
        backing.path(),
    ));
    let weak_backing = Arc::downgrade(&backing);
    let weak_scope = Arc::downgrade(&scope);
    let materializer =
        RepositoryMaterializer::new_scoped(workspace.clone(), scope.clone()).unwrap();
    let request = selected_bcr_file_session_request(&workspace, "offline", fixture.spec.clone());
    let attributes = request.repo_spec.attributes.clone();
    let attribute_owners = Arc::strong_count(&attributes);
    let runtime = selected_bcr_file_session_runtime();
    let token = begin_empty(&materializer);
    let epoch = materializer
        .materialize_native_with_runtime(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(1),
            &runtime,
            Some(&scope),
        )
        .unwrap();
    let (root, instance) = immutable_result(&materializer, "offline");
    assert_eq!(std::fs::read(root.join("value.txt")).unwrap(), b"archive\n");
    assert_eq!(
        std::fs::read(root.join("MODULE.bazel")).unwrap(),
        fixture.module_bytes
    );
    let original = epoch
        .borrowed_instance_for_namespace(&workspace, instance, 1)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(original.request(), request.as_ref()));
    assert!(Arc::ptr_eq(
        &original.request().repo_spec.attributes,
        &attributes
    ));
    assert_eq!(Arc::strong_count(&attributes), attribute_owners);
    assert_eq!(Arc::strong_count(&backing), 2);
    assert_eq!(original.generation_root(), root.as_path());
    assert_eq!(original.request().repo_spec, fixture.spec);
    assert_eq!(request.repo_spec.attributes.len(), 9);
    let counters = scope.snapshot();
    assert_eq!(counters.source_attempts, 2);
    assert_eq!(
        counters.transferred_bytes,
        (fixture.archive_bytes.len() + fixture.module_bytes.len()) as u64
    );
    assert_eq!(counters.registry_attempts, 0);
    assert!(counters.decoded_bytes > 0);
    assert!(counters.written_bytes > counters.transferred_bytes);
    assert_eq!(counters.live_buffer_bytes, 0);
    assert_eq!(scope.check(), Ok(()));
    materializer
        .accept(token, std::slice::from_ref(&request), vec![])
        .unwrap();
    std::fs::remove_file(&fixture.archive).unwrap();
    std::fs::remove_file(&fixture.module).unwrap();
    let reuse = begin_empty(&materializer);
    materializer
        .materialize_native_with_runtime(
            reuse,
            request,
            RepositoryMaterializationGeneration(2),
            &runtime,
            Some(&scope),
        )
        .unwrap();
    assert_eq!(
        immutable_result(&materializer, "offline"),
        (root.clone(), instance)
    );
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
    assert_eq!(
        scope.snapshot().transferred_bytes,
        counters.transferred_bytes
    );
    materializer.discard(reuse).unwrap();
    println!(
        "WP1080_OFFLINE_ARCHIVE_LAYOUT backing_bytes={} backing_align={} scope_bytes={} scope_align={} optional_backing_bytes={} path_bytes={}",
        std::mem::size_of::<OfflineArchiveBacking>(),
        std::mem::align_of::<OfflineArchiveBacking>(),
        std::mem::size_of::<SourceServiceScope>(),
        std::mem::align_of::<SourceServiceScope>(),
        std::mem::size_of::<Option<Arc<OfflineArchiveBacking>>>(),
        std::mem::size_of::<NormalizedAbsolutePath>()
    );
    drop(backing);
    drop(scope);
    assert!(weak_backing.upgrade().is_some());
    assert!(weak_scope.upgrade().is_some());
    drop(materializer);
    assert!(weak_backing.upgrade().is_none());
    assert!(weak_scope.upgrade().is_none());
    assert!(!root.exists());
}

#[test]
fn selected_bcr_offline_session_recaptures_missing_corrupt_and_repaired_newer_generations() {
    let (fixture, backing) = selected_bcr_offline_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let scope = selected_bcr_offline_session_scope(
        &workspace,
        Some(backing),
        SourceServiceLimits {
            allow_network: false,
            ..SourceServiceLimits::default()
        },
    );
    let materializer =
        RepositoryMaterializer::new_scoped(workspace.clone(), scope.clone()).unwrap();
    let request = selected_bcr_file_session_request(&workspace, "repair", fixture.spec);
    let runtime = selected_bcr_file_session_runtime();
    std::fs::remove_file(&fixture.archive).unwrap();
    let mut attempts = 0;
    for generation in 1..=4 {
        let token = begin_empty(&materializer);
        materializer
            .materialize_native_with_runtime(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(generation),
                &runtime,
                Some(&scope),
            )
            .unwrap();
        if generation % 2 == 1 {
            assert!(matches!(active_result(&materializer, "repair"),
                RepositoryMaterializationResult::TransportError { generation: actual, .. }
                    if actual == RepositoryMaterializationGeneration(generation)));
            assert_eq!(scope.snapshot().source_attempts, attempts + 1);
        } else {
            let (root, instance) = immutable_result(&materializer, "repair");
            assert!(root.exists());
            assert_ne!(instance.value(), 0);
            assert_eq!(
                std::fs::read(root.join("MODULE.bazel")).unwrap(),
                fixture.module_bytes
            );
            assert_eq!(scope.snapshot().source_attempts, attempts + 2);
            materializer.discard(token).unwrap();
            assert!(!root.exists());
        }
        attempts = scope.snapshot().source_attempts;
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(scope.check(), Ok(()));
        if generation % 2 == 1 {
            // The failure belongs to this generation even if its backing is repaired.
            std::fs::write(&fixture.archive, &fixture.archive_bytes).unwrap();
            materializer
                .materialize_native_with_runtime(
                    token,
                    request.clone(),
                    RepositoryMaterializationGeneration(generation),
                    &runtime,
                    Some(&scope),
                )
                .unwrap();
            assert_eq!(scope.snapshot().source_attempts, attempts);
            materializer.discard(token).unwrap();
        } else if generation == 2 {
            std::fs::write(&fixture.archive, b"corrupt archive").unwrap();
        }
    }
}

#[test]
fn selected_bcr_offline_session_refuses_mismatch_foreign_cancel_and_resource_limits() {
    let (fixture, backing) = selected_bcr_offline_session_fixture();
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let runtime = selected_bcr_file_session_runtime();
    let request = selected_bcr_file_session_request(&workspace, "refused", fixture.spec);
    for case in 0..9 {
        let mut limits = SourceServiceLimits {
            allow_network: false,
            ..SourceServiceLimits::default()
        };
        let configured = match case {
            0 => None,
            1 => Some(OfflineArchiveBacking::new(
                "https://example.invalid/foreign.tar.gz",
                *backing.integrity(),
                backing.path().clone(),
            )),
            2 => Some(OfflineArchiveBacking::new(
                OFFLINE_ARCHIVE_TEST_URL,
                [0; 32],
                backing.path().clone(),
            )),
            _ => Some(backing.clone()),
        };
        let expected = match case {
            0..=2 => SourceServiceError::NetworkDisabled,
            3 => {
                limits.transferred_bytes = 1;
                SourceServiceError::Limit(SourceCounter::TransferBytes)
            }
            4 => {
                limits.written_bytes = 1;
                SourceServiceError::Limit(SourceCounter::WrittenBytes)
            }
            5 => {
                limits.decoded_bytes = 1;
                SourceServiceError::Limit(SourceCounter::DecodedBytes)
            }
            6 => {
                limits.live_buffer_bytes = 1;
                SourceServiceError::BufferLimit
            }
            7 => SourceServiceError::ForeignScope,
            _ => SourceServiceError::Cancelled,
        };
        let scope = selected_bcr_offline_session_scope(&workspace, configured, limits);
        let weak_scope = Arc::downgrade(&scope);
        let materializer =
            RepositoryMaterializer::new_scoped(workspace.clone(), scope.clone()).unwrap();
        let token = begin_empty(&materializer);
        let foreign = selected_bcr_offline_session_scope(
            &workspace,
            Some(backing.clone()),
            SourceServiceLimits::default(),
        );
        if case == 8 {
            scope.cancel();
        }
        let supplied = if case == 7 { &foreign } else { &scope };
        assert_eq!(
            materializer.materialize_native_with_runtime(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(case + 1),
                &runtime,
                Some(supplied),
            ),
            Err(RepositorySessionError::SourceService(expected)),
            "case {case}"
        );
        let state = materializer.state.lock().unwrap();
        assert!(state.active.as_ref().unwrap().entries.is_empty());
        drop(state);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(scope.snapshot().registry_attempts, 0);
        if case >= 7 {
            assert_eq!(scope.snapshot().source_attempts, 0);
            assert_eq!(scope.snapshot().transferred_bytes, 0);
            assert_eq!(scope.snapshot().written_bytes, 0);
        }
        if case != 7 {
            assert_eq!(scope.check(), Err(expected));
        }
        materializer.discard(token).unwrap();
        drop(materializer);
        drop(scope);
        assert!(weak_scope.upgrade().is_none());
    }
}
