//! Source selectors are exercised through the real native Need/restore driver.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_bzlmod_v2::SelectedLabelSourcePolicy;
use slug_bzlmod_v2::SelectedLabelSourceRecipe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ImageDescriptorBudget;

use super::*;
use crate::runtime::ProcessHostOwner;

fn elf(entry: u64) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[7] = 3;
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&entry.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes
}

fn fixture() -> (tempfile::TempDir, WorkspaceRuntime) {
    let workspace = tempfile::tempdir().unwrap();
    fs::create_dir_all(workspace.path().join("pkg/sub/bin")).unwrap();
    fs::write(workspace.path().join("pkg/BUILD.bazel"), "").unwrap();
    fs::write(workspace.path().join("pkg/sub/marker"), "marker").unwrap();
    fs::write(workspace.path().join("pkg/sub/bin/tool"), elf(7)).unwrap();
    fs::set_permissions(
        workspace.path().join("pkg/sub/bin/tool"),
        fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    (workspace, runtime)
}

fn recipe(
    runtime: &WorkspaceRuntime,
    budget: &ImageDescriptorBudget,
    label: &str,
) -> SelectedLabelSourceRecipe {
    let workspace = runtime.repository_materializer.workspace();
    SelectedLabelSourceRecipe::new(
        ImageChargedPath::pin_existing(workspace.clone(), budget).unwrap(),
        &CanonicalLabel::parse(label).unwrap(),
        HostRepositoryLabelPathSource::Root(workspace.clone()),
        SelectedLabelSourcePolicy::plain(),
        budget,
    )
    .unwrap()
}

fn tool_key(runtime: &WorkspaceRuntime) -> SelectedLabelSourceKey {
    let budget = runtime.image_resources.descriptor_budget();
    SelectedLabelSourceKey::new(
        recipe(runtime, budget, "@@//pkg:sub/marker")
            .parent(budget)
            .unwrap()
            .child("bin/tool", budget)
            .unwrap(),
    )
}

fn entry(result: &label_source::SelectedLabelElfMetadata) -> u64 {
    match result.metadata.verified().outcome() {
        ElfMetadataOutcome::Parsed(facts) => facts.header().entry,
        other => panic!("expected actual authenticated ELF, got {other:?}"),
    }
}

fn restored(runtime: &WorkspaceRuntime) {
    let preflight = runtime.begin_native_demand_command().unwrap();
    assert!(preflight.path_observations().observations().is_empty());
    NativeDemandAbortGuard::new(preflight.into_command())
        .discard()
        .unwrap();
}

#[test]
fn selected_label_workspace_capture_elf_warm_a_b_a_and_last_drop() {
    let (workspace, runtime) = fixture();
    let key = tool_key(&runtime);
    let captured = runtime.capture_selected_label_source(key.clone()).unwrap();
    assert_eq!(captured.selected.recipe(), key.recipe());
    assert_eq!(
        captured.selected.image_key().source_root().as_path(),
        workspace.path().join("pkg/sub")
    );
    assert_eq!(
        fs::read(captured.image.test_path().join("bin/tool")).unwrap(),
        elf(7)
    );
    assert_eq!(
        fs::metadata(captured.image.test_path().join("bin/tool"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o751
    );
    let first = runtime
        .observe_selected_label_elf_metadata(key.clone(), ElfMetadataPolicy::default())
        .unwrap();
    let warm = runtime
        .observe_selected_label_elf_metadata(key.clone(), ElfMetadataPolicy::default())
        .unwrap();
    assert_eq!(first.metadata.content(), warm.metadata.content());
    assert_eq!(entry(&first), 7);
    fs::write(workspace.path().join("pkg/sub/bin/tool"), elf(9)).unwrap();
    fs::set_permissions(
        workspace.path().join("pkg/sub/bin/tool"),
        fs::Permissions::from_mode(0o750),
    )
    .unwrap();
    let changed = runtime
        .observe_selected_label_elf_metadata(key.clone(), ElfMetadataPolicy::default())
        .unwrap();
    assert_eq!(entry(&changed), 9);
    assert_ne!(first.metadata.content(), changed.metadata.content());
    assert_eq!(entry(&first), 7);
    fs::write(workspace.path().join("pkg/sub/bin/tool"), elf(7)).unwrap();
    fs::set_permissions(
        workspace.path().join("pkg/sub/bin/tool"),
        fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    let recovered = runtime
        .observe_selected_label_elf_metadata(key, ElfMetadataPolicy::default())
        .unwrap();
    assert_eq!(first.metadata.content(), recovered.metadata.content());
    let stage = captured.image.test_path().to_owned();
    let held = captured.clone();
    drop(captured);
    assert!(stage.exists());
    drop(held);
    assert!(!stage.exists());
    assert_eq!(runtime.image_resources.logical_bytes_used(), 0);
    restored(&runtime);
}

fn generated_key(runtime: &WorkspaceRuntime, entry: u64) -> SelectedLabelSourceKey {
    let (request, _) = super::tests::generated_request(runtime);
    let plan = GeneratedRepositoryFileEffectPlan::build([
        (
            "BUILD.bazel".into(),
            Arc::<[u8]>::from(&b"exports_files(['marker'])\n"[..]),
            false,
        ),
        ("marker".into(), Arc::<[u8]>::from(&b"marker"[..]), false),
        ("bin/tool".into(), Arc::<[u8]>::from(elf(entry)), true),
    ])
    .unwrap();
    let route = RootRepositoryRoute::for_generated_repo_spec(
        runtime.repository_materializer.workspace().clone(),
        ApparentRepoName::new("selected").unwrap(),
        request.id.canonical_repo.clone(),
        request.repo_spec.clone(),
        HostRepositoryLocalPathPolicy::LocalUnsupported,
        plan,
    )
    .unwrap();
    let budget = runtime.image_resources.descriptor_budget();
    let recipe = SelectedLabelSourceRecipe::new(
        ImageChargedPath::pin_existing(runtime.repository_materializer.workspace().clone(), budget)
            .unwrap(),
        &CanonicalLabel::parse("@@selected+//:marker").unwrap(),
        HostRepositoryLabelPathSource::External(HostRepositorySourceRoute::root(route)),
        SelectedLabelSourcePolicy::plain(),
        budget,
    )
    .unwrap();
    SelectedLabelSourceKey::new(
        recipe
            .parent(budget)
            .unwrap()
            .child("bin/tool", budget)
            .unwrap(),
    )
}

#[test]
fn selected_label_genuine_writer_request_namespace_and_content_a_b_a() {
    let workspace = tempfile::tempdir().unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path(), ProcessHostOwner::native()).unwrap();
    let key_a = generated_key(&runtime, 31);
    let first = runtime
        .observe_selected_label_elf_metadata(key_a.clone(), ElfMetadataPolicy::default())
        .unwrap();
    let captured = runtime
        .capture_selected_label_source(key_a.clone())
        .unwrap();
    assert_eq!(entry(&first), 31);
    assert!(matches!(
        first.selected.image_key().namespace(),
        PathObservationNamespace::Materialization(_)
    ));
    let (request, result) = first
        .selected
        .repository()
        .expect("normal current immutable request");
    let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
        generation_root,
        observation_instance,
        ..
    } = result
    else {
        panic!("genuine writer must be immutable")
    };
    assert_eq!(
        first.selected.image_key().anchor_root().as_path(),
        generation_root
    );
    assert_eq!(
        first.selected.image_key().namespace(),
        PathObservationNamespace::Materialization(*observation_instance)
    );
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind
    else {
        panic!("actual normal writer plan")
    };
    for effect in plan.effects() {
        assert_eq!(
            fs::read(captured.image.test_path().join(effect.path())).unwrap(),
            effect.content()
        );
    }
    let changed = runtime
        .observe_selected_label_elf_metadata(
            generated_key(&runtime, 47),
            ElfMetadataPolicy::default(),
        )
        .unwrap();
    assert_eq!(entry(&changed), 47);
    assert_ne!(first.selected.repository(), changed.selected.repository());
    assert_eq!(entry(&first), 31);
    let recovered = runtime
        .observe_selected_label_elf_metadata(key_a, ElfMetadataPolicy::default())
        .unwrap();
    assert_eq!(entry(&recovered), 31);
    // Generation instances can be fresh after transient restore; the actual
    // normal request/plan, rather than a guessed temp path, restores exactly.
    assert_eq!(
        first.selected.repository().unwrap().0,
        recovered.selected.repository().unwrap().0
    );
    restored(&runtime);
}

#[test]
fn selected_label_missing_nonregular_scope_and_package_restore() {
    let (workspace, runtime) = fixture();
    let budget = runtime.image_resources.descriptor_budget();
    let base = recipe(&runtime, budget, "@@//pkg:sub/marker")
        .parent(budget)
        .unwrap();
    fs::create_dir(workspace.path().join("pkg/sub/directory")).unwrap();
    for key in [
        SelectedLabelSourceKey::new(base.child("missing", budget).unwrap()),
        SelectedLabelSourceKey::new(base.child("directory", budget).unwrap()),
        SelectedLabelSourceKey::new(
            base.parent(budget)
                .unwrap()
                .child("BUILD.bazel", budget)
                .unwrap(),
        ),
        SelectedLabelSourceKey::new(recipe(&runtime, budget, "@@//absent:tool")),
    ] {
        assert!(runtime.capture_selected_label_source(key).is_err());
        assert_eq!(runtime.image_resources.logical_bytes_used(), 0);
        restored(&runtime);
    }
    fs::remove_file(workspace.path().join("pkg/BUILD.bazel")).unwrap();
    assert!(
        runtime
            .capture_selected_label_source(tool_key(&runtime))
            .is_err()
    );
    restored(&runtime);
    fs::write(workspace.path().join("pkg/BUILD.bazel"), "").unwrap();
    fs::write(workspace.path().join("pkg/sub/missing"), b"now regular").unwrap();
    let repaired = runtime
        .capture_selected_label_source(SelectedLabelSourceKey::new(
            base.child("missing", budget).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        fs::read(repaired.image.test_path().join("missing")).unwrap(),
        b"now regular"
    );
    drop(repaired);
    symlink("bin/tool", workspace.path().join("pkg/sub/alias")).unwrap();
    assert!(
        runtime
            .capture_selected_label_source(SelectedLabelSourceKey::new(
                base.child("alias", budget).unwrap()
            ))
            .is_err()
    );
    restored(&runtime);
}

#[test]
fn selected_label_foreign_budget_and_workspace_refuse_and_restore() {
    let (_workspace, runtime) = fixture();
    let foreign = ImageDescriptorBudget::new(128 * 1024);
    let key = SelectedLabelSourceKey::new(recipe(&runtime, &foreign, "@@//pkg:sub/marker"));
    let error = match runtime.capture_selected_label_source(key) {
        Err(error) => error,
        Ok(_) => panic!("foreign budget admitted"),
    };
    assert!(format!("{error:?}").contains("foreign selected Label recipe"));
    restored(&runtime);
    let other = tempfile::tempdir().unwrap();
    let budget = runtime.image_resources.descriptor_budget();
    let root = NormalizedAbsolutePath::new(other.path().to_owned()).unwrap();
    let recipe = SelectedLabelSourceRecipe::new(
        ImageChargedPath::pin_existing(root.clone(), budget).unwrap(),
        &CanonicalLabel::parse("@@//:unread").unwrap(),
        HostRepositoryLabelPathSource::Root(root),
        SelectedLabelSourcePolicy::plain(),
        budget,
    )
    .unwrap();
    let error = match runtime.capture_selected_label_source(SelectedLabelSourceKey::new(recipe)) {
        Err(error) => error,
        Ok(_) => panic!("foreign workspace admitted"),
    };
    assert!(format!("{error:?}").contains("foreign selected Label recipe"));
    restored(&runtime);
    assert_eq!(
        entry(
            &runtime
                .observe_selected_label_elf_metadata(
                    tool_key(&runtime),
                    ElfMetadataPolicy::default()
                )
                .unwrap()
        ),
        7
    );
}

#[test]
fn selected_label_final_build_frontier_is_fresh_after_capture_and_restore() {
    let (workspace, runtime) = fixture();
    let key = tool_key(&runtime);
    let build = workspace.path().join("pkg/BUILD.bazel");
    let policy = key.recipe().policy().image_policy();
    let result = runtime.prepare_declared_input_inner(
        DeclaredImageSource::SelectedLabel(key.clone()),
        policy,
        prepare::PreparationKind::Capture,
        Some(&|| fs::write(&build, "changed outside captured subtree").unwrap()),
    );
    assert!(matches!(
        result,
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource
        ))
    ));
    assert_eq!(
        runtime.image_resources.logical_bytes_used(),
        0,
        "provisional stage must be dropped on stale Label metadata"
    );
    assert_eq!(
        fs::read(workspace.path().join("pkg/sub/bin/tool")).unwrap(),
        elf(7)
    );
    restored(&runtime);
    fs::write(&build, "").unwrap();
    let repaired = runtime.capture_selected_label_source(key).unwrap();
    assert_eq!(
        fs::read(repaired.image.test_path().join("bin/tool")).unwrap(),
        elf(7)
    );
    assert!(
        repaired
            .selected
            .observations()
            .observations()
            .iter()
            .any(|(demand, _)| demand.path().as_path() == build)
    );
    drop(repaired);
    restored(&runtime);
}

#[test]
fn selected_label_exact_declaration_and_whole_batch_deny_before_source_open() {
    let (_workspace, runtime) = fixture();
    let captured = runtime
        .capture_selected_label_source(tool_key(&runtime))
        .unwrap();
    let selected = &captured.selected;
    let mut declaration = DeclaredImageRequest {
        key: selected.image_key().clone(),
        elf: None,
        deadline: std::time::Instant::now() - std::time::Duration::from_secs(1),
        repository: None,
    };
    label_source::check_declaration(&runtime, &declaration, selected).unwrap();
    let budget = runtime.image_resources.descriptor_budget();
    let anchor =
        ImageChargedPath::pin_existing(declaration.key.anchor_root().clone(), budget).unwrap();
    let valid = PathObservationDemand::input_image(
        declaration.key.namespace(),
        selected.selected_file().clone(),
        slug_workspace_v2::ImageObservationRequest::new(
            anchor.clone(),
            ImageObservationOperation::Lstat,
        ),
    )
    .unwrap();
    let invalid = PathObservationDemand::input_image(
        PathObservationNamespace::Materialization(
            slug_workspace_v2::PathObservationInstanceId::new(999),
        ),
        selected.selected_file().clone(),
        slug_workspace_v2::ImageObservationRequest::new(
            anchor.clone(),
            ImageObservationOperation::Lstat,
        ),
    )
    .unwrap();
    let changed = ObservedInputImageKey::from_charged(
        declaration.key.namespace(),
        anchor.clone(),
        anchor,
        *declaration.key.policy(),
    )
    .unwrap();
    let original = std::mem::replace(&mut declaration.key, changed);
    assert!(label_source::check_declaration(&runtime, &declaration, selected).is_err());
    declaration.key = original;
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard.command.as_mut().unwrap().image_request = Some(declaration);
    // A valid batch would fail at the expired source declaration. The later
    // foreign member must win before even that source-opening step is reached.
    let error = observe_command_paths(guard.command(), vec![valid.clone(), invalid]).unwrap_err();
    assert!(format!("{error:?}").contains("foreign image observation"));
    let error = observe_command_paths(guard.command(), vec![valid]).unwrap_err();
    assert!(format!("{error:?}").contains("image preparation deadline"));
    let foreign_budget = ImageDescriptorBudget::new(128 * 1024);
    let foreign_resources = input_image::WorkspaceImageResources::new(foreign_budget.clone(), 1024);
    let baseline = foreign_budget.used_bytes();
    let stage = input_image::prepare_input_image_stage(&foreign_resources).unwrap();
    let stage_path = stage.test_path().to_owned();
    assert_eq!(
        foreign_budget.used_bytes() - baseline,
        4096 + 256 + std::mem::size_of::<input_image::PreparedInputImageStage>()
    );
    let source = runtime
        .repository_materializer
        .image_source(
            guard.command().repository_session,
            captured.image.test_certificate().namespace(),
            captured.image.test_certificate().anchor_root(),
            None,
        )
        .unwrap();
    let active_calls = std::cell::Cell::new(0);
    let active = || {
        active_calls.set(active_calls.get() + 1);
        false
    };
    let result = input_image::capture_prepared_input_image(
        input_image::AuthorizedObservedInputImage::new(
            source,
            captured.image.test_certificate().clone(),
        ),
        &runtime.image_resources,
        &active,
        stage,
    );
    assert!(matches!(
        result,
        Err(input_image::InputImageCaptureError::Unauthorized)
    ));
    assert_eq!(
        active_calls.get(),
        0,
        "foreign stage must reject before the source IO/cancellation check"
    );
    assert!(
        !stage_path.exists(),
        "rejected opaque stage retires without export"
    );
    assert_eq!(foreign_budget.used_bytes(), baseline);
    assert_eq!(foreign_resources.logical_bytes_used(), 0);
    guard.discard().unwrap();
    restored(&runtime);
}
