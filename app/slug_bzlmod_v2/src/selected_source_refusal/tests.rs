// Included in selected_repo_spec's private tests: retained error fields never
// become public merely to manufacture classifier fixtures.
fn refusal_registry_root(
    error: crate::registry_dice::RegistryFileError,
    secret: &str,
) -> HostRootRepositoryMappingError {
    HostRootRepositoryMappingError {
        workspace: NormalizedAbsolutePath::new(format!("/{secret}")).unwrap(),
        inner: PrivateRootRepositoryMappingError::Predecessor(Arc::new(Err(
            HostSelectedExtensionMappingsError::Routes(HostSelectedModuleRoutesError::Graph(
                crate::selected_graph::HostSelectedModuleGraphError::DiscoveryLeaf {
                    module: HostGraphModuleKey::Root,
                    error: crate::source_preparation::HostDiscoveredModuleError::SourcePreparation(
                        crate::source_preparation::ModuleSourcePreparationError::RegistryFile {
                            url: crate::registry_dice::RegistryFileUrl::new(format!(
                                "https://{secret}.invalid/MODULE.bazel"
                            )),
                            prior_not_found_attempts: Arc::from([]),
                            error,
                        },
                    ),
                },
            )),
        ))),
    }
}

#[test]
fn selected_source_refusal_preserves_only_selected_typed_registry_cause() {
    use crate::SelectedSourceRefusalCode as Code;
    use crate::SelectedSourceRefusalTermination as Termination;
    use crate::registry_dice::RegistryFileError as Error;

    let cause = crate::RegistryRefusalCause {
        site: crate::RegistryRefusalSite::HttpStatus,
        detail: crate::RegistryRefusalDetail::Status(503),
    };
    let refusal = |secret: &str, cause| {
        refusal_registry_root(
            Error::Transport {
                url: crate::registry_dice::RegistryFileUrl::new(format!(
                    "https://{secret}.invalid"
                )),
                message: secret.into(),
                cause,
            },
            secret,
        )
        .selected_source_refusal()
    };
    let a = refusal("secret_a", Some(cause));
    assert_eq!(
        a.codes(),
        &[
            Code::RootMappingPredecessor,
            Code::ExtensionMappingsRoutes,
            Code::RoutesGraph,
            Code::GraphDiscoveryLeaf,
            Code::DiscoverySourcePreparation,
            Code::ModuleSourceRegistryFile,
            Code::RegistryFileTransport,
        ]
    );
    assert_eq!(a.termination(), Termination::Classified);
    assert!(a.variant_path_complete() && a.native_detail_observed());
    assert_eq!(a.native_cause(), Some(cause));
    assert_eq!(a, refusal("secret_b", Some(cause)));
    let formatted = format!("{a:?}");
    assert!(!formatted.contains("secret_a") && !formatted.contains("https"));
    let opaque = refusal("opaque", None);
    assert!(opaque.variant_path_complete());
    assert_eq!(opaque.termination(), Termination::OpaqueLeaf);
    assert!(!opaque.native_detail_observed() && opaque.native_cause().is_none());
    let opaque_native = refusal(
        "opaque",
        Some(crate::RegistryRefusalCause {
            detail: crate::RegistryRefusalDetail::Opaque,
            ..cause
        }),
    );
    assert_eq!(opaque_native.termination(), Termination::OpaqueLeaf);
    assert!(!opaque_native.native_detail_observed());
    let generation =
        refusal_registry_root(Error::MissingRequestGeneration("secret".into()), "other")
            .selected_source_refusal();
    assert_eq!(
        generation.codes().last(),
        Some(&Code::RegistryFileMissingRequestGeneration)
    );
    assert!(generation.native_cause().is_none());
    let missing_checksum = refusal_registry_root(
        Error::MissingChecksumInError {
            url: crate::registry_dice::RegistryFileUrl::new("https://secret.invalid"),
        },
        "hidden",
    )
    .selected_source_refusal();
    assert_eq!(missing_checksum.termination(), Termination::Classified);
    assert!(missing_checksum.native_cause().is_none());
    let local = refusal_registry_root(
        Error::LocalRead {
            path: std::path::PathBuf::from("/hidden/local"),
            message: "secret".into(),
            cause: Some(cause),
        },
        "hidden",
    )
    .selected_source_refusal();
    assert_eq!(local.codes().last(), Some(&Code::RegistryFileLocalRead));
    assert_eq!(local.native_cause(), Some(cause));
    assert_ne!(local.codes(), a.codes());
}

