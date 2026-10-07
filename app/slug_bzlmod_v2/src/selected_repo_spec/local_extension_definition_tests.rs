use super::*;

/// Pure owner fixture. It has no materialization result, observation or source
/// certificate; Loading's runtime gate separately derives those normal owners.
pub(crate) fn testing_selected_command_local_source(
    spec: RepoSpec,
    policy: crate::HostRepositoryLocalPathPolicy,
    pairs: &[(&str, &str)],
) -> HostSelectedExtensionDefinitionSource {
    let canonical_repo = CanonicalRepoName::new("dep+1.0").unwrap();
    let mapping = HostSelectedRepositoryMapping {
        context_repo: canonical_repo.clone(),
        entries: Arc::new(SmallMap::from_iter(pairs.iter().map(|(name, target)| {
            (
                ApparentRepoName::new(*name).unwrap(),
                CanonicalRepoName::new(*target).unwrap(),
            )
        }))),
        order: pairs
            .iter()
            .map(|(name, _)| ApparentRepoName::new(*name).unwrap())
            .collect(),
    };
    let route = HostSelectedModuleRoute {
        entry: HostSelectedModuleEntry {
            key: HostGraphModuleKey::Module {
                name: "dep".into(),
                version: crate::module_version::BazelModuleVersion::parse("1.0+build").unwrap(),
            },
            source: HostGraphModuleSource::Discovered(Arc::new(crate::source_preparation::HostDiscoveredModule {
                module: crate::interim_module::NonrootModuleBuilder::new(
                    crate::interim_module::NonrootModuleKey::new("dep", "1.0+build"),
                    "dep", "0.2.17", "dep",
                ).build().unwrap(),
                provenance: crate::source_preparation::HostDiscoveredModuleProvenance::NonRegistry {
                    closure: crate::source_preparation::HostNonregistryPreparedClosure::for_selected_local_test(spec, policy),
                },
            })),
            dependencies: Arc::from([]),
            original_dependencies: Arc::from([]),
            nodep_dependencies: Arc::from([]),
        },
        canonical_repo,
        mapping,
        registry_repo_spec: None,
    };
    HostSelectedExtensionDefinitionSource::Selected {
        definition: HostCanonicalSelectedModuleDefinition {
            routes: Arc::new(Ok(HostSelectedModuleRoutes {
                entries: Arc::from([route]),
                extension_projection: testing_extension_mapping_projection(),
            })),
            ordinal: 0,
        },
        apparent_repo: ApparentRepoName::new("dep").unwrap(),
    }
}
