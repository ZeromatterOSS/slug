//! Offline selected-registry routes over the same authored workspace/corpus.
//! This supplies transport/materialization observations, never alternate bodies.
use slug_bzlmod_v2::HostRepositoryMaterializationDisposition;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RootRepositoryRouteObservationKey;
use slug_bzlmod_v2::host_repository_materialization_request;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::install_registry_io;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::PathObservationInstanceId;

use super::*;

const REGISTRY: &str = "https://registry.invalid";

mod version_inventory;

pub(super) fn prepare_workspace(root: &std::path::Path) {
    // Prepare the builtin source closure before establishing this fixture's
    // deliberately selected-registry routes. Later snapshots only observe it.
    builtin_fixture::prepare_if_ordinary(root);
    let path = root.join("MODULE.bazel");
    let original = fs::read_to_string(&path).unwrap();
    let mut module = original;
    for (_, repository) in REPOSITORIES {
        let directive =
            format!("local_path_override(module_name = '{repository}', path = '{repository}')\n");
        assert!(module.contains(&directive));
        module = module.replace(&directive, "");
    }
    fs::write(path, module).unwrap();
}

struct FixtureRegistry(Vec<(String, Arc<[u8]>)>);
#[async_trait::async_trait]
impl RegistryIo for FixtureRegistry {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        Ok(self
            .0
            .iter()
            .find(|(name, _)| name == url.as_str())
            .map_or(RegistryIoOutcome::NotFound, |(_, bytes)| {
                RegistryIoOutcome::Found(bytes.clone())
            }))
    }
}

// This fixture deliberately projects its authored synthetic MODULE declaration
// version for each finite request from the actual prepared graph. Rule/program
// bodies and all dependency/alias declarations remain byte-for-byte unchanged.
fn registry_files(root: &std::path::Path) -> Vec<(String, Arc<[u8]>)> {
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    use slug_bzlmod_v2::ModuleFile;

    let mut sources = vec![
        fs::read_to_string(root.join("MODULE.bazel")).unwrap(),
        include_str!("../../../../slug_bzlmod_v2/builtin/bazel_tools/MODULE.bazel").to_owned(),
    ];
    let mut paths = builtin_fixture::local_repositories(root)
        .into_iter()
        .map(|(_, path)| root.join(path).join("MODULE.bazel"))
        .collect::<BTreeSet<_>>();
    paths.extend(
        REPOSITORIES
            .iter()
            .map(|(_, path)| root.join(path).join("MODULE.bazel")),
    );
    sources.extend(paths.iter().map(|path| fs::read_to_string(path).unwrap()));
    let mut versions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut request = |name: &str, version: &str| {
        if REPOSITORIES
            .iter()
            .any(|(_, repository)| *repository == name)
        {
            versions
                .entry(name.to_owned())
                .or_default()
                .insert(version.to_owned());
        }
    };
    for source in &sources {
        for (name, version) in version_inventory::requests(source) {
            request(&name, &version);
        }
    }
    let mut files = Vec::new();
    for (_, repository) in REPOSITORIES {
        let module = fs::read_to_string(root.join(repository).join("MODULE.bazel")).unwrap();
        let parsed = ModuleFile::parse(&module).unwrap();
        let header = parsed.module.unwrap();
        assert_eq!(header.name, *repository);
        let original_version = header.version.unwrap();
        versions
            .entry((*repository).to_owned())
            .or_default()
            .insert(original_version.clone());
        let declared = format!("module(name = '{repository}', version = '{original_version}')");
        assert!(module.contains(&declared));
        for version in &versions[*repository] {
            let metadata = module.replacen(
                &declared,
                &format!("module(name = '{repository}', version = '{version}')"),
                1,
            );
            files.push((
                format!("{REGISTRY}/modules/{repository}/{version}/MODULE.bazel"),
                Arc::<[u8]>::from(metadata.into_bytes()),
            ));
            files.push((format!("{REGISTRY}/modules/{repository}/{version}/source.json"),
                Arc::<[u8]>::from(format!(r#"{{"url":"https://origin.invalid/{repository}.tgz","integrity":"sha256-test"}}"#).into_bytes())));
        }
    }
    files
}

pub(super) fn dice(root: &std::path::Path) -> Arc<Dice> {
    let mut builder = Dice::builder();
    install_registry_io(
        &mut builder,
        Arc::new(FixtureRegistry(registry_files(root))),
    );
    builder.build(DetectCycles::Enabled)
}

// Same local materialization schema as starlark_rule's existing fixture owner.
// The epoch deliberately combines these with selected-registry requests.
fn local_entry(
    root: &NormalizedAbsolutePath,
    canonical: &str,
    directory: &str,
) -> RepositoryMaterializationEpochEntry {
    let mut attributes = SmallMap::new();
    let path = if canonical == "bazel_tools+" {
        root.as_path().join(directory).display().to_string()
    } else {
        directory.to_owned()
    };
    attributes.insert("path".into(), OverrideAttributeValue::String(path.into()));
    RepositoryMaterializationEpochEntry {
        request: Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: root.clone(),
                canonical_repo: CanonicalRepoName::new(canonical).unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(attributes),
            },
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(root.as_path().join(directory)).unwrap(),
            },
        }),
        result: RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
    }
}