#[test]
fn selected_source_refusal_depth_unknown_and_observation_do_not_invent_detail() {
    use crate::SelectedSourceRefusal as Refusal;
    use crate::SelectedSourceRefusalCode as Code;
    use crate::SelectedSourceRefusalTermination as Termination;
    use crate::selected_source_refusal::SelectedSourceRefusalBuilder as Builder;

    assert!(std::mem::size_of::<Refusal>() <= 128);
    let empty = Refusal::default();
    assert!(empty.codes().is_empty() && !empty.variant_path_complete());
    assert!(!empty.native_detail_observed() && empty.native_cause().is_none());
    let mut builder = Builder::new();
    for _ in 0..16 {
        assert!(builder.push(Code::RootMappingPredecessor));
    }
    assert!(!builder.push(Code::RegistryFileTransport));
    // Overflow stops before borrowing any deeper selected native payload.
    builder.registry_file(&crate::registry_dice::RegistryFileError::Transport {
        url: crate::registry_dice::RegistryFileUrl::new("https://hidden.invalid"),
        message: "secret".into(),
        cause: Some(crate::RegistryRefusalCause {
            site: crate::RegistryRefusalSite::HttpStatus,
            detail: crate::RegistryRefusalDetail::Status(503),
        }),
    });
    let depth = builder.finish();
    assert_eq!(depth.codes().len(), 16);
    assert_eq!(depth.termination(), Termination::DepthLimit);
    assert!(!depth.variant_path_complete() && depth.native_cause().is_none());
    let mut unknown = Builder::new();
    unknown.push(Code::ModuleSourcePatchResolution);
    unknown.terminal(Termination::Unknown);
    let unknown = unknown.finish();
    assert!(!unknown.variant_path_complete() && unknown.native_cause().is_none());
    let mut observation = Builder::new();
    observation.path_frontier(&ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::DuplicateDemand(
            slug_workspace_v2::PathObservationDemand::new(
                slug_workspace_v2::PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/private").unwrap(),
                slug_workspace_v2::PathObservationOperation::Lstat,
            ),
        ),
    ));
    let observation = observation.finish();
    assert_eq!(
        observation.codes(),
        &[Code::PathEpoch, Code::PathDuplicateDemand]
    );
    assert_eq!(observation.termination(), Termination::Classified);
    assert!(observation.native_cause().is_none());
}

#[tokio::test]
async fn selected_source_refusal_successful_predecessor_is_not_a_failure_path() {
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, Arc::new(TrackingRegistryIo::new([])));
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let (mapping, _, _) = observed_root_mapping_state(
        &dice,
        &root_mapping_source("plain_a", false, false, false),
        0,
    )
    .await;
    let success = mapping.result.as_ref().as_ref().unwrap();
    let error = HostRootRepositoryMappingError {
        workspace: NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        inner: PrivateRootRepositoryMappingError::Predecessor(success.predecessor.dupe()),
    };
    assert!(error.selected_module_registry_file().is_none());
    let refusal = error.selected_source_refusal();
    assert_eq!(
        refusal.termination(),
        crate::SelectedSourceRefusalTermination::UnexpectedSuccess
    );
    assert!(!refusal.variant_path_complete() && refusal.native_cause().is_none());
}

