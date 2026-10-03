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
    let refusal = error.selected_source_refusal();
    assert_eq!(
        refusal.termination(),
        crate::SelectedSourceRefusalTermination::UnexpectedSuccess
    );
    assert!(!refusal.variant_path_complete() && refusal.native_cause().is_none());
}
