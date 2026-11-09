use super::*;

fn assert_leaf(error: &ModuleRegistrationExpansionError, expected: &str) -> &'static str {
    let leaf = error.terminal_leaf();
    assert_eq!(leaf, expected);
    assert!(leaf.is_ascii() && leaf.len() <= 64);
    assert!(format!(r#"{{"terminal_leaf":"{leaf}"}}"#).len() <= 96);
    leaf
}

fn with_innate(
    missing: &ModuleRegistrationExpansionError,
    error: Innate,
) -> ModuleRegistrationExpansionError {
    generated_from_missing(
        missing,
        Generated::Loading(HostSelectedExtensionOwnerCertificateError(
            Certificate::InnatePure(error),
        )),
    )
}

#[tokio::test]
async fn terminal_leaf_opaque_payloads_preserve_identity_and_borrow_actual_owners() {
    let missing =
        crate::registration_expansion_tests::registration_diagnostic_missing_route("GRAPH_POISON")
            .await;
    assert_leaf(&missing, "route_missing");
    let Registration::CanonicalRoute(route) = missing.kind() else {
        panic!("expected actual canonical route");
    };
    let certificate = effect_certificate().await;
    let weak = Arc::downgrade(&certificate);
    let wrap = |effect| {
        ModuleRegistrationExpansionError::diagnostic_test(Registration::CanonicalRoute(
            HostCanonicalRepositoryLoadRouteError {
                canonical_repo: route.canonical_repo.clone(),
                kind: Load::Effect(effect),
            },
        ))
    };
    let a = wrap(Effect::Invocation {
        certificate: certificate.clone(),
        ordinal: 0,
        message: "opaque URL argv environment SECRET\n\0λ".into(),
    });
    let b = wrap(Effect::Invocation {
        certificate: certificate.clone(),
        ordinal: 7,
        message: "different opaque payload".repeat(4096).into(),
    });
    assert_ne!(a, b, "the static tag must not replace full error equality");
    let count = Arc::strong_count(&certificate);
    let retained_tag = assert_leaf(&a, "effect_invocation_opaque");
    assert_leaf(&b, retained_tag);
    assert_eq!(Arc::strong_count(&certificate), count);
    let path = wrap(Effect::Path {
        certificate: certificate.clone(),
        ordinal: 0,
        error: Plan::InvalidPath("PRIVATE_PATH".into()),
    });
    assert_leaf(&path, "plan_invalid_path");
    // Classifying a retained certificate neither acquires nor extends custody.
    drop(certificate);
    assert!(weak.upgrade().is_some());
    drop((a, b, path));
    assert!(weak.upgrade().is_none());
    assert_eq!(retained_tag, "effect_invocation_opaque");

    for (error, owner) in
        crate::registration_expansion_tests::registration_diagnostic_bzl_errors().await
    {
        let expected = match owner {
            "Source" => "host_bzl_source_opaque",
            "Evaluation" => "host_bzl_evaluation_opaque",
            "SourceObservationLegacy" | "SourceObservationObserved" => {
                "external_bzl_source_observation_opaque"
            }
            _ => panic!("unexpected natural fixture owner"),
        };
        assert_leaf(&with_innate(&missing, error), expected);
    }
    let cycle = crate::cycle_detector::HostBzlLoadCycle {
        path: Arc::from([]),
        keys: Arc::from([]),
    };
    assert_leaf(
        &with_innate(&missing, Innate::RootBzl(HostBzlModuleError::Cycle(cycle))),
        "host_bzl_cycle",
    );
    for payload in ["private", "unknown\nURL SECRET"] {
        let error =
            ModuleRegistrationExpansionError::diagnostic_test(Registration::CanonicalPackage(
                crate::bzl_module::RepositoryPackageLoadError::diagnostic_test_attempt_loading(
                    payload,
                ),
            ));
        assert_leaf(&error, "repository_package_loading_opaque");
    }
}

#[tokio::test]
async fn terminal_leaf_factored_budget_and_noncausal_prior_custody() {
    use crate::module_extension::HostSelectedExtensionOwnerPureKey;
    use crate::module_extension_repository_instantiation::tests::WORKSPACE;

    let (mut tx, owner) = owner_fixture(false).await;
    let result = complete(
        tx.compute(&HostSelectedExtensionOwnerPureKey::new(
            slug_workspace_v2::NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            owner,
        ))
        .await
        .unwrap(),
    );
    let factors = result.as_ref().as_ref().unwrap().factors.clone();
    let missing =
        crate::registration_expansion_tests::registration_diagnostic_missing_route("P").await;
    let baseline = Arc::strong_count(&factors);
    for (wrappers, expected) in [
        (26, "pure_compute_opaque"),
        (27, "registration_depth_limit"),
    ] {
        let mut cause = Pure::Compute("PRIVATE_LEAF".into());
        for _ in 0..wrappers {
            cause = Pure::Factored {
                factors: factors.clone(),
                error: Arc::new(cause),
            };
        }
        // Registration, Load, Route, Generated, Certificate and terminal Pure
        // consume six nodes, independently of the retained factors' contents.
        let error = generated_from_missing(
            &missing,
            Generated::Loading(HostSelectedExtensionOwnerCertificateError(
                Certificate::Pure(cause),
            )),
        );
        let held = Arc::strong_count(&factors);
        assert_leaf(&error, expected);
        assert_eq!(Arc::strong_count(&factors), held);
        drop(error);
        assert_eq!(Arc::strong_count(&factors), baseline);
    }
    let mut prior = Arc::new(Pure::Compute("NONCAUSAL_POISON".into()));
    for _ in 0..64 {
        prior = Arc::new(Pure::Factored {
            factors: factors.clone(),
            error: prior,
        });
    }
    let weak = Arc::downgrade(&prior);
    let error = generated_from_missing(
        &missing,
        Generated::Loading(HostSelectedExtensionOwnerCertificateError(
            Certificate::Pure(Pure::SourceOperational {
                error: slug_bzlmod_v2::SelectedLabelSourceError::Budget(
                    slug_workspace_v2::ImageDescriptorBudgetError::ResourceLimit,
                ),
                prior,
            }),
        )),
    );
    let tag = assert_leaf(&error, "pure_source_operational_opaque");
    assert_eq!(weak.strong_count(), 1);
    drop(error);
    assert!(weak.upgrade().is_none());
    assert_eq!(Arc::strong_count(&factors), baseline);
    assert_eq!(tag, "pure_source_operational_opaque");
}

#[tokio::test]
async fn terminal_leaf_child_budget_counts_nested_external_route_cause() {
    let missing =
        crate::registration_expansion_tests::registration_diagnostic_missing_route("P").await;
    let label = CanonicalLabel::parse("@@repo//:extension.bzl").unwrap();
    for (wrappers, expected) in [
        (24, "external_bzl_route_compute_opaque"),
        (25, "registration_depth_limit"),
    ] {
        let leaf = Arc::new(ExternalBzlModuleError::Route {
            source: label.clone(),
            load: "PRIVATE_LOAD".into(),
            error: ExternalBzlRouteError::Compute("PRIVATE_CAUSE".into()),
        });
        let weak = Arc::downgrade(&leaf);
        let mut cause = leaf;
        for _ in 0..wrappers {
            cause = Arc::new(ExternalBzlModuleError::Child {
                raw_load: "PRIVATE_CHILD".into(),
                canonical_label: label.clone(),
                error: cause,
            });
        }
        // Six envelope nodes, one ExternalBzl::Route, then its separately
        // budgeted ExternalBzlRoute::Compute: exactly eight plus Child wrappers.
        let error = with_innate(&missing, Innate::ExternalBzl(cause.as_ref().clone()));
        drop(cause);
        let held = weak.strong_count();
        let tag = assert_leaf(&error, expected);
        assert_eq!(weak.strong_count(), held);
        drop(error);
        assert!(weak.upgrade().is_none());
        assert_eq!(tag, expected);
    }
}