// The test constructor keeps private graph fields at their existing owner.
fn registry_discovery_root(graph: HostSelectedModuleGraphError) -> HostRootRepositoryMappingError {
    HostRootRepositoryMappingError {
        workspace: NormalizedAbsolutePath::new("/registry-association").unwrap(),
        inner: PrivateRootRepositoryMappingError::Predecessor(Arc::new(Err(
            HostSelectedExtensionMappingsError::Routes(HostSelectedModuleRoutesError::Graph(graph)),
        ))),
    }
}

fn registry_discovery_fixture(name: &str, version: &str) -> HostRootRepositoryMappingError {
    registry_discovery_root(HostSelectedModuleGraphError::DiscoveryLeaf {
        module: HostGraphModuleKey::Module {
            name: name.into(),
            version: crate::module_version::BazelModuleVersion::parse(version).unwrap(),
        },
        error: crate::source_preparation::HostDiscoveredModuleError::SourcePreparation(
            crate::source_preparation::ModuleSourcePreparationError::RegistryFile {
                url: RegistryFileUrl::new("https://registry.invalid/outer/MODULE.bazel"),
                prior_not_found_attempts: Arc::from([RegistryModuleFileAttempt {
                    url: RegistryFileUrl::new("https://prior.invalid/other/MODULE.bazel"),
                    sha256: None,
                }]),
                error: RegistryFileError::Transport {
                    url: RegistryFileUrl::new("https://nested.invalid/different/MODULE.bazel"),
                    message: "opaque transport detail".into(),
                    cause: None,
                },
            },
        ),
    })
}

#[test]
fn selected_module_registry_file_borrows_discovery_identity_and_outer_request() {
    for (name, spelling, canonical) in [
        ("bazel_features", "1.50.0+build", "1.50.0"),
        (
            "a_long_module_name_for_borrowed_identity",
            "1.2.3-long-prerelease-for-borrowing+build",
            "1.2.3-long-prerelease-for-borrowing",
        ),
    ] {
        let owner = registry_discovery_fixture(name, spelling);
        let before = owner.selected_source_refusal();
        let unrelated = RegistryFileUrl::new("https://unrelated.invalid/first-row/MODULE.bazel");
        let (selected_name, version, requested) = owner.selected_module_registry_file().unwrap();
        assert_eq!(selected_name, name);
        assert_eq!(version, canonical);
        assert_eq!(
            requested.as_str(),
            "https://registry.invalid/outer/MODULE.bazel"
        );
        assert_ne!(requested, &unrelated);
        let PrivateRootRepositoryMappingError::Predecessor(predecessor) = &owner.inner else {
            panic!("fixture predecessor");
        };
        let Err(HostSelectedExtensionMappingsError::Routes(HostSelectedModuleRoutesError::Graph(
            HostSelectedModuleGraphError::DiscoveryLeaf {
                module:
                    HostGraphModuleKey::Module {
                        name,
                        version: stored_version,
                    },
                error:
                    crate::source_preparation::HostDiscoveredModuleError::SourcePreparation(
                        crate::source_preparation::ModuleSourcePreparationError::RegistryFile {
                            url,
                            prior_not_found_attempts,
                            error: RegistryFileError::Transport { url: nested, .. },
                        },
                    ),
            },
        ))) = predecessor.as_ref()
        else {
            panic!("fixture discovery leaf");
        };
        assert!(std::ptr::eq(selected_name, name.as_str()));
        assert!(std::ptr::eq(version, stored_version.normalized()));
        assert!(std::ptr::eq(requested, url));
        assert!(std::ptr::eq(requested.as_str(), url.as_str()));
        assert_ne!(requested, nested);
        assert_ne!(requested, &prior_not_found_attempts[0].url);
        assert_eq!(before, owner.selected_source_refusal());
        assert!(!before.native_detail_observed());
        assert_eq!(
            before.codes().last(),
            Some(&crate::SelectedSourceRefusalCode::RegistryFileTransport)
        );
    }
}

