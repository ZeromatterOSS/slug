use std::sync::Arc;

use super::*;

fn render_node(node: Node<'_>) -> String {
    let mut out = Buffer::new();
    let result = walk(&mut out, node);
    assert!(result.is_ok() || out.stopped);
    out.text().to_owned()
}

fn complete<T>(value: slug_bzlmod_v2::SourcePreparationOutcome<T>) -> T {
    match value {
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => value,
        _ => panic!("synthetic owner fixture returned Need"),
    }
}

async fn owner_fixture(
    innate: bool,
) -> (
    dice::DiceTransaction,
    Arc<slug_bzlmod_v2::HostSelectedExtensionOwner>,
) {
    use slug_bzlmod_v2::HostRootRepositoryMappingKey;
    use slug_bzlmod_v2::HostSelectedExtensionDemandKey;

    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    use crate::module_extension_repository_instantiation::tests::transaction_untracked;
    let dice = Arc::new(dice::Dice::builder().build(dice::DetectCycles::Enabled));
    let ordinary = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first')\n";
    let innate_module =
        "module(name='bazel_tools')\nr=use_repo_rule('//:ext.bzl','repo')\nr(name='first')\n";
    let extension = "repo=repository_rule(lambda ctx:None)\ndef impl(ctx):\n    repo(name='first')\next=module_extension(implementation=impl)\n";
    let mut tx = transaction_untracked(
        &dice,
        if innate { innate_module } else { ordinary },
        extension,
        true,
    )
    .await;
    let workspace = slug_workspace_v2::NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let mapping = complete(
        tx.compute(&HostRootRepositoryMappingKey::new(workspace.clone()))
            .await
            .unwrap(),
    );
    let requested = mapping
        .as_ref()
        .as_ref()
        .unwrap()
        .view()
        .unwrap()
        .mapping()
        .find(|(name, _)| name.as_str() == "first")
        .unwrap()
        .1
        .clone();
    let demand = complete(
        tx.compute(&HostSelectedExtensionDemandKey::new(workspace, requested))
            .await
            .unwrap(),
    );
    (tx, Arc::clone(demand.as_ref().as_ref().unwrap().owner()))
}

