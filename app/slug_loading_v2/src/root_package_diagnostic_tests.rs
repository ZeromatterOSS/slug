use std::fmt::Write;

use super::*;

fn root_label() -> HostRootBzlLabel {
    HostRootBzlLabel::new(
        PackagePath::parse("pkg").unwrap(),
        RootPackageBzlTarget::parse("root.bzl").unwrap(),
    )
}

#[test]
#[rustfmt::skip]
fn root_package_diagnostic_preserves_borrowed_stage_and_nested_cause() {
    use RootPackageLoadErrorInner as E;
    let package = PackagePath::parse("pkg").unwrap();
    for (inner, stage, detail) in [
        (E::Parse { package: package.clone(), message: "CAUSE".into() }, "parse", "//pkg: CAUSE"),
        (E::RepositoryMapping { message: "CAUSE".into() }, "repository_mapping", "CAUSE"),
        (E::RepositoryMappingInfrastructure { message: "CAUSE".into() }, "repository_mapping_infrastructure", "CAUSE"),
        (E::ExternalInfrastructure { load: "@repo//:ext.bzl".into(), message: "CAUSE".into() }, "external_infrastructure", "@repo//:ext.bzl: CAUSE"),
        (E::LoadLabel { package: package.clone(), error: HostLoadLabelError::Invalid { load: "POISON".into(), message: "POISON".into() } }, "load_label", "//pkg [diagnostic incomplete: LoadLabel]"),
        (E::Input(HostSourceInputError::UnsupportedSourceEncoding { logical_path: NormalizedAbsolutePath::new("/POISON").unwrap() }), "input", "UnsupportedSourceEncoding path=hex:2f504f49534f4e"),
        (E::Attempt(HostPackageAttemptError::Loading(LoadingError::new("CAUSE"))), "attempt_loading", "CAUSE"),
        (E::Attempt(HostPackageAttemptError::Invariant("CAUSE")), "attempt_invariant", "CAUSE"),
        (E::Attempt(HostPackageAttemptError::Glob(crate::package::HostGlobAttemptError::UnsupportedPath { path: Arc::from(b"POISON".as_slice()) })), "attempt_glob", "[diagnostic incomplete: Attempt.Glob]"),
    ] {
        let owner = RootPackageLoadError::new(inner);
        assert_eq!(owner.diagnostic_stage(), stage);
        assert_eq!(owner.diagnostic().to_string(), format!("{stage}: {detail}"));
        assert!(!owner.diagnostic().to_string().contains("POISON"));
    }

    let message: Arc<str> = Arc::from("CAUSE");
    let message_weak = Arc::downgrade(&message);
    let leaf = Arc::new(HostBzlModuleError::Parse { label: root_label(), message });
    let leaf_weak = Arc::downgrade(&leaf);
    let child = Arc::new(HostBzlModuleError::Child { load: ":child.bzl".into(), label: root_label(), error: leaf });
    let owner = Arc::new(RootPackageLoadError::new(E::Bzl { origin: "pkg/BUILD.bazel".into(), load: ":root.bzl".into(), label: root_label(), error: child.clone() }));
    let owner_weak = Arc::downgrade(&owner);
    let RootPackageLoadDiagnostic::Bzl { origin, load, error } = owner.diagnostic_view() else { panic!("borrowed bzl view") };
    assert_eq!((origin, load), ("pkg/BUILD.bazel", ":root.bzl"));
    assert!(std::ptr::eq(error, child.as_ref()));
    let HostBzlModuleError::Child { error: retained_leaf, .. } = error else { panic!("child") };
    assert_eq!(retained_leaf.as_ref() as *const _, leaf_weak.as_ptr());
    let HostBzlModuleError::Parse { message, .. } = retained_leaf.as_ref() else { panic!("leaf") };
    assert_eq!(message.as_ptr(), message_weak.as_ptr() as *const u8);
    let count = Arc::strong_count(&child);
    let diagnostic = owner.diagnostic();
    assert_eq!(Arc::strong_count(&child), count);
    assert_eq!(diagnostic.to_string(), "bzl: pkg/BUILD.bazel load :root.bzl: RootBzl: Child :child.bzl: RootBzl: Parse: CAUSE");
    drop(diagnostic);
    drop(child);
    assert!(leaf_weak.upgrade().is_some());
    drop(owner);
    assert!(owner_weak.upgrade().is_none() && leaf_weak.upgrade().is_none() && message_weak.upgrade().is_none());

    let label = CanonicalLabel::parse("@@repo+//pkg:ext.bzl").unwrap();
    let error = Arc::new(ExternalBzlModuleError::Evaluation { label: label.clone(), message: "CAUSE".into() });
    let weak = Arc::downgrade(&error);
    let owner = RootPackageLoadError::new(E::ExternalBzl { origin: "pkg/BUILD.bazel".into(), load: "@repo//pkg:ext.bzl".into(), label, error });
    let RootPackageLoadDiagnostic::ExternalBzl { error, .. } = owner.diagnostic_view() else { panic!("external view") };
    assert_eq!(error as *const _, weak.as_ptr());
    assert_eq!(owner.diagnostic_stage(), "external_bzl");
    assert_eq!(owner.diagnostic().to_string(), "external_bzl: pkg/BUILD.bazel load @repo//pkg:ext.bzl @@repo+//pkg:ext.bzl: ExternalBzl: Evaluation: CAUSE");
    drop(owner);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn root_package_diagnostic_escapes_bounds_and_retires_original_owner() {
    use RootPackageLoadErrorInner as E;
    let owner = RootPackageLoadError::new(E::Attempt(HostPackageAttemptError::Loading(
        LoadingError::new("quoted \"cause\"\n\u{e9}\t"),
    )));
    assert_eq!(
        owner.diagnostic().to_string(),
        "attempt_loading: quoted \"cause\"\\n\\u{e9}\\t"
    );
    let message: Arc<str> = Arc::from(format!("{}TAIL_POISON", "\u{e9}".repeat(3072)));
    let weak = Arc::downgrade(&message);
    let owner = RootPackageLoadError::new(E::RepositoryMapping { message });
    let text = owner.diagnostic().to_string();
    assert!(
        text.is_ascii()
            && text.len() <= 3072
            && text.ends_with("[diagnostic incomplete: output limit]")
            && !text.contains("TAIL_POISON")
    );
    drop(owner);
    assert!(weak.upgrade().is_none());

    let mut error = Arc::new(HostBzlModuleError::Parse {
        label: root_label(),
        message: "LEAF".into(),
    });
    let weak = Arc::downgrade(&error);
    for _ in 0..31 {
        error = Arc::new(HostBzlModuleError::Child {
            load: "x".into(),
            label: root_label(),
            error,
        });
    }
    let owner = RootPackageLoadError::new(E::Bzl {
        origin: "pkg/BUILD".into(),
        load: ":root.bzl".into(),
        label: root_label(),
        error: error.clone(),
    });
    assert!(owner.diagnostic().to_string().ends_with("Parse: LEAF"));
    drop(owner);
    error = Arc::new(HostBzlModuleError::Child {
        load: "x".into(),
        label: root_label(),
        error,
    });
    let owner = RootPackageLoadError::new(E::Bzl {
        origin: "pkg/BUILD".into(),
        load: ":root.bzl".into(),
        label: root_label(),
        error,
    });
    let text = owner.diagnostic().to_string();
    assert!(text.ends_with("[diagnostic incomplete: depth limit]") && !text.contains("LEAF"));
    struct Reject;
    impl std::fmt::Write for Reject {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    assert!(write!(&mut Reject, "{}", owner.diagnostic()).is_err());
    drop(owner);
    assert!(weak.upgrade().is_none());

    let owner = RootPackageLoadError::new(E::Bzl {
        origin: "pkg/BUILD".into(),
        load: ":root.bzl".into(),
        label: root_label(),
        error: Arc::new(HostBzlModuleError::Cycle(
            crate::cycle_detector::HostBzlLoadCycle {
                path: Arc::from([]),
                keys: Arc::from([]),
            },
        )),
    });
    assert_eq!(
        owner.diagnostic().to_string(),
        "bzl: pkg/BUILD load :root.bzl: RootBzl: Cycle"
    );

    use dice::UserCycleDetectorGuard;

    use crate::dependency_cycle::DependencyCycleGraph;
    let key = dice::DynKey::from_key(
        crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteKey::new(
            NormalizedAbsolutePath::new("/CYCLE_KEY_POISON").unwrap(),
            slug_identity_v2::CanonicalRepoName::new("cycle_key_poison").unwrap(),
        ),
    );
    let graph = Arc::new(DependencyCycleGraph::default());
    let graph_weak = Arc::downgrade(&graph);
    let guard = graph.start(&key).unwrap();
    guard.add_edge(&key);
    let cycle = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        guard.guard_this(std::future::pending::<()>()),
    )
    .await
    .expect("self-cycle notification is immediate")
    .unwrap_err();
    let error = Arc::new(ExternalBzlModuleError::DependencyCycle(cycle));
    let error_weak = Arc::downgrade(&error);
    // Legacy presentation retains its bounded key detail; the new strict
    // facade must not visit those identities even when the same error is held.
    assert!(error.to_string().contains("CYCLE_KEY_POISON"));
    let owner = RootPackageLoadError::new(E::ExternalBzl {
        origin: "pkg/BUILD".into(),
        load: "@repo//:ext.bzl".into(),
        label: CanonicalLabel::parse("@@repo+//:ext.bzl").unwrap(),
        error,
    });
    let RootPackageLoadDiagnostic::ExternalBzl { error, .. } = owner.diagnostic_view() else {
        panic!("borrowed dependency cycle")
    };
    assert_eq!(error as *const _, error_weak.as_ptr());
    let text = owner.diagnostic().to_string();
    assert_eq!(
        text,
        "external_bzl: pkg/BUILD load @repo//:ext.bzl @@repo+//:ext.bzl: ExternalBzl: DependencyCycle"
    );
    assert!(!text.contains("POISON") && !text.contains("cycle_key_poison"));
    drop(guard);
    drop(graph);
    assert!(graph_weak.upgrade().is_none());
    drop(owner);
    assert!(error_weak.upgrade().is_none());
}