#[test]
fn selected_module_registry_file_requires_exact_retained_error_chain() {
    use crate::source_preparation::HostDiscoveredModuleError as Discovery;
    use crate::source_preparation::ModuleSourcePreparationError as Preparation;
    let message = "https://registry.invalid/outer/MODULE.bazel";
    let module = || HostGraphModuleKey::Module {
        name: "bazel_features".into(),
        version: crate::module_version::BazelModuleVersion::parse("1.50.0").unwrap(),
    };
    let transport = || RegistryFileError::Transport {
        url: RegistryFileUrl::new(message),
        message: message.into(),
        cause: None,
    };
    for error in [
        Discovery::SourcePreparationCompute(message.into()),
        Discovery::RootModuleFiles(message.into()),
        Discovery::SourcePreparation(Preparation::RegistryFileCompute {
            url: RegistryFileUrl::new(message),
            prior_not_found_attempts: Arc::from([]),
            message: message.into(),
        }),
        Discovery::SourcePreparation(Preparation::RegistryPolicy(transport())),
        Discovery::SourcePreparation(Preparation::MissingVersion),
    ] {
        let owner = registry_discovery_root(HostSelectedModuleGraphError::DiscoveryLeaf {
            module: module(),
            error,
        });
        assert!(owner.selected_module_registry_file().is_none());
    }
    let root = refusal_registry_root(transport(), "root-key");
    assert!(root.selected_module_registry_file().is_none());
    let compute = registry_discovery_root(HostSelectedModuleGraphError::DiscoveryCompute {
        module: module(),
        message: message.into(),
    });
    assert!(compute.selected_module_registry_file().is_none());
    let owner = registry_discovery_fixture("bazel_features", "1.50.0");
    let PrivateRootRepositoryMappingError::Predecessor(predecessor) = owner.inner else {
        panic!("fixture predecessor");
    };
    for inner in [
        PrivateRootRepositoryMappingError::Compute(message.into()),
        PrivateRootRepositoryMappingError::Invalid {
            predecessor,
            reason: RootMappingInvalid::Missing,
        },
        PrivateRootRepositoryMappingError::Predecessor(Arc::new(Err(
            HostSelectedExtensionMappingsError::RoutesCompute(message.into()),
        ))),
        PrivateRootRepositoryMappingError::Predecessor(Arc::new(Err(
            HostSelectedExtensionMappingsError::Routes(
                HostSelectedModuleRoutesError::GraphCompute(message.into()),
            ),
        ))),
        PrivateRootRepositoryMappingError::Predecessor(Arc::new(Err(
            HostSelectedExtensionMappingsError::Routes(HostSelectedModuleRoutesError::RepoSpecs(
                HostSelectedRegistryRepoSpecsError::RegistryFile {
                    module: module(),
                    url: RegistryFileUrl::new(message),
                    error: transport(),
                },
            )),
        ))),
    ] {
        let owner = HostRootRepositoryMappingError {
            workspace: NormalizedAbsolutePath::new("/registry-association").unwrap(),
            inner,
        };
        assert!(owner.selected_module_registry_file().is_none());
    }
}

#[test]
fn selected_module_registry_file_view_does_not_retain_predecessor() {
    let owner = registry_discovery_fixture("bazel_features", "1.50.0");
    let PrivateRootRepositoryMappingError::Predecessor(predecessor) = &owner.inner else {
        panic!("fixture predecessor");
    };
    let weak = Arc::downgrade(predecessor);
    let before = Arc::strong_count(predecessor);
    let first = owner.selected_module_registry_file().unwrap();
    for _ in 0..16 {
        let next = owner.selected_module_registry_file().unwrap();
        assert!(std::ptr::eq(first.0, next.0));
        assert!(std::ptr::eq(first.1, next.1));
        assert!(std::ptr::eq(first.2, next.2));
    }
    assert_eq!(Arc::strong_count(predecessor), before);
    assert_eq!(first.0, "bazel_features");
    assert_eq!(first.1, "1.50.0");
    drop(owner);
    assert!(weak.upgrade().is_none());
}