#[tokio::test]
async fn registration_diagnostic_owner_plans_are_not_causal_text() {
    use crate::module_extension::HostSelectedExtensionOwnerPureKey;
    use crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerKey;
    use crate::module_extension_repository_instantiation::HostInstantiateModuleExtensionRequestError;
    use crate::module_extension_repository_instantiation::instantiate_innate_request;
    use crate::module_extension_repository_instantiation::instantiate_request;
    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    use crate::module_extension_repository_validation::HostModuleExtensionValidationOffender;
    let workspace = slug_workspace_v2::NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let (mut tx, owner) = owner_fixture(false).await;
    let result = complete(
        tx.compute(&HostSelectedExtensionOwnerPureKey::new(
            workspace.clone(),
            owner,
        ))
        .await
        .unwrap(),
    );
    let pure = Arc::new(result.as_ref().as_ref().unwrap().clone());
    let after = |calls| Pure::AfterInputs {
        inputs: Arc::clone(&pure.inputs),
        request: pure.receipt.request.clone(),
        current_calls: calls,
        message: "CAUSE".into(),
    };
    let a = after(Arc::clone(&pure.receipt.repository_rule_calls));
    let b = after(Arc::from([]));
    assert!(a != b);
    assert_eq!(render_node(Node::Pure(&a)), render_node(Node::Pure(&b)));
    assert_eq!(render_node(Node::Pure(&a)), "Pure: AfterInputs: CAUSE");
    let instantiated = instantiate_request(&pure.receipt).unwrap();
    let request_error = || HostInstantiateModuleExtensionRequestError {
        current: Arc::from(instantiated.parts().1),
        call: None,
        error: Instantiate::Attribute("CAUSE".into()),
    };
    let error = HostSelectedExtensionOwnerCertificateError(Certificate::Instantiation {
        pure: Arc::clone(&pure),
        error: request_error(),
    });
    assert_eq!(
        render_node(Node::Certificate(&error)),
        "Loading: Instantiation: Attribute: CAUSE"
    );
    let offender = HostModuleExtensionValidationOffender::Import(
        pure.receipt.request.validation_parts().0[0].clone(),
    );
    for (reason, text) in [
        (ValidationReason::MissingImport, "MissingImport"),
        (ValidationReason::MissingOverride, "MissingOverride"),
        (ValidationReason::InjectCollision, "InjectCollision"),
    ] {
        let error = HostSelectedExtensionOwnerCertificateError(Certificate::Validation {
            pure: Arc::clone(&pure),
            instantiated: instantiated.clone(),
            error: Validate::Validation {
                offender: offender.clone(),
                error: reason,
            },
        });
        assert_eq!(
            render_node(Node::Certificate(&error)),
            format!("Loading: Validation: {text}")
        );
    }
    let (mut tx, owner) = owner_fixture(true).await;
    let result = complete(
        tx.compute(&HostPureInnateRepositoryOwnerKey::new(workspace, owner))
            .await
            .unwrap(),
    );
    let innate = Arc::new(result.as_ref().as_ref().unwrap().clone());
    let instantiated_innate =
        instantiate_innate_request(Arc::clone(&innate.inputs), &innate.repository_rule_calls)
            .unwrap();
    for (error, expected) in [
        (
            Certificate::InnatePure(Innate::Call("CAUSE".into())),
            "Loading: InnatePure: Call: CAUSE",
        ),
        (
            Certificate::Pure(Pure::Compute("CAUSE".into())),
            "Loading: Pure: Compute: CAUSE",
        ),
        (
            Certificate::InnateInstantiation {
                pure: Arc::clone(&innate),
                error: request_error(),
            },
            "Loading: InnateInstantiation: Attribute: CAUSE",
        ),
        (
            Certificate::InnateValidation {
                pure: innate,
                instantiated: instantiated_innate,
                error: Validate::Join("CAUSE".into()),
            },
            "Loading: InnateValidation: Join: CAUSE",
        ),
    ] {
        assert_eq!(
            render_node(Node::Certificate(
                &HostSelectedExtensionOwnerCertificateError(error)
            )),
            expected
        );
    }
}

#[tokio::test]
async fn registration_diagnostic_bzlmod_handoffs_use_natural_errors() {
    use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
    use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsKey;
    use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputsKey;

    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    let workspace = slug_workspace_v2::NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let (mut tx, ordinary) = owner_fixture(false).await;
    let requested = slug_identity_v2::CanonicalRepoName::new("absent+").unwrap();
    let result = complete(
        tx.compute(&HostSelectedExtensionDemandKey::new(
            workspace.clone(),
            requested.clone(),
        ))
        .await
        .unwrap(),
    );
    let demand = result.as_ref().as_ref().unwrap_err().clone();
    let generated = HostGeneratedRepositoryDefinitionError {
        requested,
        kind: Generated::Demand(demand),
    };
    assert_eq!(
        render_node(Node::Generated(&generated)),
        "Generated absent+: Demand: Missing absent+"
    );
    let result = complete(
        tx.compute(&HostSelectedInnateRepositoryOwnerInputsKey::new(
            workspace.clone(),
            ordinary,
        ))
        .await
        .unwrap(),
    );
    let error = Innate::Inputs(result.as_ref().as_ref().unwrap_err().clone());
    let text = render_node(Node::Innate(&error));
    assert!(text.starts_with("InnatePure: Inputs: Unsupported ") && !text.contains("HostSelected"));
    let (mut tx, innate) = owner_fixture(true).await;
    let result = complete(
        tx.compute(&HostSelectedExtensionOwnerInputsKey::new(workspace, innate))
            .await
            .unwrap(),
    );
    let error = Pure::Inputs(result.as_ref().as_ref().unwrap_err().clone());
    let text = render_node(Node::Pure(&error));
    assert!(text.starts_with("Pure: Inputs: Unsupported ") && !text.contains("HostSelected"));
}

