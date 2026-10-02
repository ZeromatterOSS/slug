//! Actual normal issued data -> exact leaf capture -> SAME restored view.
use std::os::fd::AsRawFd;

use selected_label_data::tests::enabled;
use selected_label_data::tests::fixture;
use slug_bzlmod_v2::GuestLabelDataPolicy;
use slug_loading_v2::SelectedDataLabelBinding;
use slug_workspace_v2::InputImageEntryKind;

use super::*;
use crate::runtime::input_image;

fn run(
    f: &Fixture,
    p: &RuntimeSourceDeclarationPolicy,
    k: &SelectedGuestNamespaceFactsKey,
    g: GuestNamespacePolicy,
) -> PreparedGuestNamespaceViewLease {
    f.runtime
        .prepare_selected_guest_namespace_view(k.clone(), Some(p.dupe()), Some(g))
        .unwrap()
}
fn originals(view: &PreparedGuestNamespaceViewLease) -> Vec<String> {
    view.facts()
        .data_sources()
        .unwrap()
        .requests()
        .requests()
        .map(|r| {
            r.selection()
                .selected_file()
                .as_path()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}
fn stage(view: &PreparedGuestNamespaceViewLease, index: usize) -> PathBuf {
    view.0
        .get()
        .data
        .as_ref()
        .unwrap()
        .test_path(index)
        .unwrap()
}

#[test]
fn data_view_config_only_root_and_genuine_writer_original_reads_without_sibling_tree() {
    let (f, _host, p, k) = fixture("//:data");
    let huge = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(f.workspace.path().join("unrelated-huge"))
        .unwrap();
    huge.set_len(8u64 << 30).unwrap();
    drop(huge);
    fs::set_permissions(
        f.workspace.path().join("data"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    let view = run(&f, &p, &k, enabled(2));
    let paths = originals(&view);
    assert_eq!(paths.len(), 2);
    let data = view.facts().data_sources().unwrap();
    assert!(
        data.requests()
            .requests()
            .next()
            .unwrap()
            .selection()
            .repository()
            .is_none()
    );
    assert!(
        data.requests()
            .requests()
            .nth(1)
            .unwrap()
            .selection()
            .repository()
            .is_some()
    );
    assert_eq!(data.profile().tool_sources().sources().len(), 1);
    assert!(
        data.profile()
            .tool_sources()
            .sources()
            .next()
            .unwrap()
            .scratch_result()
            .is_some()
    );
    for (row, source) in data.requests().requests().zip(data.sources()) {
        let SelectedDataLabelBinding::Leaf { image } = source.binding() else {
            panic!("config-only data must use exact files")
        };
        assert!(image.is_regular_file_root());
        assert_eq!(image.entries().len(), 1);
        assert_eq!(
            image.entries()[0].path().as_path(),
            row.selection().selected_file().as_path()
        );
        assert!(matches!(
            image.entries()[0].kind(),
            InputImageEntryKind::RegularFile(_)
        ));
        assert!(image.observations().observations().keys().all(|d|!matches!(d.operation(),PathObservationOperation::InputImage(scope) if matches!(scope.operation(),slug_workspace_v2::ImageObservationOperation::BoundedDirectory(_)))));
    }
    f.restored();
    assert_eq!(read(&view, &paths[0]), b"root-A");
    assert_eq!(read(&view, &paths[1]), b"generated-data");
    assert_eq!(
        view.lookup(&paths[0], GuestNamespaceOperation::Stat)
            .unwrap()
            .mode(),
        0o640
    );
    let config = read(&view, "/__slug_module/config.json");
    assert_eq!(config, format!("{}\n{}", paths[0], paths[1]).as_bytes());
    assert!(
        f.runtime.image_resources.logical_bytes_used() < 4096,
        "8GiB unrelated sibling must not enter any captured image"
    );
    assert_eq!(
        fs::metadata(stage(&view, 0)).unwrap().permissions().mode() & 0o7777,
        0o640
    );
    let captured = stage(&view, 0);
    let unused = view.0.get().data.as_ref().unwrap().dupe();
    let clone = view.dupe();
    let logical = f.runtime.image_resources.logical_bytes_used();
    drop(view);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), logical);
    assert!(captured.exists());
    drop(clone);
    assert!(
        captured.exists(),
        "separate retained exact-file lease owns the file"
    );
    drop(unused);
    assert!(!captured.exists());
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn data_view_sparse_parents_unknown_and_listing_are_sticky_unowned() {
    let (f, _host, p, k) = fixture("//:data");
    for listing in [false, true] {
        let view = run(&f, &p, &k, enabled(2));
        let paths = originals(&view);
        let parent = Path::new(&paths[0]).parent().unwrap().to_str().unwrap();
        assert!(matches!(
            view.lookup(parent, GuestNamespaceOperation::Lstat)
                .unwrap()
                .binding(),
            GuestNamespaceBinding::DataScaffold
        ));
        assert_eq!(read(&view, &paths[0]), b"root-A");
        let clone = view.dupe();
        let result = if listing {
            view.list_directory(parent).map(|_| ())
        } else {
            view.lookup(&format!("{parent}/unissued"), GuestNamespaceOperation::Stat)
                .map(|_| ())
        };
        assert!(matches!(
            result,
            Err(ViewError::Lookup(GuestLookupError::Unowned))
        ));
        assert!(!clone.is_valid());
        assert!(matches!(
            clone.lookup(&paths[0], GuestNamespaceOperation::Stat),
            Err(ViewError::Invalid)
        ));
        drop(view);
        drop(clone);
        f.restored();
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    }
}

#[test]
fn data_view_current_source_mode_frontier_policy_a_b_a_and_stale_refusal() {
    let (f, _host, p, k) = fixture("//:data");
    let source = f.workspace.path().join("data");
    let a = run(&f, &p, &k, enabled(2));
    let paths = originals(&a);
    fs::write(&source, "root-B").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let b = run(&f, &p, &k, enabled(2));
    assert_eq!(read(&b, &paths[0]), b"root-B");
    assert_eq!(
        b.lookup(&paths[0], GuestNamespaceOperation::Stat)
            .unwrap()
            .mode(),
        0o640
    );
    assert_eq!(
        read(&a, &paths[0]),
        b"root-A",
        "historical view reads its own immutable captured bytes"
    );
    assert_ne!(
        a.facts().data_sources().unwrap(),
        b.facts().data_sources().unwrap()
    );
    fs::write(&source, "root-A").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    let again = run(&f, &p, &k, enabled(2));
    assert_eq!(read(&again, &paths[0]), b"root-A");
    assert_eq!(
        a.facts()
            .data_sources()
            .unwrap()
            .sources()
            .next()
            .unwrap()
            .leaf_image()
            .unwrap()
            .content(),
        again
            .facts()
            .data_sources()
            .unwrap()
            .sources()
            .next()
            .unwrap()
            .leaf_image()
            .unwrap()
            .content()
    );
    let too_small = f.runtime.prepare_selected_guest_namespace_view(
        k.clone(),
        Some(p.dupe()),
        Some(enabled(1)),
    );
    assert!(too_small.is_err());
    f.restored();
    let changed = Cell::new(false);
    let stale = f.runtime.prepare_selected_guest_namespace_view_inner(
        k.clone(),
        Some(p.dupe()),
        Some(enabled(2)),
        Some(&|phase| {
            if matches!(phase, selected_tool_source::ToolPhase::BeforeCapture)
                && !changed.replace(true)
            {
                fs::write(
                    f.workspace.path().join("BUILD.bazel"),
                    "exports_files(['marker','data'])\n# changed after normal facts\n",
                )
                .unwrap();
            }
            Ok(())
        }),
    );
    assert!(changed.get());
    assert!(stale.is_err());
    f.restored();
    let repaired = run(&f, &p, &k, enabled(2));
    assert_eq!(read(&repaired, &paths[0]), b"root-A");
    drop(a);
    drop(b);
    drop(again);
    drop(repaired);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn data_view_file_tamper_and_after_first_chunk_cancel_poison_shared_view() {
    let (f, _host, p, k) = fixture("//:data");
    for mutation in 0..4 {
        let view = run(&f, &p, &k, enabled(2));
        let paths = originals(&view);
        let leaf = stage(&view, 0);
        match mutation {
            0 => fs::write(&leaf, "evil-A").unwrap(),
            1 => fs::write(&leaf, "different length").unwrap(),
            2 => fs::set_permissions(&leaf, fs::Permissions::from_mode(0o600)).unwrap(),
            _ => {
                fs::remove_file(&leaf).unwrap();
                fs::create_dir(&leaf).unwrap();
            }
        }
        let clone = view.dupe();
        assert!(
            view.read_regular(&paths[0], &mut |_| Ok(()), &|| true)
                .is_err()
        );
        assert!(!clone.is_valid());
        drop(clone);
        drop(view);
        f.restored();
        assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    }
    fs::write(f.workspace.path().join("data"), vec![7; 128 * 1024]).unwrap();
    let view = run(&f, &p, &k, enabled(2));
    let paths = originals(&view);
    let before = view.budget().used_bytes();
    let active = Cell::new(true);
    let chunks = Cell::new(0usize);
    let result = view.read_regular(
        &paths[0],
        &mut |bytes| {
            assert_eq!(bytes.len(), 64 * 1024);
            chunks.set(chunks.get() + 1);
            active.set(false);
            Ok(())
        },
        &|| active.get(),
    );
    assert!(matches!(result, Err(ViewError::Cancelled)));
    assert_eq!(chunks.get(), 1);
    assert!(!view.is_valid());
    assert_eq!(view.budget().used_bytes(), before);
    drop(view);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
}

#[test]
fn data_view_descriptor_and_logical_pressure_repair_and_history_last_drop() {
    let (f, host, p, k) = fixture("//:data");
    let budget = f.runtime.image_resources.descriptor_budget().dupe();
    let payload = f.runtime.scratch_resources.payload_budget().dupe();
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert!(
        f.runtime
            .prepare_selected_guest_namespace_view(k.clone(), Some(p.dupe()), Some(enabled(2)))
            .is_err()
    );
    drop(held);
    f.restored();
    let logical = f
        .runtime
        .image_resources
        .reserve_logical(2u64 << 30)
        .unwrap();
    assert!(
        f.runtime
            .prepare_selected_guest_namespace_view(k.clone(), Some(p.dupe()), Some(enabled(2)))
            .is_err()
    );
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 2u64 << 30);
    drop(logical);
    f.restored();
    let view = run(&f, &p, &k, enabled(2));
    let path = originals(&view)[0].clone();
    let stored = stage(&view, 0);
    let clone = view.dupe();
    let charged = budget.used_bytes();
    assert!(charged > 0);
    drop(view);
    assert!(stored.exists());
    assert_eq!(budget.used_bytes(), charged);
    drop(p);
    drop(k);
    drop(host);
    drop(f);
    assert_eq!(read(&clone, &path), b"root-A");
    assert!(stored.exists());
    assert!(budget.used_bytes() > 0);
    assert!(payload.used_bytes() > 0);
    drop(clone);
    assert!(!stored.exists());
    retire(&payload, &budget);
}

#[test]
fn data_view_covered_tool_directories_and_relocated_runtime_file_reuse_captures() {
    let (f, _download, _host, p, k, _generator) = composite_fixture();
    // Existing marker, dirname and child witnesses all belong to the admitted
    // complete original-spelling tool tree. No exact-file copy is appropriate.
    let view = run(&f, &p, &k, enabled(8));
    let data = view.facts().data_sources().unwrap();
    assert!(data.sources().len() >= 3);
    assert!(
        data.sources()
            .all(|s| matches!(s.binding(), SelectedDataLabelBinding::ToolMember { .. }))
    );
    assert!(view.0.get().data.as_ref().unwrap().test_path(0).is_none());
    let dir = data
        .requests()
        .requests()
        .find(|r| {
            data.sources().nth(r.occurrence_index()).is_some_and(|s| {
                data.member(s).is_some_and(|(image, index)| {
                    matches!(
                        image.entries()[index].kind(),
                        InputImageEntryKind::Directory
                    )
                })
            })
        })
        .unwrap()
        .selection()
        .selected_file()
        .as_path()
        .to_str()
        .unwrap();
    assert!(!view.list_directory(dir).unwrap().is_empty());
    let node = view.lookup(dir, GuestNamespaceOperation::Lstat).unwrap();
    let GuestNamespaceBinding::ToolImage { role, entry } = node.binding() else {
        panic!("covered directory must retain its authentic complete tool binding")
    };
    let actual = view
        .with_bound_source(node, |root, relative| {
            Ok((root.as_raw_fd(), relative.to_owned()))
        })
        .unwrap();
    let expected = view
        .0
        .get()
        .sources
        .tools()
        .with_image_root(role, |root, image| {
            let member = &image.entries()[entry];
            (
                root.as_raw_fd(),
                member
                    .path()
                    .as_path()
                    .strip_prefix(image.source_root().as_path())
                    .unwrap()
                    .to_owned(),
            )
        })
        .unwrap();
    assert_eq!(
        actual, expected,
        "directory data must borrow the very same retained tool root FD/member"
    );
    f.restored();
    drop(view);
    let (f, _host, _p, k) = fixture("//runtime:asset");
    fs::create_dir(f.workspace.path().join("runtime")).unwrap();
    fs::write(
        f.workspace.path().join("runtime/BUILD.bazel"),
        "exports_files(['asset'])\n",
    )
    .unwrap();
    fs::write(f.workspace.path().join("runtime/asset"), "runtime-data").unwrap();
    let budget = f.runtime.image_resources.descriptor_budget();
    let workspace = ImageChargedPath::pin_existing(
        f.runtime.repository_materializer.workspace().dupe(),
        budget,
    )
    .unwrap();
    let p = RuntimeSourceDeclarationPolicy::new(
        &workspace,
        &[RuntimeSourceDeclarationInput {
            source: RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
                relative: "runtime",
            },
            guest_prefix: "/relocated",
        }],
        RuntimeSourceLimits::default(),
        budget,
    )
    .unwrap();
    let view = run(&f, &p, &k, enabled(2));
    let data = view.facts().data_sources().unwrap();
    assert!(matches!(
        data.sources().next().unwrap().binding(),
        SelectedDataLabelBinding::RuntimeMember { declaration: 0, .. }
    ));
    assert!(view.0.get().data.as_ref().unwrap().test_path(0).is_none());
    let original = originals(&view)[0].clone();
    assert_eq!(read(&view, &original), b"runtime-data");
    assert_eq!(read(&view, "/relocated/asset"), b"runtime-data");
    let a = view
        .lookup(&original, GuestNamespaceOperation::Lstat)
        .unwrap();
    let b = view
        .lookup("/relocated/asset", GuestNamespaceOperation::Lstat)
        .unwrap();
    assert!(matches!(
        a.binding(),
        GuestNamespaceBinding::RuntimeImage { declaration: 0, .. }
    ));
    assert_eq!(a.kind(), b.kind());
    let original_source = view
        .with_bound_source(a, |root, relative| {
            Ok((root.as_raw_fd(), relative.to_owned()))
        })
        .unwrap();
    let relocated_source = view
        .with_bound_source(b, |root, relative| {
            Ok((root.as_raw_fd(), relative.to_owned()))
        })
        .unwrap();
    let runtime_fd = view
        .0
        .get()
        .sources
        .with_image_root(0, |root, _| root.as_raw_fd())
        .unwrap();
    assert_eq!(original_source, (runtime_fd, PathBuf::from("asset")));
    assert_eq!(
        original_source, relocated_source,
        "original leaf and configured prefix must share exact runtime FD and relative member"
    );
    f.restored();
}

#[test]
fn data_file_capture_rejects_directory_adapter_and_foreign_stage_before_source_io() {
    let (f, _host, p, k) = fixture("//:data");
    let view = run(&f, &p, &k, enabled(2));
    let image = view
        .facts()
        .data_sources()
        .unwrap()
        .sources()
        .next()
        .unwrap()
        .leaf_image()
        .unwrap();
    let logical = f.runtime.image_resources.logical_bytes_used();
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let source = || {
        f.runtime
            .repository_materializer
            .image_source(
                guard.command().repository_session,
                image.namespace(),
                image.anchor_root(),
                None,
            )
            .unwrap()
    };
    let prepared = input_image::prepare_input_image_stage(&f.runtime.image_resources).unwrap();
    let path = prepared.test_path().to_owned();
    assert!(matches!(
        input_image::capture_prepared_input_image(
            input_image::AuthorizedObservedInputImage::new(source(), image.dupe()),
            &f.runtime.image_resources,
            &|| true,
            prepared
        ),
        Err(input_image::InputImageCaptureError::Unauthorized)
    ));
    assert!(!path.exists());
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), logical);
    let foreign_budget = slug_workspace_v2::ImageDescriptorBudget::new(1 << 20);
    let foreign = input_image::WorkspaceImageResources::new(foreign_budget.dupe(), 1 << 20);
    let prepared = input_image::prepare_input_image_stage(&foreign).unwrap();
    let path = prepared.test_path().to_owned();
    let authorized = input_image::AuthorizedObservedInputImage::new(source(), image.dupe());
    struct Restore {
        old: PathBuf,
        new: PathBuf,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::rename(&self.new, &self.old);
        }
    }
    let moved = f.workspace.path().with_extension("foreign-stage-hidden");
    fs::rename(f.workspace.path(), &moved).unwrap();
    let restore = Restore {
        old: f.workspace.path().to_owned(),
        new: moved,
    };
    let result = input_image::capture_prepared_input_file(
        authorized,
        &f.runtime.image_resources,
        &|| true,
        prepared,
    );
    drop(restore);
    assert!(
        matches!(
            result,
            Err(input_image::InputImageCaptureError::Unauthorized)
        ),
        "foreign stage must deny before stale named anchor IO"
    );
    assert!(!path.exists());
    assert_eq!(foreign_budget.used_bytes(), 0);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), logical);
    guard.discard().unwrap();
    f.restored();
}

