use super::*;
use crate::runtime::materialization_diagnostic::BorrowedHex;
use crate::runtime::source_attempt_context::SourceAttemptContext;

fn source_demand(instance: u64, path: &str) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Materialization(
            slug_workspace_v2::PathObservationInstanceId::new(instance),
        ),
        NormalizedAbsolutePath::new(path).unwrap(),
        PathObservationOperation::FileBytes,
    )
}

#[test]
fn source_attempt_context_preserves_current_owner_phase_and_bounds() {
    let bodies = namespace_test_bodies();
    let workspace = &bodies.roots[0];
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let request = Arc::new(generated_globals_request(
        &bodies,
        small_generated_plan(&[]),
    ));
    let weak = Arc::downgrade(&request);
    let epoch = namespace_test_epoch_at(&bodies, [request.clone()], instance, "/generation");
    let count = Arc::strong_count(&request);
    let demand = source_demand(17, "/generation/globals.bzl");
    for (context, phase, source) in [
        (
            SourceAttemptContext::need_paths(workspace, Some(&epoch)),
            "need_paths",
            "command",
        ),
        (
            SourceAttemptContext::certificate_precheck(workspace, Some(&epoch)),
            "certificate_precheck",
            "selected_completion",
        ),
        (
            SourceAttemptContext::certificate_finalize(workspace, Some(&epoch)),
            "certificate_finalize",
            "selected_finalization",
        ),
    ] {
        use crate::runtime::dice::source_service::SourceChargePhase;
        assert!(matches!(
            (phase, context.charge_phase()),
            ("need_paths", SourceChargePhase::NeedPaths)
                | (
                    "certificate_precheck",
                    SourceChargePhase::CertificatePrecheck
                )
                | (
                    "certificate_finalize",
                    SourceChargePhase::CertificateFinalize
                )
        ));
        let diagnostic = context.diagnostic(&demand);
        assert!(std::ptr::eq(diagnostic.demand, &demand));
        assert_eq!(
            (diagnostic.association, diagnostic.root_relation),
            ("unique", "descendant")
        );
        let owner = diagnostic.owner.as_ref().unwrap();
        for (actual, original) in owner
            .fields
            .iter()
            .zip(MaterializationDiagnostic::new(request.as_ref(), "observation_instance").fields)
        {
            assert_eq!(actual.map(<[u8]>::as_ptr), original.map(<[u8]>::as_ptr));
        }
        assert_eq!(Arc::strong_count(&request), count);
        let encoded = format!("{diagnostic}\n");
        let (header, owner_line) = encoded.split_once('\n').unwrap();
        assert_eq!(
            header,
            format!(
                "WP990_SOURCE_CONTEXT phase={phase} epoch_source={source} association=unique namespace=materialization instance=17 operation=file_bytes root_relation=descendant"
            )
        );
        assert!(header.len() + 1 <= 1024);
        assert_eq!(
            owner_line,
            format!("{owner}\n").replacen("WP982_MATERIALIZATION", "WP990_SOURCE_OWNER", 1)
        );
        assert!(owner_line.len() <= MATERIALIZATION_MARKER_BYTES);
    }
    assert!(matches!(
        SourceAttemptContext::command_preflight(workspace).charge_phase(),
        crate::runtime::dice::source_service::SourceChargePhase::CommandPreflight
    ));
    let context = SourceAttemptContext::need_paths(workspace, Some(&epoch));
    for (path, association, relation) in [
        ("/generation", "unique", "root"),
        ("/", "path_mismatch", "ancestor"),
        ("/generation-other/globals.bzl", "path_mismatch", "outside"),
    ] {
        let demand = source_demand(17, path);
        let diagnostic = context.diagnostic(&demand);
        assert_eq!(
            (diagnostic.association, diagnostic.root_relation),
            (association, relation)
        );
        assert_eq!(diagnostic.owner.is_some(), association == "unique");
        assert_eq!(
            diagnostic.to_string().contains("WP990_SOURCE_OWNER"),
            association == "unique"
        );
    }
    let host = PathObservationDemand::new(
        PathObservationNamespace::Host,
        demand.path().dupe(),
        PathObservationOperation::FileBytes,
    );
    let host = context.diagnostic(&host);
    assert_eq!(
        (host.association, host.root_relation),
        ("host", "not_applicable")
    );
    assert!(host.owner.is_none());
    assert!(
        host.to_string()
            .contains("namespace=host instance=none operation=file_bytes")
    );
    for unavailable in [
        SourceAttemptContext::command_preflight(workspace),
        SourceAttemptContext::need_paths(workspace, None),
        SourceAttemptContext::certificate_precheck(workspace, None),
        SourceAttemptContext::certificate_finalize(workspace, None),
    ] {
        let diagnostic = unavailable.diagnostic(&demand);
        assert_eq!(
            (diagnostic.association, diagnostic.root_relation),
            ("unavailable", "unavailable")
        );
        assert!(diagnostic.owner.is_none());
        assert!(diagnostic.to_string().contains("epoch_source=unavailable"));
    }
    let check = |context: SourceAttemptContext<'_>, demand: &PathObservationDemand, expected| {
        let diagnostic = context.diagnostic(demand);
        assert_eq!(diagnostic.association, expected);
        assert_eq!(diagnostic.root_relation, "unavailable");
        assert!(diagnostic.owner.is_none());
        assert!(!diagnostic.to_string().contains("WP990_SOURCE_OWNER"));
    };
    check(
        SourceAttemptContext::need_paths(&bodies.roots[1], Some(&epoch)),
        &demand,
        "wrong_workspace",
    );
    check(
        SourceAttemptContext::need_paths(workspace, Some(&epoch)),
        &source_demand(0, "/generation/file"),
        "invalid_instance",
    );
    check(
        SourceAttemptContext::need_paths(workspace, Some(&epoch)),
        &source_demand(18, "/generation/file"),
        "missing",
    );
    let mut other = request.as_ref().clone();
    other.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let ambiguous = namespace_test_epoch(&bodies, [request.clone(), Arc::new(other)], instance);
    check(
        SourceAttemptContext::need_paths(workspace, Some(&ambiguous)),
        &demand,
        "ambiguous",
    );
    let invalid = namespace_test_epoch_at(&bodies, [request.clone()], instance, "relative");
    check(
        SourceAttemptContext::need_paths(workspace, Some(&invalid)),
        &demand,
        "invalid_generation_root",
    );
    let many = |length| {
        namespace_test_epoch(
            &bodies,
            (0..length).map(|i| {
                let mut row = request.as_ref().clone();
                row.id.canonical_repo =
                    slug_identity_v2::CanonicalRepoName::new(format!("row{i}+")).unwrap();
                Arc::new(row)
            }),
            instance,
        )
    };
    let exact = many(32);
    check(
        SourceAttemptContext::need_paths(workspace, Some(&exact)),
        &source_demand(18, "/generation/file"),
        "missing",
    );
    let excessive = many(33);
    check(
        SourceAttemptContext::need_paths(workspace, Some(&excessive)),
        &demand,
        "bounds_refused",
    );
    let mut oversized = request.as_ref().clone();
    oversized.repo_spec.rule_id.rule_name = "x".repeat(MATERIALIZATION_FIELD_BYTES + 1).into();
    let bounded = namespace_test_epoch(&bodies, [Arc::new(oversized)], instance);
    let bounded_context = SourceAttemptContext::need_paths(workspace, Some(&bounded));
    let diagnostic = bounded_context.diagnostic(&demand);
    assert_eq!(diagnostic.association, "unique");
    assert!(!diagnostic.owner.as_ref().unwrap().captured());
    let encoded = format!("{diagnostic}\n");
    assert!(encoded.contains("WP990_SOURCE_OWNER capture=bounds_refused"));
    assert_eq!(encoded.matches("_hex=none").count(), 7);
    for bytes in [None, Some(&b""[..]), Some(&b"\xff\n\0"[..])] {
        assert_eq!(
            BorrowedHex(bytes).to_string(),
            RegistryHex(bytes).to_string()
        );
    }
    drop((
        epoch, ambiguous, invalid, exact, excessive, bounded, request,
    ));
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn source_attempt_context_current_epoch_restores_original_owner() {
    let bodies = namespace_test_bodies();
    let workspace = &bodies.roots[0];
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let a = Arc::new(generated_globals_request(
        &bodies,
        small_generated_plan(&[]),
    ));
    let mut b = a.as_ref().clone();
    b.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let b = Arc::new(b);
    let weak_a = Arc::downgrade(&a);
    let weak_b = Arc::downgrade(&b);
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let key = slug_bzlmod_v2::RepositoryMaterializationResultEpochKey {
        workspace: workspace.dupe(),
    };
    let epoch_a = namespace_test_epoch_at(&bodies, [a.clone()], instance, "/generation");
    let epoch_b = namespace_test_epoch_at(&bodies, [b.clone()], instance, "/other-generation");
    let mut held = None;
    for (index, (epoch, request, root)) in [
        (&epoch_a, a.as_ref(), "/generation"),
        (&epoch_b, b.as_ref(), "/other-generation"),
        (&epoch_a, a.as_ref(), "/generation"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(key.clone(), epoch.clone())])
            .unwrap();
        let mut tx = updater.commit().await;
        let current = current_materialization_epoch(&mut tx, workspace)
            .await
            .unwrap();
        let view = current
            .borrowed_instance_for_namespace(workspace, instance, 32)
            .unwrap()
            .unwrap();
        assert!(std::ptr::eq(view.request(), request));
        let original_root = epoch
            .borrowed_instance_for_namespace(workspace, instance, 32)
            .unwrap()
            .unwrap()
            .generation_root();
        assert!(std::ptr::eq(
            view.generation_root().as_os_str().as_bytes().as_ptr(),
            original_root.as_os_str().as_bytes().as_ptr()
        ));
        let request_count = if index == 1 {
            Arc::strong_count(&b)
        } else {
            Arc::strong_count(&a)
        };
        let context = SourceAttemptContext::need_paths(workspace, Some(&current));
        let demand = source_demand(
            17,
            if index == 1 {
                "/other-generation/globals.bzl"
            } else {
                "/generation/globals.bzl"
            },
        );
        let diagnostic = context.diagnostic(&demand);
        assert_eq!(
            (diagnostic.association, diagnostic.root_relation),
            ("unique", "descendant")
        );
        assert_eq!(
            diagnostic.owner.as_ref().unwrap().fields[1]
                .unwrap()
                .as_ptr(),
            request.id.canonical_repo.as_str().as_bytes().as_ptr()
        );
        assert_eq!(view.generation_root(), Path::new(root));
        assert_eq!(
            if index == 1 {
                Arc::strong_count(&b)
            } else {
                Arc::strong_count(&a)
            },
            request_count
        );
        let wrong = source_demand(
            17,
            if index == 1 {
                "/generation/globals.bzl"
            } else {
                "/other-generation/globals.bzl"
            },
        );
        assert_eq!(context.diagnostic(&wrong).association, "path_mismatch");
        if index == 0 {
            held = Some(current);
        }
        if let Some(old) = &held {
            let old_context = SourceAttemptContext::need_paths(workspace, Some(old));
            let old_demand = source_demand(17, "/generation/globals.bzl");
            assert_eq!(
                old_context.diagnostic(&old_demand).owner.unwrap().fields[1]
                    .unwrap()
                    .as_ptr(),
                a.id.canonical_repo.as_str().as_bytes().as_ptr()
            );
        }
    }
    drop((held, epoch_a, epoch_b, a, b));
    assert!(weak_a.upgrade().is_some()); // Original DICE history, not diagnostic retention.
    dice.wait_for_idle().await;
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while weak_a.upgrade().is_some() || weak_b.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("original requests retire after bounded DICE actor shutdown");
    assert!(weak_a.upgrade().is_none());
    assert!(weak_b.upgrade().is_none());
}