pub(super) async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let workspace = NormalizedAbsolutePath::new(root.to_path_buf()).unwrap();
    let base = observation_epoch(root);
    let mut observations = base
        .observations()
        .iter()
        .map(|(demand, value)| (demand.clone(), value.clone()))
        .collect::<Vec<_>>();
    for (index, (_, repository)) in REPOSITORIES.iter().enumerate() {
        let directory = root.join(repository);
        let namespace = PathObservationNamespace::Materialization(PathObservationInstanceId::new(
            7680 + index as u64,
        ));
        for (demand, value) in base.observations() {
            let path = demand.path().as_path();
            if path.starts_with(&directory) || directory.starts_with(path) {
                observations.push((
                    PathObservationDemand::new(
                        namespace,
                        demand.path().clone(),
                        demand.operation(),
                    ),
                    value.clone(),
                ));
            }
        }
    }
    let epoch = PathObservationEpoch::from_shared(observations).unwrap();
    let prepared_local = builtin_fixture::local_repositories(root);
    let local = prepared_local
        .iter()
        .map(|(canonical, directory)| (canonical.as_str(), directory.as_str()))
        .chain(HOST_PLATFORM_REPOSITORIES.iter().copied())
        .collect::<Vec<_>>();
    let mut data = UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    };
    let host = builtin_fixture::RepositoryHostInputs::prepare(root, &mut data);
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    inject_root_target_inputs_with_policy(&mut updater, root, epoch, &local, false);
    inject_registry_request_inputs(
        &mut updater,
        root,
        RegistryUrls::new([REGISTRY]),
        RegistryRequestGeneration(0),
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let mut entries = local
        .iter()
        .map(|(canonical, directory)| local_entry(&workspace, canonical, directory))
        .collect::<Vec<_>>();
    for (index, (_, repository)) in REPOSITORIES.iter().enumerate() {
        let route = transaction
            .compute(
                &RootRepositoryRouteObservationKey::for_root_build(
                    workspace.clone(),
                    ApparentRepoName::new(*repository).unwrap(),
                )
                .unwrap(),
            )
            .await
            .map_err(|error| error.to_string())?;
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(route)) = route else {
            return Err(format!("selected fixture route: {route:?}"));
        };
        let route = route
            .result()
            .as_ref()
            .as_ref()
            .map_err(ToString::to_string)?;
        let HostRepositoryMaterializationDisposition::Request(request) =
            host_repository_materialization_request(&route.source_capability())
                .map_err(|error| format!("{error:?}"))?
        else {
            return Err("selected fixture route did not request materialization".into());
        };
        entries.push(RepositoryMaterializationEpochEntry {
            request,
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    path_policy: slug_bzlmod_v2::RepositoryMaterializationPathPolicy::LegacyUnbounded,
                    source_identity: Arc::from("sha256-test"),
                    generation_root: root.join(repository),
                    observation_instance: PathObservationInstanceId::new(7680 + index as u64),
                },
            ),
        });
    }
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace.clone(), entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let key = match slug_analysis_v2::prepare_configured_node_analysis_observed(
        &mut transaction,
        workspace,
        CanonicalLabel::parse(&if target.starts_with("@@") {
            target.to_owned()
        } else {
            format!("@@rules_rust+//:{target}")
        })
        .unwrap(),
        configuration,
    )
    .await
    {
        AnalysisPreparationOutcome::Complete(Ok(Ok(key))) => key,
        other => return Err(format!("selected fixture preparation: {other:?}")),
    };
    match transaction
        .compute(&key)
        .await
        .map_err(|error| error.to_string())?
    {
        AnalysisPreparationOutcome::Complete(Ok(value)) => value
            .as_ref()
            .as_ref()
            .cloned()
            .map_err(ToString::to_string),
        other => Err(format!("selected fixture analysis: {other:?}")),
    }
}

#[test]
fn finite_registry_metadata_covers_actual_requested_versions_without_relocalizing_sources() {
    // The complete compile-context fixture already selects registry routes.
    let root = super::workspace();
    let module_path = root.join("MODULE.bazel");
    let before = fs::read(&module_path).unwrap();
    let files = registry_files(&root);
    let metadata = |name: &str, version: &str| {
        let url = format!("{REGISTRY}/modules/{name}/{version}/MODULE.bazel");
        let (_, bytes) = files.iter().find(|(key, _)| key == &url).unwrap();
        slug_bzlmod_v2::ModuleFile::parse(std::str::from_utf8(bytes).unwrap()).unwrap()
    };
    for (name, version) in [
        ("rules_cc", "1.0"),
        ("rules_cc", "0.2.17"),
        ("bazel_skylib", "1.0"),
        ("bazel_skylib", "1.8.2"),
    ] {
        let header = metadata(name, version).module.unwrap();
        assert_eq!(header.name, name);
        assert_eq!(header.version.as_deref(), Some(version));
    }
    assert!(
        !files
            .iter()
            .any(|(url, _)| url.contains("/bazel_skylib/999/"))
    );
    assert_eq!(fs::read(&module_path).unwrap(), before);
    assert!(
        !builtin_fixture::local_repositories(&root)
            .iter()
            .any(|(canonical, _)| REPOSITORIES
                .iter()
                .any(|(selected, _)| canonical == selected))
    );
}