#[test]
fn data_file_capture_cancel_after_written_chunk_releases_stage_and_logical_credit() {
    let (f, _host, p, k) = fixture("//:data");
    fs::write(f.workspace.path().join("data"), vec![7; 128 * 1024]).unwrap();
    let view = run(&f, &p, &k, enabled(2));
    let image = view
        .facts()
        .data_sources()
        .unwrap()
        .sources()
        .next()
        .unwrap()
        .leaf_image()
        .unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        f.runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    let source = f
        .runtime
        .repository_materializer
        .image_source(
            guard.command().repository_session,
            image.namespace(),
            image.anchor_root(),
            None,
        )
        .unwrap();
    let stage = input_image::prepare_input_image_stage(&f.runtime.image_resources).unwrap();
    let output = stage.test_path().to_owned();
    let credit_before = f.runtime.image_resources.descriptor_budget().used_bytes();
    let logical_before = f.runtime.image_resources.logical_bytes_used();
    let seen = Cell::new(false);
    let active = || {
        if fs::metadata(output.join("leaf")).is_ok_and(|m| m.len() >= 64 * 1024) {
            seen.set(true);
            false
        } else {
            true
        }
    };
    let result = input_image::capture_prepared_input_file(
        input_image::AuthorizedObservedInputImage::new(source, image.dupe()),
        &f.runtime.image_resources,
        &active,
        stage,
    );
    assert!(seen.get(), "cancel only after real staged bytes exist");
    assert!(matches!(
        result,
        Err(input_image::InputImageCaptureError::Cancelled)
    ));
    assert!(!output.exists());
    assert_eq!(
        f.runtime.image_resources.logical_bytes_used(),
        logical_before
    );
    assert_eq!(
        f.runtime.image_resources.descriptor_budget().used_bytes(),
        credit_before - 4352
    );
    guard.discard().unwrap();
    drop(view);
    assert_eq!(f.runtime.image_resources.logical_bytes_used(), 0);
    f.restored();
}