#[tokio::test]
#[rustfmt::skip]
async fn registration_diagnostic_route_leaves_and_incomplete_boundaries() {
    use slug_bzlmod_v2::HostCanonicalRepositorySourceInputError;
    use crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectError;
    let missing = crate::registration_expansion_tests::registration_diagnostic_missing_route("POISON").await;
    let Registration::CanonicalRoute(load) = missing.kind() else { panic!("expected load") };
    let Load::Route(route) = &load.kind else { panic!("expected route") };
    let Route::Missing { selected_missing, .. } = &route.kind else { panic!("expected missing") };
    assert_eq!(render_node(Node::Route(route)), "Route unknown+: Missing: selected and generated lookups missed");
    for (kind, tail) in [
        (Route::BuiltinCompute("CAUSE".into()), "BuiltinCompute: CAUSE"),
        (Route::SelectedCompute("CAUSE".into()), "SelectedCompute: CAUSE"),
        (Route::Selected(selected_missing.clone()), "[diagnostic incomplete: Selected]"),
        (Route::GeneratedCompute { selected_missing: selected_missing.clone(), message: "CAUSE".into() }, "GeneratedCompute: CAUSE"),
    ] {
        let error = HostCanonicalRepositoryRouteError { canonical_repo: route.canonical_repo.clone(), kind };
        let text = render_node(Node::Route(&error));
        assert!(text.ends_with(tail) && !text.contains("POISON"));
    }
    for (kind, tail) in [
        (Load::RouteCompute("CAUSE".into()), "RouteCompute: CAUSE"),
        (Load::EffectCompute("CAUSE".into()), "EffectCompute: CAUSE"),
        (Load::Effect(HostSelectedRepositoryFileEffectError::Compute("POISON".into())), "[diagnostic incomplete: Effect]"),
        (Load::Projection(HostCanonicalRepositorySourceInputError::Root), "[diagnostic incomplete: Projection]"),
    ] {
        let error = Arc::new(HostCanonicalRepositoryLoadRouteError { canonical_repo: load.canonical_repo.clone(), kind });
        let text = render_node(Node::Load(&error));
        assert!(text.ends_with(tail) && !text.contains("POISON"));
        assert_eq!(render_node(Node::Innate(&Innate::LoadRoute(error))), format!("InnatePure: {text}"));
    }
    let error = ModuleRegistrationExpansionError::diagnostic_test(Registration::Configuration(
        slug_configuration_v2::SlugConfigurationError::MissingAutoCpu));
    assert!(error.diagnostic().to_string().ends_with("[diagnostic incomplete: Configuration]"));
}

#[test]
#[rustfmt::skip]
fn registration_diagnostic_root_bzl_cause_and_boundaries() {
    use crate::bzl_module::{HostRootBzlLabel, HostLoadLabelError, HostSourceInputError, ExternalLoadLabelError};
    use crate::cycle_detector::{HostBzlLoadCycle, ExternalBzlLoadCycle};
    let label = HostRootBzlLabel::new(slug_identity_v2::PackagePath::parse("").unwrap(),
        slug_bzlmod_v2::RootPackageBzlTarget::parse("ext.bzl").unwrap());
    for (error, tail) in [
        (HostBzlModuleError::Parse { label: label.clone(), message: "CAUSE".into() }, "Parse: CAUSE"),
        (HostBzlModuleError::Freeze { label: label.clone(), message: "CAUSE".into() }, "Freeze: CAUSE"),
        (HostBzlModuleError::Cycle(HostBzlLoadCycle { path: Arc::from([]), keys: Arc::from([]) }), "Cycle"),
        (HostBzlModuleError::Input(HostSourceInputError::UnsupportedSourceEncoding {
            logical_path: slug_workspace_v2::NormalizedAbsolutePath::new("/POISON").unwrap() }), "[diagnostic incomplete: Input]"),
        (HostBzlModuleError::LoadLabel { source: label.clone(), error: HostLoadLabelError::Invalid {
            load: "POISON".into(), message: "POISON".into() } }, "[diagnostic incomplete: LoadLabel]"),
    ] {
        assert_eq!(render_node(Node::RootBzl(&error)), format!("RootBzl: {tail}"));
        assert_eq!(render_node(Node::Innate(&Innate::RootBzl(error))), format!("InnatePure: RootBzl: {tail}"));
    }
    let mut error = HostBzlModuleError::Parse { label: label.clone(), message: "CAUSE".into() };
    for _ in 0..31 {
        error = HostBzlModuleError::Child { load: "child".into(), label: label.clone(), error: Arc::new(error) };
    }
    assert!(render_node(Node::RootBzl(&error)).ends_with("Parse: CAUSE"));
    error = HostBzlModuleError::Child { load: "child".into(), label, error: Arc::new(error) };
    assert!(render_node(Node::RootBzl(&error)).ends_with(DEPTH_STOP));
    let label = CanonicalLabel::parse("@@repo//:ext.bzl").unwrap();
    for (error, tail) in [
        (ExternalBzlModuleError::Cycle(ExternalBzlLoadCycle { path: Arc::from([]), keys: Arc::from([]) }), "Cycle"),
        (ExternalBzlModuleError::Source { label: label.clone(), error: slug_bzlmod_v2::RepositorySourceFileError::InvalidRepoRelativePath {
            requested_path: Arc::new(std::path::PathBuf::from("POISON")) } }, "[diagnostic incomplete: Source]"),
        (ExternalBzlModuleError::LoadLabel { source: label, error: ExternalLoadLabelError::Invalid {
            load: "POISON".into(), message: "POISON".into() } }, "[diagnostic incomplete: LoadLabel]"),
    ] {
        assert_eq!(render_node(Node::ExternalBzl(&error)), format!("ExternalBzl: {tail}"));
        assert_eq!(render_node(Node::Innate(&Innate::ExternalBzl(error))), format!("InnatePure: ExternalBzl: {tail}"));
    }
}

