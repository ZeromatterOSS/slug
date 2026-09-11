use super::*;

fn demand(error: DemandError) -> String {
    let mut text = String::new();
    HostSelectedExtensionDemandError(error)
        .write_registration_diagnostic(&mut text)
        .unwrap();
    text
}

#[test]
fn registration_diagnostic_demand_and_owner_inputs_are_causal() {
    let requested = CanonicalRepoName::new("requested").unwrap();
    let owner = Arc::new(HostSelectedExtensionOwner::testing("first"));
    let conflicting = Arc::new(HostSelectedExtensionOwner::testing("second"));
    assert_eq!(
        demand(DemandError::Missing {
            requested: requested.clone()
        }),
        "Missing requested"
    );
    assert_eq!(
        demand(DemandError::Ambiguous {
            requested: requested.clone(),
            first: owner.clone(),
            conflicting
        }),
        "Ambiguous requested: first / second"
    );
    assert_eq!(
        demand(DemandError::Inconsistent {
            requested,
            owner: owner.clone()
        }),
        "Inconsistent requested: first"
    );
    for (error, expected) in [
        (
            OwnerInputsError::Missing {
                owner: owner.clone(),
            },
            "Missing first",
        ),
        (
            OwnerInputsError::Inconsistent {
                owner: owner.clone(),
            },
            "Inconsistent first",
        ),
        (
            OwnerInputsError::Unsupported {
                owner: owner.clone(),
            },
            "Unsupported first",
        ),
        (
            OwnerInputsError::Invalid {
                owner,
                message: "cause".into(),
            },
            "Invalid first: cause",
        ),
        (
            OwnerInputsError::Mappings(HostSelectedExtensionMappingsError::RootFiles(
                "cause".into(),
            )),
            "Mappings: RootFiles: cause",
        ),
    ] {
        let mut ordinary = String::new();
        let mut innate = String::new();
        HostSelectedExtensionOwnerInputsError(error.clone())
            .write_registration_diagnostic(&mut ordinary)
            .unwrap();
        HostSelectedInnateRepositoryOwnerInputsError(error)
            .write_registration_diagnostic(&mut innate)
            .unwrap();
        assert_eq!(ordinary, expected);
        assert_eq!(innate, expected);
    }
}

#[test]
fn registration_diagnostic_mapping_and_routes_never_format_graphs() {
    use HostSelectedExtensionMappingsError as M;
    use HostSelectedModuleRoutesError as R;
    let key = HostGraphModuleKey::Module {
        name: "mod".into(),
        version: crate::module_version::BazelModuleVersion::parse("1.0").unwrap(),
    };
    for (error, expected) in [
        (
            M::RoutesCompute("cause".into()),
            "Mappings: RoutesCompute: cause",
        ),
        (M::RootFiles("cause".into()), "Mappings: RootFiles: cause"),
        (
            M::RootFilesCompute("cause".into()),
            "Mappings: RootFilesCompute: cause",
        ),
        (
            M::Invalid {
                owner: key.clone(),
                message: "cause".into(),
            },
            "Mappings: Invalid mod@1.0: cause",
        ),
        (
            M::Routes(R::GraphCompute("cause".into())),
            "Mappings: Routes: GraphCompute: cause",
        ),
        (
            M::Routes(R::RepoSpecsCompute("cause".into())),
            "Mappings: Routes: RepoSpecsCompute: cause",
        ),
        (
            M::Routes(R::Invalid {
                module: key.clone(),
                message: "cause".into(),
            }),
            "Mappings: Routes: Invalid mod@1.0: cause",
        ),
        (
            M::Routes(R::RegistryMismatch {
                module: key.clone(),
                message: "cause".into(),
            }),
            "Mappings: Routes: RegistryMismatch mod@1.0: cause",
        ),
        (
            M::Routes(R::CanonicalCollision {
                canonical_repo: CanonicalRepoName::new("repo").unwrap(),
                first: HostGraphModuleKey::Root,
                second: key,
            }),
            "Mappings: Routes: CanonicalCollision repo: <root> / mod@1.0",
        ),
        (
            M::Routes(R::Graph(HostSelectedModuleGraphError::IncompatibleNeeds(
                "NEVER_RENDER".into(),
            ))),
            "Mappings: Routes: [diagnostic incomplete: Graph]",
        ),
        (
            M::Routes(R::RepoSpecs(
                HostSelectedRegistryRepoSpecsError::GraphCompute("NEVER_RENDER".into()),
            )),
            "Mappings: Routes: [diagnostic incomplete: RepoSpecs]",
        ),
    ] {
        assert_eq!(demand(DemandError::Mappings(error)), expected);
    }
}

#[test]
fn registration_diagnostic_propagates_writer_stop_without_visiting_later_fields() {
    struct Stop(usize);
    impl fmt::Write for Stop {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.0 += 1;
            Err(fmt::Error)
        }
    }
    let mut out = Stop(0);
    let error = HostSelectedExtensionDemandError(DemandError::Mappings(
        HostSelectedExtensionMappingsError::Invalid {
            owner: HostGraphModuleKey::Root,
            message: "x".repeat(1024 * 1024).into(),
        },
    ));
    assert!(error.write_registration_diagnostic(&mut out).is_err());
    assert_eq!(out.0, 1);
}