#[test]
fn data_view_enabled_some_success_and_bound_failure_restore_full_prior_some() {
    let (f, _host, p, k) = fixture("//:data");
    let a = enabled(64);
    let b = enabled(4);
    let (mut seed, _, reachable) = runtime_sources::tests::current_plan(&f, &p, k.profile_key());
    let mut inputs = seed.command().inputs.clone();
    inputs.request.guest_namespace = Some(a);
    seed.command.as_mut().unwrap().inputs = inputs;
    seed.accept_selected_for_test(reachable).unwrap();
    drop(seed);
    let prior = f
        .runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(prior.inputs.request.guest_namespace, Some(a));
    let assert_restored = || {
        assert_eq!(
            f.runtime
                .native_demand_sessions
                .state
                .lock()
                .unwrap()
                .accepted
                .inputs,
            prior.inputs
        );
        f.runtime.runtime.block_on(async {
            let updater = f
                .runtime
                .dice
                .updater_with_data(f.runtime.user_computation_data(None).unwrap());
            let mut tx = updater.existing_state().await;
            f.runtime.request_revision.bind_cycle_detector(&tx);
            let raw = RawGuestNamespacePolicyKey::new(
                f.runtime.repository_materializer.workspace().dupe(),
            );
            assert_eq!(tx.compute(&raw).await.unwrap(), Some(a));
            drop(tx);
        });
        f.runtime.runtime.block_on(f.runtime.dice.wait_for_idle());
    };
    let view = run(&f, &p, &k, b);
    assert_eq!(view.facts().policy(), b);
    assert_eq!(view.facts().data_sources().unwrap().requests().policy(), b);
    let paths = originals(&view);
    assert_eq!(read(&view, &paths[0]), b"root-A");
    assert_eq!(read(&view, &paths[1]), b"generated-data");
    assert_restored();
    let failed = f.runtime.prepare_selected_guest_namespace_view(
        k.clone(),
        Some(p.dupe()),
        Some(enabled(1)),
    );
    let Err(NativeDemandSessionError::Computation(error)) = failed else {
        panic!("two genuine witnesses must fail the one-witness policy")
    };
    assert!(matches!(
        error.downcast_ref::<slug_loading_v2::SelectedDataLabelError>(),
        Some(slug_loading_v2::SelectedDataLabelError::WitnessLimit)
    ));
    assert_restored();
    // The rejected attempt cannot revoke the already exported immutable view.
    assert_eq!(read(&view, &paths[0]), b"root-A");
    assert!(view.is_valid());
}