#[test]
fn registration_diagnostic_utf8_escapes_budget_and_writer_failure() {
    for source in ["simple", "quote' \" slash\\", "λ\n\0😀"] {
        let mut out = Buffer::new();
        out.write_str(source).unwrap();
        assert!(out.text().is_ascii());
        assert!(out.text().len() <= LIMIT);
    }
    let mut out = Buffer::new();
    out.write_str("λ\n\0😀").unwrap();
    assert_eq!(out.text(), "\\u{3bb}\\n\\u{0}\\u{1f600}");
    for message in [
        "x".repeat(1024 * 1024),
        "😀".repeat(4096),
        "\\\"".repeat(8192),
    ] {
        let error =
            ModuleRegistrationExpansionError::diagnostic_test(Registration::Parse(message.into()));
        let text = error.diagnostic().to_string();
        assert!(text.len() <= LIMIT && text.ends_with(OUTPUT_STOP));
        assert!(text.is_ascii());
        assert!(format!("{text:?}").len() < 8192);
    }
    let mut out = Buffer::new();
    out.write_str(&"x".repeat(LIMIT - OUTPUT_STOP.len() - 1))
        .unwrap();
    assert!(out.write_str("😀").is_err());
    assert_eq!(out.text().len(), LIMIT - 1);
    assert!(out.text().ends_with(OUTPUT_STOP));
    let before = out.text().to_owned();
    assert!(out.write_str("later").is_err());
    assert_eq!(out.text(), before);
    struct Fail;
    impl Write for Fail {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    let error = ModuleRegistrationExpansionError::diagnostic_test(Registration::RowOverflow);
    assert!(write!(Fail, "{}", error.diagnostic()).is_err());
}

#[tokio::test]
async fn registration_diagnostic_natural_bzl_incomplete_boundaries() {
    let mut source_observation_text = Vec::new();
    for (error, owner) in
        crate::registration_expansion_tests::registration_diagnostic_bzl_errors().await
    {
        let text = render_node(Node::Innate(&error));
        if owner.starts_with("SourceObservation") {
            assert!(
                text.ends_with("SourceObservation @@dep+//:ext.bzl: CanonicalRequest: Request.WrongKind path=hex:6578742e627a6c actual=Directory")
                    && !text.contains("POISON")
            );
            source_observation_text.push(text.clone());
            if owner == "SourceObservationObserved" {
                use ExternalBzlModuleError as E;
                let Innate::ExternalBzl(leaf) = error else {
                    unreachable!()
                };
                let leaf = Arc::new(leaf);
                let weak = Arc::downgrade(&leaf);
                let wrapped = Innate::ExternalBzl(E::Child {
                    raw_load: "@raw//:leaf.bzl".into(),
                    canonical_label: CanonicalLabel::parse("@@outer//:leaf.bzl").unwrap(),
                    error: leaf.clone(),
                });
                let before = Arc::strong_count(&leaf);
                assert!(
                    render_node(Node::Innate(&wrapped))
                        .contains("Child @raw//:leaf.bzl: ExternalBzl: SourceObservation")
                );
                assert_eq!(Arc::strong_count(&leaf), before);
                drop(wrapped);
                assert!(weak.upgrade().is_some());
                drop(leaf);
                assert!(weak.upgrade().is_none());
            }
        } else {
            assert!(
                text.ends_with(&format!("[diagnostic incomplete: {owner}]"))
                    && !text.contains("POISON")
            );
        }
    }
    assert_eq!(source_observation_text.len(), 2);
    assert_eq!(source_observation_text[0], source_observation_text[1]);
}

#[test]
fn registration_diagnostic_scalar_and_innate_causes() {
    for (kind, tail) in [
        (Registration::Parse("cause".into()), "Parse: cause"),
        (Registration::RowOverflow, "RowOverflow"),
        (
            Registration::RootMappingUnavailable,
            "RootMappingUnavailable",
        ),
        (
            Registration::MissingTarget(CanonicalLabel::parse("@@repo//pkg:target").unwrap()),
            "MissingTarget @@repo//pkg:target",
        ),
    ] {
        let error = ModuleRegistrationExpansionError::diagnostic_test(kind);
        assert_eq!(
            error.diagnostic().to_string(),
            format!("toolchains registration row 3: {tail}")
        );
    }
    for (error, expected) in [
        (Innate::Compute("cause".into()), "Compute: cause"),
        (Innate::Label("cause".into()), "Label: cause"),
        (Innate::Export("cause".into()), "Export: cause"),
        (Innate::Call("cause".into()), "Call: cause"),
        (Innate::Drift, "Drift"),
    ] {
        assert_eq!(
            render_node(Node::Innate(&error)),
            format!("InnatePure: {expected}")
        );
    }
    for (error, expected) in [
        (Instantiate::Join("cause".into()), "Join: cause"),
        (Instantiate::Namespace("cause".into()), "Namespace: cause"),
        (Instantiate::Attribute("cause".into()), "Attribute: cause"),
    ] {
        let mut out = Buffer::new();
        instantiation(&mut out, &error).unwrap();
        assert_eq!(out.text(), expected);
    }
    assert_eq!(
        render_node(Node::Pure(&Pure::Compute("cause".into()))),
        "Pure: Compute: cause"
    );
    let error = HostSelectedExtensionOwnerCertificateError(Certificate::Compute("cause".into()));
    assert_eq!(
        render_node(Node::Certificate(&error)),
        "Loading: Compute: cause"
    );
}

#[test]
fn registration_diagnostic_recursive_bzl_cause_and_depth() {
    use ExternalBzlModuleError as E;
    let label = CanonicalLabel::parse("@@repo//:extension.bzl").unwrap();
    for (error, tail) in [
        (
            E::SourceCompute {
                label: label.clone(),
                message: "cause".into(),
            },
            "SourceCompute: cause",
        ),
        (
            E::Parse {
                label: label.clone(),
                message: "cause".into(),
            },
            "Parse: cause",
        ),
        (
            E::Evaluation {
                label: label.clone(),
                message: "cause".into(),
            },
            "Evaluation: cause",
        ),
        (
            E::Freeze {
                label: label.clone(),
                message: "cause".into(),
            },
            "Freeze: cause",
        ),
        (
            E::Route {
                source: label.clone(),
                load: "load".into(),
                message: "cause".into(),
            },
            "Route: cause",
        ),
        (
            E::Absent {
                label: label.clone(),
            },
            "Absent @@repo//:extension.bzl",
        ),
        (
            E::Encoding {
                label: label.clone(),
            },
            "Encoding @@repo//:extension.bzl",
        ),
    ] {
        assert_eq!(
            render_node(Node::ExternalBzl(&error)),
            format!("ExternalBzl: {tail}")
        );
    }
    let mut error = E::Parse {
        label: label.clone(),
        message: "LEAF".into(),
    };
    for _ in 0..31 {
        error = E::Child {
            raw_load: "child".into(),
            canonical_label: label.clone(),
            error: Arc::new(error),
        };
    }
    assert!(render_node(Node::ExternalBzl(&error)).ends_with("Parse: LEAF"));
    error = E::Child {
        raw_load: "child".into(),
        canonical_label: label,
        error: Arc::new(error),
    };
    let text = render_node(Node::ExternalBzl(&error));
    assert!(text.ends_with(DEPTH_STOP) && !text.contains("LEAF"));
}

fn generated_from_missing(
    error: &ModuleRegistrationExpansionError,
    kind: Generated,
) -> ModuleRegistrationExpansionError {
    let Registration::CanonicalRoute(load) = error.kind() else {
        panic!("expected route")
    };
    let Load::Route(route) = &load.kind else {
        panic!("expected route cause")
    };
    let Route::Missing {
        selected_missing, ..
    } = &route.kind
    else {
        panic!("expected selected miss")
    };
    let generated = HostGeneratedRepositoryDefinitionError {
        requested: route.canonical_repo.clone(),
        kind,
    };
    ModuleRegistrationExpansionError::diagnostic_test(Registration::CanonicalRoute(
        HostCanonicalRepositoryLoadRouteError {
            canonical_repo: load.canonical_repo.clone(),
            kind: Load::Route(HostCanonicalRepositoryRouteError {
                canonical_repo: route.canonical_repo.clone(),
                kind: Route::Generated {
                    selected_missing: selected_missing.clone(),
                    error: generated,
                },
            }),
        },
    ))
}

#[tokio::test]
async fn registration_diagnostic_generated_retains_identity_without_graph_rendering() {
    use crate::registration_expansion_tests::registration_diagnostic_missing_route;
    let a = registration_diagnostic_missing_route("GRAPH_POISON_A").await;
    let b = registration_diagnostic_missing_route("GRAPH_POISON_B").await;
    let make = |error: &ModuleRegistrationExpansionError| {
        generated_from_missing(error, Generated::LoadingCompute("LEAF".into()))
    };
    let first = make(&a);
    let changed = make(&b);
    let restored = make(&a);
    assert!(first != changed && first == restored);
    assert_eq!(
        first.diagnostic().to_string(),
        changed.diagnostic().to_string()
    );
    let text = first.diagnostic().to_string();
    assert!(text.ends_with("LoadingCompute: LEAF") && !text.contains("GRAPH_POISON"));
    assert!(first.to_string().contains("GRAPH_POISON_A")); // Discriminating old-format control.
    for (kind, tail) in [
        (
            Generated::DemandCompute("cause".into()),
            "DemandCompute: cause",
        ),
        (
            Generated::LoadingCompute("cause".into()),
            "LoadingCompute: cause",
        ),
        (
            Generated::Loading(HostSelectedExtensionOwnerCertificateError(
                Certificate::Compute("cause".into()),
            )),
            "Loading: Compute: cause",
        ),
        (
            Generated::Duplicate {
                first: 1,
                conflicting: 2,
            },
            "Duplicate: 1 / 2",
        ),
        (Generated::Missing {}, "Missing"),
    ] {
        assert!(
            generated_from_missing(&a, kind)
                .diagnostic()
                .to_string()
                .ends_with(tail)
        );
    }
    // Fixed, small synthetic A/B/B/A rendering experiment, not an authentic graph replay.
    let large = registration_diagnostic_missing_route(&"GRAPH_POISON".repeat(1024)).await;
    let candidate = make(&large);
    for bounded in [false, true, true, false] {
        let start = std::time::Instant::now();
        let text = if bounded {
            candidate.diagnostic().to_string()
        } else {
            candidate.to_string()
        };
        assert!(text.contains("LEAF"));
        if bounded {
            assert!(text.len() <= LIMIT && !text.contains("GRAPH_POISON"));
        }
        eprintln!(
            "registration_diagnostic bounded={bounded} bytes={} capacity={} wall_us={}",
            text.len(),
            text.capacity(),
            start.elapsed().as_micros()
        );
    }
}
