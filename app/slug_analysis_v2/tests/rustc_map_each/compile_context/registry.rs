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

pub(super) fn prepare_workspace(root: &std::path::Path) {
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

pub(super) fn dice(root: &std::path::Path) -> Arc<Dice> {
    let mut files = Vec::new();
    for (_, repository) in REPOSITORIES {
        let module = fs::read_to_string(root.join(repository).join("MODULE.bazel")).unwrap();
        // Pinned builtin/bazel_tools/MODULE.bazel:37 requests rules_cc 0.2.17
        // during discovery even though this fixture's root selects version 1.0.
        // Serve that one additional metadata node, with its matching declaration;
        // do not turn the offline transport into an arbitrary-version registry.
        let versions: &[&str] = if *repository == "rules_cc" {
            &["1.0", "0.2.17"]
        } else {
            &["1.0"]
        };
        for version in versions {
            let declared = format!("module(name = '{repository}', version = '1.0')");
            assert!(module.contains(&declared));
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
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(FixtureRegistry(files)));
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
    let base = root_epoch(root);
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
    let builtins = BUILTIN_MODULES
        .iter()
        .map(|(name, _)| (format!("{name}+"), *name))
        .collect::<Vec<_>>();
    let local = builtins
        .iter()
        .map(|(canonical, directory)| (canonical.as_str(), *directory))
        .chain(HOST_PLATFORM_REPOSITORIES.iter().copied())
        .collect::<Vec<_>>();
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    });
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
