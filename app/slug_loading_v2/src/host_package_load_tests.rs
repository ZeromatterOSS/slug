use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceTransaction;
use dice::DynKey;
use dice::Key;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::HostRepositoryMaterializationDisposition;
use slug_bzlmod_v2::HostRepositorySourceFileObservationKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RepositoryPackageSourceObservationKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::RootRepositoryRouteObservationKey;
use slug_bzlmod_v2::host_repository_materialization_request;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_bzlmod_v2::install_registry_io;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark::values::structs::StructRef;
use starlark_map::small_map::SmallMap;

use super::ExternalBzlCycleIdentity;
use super::ExternalBzlModuleError;
use super::ExternalBzlModuleEvalKey;
use super::ExternalBzlModuleObservationKey;
use super::ForceRootPackageObservationOuter;
use super::HostPackageLoadMode;
use super::ObservedRootPackageLoad;
use super::RepositoryBzlLabel;
use super::RepositoryPackageLoadError;
use super::RepositoryPackageLoadErrorInner;
use super::RepositoryPackageLoadKey;
use super::RepositoryPackageLoadObservationKey;
use super::RootPackageDirectLoad;
use super::RootPackageLoadObservationKey;
use super::merge_root_package_observations;
use super::resolve_external_load_label;
use super::resolve_host_load_label;
use super::resolve_root_package_direct_load;
use crate::LoadingPreparationOutcome;
use crate::RootPackageLoadKey;
use crate::cycle_detector::bzl_load_cycle_detector;
use crate::provider::FrozenUserProviderCallable;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

struct SelectedRegistryIo;

#[async_trait]
impl RegistryIo for SelectedRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        let bytes: Option<&'static [u8]> = match url.as_str() {
            "https://registry.invalid/modules/dep/1/MODULE.bazel" => {
                Some(b"module(name='dep', version='1')\n")
            }
            "https://registry.invalid/modules/dep/1/source.json" => {
                Some(br#"{"url":"https://origin.invalid/dep.tgz","integrity":"sha256-test"}"#)
            }
            "https://registry.invalid/bazel_registry.json" => None,
            _ => None,
        };
        Ok(bytes.map_or(RegistryIoOutcome::NotFound, |bytes| {
            RegistryIoOutcome::Found(Arc::from(bytes))
        }))
    }
}

#[derive(Default)]
struct EpochBuilder {
    entries: SmallMap<PathObservationDemand, PathObservationResult>,
}

impl EpochBuilder {
    fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    }

    fn node(&mut self, path: &str, kind: PathNodeKind, variant: i64) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            ))),
        );
    }

    fn directory(&mut self, path: &str, variant: i64) {
        self.node(path, PathNodeKind::Directory, variant);
    }

    fn missing(&mut self, path: &str) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }

    fn materialized_file(
        &mut self,
        instance: PathObservationInstanceId,
        path: &str,
        source: impl AsRef<[u8]>,
        variant: i64,
    ) {
        let namespace = PathObservationNamespace::Materialization(instance);
        let path = NormalizedAbsolutePath::new(path).unwrap();
        self.entries.insert(
            PathObservationDemand::new(
                namespace.clone(),
                path.dupe(),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                variant,
                variant,
                variant,
                variant,
                0o755,
            ))),
        );
        self.entries.insert(
            PathObservationDemand::new(namespace, path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_ref(),
            ))),
        );
    }

    fn materialized_directory(
        &mut self,
        instance: PathObservationInstanceId,
        path: &str,
        variant: i64,
    ) {
        self.entries.insert(
            PathObservationDemand::new(
                PathObservationNamespace::Materialization(instance),
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                variant,
                variant,
                variant,
                variant,
                0o755,
            ))),
        );
    }

    fn materialized_missing(&mut self, instance: PathObservationInstanceId, path: &str) {
        self.entries.insert(
            PathObservationDemand::new(
                PathObservationNamespace::Materialization(instance),
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }

    fn file(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
        self.node(path, PathNodeKind::RegularFile, variant);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_ref(),
            ))),
        );
    }

    fn listing(&mut self, path: &str, entries: &[(&[u8], PathDirectoryEntryKind)]) {
        let entries = PathDirectoryEntries::new(entries.iter().map(|(name, kind)| {
            PathDirectoryEntry::new(
                PathDirectoryName::new(OsString::from_vec(name.to_vec())).unwrap(),
                *kind,
            )
        }));
        self.entries.insert(
            Self::demand(path, PathObservationOperation::DirectoryEntries),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)),
        );
    }

    fn workspace_sources(module: &str, build: &str, bzl: &[(&str, &str)], variant: i64) -> Self {
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", module, variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/pkg", variant);
        builder.file("/workspace/pkg/BUILD.bazel", build, variant);
        for (name, source) in bzl {
            builder.file(&format!("/workspace/pkg/{name}"), source, variant);
        }
        builder
    }

    fn external_sources(bzl: &[(&str, &[u8])], variant: i64) -> Self {
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file(
            "/workspace/MODULE.bazel",
            "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
            variant,
        );
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/dep", variant);
        builder.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = \"dep\", version = \"1.0.0\")\n",
            variant,
        );
        builder.missing("/workspace/dep/REPO.bazel");
        builder.missing("/workspace/dep/.bazelignore");
        for (name, source) in bzl {
            builder.file(&format!("/workspace/dep/{name}"), source, variant);
        }
        builder
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

fn package_policy() -> RootPackagePolicyInputs {
    package_policy_with_deleted(&[])
}

fn package_policy_with_deleted(deleted: &[&str]) -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        workspace(),
        [workspace()],
        deleted.iter().copied(),
        None,
        Some("warning"),
    )
    .unwrap()
}

#[derive(Debug, Clone)]
struct TrackedBatch {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct EventTracker {
    batches: Mutex<Vec<TrackedBatch>>,
    package_dependencies: Mutex<Vec<Vec<String>>>,
    route_dependencies: Mutex<Vec<Vec<String>>>,
}

impl EventTracker {
    fn take(&self) -> Vec<TrackedBatch> {
        std::mem::take(&mut *self.batches.lock().unwrap())
    }

    fn take_package_dependencies(&self) -> Vec<Vec<String>> {
        std::mem::take(&mut *self.package_dependencies.lock().unwrap())
    }

    fn take_route_dependencies(&self) -> Vec<Vec<String>> {
        std::mem::take(&mut *self.route_dependencies.lock().unwrap())
    }
}

impl ActivationTracker for EventTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        if key.downcast_ref::<RootPackageLoadKey>().is_some()
            || key
                .downcast_ref::<RootPackageLoadObservationKey>()
                .is_some()
            || key.downcast_ref::<RepositoryPackageLoadKey>().is_some()
            || key
                .downcast_ref::<RepositoryPackageLoadObservationKey>()
                .is_some()
        {
            self.package_dependencies
                .lock()
                .unwrap()
                .push(deps.map(ToString::to_string).collect());
        } else if key
            .downcast_ref::<RootRepositoryRouteObservationKey>()
            .is_some()
        {
            self.route_dependencies
                .lock()
                .unwrap()
                .push(deps.map(ToString::to_string).collect());
        }
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        let name = key.to_string();
        if !name.starts_with("host-root-module-file:")
            && !name.starts_with("observed-host-root-module-file:")
            && !name.starts_with("host-bzl-module:")
            && !name.starts_with("observed-host-bzl-module:")
            && !name.starts_with("host-package-load:")
            && !name.starts_with("observed-host-package-load:")
            && !name.starts_with("external-bzl-module:")
            && !name.starts_with("observed-external-bzl-module:")
            && !name.starts_with("host-repository-source-file:")
            && !name.starts_with("observed-host-repository-source-file:")
            && !name.starts_with("host-route-repo-file:")
            && !name.starts_with("repository-package-source:")
            && !name.starts_with("observed-repository-package-source:")
            && !name.starts_with("repository-package-load:")
            && !name.starts_with("observed-repository-package-load:")
        {
            return;
        }
        self.batches.lock().unwrap().push(TrackedBatch {
            key: name,
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

async fn transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    capture_events: bool,
    tracker: Option<Arc<EventTracker>>,
) -> DiceTransaction {
    transaction_with_policy(
        dice,
        epoch,
        package_policy(),
        capture_events,
        tracker,
        false,
    )
    .await
}

async fn transaction_with_policy(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    policy: RootPackagePolicyInputs,
    capture_events: bool,
    tracker: Option<Arc<EventTracker>>,
    force_outer: bool,
) -> DiceTransaction {
    let mut user_data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    if capture_events {
        user_data.data.set(CaptureEvaluationEvents);
    }
    if force_outer {
        let error = PathObservationEpoch::from_shared([(
            EpochBuilder::demand("/mismatch", PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Missing,
            )),
        )])
        .unwrap_err()
        .into();
        user_data.data.set(ForceRootPackageObservationOuter(error));
    }
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut attributes = SmallMap::new();
    attributes.insert("path".into(), OverrideAttributeValue::String("dep".into()));
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(attributes),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
        },
    });
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            RepositoryMaterializationResultEpoch::new(
                workspace(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    inject_root_package_policy_inputs(&mut updater, policy).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace().as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater.commit().await
}

fn package_key() -> RootPackageLoadKey {
    RootPackageLoadKey::new(workspace(), PackagePath::parse("pkg").unwrap())
}

fn observed_package_key() -> RootPackageLoadObservationKey {
    RootPackageLoadObservationKey::new(workspace(), PackagePath::parse("pkg").unwrap())
}

async fn external_route(transaction: &mut DiceTransaction) -> RootRepositoryRoute {
    external_route_named(transaction, "dep").await
}

async fn external_route_named(
    transaction: &mut DiceTransaction,
    apparent_repo: &str,
) -> RootRepositoryRoute {
    let key = RootRepositoryRouteKey::new(
        workspace(),
        ApparentRepoName::new(apparent_repo).expect("valid apparent repository"),
    )
    .unwrap();
    match transaction.compute(&key).await.unwrap() {
        LoadingPreparationOutcome::Need(_) => panic!("complete external epoch returned Need"),
        LoadingPreparationOutcome::Complete(route) => route.as_ref().as_ref().unwrap().clone(),
    }
}

fn external_bzl_key(
    route: RootRepositoryRoute,
    package: &str,
    target: &str,
) -> ExternalBzlModuleEvalKey {
    let package = PackagePath::parse(package).unwrap();
    let label = resolve_external_load_label(&package, &format!(":{target}")).unwrap();
    ExternalBzlModuleEvalKey::new(route, label)
}

fn observed_external_bzl_key(
    route: RootRepositoryRoute,
    package: &str,
    target: &str,
) -> ExternalBzlModuleObservationKey {
    let package = PackagePath::parse(package).unwrap();
    let label = resolve_external_load_label(&package, &format!(":{target}")).unwrap();
    ExternalBzlModuleObservationKey::new(route, label)
}

fn event_texts(batch: &EventBatch) -> Vec<&str> {
    batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
        })
        .collect()
}

type HostPackageOutcome = <RootPackageLoadKey as Key>::Value;
type ObservedHostPackageOutcome = <RootPackageLoadObservationKey as Key>::Value;

fn observed_package(value: &ObservedHostPackageOutcome) -> &ObservedRootPackageLoad {
    let LoadingPreparationOutcome::Complete(Ok(value)) = value else {
        panic!("expected complete observed package outcome: {value:?}")
    };
    value
}

fn target_names(outcome: &HostPackageOutcome) -> Vec<&str> {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    value
        .as_ref()
        .as_ref()
        .unwrap()
        .targets
        .iter()
        .map(|target| target.name.as_str())
        .collect()
}

fn terminal_error(outcome: &HostPackageOutcome) -> String {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err().to_string()
}

async fn compute_package(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    policy: RootPackagePolicyInputs,
) -> HostPackageOutcome {
    transaction_with_policy(dice, epoch, policy, false, None, false)
        .await
        .compute(&package_key())
        .await
        .unwrap()
}

fn observed_glob_epoch(build: &str, variant: i64) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::workspace_sources(
        "print('ROOT')\n",
        build,
        &[
            (
                "one.bzl",
                "load(':nested.bzl', 'NESTED')\nprint('ONE')\nONE = NESTED\n",
            ),
            ("two.bzl", "print('TWO')\nTWO = 2\n"),
            ("nested.bzl", "print('NESTED')\nNESTED = 1\n"),
        ],
        variant,
    );
    epoch.node("/workspace/pkg/a.txt", PathNodeKind::RegularFile, variant);
    epoch.directory("/workspace/pkg/sub", variant);
    epoch.node(
        "/workspace/pkg/sub/b.txt",
        PathNodeKind::RegularFile,
        variant,
    );
    epoch.node(
        "/workspace/pkg/sub/no.txt",
        PathNodeKind::RegularFile,
        variant,
    );
    epoch.missing("/workspace/pkg/sub/BUILD.bazel");
    epoch.missing("/workspace/pkg/sub/BUILD");
    epoch.listing(
        "/workspace/pkg",
        &[
            (b"BUILD.bazel", PathDirectoryEntryKind::File),
            (b"one.bzl", PathDirectoryEntryKind::File),
            (b"two.bzl", PathDirectoryEntryKind::File),
            (b"a.txt", PathDirectoryEntryKind::File),
            (b"sub", PathDirectoryEntryKind::Directory),
        ],
    );
    epoch.listing(
        "/workspace/pkg/sub",
        &[
            (b"b.txt", PathDirectoryEntryKind::File),
            (b"no.txt", PathDirectoryEntryKind::File),
        ],
    );
    epoch.build()
}

#[test]
fn root_load_resolution_is_mapping_free_and_rejects_path_escape() {
    let package = PackagePath::parse("pkg").unwrap();
    let expected = resolve_host_load_label(&package, ":defs/x.bzl").unwrap();
    for spelling in [
        "//pkg:defs/x.bzl",
        "@//pkg:defs/x.bzl",
        "@@//pkg:defs/x.bzl",
    ] {
        assert_eq!(
            resolve_host_load_label(&package, spelling).unwrap(),
            expected
        );
    }
    for invalid in [
        ":../x.bzl",
        ":./x.bzl",
        ":a/../x.bzl",
        ":a/./x.bzl",
        ":a//x.bzl",
        ":a\\x.bzl",
        ":a:x.bzl",
        ":a/x.bzl/",
        ":x.scl",
        "@repo//pkg:x.bzl",
        "@@repo//pkg:x.bzl",
        "//external:x.bzl",
    ] {
        assert!(
            resolve_host_load_label(&package, invalid).is_err(),
            "{invalid:?} entered Host key identity"
        );
    }

    let root = resolve_host_load_label(&PackagePath::parse("").unwrap(), ":a.bzl").unwrap();
    let error = super::HostBzlModuleError::Parse {
        label: root,
        message: Arc::from("broken"),
    };
    assert_eq!(
        error.to_string(),
        "parsing //:a.bzl: broken\ncompilation of module 'a.bzl' failed"
    );
}

#[test]
fn root_package_direct_load_keeps_root_and_apparent_external_dispatch_distinct() {
    let package = PackagePath::parse("pkg").unwrap();
    assert!(matches!(
        resolve_root_package_direct_load(&package, ":defs.bzl"),
        Ok(RootPackageDirectLoad::Root(_))
    ));
    assert!(matches!(
        resolve_root_package_direct_load(&package, "@dep//tools:defs.bzl"),
        Ok(RootPackageDirectLoad::External { apparent_repo, .. }) if apparent_repo.as_str() == "dep"
    ));
    assert!(resolve_root_package_direct_load(&package, "@@dep+//tools:defs.bzl").is_err());
}

fn selected_registry_root_package_epoch() -> (PathObservationEpoch, PathObservationInstanceId) {
    let root =
        "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\n";
    let build = concat!(
        "load(':root_defs.bzl', 'ROOT_VALUE')\n",
        "load('@dep_alias//:defs.bzl', 'SELECTED_NAME')\n",
        "print('SELECTED_BUILD')\n",
        "filegroup(name=SELECTED_NAME)\n",
    );
    let mut epoch = EpochBuilder::workspace_sources(
        root,
        build,
        &[("root_defs.bzl", "print('ROOT_BZL')\nROOT_VALUE='root'\n")],
        901,
    );
    epoch.missing("/workspace/MODULE.bazel.lock");
    let instance = PathObservationInstanceId::new(77);
    epoch.materialized_directory(instance, "/", 901);
    epoch.materialized_directory(instance, "/registry-dep", 901);
    epoch.materialized_file(
        instance,
        "/registry-dep/MODULE.bazel",
        "module(name='dep', version='1')\n",
        901,
    );
    epoch.materialized_missing(instance, "/registry-dep/REPO.bazel");
    epoch.materialized_missing(instance, "/registry-dep/.bazelignore");
    epoch.materialized_file(
        instance,
        "/registry-dep/defs.bzl",
        "load(':nested.bzl', 'NESTED_NAME')\nprint('SELECTED_BZL')\nSELECTED_NAME=NESTED_NAME\n",
        901,
    );
    epoch.materialized_file(
        instance,
        "/registry-dep/nested.bzl",
        "print('SELECTED_NESTED')\nNESTED_NAME='selected_target'\n",
        901,
    );
    (epoch.build(), instance)
}

#[tokio::test]
async fn root_package_loads_selected_registry_bzl_through_admitted_route() {
    let (epoch, instance) = selected_registry_root_package_epoch();
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, Arc::new(SelectedRegistryIo));
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(EventTracker::default());
    let transaction = transaction(&dice, epoch, true, Some(tracker.dupe())).await;
    let mut updater = transaction.into_updater();
    inject_registry_request_inputs(
        &mut updater,
        workspace().as_path(),
        RegistryUrls::new(["https://registry.invalid"]),
        RegistryRequestGeneration(1),
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let route = transaction
        .compute(
            &RootRepositoryRouteObservationKey::for_root_build(
                workspace(),
                ApparentRepoName::new("dep_alias").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(route)) = route else {
        panic!("selected route returned Need")
    };
    let route = route.result().as_ref().as_ref().unwrap();
    assert_eq!(route.apparent_repo().as_str(), "dep_alias");
    assert_eq!(route.canonical_repo().as_str(), "dep+");
    let HostRepositoryMaterializationDisposition::Request(request) =
        host_repository_materialization_request(&route.source_capability()).unwrap()
    else {
        panic!("selected registry route must request materialization")
    };
    assert_eq!(request.kind, RepositoryMaterializationKind::Immutable);
    tracker.take();
    tracker.take_package_dependencies();
    let route_dependencies = tracker.take_route_dependencies();
    let route_dependencies = route_dependencies.last().unwrap();
    let root = route_dependencies
        .iter()
        .position(|dep| dep.starts_with("bzlmod-observed-host-root-module-file:"))
        .unwrap();
    let mapping = route_dependencies
        .iter()
        .position(|dep| dep.starts_with("observed-host-root-repository-mapping:"))
        .unwrap();
    let definition = route_dependencies
        .iter()
        .position(|dep| dep.starts_with("observed-host-canonical-selected-module-definition:"))
        .unwrap();
    assert!(root < mapping && mapping < definition);
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            RepositoryMaterializationResultEpoch::new(
                workspace(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            source_identity: Arc::from("sha256-test"),
                            generation_root: PathBuf::from("/registry-dep"),
                            observation_instance: instance,
                        },
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let key = observed_package_key();
    let cold = transaction.compute(&key).await.unwrap();
    let loaded = observed_package(&cold)
        .result()
        .as_ref()
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["selected_target"]
    );
    let rows = tracker.take_package_dependencies();
    let row = rows
        .iter()
        .find(|row| {
            row.iter()
                .any(|dep| dep.starts_with("observed-root-build-repository-route:"))
        })
        .expect("root package must depend on the admitted route");
    let route = row
        .iter()
        .position(|dep| dep.starts_with("observed-root-build-repository-route:"))
        .unwrap();
    let root_bzl = row
        .iter()
        .position(|dep| dep.starts_with("observed-host-bzl-module:"))
        .unwrap();
    let bzl = row
        .iter()
        .position(|dep| dep.starts_with("observed-external-bzl-module:"))
        .unwrap();
    assert!(root_bzl < route && route < bzl);
    let events = tracker.take();
    assert!(events.iter().any(|entry| {
        entry.key.starts_with("observed-host-bzl-module:")
            && entry
                .batch
                .as_ref()
                .is_some_and(|batch| event_texts(batch) == ["ROOT_BZL"])
    }));
    assert!(events.iter().any(|entry| {
        entry.key.starts_with("observed-external-bzl-module:")
            && entry
                .batch
                .as_ref()
                .is_some_and(|batch| event_texts(batch) == ["SELECTED_BZL"])
    }));
    assert!(events.iter().any(|entry| {
        entry.key.starts_with("observed-host-package-load:")
            && entry
                .batch
                .as_ref()
                .is_some_and(|batch| event_texts(batch) == ["SELECTED_BUILD"])
    }));

    let warm = transaction.compute(&key).await.unwrap();
    assert!(RootPackageLoadObservationKey::equality(&cold, &warm));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
}

#[tokio::test]
async fn observed_root_package_preserves_semantics_arcs_order_events_and_families() {
    const BUILD: &str = concat!(
        "load(':one.bzl', 'ONE')\n",
        "load(':two.bzl', 'TWO')\n",
        "print('BUILD')\n",
        "exports_files(glob(['*.txt']) + glob(['sub/*.txt'], exclude = ['sub/no.txt']))\n",
        "filegroup(name = 'g', srcs = glob(['*.txt']))\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = observed_glob_epoch(BUILD, 1);
    let mut transaction = transaction(&dice, epoch.dupe(), true, Some(tracker.dupe())).await;
    let observed_key = observed_package_key();
    let observed = transaction.compute(&observed_key).await.unwrap();
    let legacy = transaction.compute(&package_key()).await.unwrap();
    let observed_value = observed_package(&observed);
    let LoadingPreparationOutcome::Complete(legacy_value) = &legacy else {
        panic!("legacy package must complete")
    };
    assert_eq!(observed_value.result().as_ref(), legacy_value.as_ref());
    assert_eq!(
        observed_value
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["a.txt", "sub/b.txt", "g"]
    );
    assert!(
        observed_value
            .observations()
            .get(&EpochBuilder::demand(
                "/workspace/pkg",
                PathObservationOperation::DirectoryEntries,
            ))
            .is_some()
    );
    for (demand, result) in observed_value.observations().observations() {
        assert!(Arc::ptr_eq(result, epoch.get(demand).unwrap()));
    }

    let batches = tracker.take();
    let batch = |prefix: &str| {
        batches
            .iter()
            .find(|entry| entry.key.starts_with(prefix))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap()
    };
    assert_eq!(event_texts(batch("observed-host-package-load:")), ["BUILD"]);
    assert_eq!(event_texts(batch("host-package-load:")), ["BUILD"]);
    let observed_bzl_batch = |suffix: &str| {
        batches
            .iter()
            .find(|entry| {
                entry.key.starts_with("observed-host-bzl-module:") && entry.key.ends_with(suffix)
            })
            .and_then(|entry| entry.batch.as_ref())
            .unwrap()
    };
    assert_eq!(event_texts(observed_bzl_batch("one.bzl")), ["ONE"]);
    assert_eq!(event_texts(observed_bzl_batch("nested.bzl")), ["NESTED"]);
    for path in ["one.bzl", "nested.bzl"] {
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ] {
            let demand = EpochBuilder::demand(&format!("/workspace/pkg/{path}"), operation);
            assert!(Arc::ptr_eq(
                observed_value.observations().get(&demand).unwrap(),
                epoch.get(&demand).unwrap()
            ));
        }
    }

    let dependencies = tracker.take_package_dependencies();
    assert_eq!(dependencies.len(), 2);
    let observed_dependencies = dependencies
        .iter()
        .find(|deps| {
            deps.first()
                .is_some_and(|dep| dep.starts_with("observed-root-module-loading-anchor:"))
        })
        .unwrap();
    let legacy_dependencies = dependencies
        .iter()
        .find(|deps| {
            deps.first()
                .is_some_and(|dep| dep.starts_with("root-module-loading-anchor:"))
        })
        .unwrap();
    assert!(observed_dependencies.iter().all(
        |dep| !dep.starts_with("host-bzl-module:") && !dep.starts_with("host-glob-traversal:")
    ));
    assert!(
        legacy_dependencies
            .iter()
            .all(|dep| !dep.starts_with("observed-"))
    );
    assert_eq!(
        observed_dependencies
            .iter()
            .filter(|dep| dep.starts_with("observed-host-bzl-module:"))
            .map(|dep| dep.rsplit(':').next().unwrap())
            .collect::<Vec<_>>(),
        ["one.bzl", "two.bzl"]
    );
    assert_eq!(
        observed_dependencies
            .iter()
            .filter(|dep| dep.starts_with("observed-host-glob-traversal:"))
            .map(|dep| dep.rsplit(':').next().unwrap())
            .collect::<Vec<_>>(),
        ["2a2e747874", "7375622f2a2e747874", "7375622f6e6f2e747874"]
    );
    assert!(RootPackageLoadObservationKey::validity(&observed));
    assert!(RootPackageLoadObservationKey::equality(
        &observed, &observed
    ));
}

#[tokio::test]
async fn observed_root_package_reuses_restores_and_preserves_terminal_polarity() {
    const BUILD_A: &str = "exports_files(['a.txt'])\n";
    const BUILD_B: &str = "filegroup(name = 'changed')\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = observed_package_key();

    let mut cold = transaction(&dice, observed_glob_epoch(BUILD_A, 1), false, None).await;
    let first = cold.compute(&key).await.unwrap();
    let mut warm = transaction(&dice, observed_glob_epoch(BUILD_A, 1), false, None).await;
    let repeated = warm.compute(&key).await.unwrap();
    assert!(RootPackageLoadObservationKey::equality(&first, &repeated));

    let mut changed = transaction(&dice, observed_glob_epoch(BUILD_B, 2), false, None).await;
    let changed = changed.compute(&key).await.unwrap();
    assert!(!RootPackageLoadObservationKey::equality(&first, &changed));
    let mut restored = transaction(&dice, observed_glob_epoch(BUILD_A, 1), false, None).await;
    let restored = restored.compute(&key).await.unwrap();
    assert!(RootPackageLoadObservationKey::equality(&first, &restored));

    let mut deleted_epoch = EpochBuilder::workspace_sources("", "", &[], 3);
    deleted_epoch.missing("/workspace/pkg/BUILD.bazel");
    deleted_epoch.missing("/workspace/pkg/BUILD");
    let mut deleted = transaction(&dice, deleted_epoch.build(), false, None).await;
    let deleted = deleted.compute(&key).await.unwrap();
    assert!(observed_package(&deleted).result().is_err());
    let mut recreated = transaction(&dice, observed_glob_epoch(BUILD_A, 1), false, None).await;
    let recreated = recreated.compute(&key).await.unwrap();
    assert!(RootPackageLoadObservationKey::equality(&first, &recreated));

    let need_tracker = Arc::new(EventTracker::default());
    let mut need = transaction(
        &dice,
        PathObservationEpoch::empty(),
        true,
        Some(need_tracker.dupe()),
    )
    .await;
    let need = need.compute(&key).await.unwrap();
    assert!(matches!(need, LoadingPreparationOutcome::Need(_)));
    assert!(!RootPackageLoadObservationKey::validity(&need));
    assert!(
        need_tracker
            .take()
            .iter()
            .all(|entry| entry.batch.is_none())
    );

    let outer_tracker = Arc::new(EventTracker::default());
    let outer_dice = Dice::builder().build(DetectCycles::Enabled);
    let mut outer = transaction_with_policy(
        &outer_dice,
        observed_glob_epoch(BUILD_A, 1),
        package_policy(),
        true,
        Some(outer_tracker.dupe()),
        true,
    )
    .await;
    let outer = outer.compute(&key).await.unwrap();
    assert!(matches!(outer, LoadingPreparationOutcome::Complete(Err(_))));
    assert!(RootPackageLoadObservationKey::validity(&outer));
    assert!(RootPackageLoadObservationKey::equality(&outer, &outer));
    assert!(outer_tracker.take().iter().any(|entry| {
        entry.key.starts_with("observed-host-package-load:") && entry.batch.is_none()
    }));

    let invalid_dice = Dice::builder().build(DetectCycles::Enabled);
    let invalid_epoch = EpochBuilder::workspace_sources("", "[", &[], 9).build();
    let mut invalid = transaction(&invalid_dice, invalid_epoch.dupe(), true, None).await;
    let invalid = invalid.compute(&key).await.unwrap();
    let invalid = observed_package(&invalid);
    assert!(invalid.result().is_err());
    assert!(
        invalid
            .observations()
            .get(&EpochBuilder::demand(
                "/workspace/pkg",
                PathObservationOperation::DirectoryEntries,
            ))
            .is_none()
    );
    for (demand, result) in invalid.observations().observations() {
        assert!(Arc::ptr_eq(result, invalid_epoch.get(demand).unwrap()));
    }
}

#[test]
fn root_package_epoch_union_is_left_stable_and_conflicts_are_outer() {
    let result = |variant| {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::RegularFile,
            variant,
            variant,
            variant,
            variant,
            0o644,
        )))
    };
    let demand = EpochBuilder::demand("/same", PathObservationOperation::Lstat);
    let first = PathObservationEpoch::new([(demand.dupe(), result(1))]).unwrap();
    let duplicate = PathObservationEpoch::new([(demand.dupe(), result(1))]).unwrap();
    let conflict = PathObservationEpoch::new([(demand.dupe(), result(2))]).unwrap();
    let first_arc = first.get(&demand).unwrap().dupe();
    let merged =
        merge_root_package_observations(HostPackageLoadMode::Observed, first.dupe(), &duplicate)
            .unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first_arc));
    assert!(
        merge_root_package_observations(HostPackageLoadMode::Observed, merged, &conflict).is_err()
    );
}

#[tokio::test]
async fn observed_root_package_cancellation_publishes_no_parent_and_recovers() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = observed_glob_epoch("exports_files(['a.txt'])\n", 1);
    let mut cancelled = transaction(&dice, epoch.dupe(), true, Some(tracker.dupe())).await;
    let key = observed_package_key();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(tracker.take().is_empty());
    drop(cancelled);

    let mut successor = transaction(&dice, epoch, true, Some(tracker)).await;
    let recovered = successor.compute(&key).await.unwrap();
    assert!(observed_package(&recovered).result().is_ok());
}

#[tokio::test]
async fn host_package_need_is_transient_and_root_anchor_precedes_source() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let key = package_key();
    let outcome = transaction.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(need) = &outcome else {
        panic!("empty Host epoch did not request the root observation");
    };
    assert_eq!(
        need.path_observations().unwrap().demands()[0]
            .path()
            .as_path(),
        std::path::Path::new("/")
    );
    assert_ne!(
        key,
        RootPackageLoadKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
            PackagePath::parse("pkg").unwrap(),
        )
    );
    assert_ne!(
        key,
        RootPackageLoadKey::new(workspace(), PackagePath::parse("other").unwrap())
    );
    assert!(!RootPackageLoadKey::validity(&outcome));
    assert!(!RootPackageLoadKey::equality(&outcome, &outcome));
}

#[tokio::test]
async fn host_package_loads_bzl_and_owns_only_local_complete_events() {
    let module = "print(\"ROOT\")\n";
    let build =
        "load(\":defs.bzl\", \"make\")\nprint(\"BUILD\")\nmake()\nfilegroup(name = \"x\")\n";
    let defs = "print(\"BZL\")\ndef make():\n    print(\"MACRO\")\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let mut transaction = transaction(
        &dice,
        EpochBuilder::workspace_sources(module, build, &[("defs.bzl", defs)], 1).build(),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let outcome = transaction.compute(&package_key()).await.unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    let package = value.as_ref().as_ref().unwrap();
    assert_eq!(package.targets.len(), 1);
    assert_eq!(package.targets[0].name, "x");

    let batches = tracker.take();
    let batch = |prefix: &str| {
        let entry = batches
            .iter()
            .find(|entry| entry.key.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing activation for {prefix}: {batches:?}"));
        assert!(matches!(
            entry.kind,
            ActivationKind::Evaluated | ActivationKind::Reused
        ));
        event_texts(entry.batch.as_ref().unwrap())
    };
    assert_eq!(batch("host-root-module-file:"), ["ROOT"]);
    assert_eq!(batch("host-bzl-module:"), ["BZL"]);
    assert_eq!(batch("host-package-load:"), ["BUILD", "MACRO"]);
}

#[tokio::test]
async fn host_native_toolchain_targets_preserve_root_load_lifecycle_and_ownership() {
    let build = |event: &str, constraint: &str| {
        format!(
            concat!(
                "print(\"{event}\")\n",
                "constraint_setting(name = \"setting\")\n",
                "constraint_value(name = \"first\", constraint_setting = \":setting\")\n",
                "constraint_value(name = \"second\", constraint_setting = \":setting\")\n",
                "platform(name = \"exec\", constraint_values = [\":{constraint}\"])\n",
                "toolchain_type(name = \"type\")\n",
                "toolchain(name = \"registered\", exec_compatible_with = [\":{constraint}\"], ",
                "toolchain = \":implementation\", toolchain_type = \":type\")\n",
            ),
            event = event,
            constraint = constraint,
        )
    };
    let epoch = |source: Option<&str>, build_variant| {
        let mut epoch = EpochBuilder::workspace_sources(
            "print(\"ROOT\")\n",
            source.unwrap_or_default(),
            &[],
            1,
        );
        match source {
            Some(source) => epoch.file("/workspace/pkg/BUILD.bazel", source, build_variant),
            None => {
                epoch.missing("/workspace/pkg/BUILD.bazel");
                epoch.missing("/workspace/pkg/BUILD");
            }
        }
        epoch.build()
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = package_key();
    let build_a = build("BUILD_A", "first");
    let build_b = build("BUILD_B", "second");

    let cold_tracker = Arc::new(EventTracker::default());
    let mut cold_tx = transaction(
        &dice,
        epoch(Some(&build_a), 10),
        true,
        Some(cold_tracker.dupe()),
    )
    .await;
    let cold = cold_tx.compute(&key).await.unwrap();
    assert_eq!(
        target_names(&cold),
        ["setting", "first", "second", "exec", "type", "registered"]
    );
    assert!(RootPackageLoadKey::validity(&cold));
    let cold_batches = cold_tracker.take();
    assert_eq!(cold_batches.len(), 2);
    assert_eq!(
        event_texts(
            cold_batches
                .iter()
                .find(|entry| entry.key.starts_with("host-root-module-file:"))
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["ROOT"]
    );
    assert_eq!(
        event_texts(
            cold_batches
                .iter()
                .find(|entry| entry.key.starts_with("host-package-load:"))
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["BUILD_A"]
    );
    let cold_dependencies = cold_tracker.take_package_dependencies();
    assert_eq!(cold_dependencies.len(), 1);
    assert_eq!(
        cold_dependencies[0].first().map(String::as_str),
        Some("root-module-loading-anchor:\"/workspace\"")
    );

    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm_tx = transaction(
        &dice,
        epoch(Some(&build_a), 10),
        true,
        Some(warm_tracker.dupe()),
    )
    .await;
    let warm = warm_tx.compute(&key).await.unwrap();
    assert!(RootPackageLoadKey::equality(&cold, &warm));
    assert!(
        warm_tracker
            .take()
            .iter()
            .all(|entry| { entry.kind == ActivationKind::Reused && entry.batch.is_none() })
    );

    let changed_tracker = Arc::new(EventTracker::default());
    let mut changed_tx = transaction(
        &dice,
        epoch(Some(&build_b), 11),
        true,
        Some(changed_tracker.dupe()),
    )
    .await;
    let changed = changed_tx.compute(&key).await.unwrap();
    assert!(!RootPackageLoadKey::equality(&cold, &changed));
    let changed_batches = changed_tracker.take();
    assert!(
        changed_batches
            .iter()
            .all(|entry| { entry.key.starts_with("host-package-load:") || entry.batch.is_none() })
    );
    assert_eq!(
        event_texts(
            changed_batches
                .iter()
                .find(|entry| entry.key.starts_with("host-package-load:"))
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["BUILD_B"]
    );
    let changed_dependencies = changed_tracker.take_package_dependencies();
    assert_eq!(changed_dependencies.len(), 1);
    assert_eq!(
        changed_dependencies[0].first(),
        cold_dependencies[0].first()
    );

    let restored = compute_package(&dice, epoch(Some(&build_a), 12), package_policy()).await;
    assert!(RootPackageLoadKey::equality(&cold, &restored));

    let deleted = compute_package(&dice, epoch(None, 13), package_policy()).await;
    assert_eq!(
        terminal_error(&deleted),
        "no BUILD.bazel or BUILD file in package //pkg"
    );
    assert!(RootPackageLoadKey::validity(&deleted));

    let recreated = compute_package(&dice, epoch(Some(&build_a), 14), package_policy()).await;
    assert!(RootPackageLoadKey::equality(&cold, &recreated));
}

#[tokio::test]
async fn host_package_retained_graph_replays_all_input_lifecycles() {
    let module = "";
    let package_epoch = |build: Option<(&str, &str)>, bzl: &[(&str, &str)], variant| {
        let mut builder = EpochBuilder::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", module, variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/pkg", variant);
        match build {
            Some(("BUILD", source)) => {
                builder.missing("/workspace/pkg/BUILD.bazel");
                builder.file("/workspace/pkg/BUILD", source, variant);
            }
            Some(("BUILD.bazel", source)) => {
                builder.file("/workspace/pkg/BUILD.bazel", source, variant);
            }
            Some((name, _)) => panic!("unsupported BUILD name {name}"),
            None => {
                builder.missing("/workspace/pkg/BUILD.bazel");
                builder.missing("/workspace/pkg/BUILD");
            }
        }
        for (name, source) in bzl {
            builder.file(&format!("/workspace/pkg/{name}"), source, variant);
        }
        builder
    };

    let b_cycle = "load(\":a.bzl\", \"x\")\nx = 1\n";
    let b_fixed = "x = 1\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);

    let missing_marker = compute_package(
        &dice,
        package_epoch(None, &[], 10).build(),
        package_policy(),
    )
    .await;
    assert_eq!(
        terminal_error(&missing_marker),
        "no BUILD.bazel or BUILD file in package //pkg"
    );

    let fallback_build = "load(\":a.bzl\", \"value\")\nfilegroup(name = value)\n";
    let fallback = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD", fallback_build)),
            &[("a.bzl", "value = \"fallback\"\n")],
            11,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&fallback), ["fallback"]);

    let edited_build = compute_package(
        &dice,
        package_epoch(
            Some((
                "BUILD.bazel",
                "load(\":a.bzl\", \"value\")\nfilegroup(name = \"edited_build\")\n",
            )),
            &[("a.bzl", "value = \"ignored\"\n")],
            12,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&edited_build), ["edited_build"]);

    let edited_bzl = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"edited_bzl\"\n")],
            13,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&edited_bzl), ["edited_bzl"]);

    let invalid_bzl = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value =\n")],
            14,
        )
        .build(),
        package_policy(),
    )
    .await;
    let invalid_error = terminal_error(&invalid_bzl);
    assert!(invalid_error.contains("loading `:a.bzl`: parsing //pkg:a.bzl"));
    assert!(invalid_error.contains("compilation of module 'pkg/a.bzl' failed"));

    let nested_build = "load(\":sub/n.bzl\", \"value\")\nfilegroup(name = value)\n";
    let mut nested_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested\"\n")],
        15,
    );
    nested_epoch.directory("/workspace/pkg/sub", 15);
    nested_epoch.missing("/workspace/pkg/sub/BUILD.bazel");
    nested_epoch.missing("/workspace/pkg/sub/BUILD");
    let nested = compute_package(&dice, nested_epoch.build(), package_policy()).await;
    assert_eq!(target_names(&nested), ["nested"]);

    let mut crossing_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested\"\n")],
        16,
    );
    crossing_epoch.directory("/workspace/pkg/sub", 16);
    crossing_epoch.node(
        "/workspace/pkg/sub/BUILD.bazel",
        PathNodeKind::RegularFile,
        16,
    );
    let crossing = compute_package(&dice, crossing_epoch.build(), package_policy()).await;
    assert_eq!(
        terminal_error(&crossing),
        "loading `:sub/n.bzl`: label in package //pkg crosses boundary of subpackage //pkg/sub"
    );

    let mut restored_nested_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested_restored\"\n")],
        17,
    );
    restored_nested_epoch.directory("/workspace/pkg/sub", 17);
    restored_nested_epoch.missing("/workspace/pkg/sub/BUILD.bazel");
    restored_nested_epoch.missing("/workspace/pkg/sub/BUILD");
    let restored_nested =
        compute_package(&dice, restored_nested_epoch.build(), package_policy()).await;
    assert_eq!(target_names(&restored_nested), ["nested_restored"]);

    let load_edge = compute_package(
        &dice,
        package_epoch(
            Some((
                "BUILD.bazel",
                "load(\":b.bzl\", \"value\")\nfilegroup(name = value)\n",
            )),
            &[("a.bzl", "value =\n"), ("b.bzl", "value = \"new_edge\"\n")],
            18,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&load_edge), ["new_edge"]);

    let cycle_build = "load(\":a.bzl\", \"x\")\nfilegroup(name = \"cycle_ok\")\n";
    let a_cycle = "load(\":b.bzl\", \"x\")\nx = 1\n";
    let cycle = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_cycle)],
            19,
        )
        .build(),
        package_policy(),
    )
    .await;
    let cycle = terminal_error(&cycle);
    assert!(cycle.starts_with("cycle detected in extension files: \n    pkg/BUILD.bazel"));
    assert!(cycle.contains("//pkg:a.bzl"));
    assert!(cycle.contains("//pkg:b.bzl"));

    let fixed = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_fixed)],
            20,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&fixed), ["cycle_ok"]);

    let restored_cycle = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_cycle)],
            21,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert!(terminal_error(&restored_cycle).starts_with("cycle detected in extension files:"));

    let deleted = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"policy_restored\"\n")],
            22,
        )
        .build(),
        package_policy_with_deleted(&["//pkg"]),
    )
    .await;
    assert_eq!(
        terminal_error(&deleted),
        "package //pkg is deleted or ignored"
    );

    let restored = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"policy_restored\"\n")],
            23,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&restored), ["policy_restored"]);
}

type ExternalBzlOutcome = <ExternalBzlModuleEvalKey as Key>::Value;
type ObservedExternalBzlOutcome = <ExternalBzlModuleObservationKey as Key>::Value;
type RepositoryPackageOutcome = <RepositoryPackageLoadKey as Key>::Value;
type ObservedRepositoryPackageOutcome = <RepositoryPackageLoadObservationKey as Key>::Value;

fn observed_repository_package(
    outcome: &ObservedRepositoryPackageOutcome,
) -> &super::ObservedRepositoryPackageLoad {
    let LoadingPreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("expected complete observed repository package: {outcome:?}");
    };
    value
}

fn observed_repository_package_error(
    outcome: &ObservedRepositoryPackageOutcome,
) -> &RepositoryPackageLoadError {
    observed_repository_package(outcome)
        .result()
        .as_ref()
        .as_ref()
        .unwrap_err()
}

fn repository_package_terminal(outcome: &RepositoryPackageOutcome) -> &crate::LoadedPackage {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap()
}

fn repository_package_error(outcome: &RepositoryPackageOutcome) -> String {
    repository_package_typed_error(outcome).to_string()
}

fn repository_package_typed_error(
    outcome: &RepositoryPackageOutcome,
) -> &RepositoryPackageLoadError {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err()
}

fn external_terminal(outcome: &ExternalBzlOutcome) -> &super::FrozenBzlModule {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap()
}

fn external_error(outcome: &ExternalBzlOutcome) -> &ExternalBzlModuleError {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err()
}

fn observed_external(outcome: &ObservedExternalBzlOutcome) -> &super::ObservedExternalBzlModule {
    let LoadingPreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("expected complete observed external bzl outcome: {outcome:?}");
    };
    value
}

fn observed_external_error(outcome: &ObservedExternalBzlOutcome) -> &ExternalBzlModuleError {
    observed_external(outcome)
        .result()
        .as_ref()
        .as_ref()
        .unwrap_err()
}

fn assert_same_epoch_arcs(left: &PathObservationEpoch, right: &PathObservationEpoch) {
    assert_eq!(left.observations().len(), right.observations().len());
    for ((left_demand, left_result), (right_demand, right_result)) in
        left.observations().iter().zip(right.observations().iter())
    {
        assert_eq!(left_demand, right_demand);
        assert_eq!(left_result, right_result);
        assert!(Arc::ptr_eq(left_result, right_result));
    }
}
#[test]
fn repository_package_observation_reducer_preserves_outer_union_and_legacy_arc() {
    let demand = EpochBuilder::demand("/same", PathObservationOperation::Lstat);
    let result = PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
        PathNodeKind::RegularFile,
        1,
        1,
        1,
        1,
        0o644,
    )));
    let first = PathObservationEpoch::new([(demand.dupe(), result.clone())]).unwrap();
    let first_arc = first.get(&demand).unwrap().dupe();
    let duplicate = PathObservationEpoch::new([(demand.dupe(), result)]).unwrap();
    let merged = super::merge_repository_package_observations(&first, &duplicate).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first_arc));
    let conflict = PathObservationEpoch::new([(
        demand.dupe(),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    )])
    .unwrap();
    assert!(super::merge_repository_package_observations(&merged, &conflict).is_err());
    let outer: slug_workspace_v2::ObservedPathFrontierError =
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::FileBytes,
        }
        .into();
    for decisive in 0..3 {
        let stopped = (0..3)
            .position(|slot| {
                let outcome = if slot == decisive {
                    LoadingPreparationOutcome::Complete(Err(outer.clone()))
                } else {
                    LoadingPreparationOutcome::Complete(Ok(()))
                };
                super::finish_repository_package_observed_child(outcome).is_break()
            })
            .unwrap();
        assert_eq!(stopped, decisive);
    }
    let terminal: ObservedRepositoryPackageOutcome =
        LoadingPreparationOutcome::Complete(Err(outer));
    assert!(RepositoryPackageLoadObservationKey::validity(&terminal));
    assert!(RepositoryPackageLoadObservationKey::equality(
        &terminal, &terminal
    ));
    let held = Arc::new(Err(RepositoryPackageLoadError::new(
        RepositoryPackageLoadErrorInner::SourceCompute {
            canonical_repo: "dep+".into(),
            package: PackagePath::parse("").unwrap(),
            message: Arc::from("held"),
        },
    )));
    let projected = super::project_legacy_repository_package_load(
        LoadingPreparationOutcome::Complete(Ok((held.dupe(), PathObservationEpoch::empty()))),
    );
    let LoadingPreparationOutcome::Complete(projected) = projected else {
        panic!("expected legacy projection");
    };
    assert!(Arc::ptr_eq(&held, &projected));
    let query = concat!(
        include_str!("../../slug_query_v2/src/graph.rs"),
        include_str!("../../slug_query_v2/src/loading_environment.rs"),
    );
    let core = include_str!("../../slug_core_v2/src/runtime/dice.rs");
    assert!(query.contains("RepositoryPackageLoadObservationKey"));
    assert!(core.contains("RepositoryPackageLoadObservationKey"));
}
#[tokio::test]
async fn observed_external_bzl_retains_recursive_epoch_arcs_and_local_events() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":left.bzl\", \"LEFT\")\nload(\":right.bzl\", \"RIGHT\")\nprint(\"ROOT_BZL\")\nRESULT = LEFT + RIGHT\n",
        ),
        (
            "left.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"LEFT_BZL\")\nLEFT = H\n",
        ),
        (
            "right.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"RIGHT_BZL\")\nRIGHT = H\n",
        ),
        ("helper.bzl", b"print(\"HELPER_BZL\")\nH = 1\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(files, 140).build();
    let mut cold = transaction(&dice, epoch.dupe(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    tracker.take();
    let key = observed_external_bzl_key(route.clone(), "", "root.bzl");
    let value = cold.compute(&key).await.unwrap();
    let carrier = observed_external(&value);
    let module = carrier.result().as_ref().as_ref().unwrap();
    assert_eq!(module.manifest.reachable.len(), 4);
    assert!(ExternalBzlModuleObservationKey::validity(&value));
    assert!(ExternalBzlModuleObservationKey::equality(&value, &value));
    let batches = tracker.take();
    assert!(batches.iter().all(|entry| {
        !entry.key.starts_with("external-bzl-module:")
            && !entry.key.starts_with("host-repository-source-file:")
            && !entry.key.starts_with("repository-package-load:")
    }));
    let evaluated = batches
        .iter()
        .filter(|entry| {
            entry.key.starts_with("observed-external-bzl-module:")
                && entry.kind == ActivationKind::Evaluated
        })
        .collect::<Vec<_>>();
    assert_eq!(evaluated.len(), 4);
    assert_eq!(
        evaluated
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        [
            "observed-external-bzl-module:@@dep+//:helper.bzl",
            "observed-external-bzl-module:@@dep+//:left.bzl",
            "observed-external-bzl-module:@@dep+//:right.bzl",
            "observed-external-bzl-module:@@dep+//:root.bzl",
        ]
    );
    assert_eq!(
        evaluated
            .iter()
            .map(|entry| event_texts(entry.batch.as_ref().unwrap()))
            .collect::<Vec<_>>(),
        [
            vec!["HELPER_BZL"],
            vec!["LEFT_BZL"],
            vec!["RIGHT_BZL"],
            vec!["ROOT_BZL"],
        ]
    );
    let mut expected = PathObservationEpoch::empty();
    for path in ["root.bzl", "left.bzl", "helper.bzl", "right.bzl"] {
        let source = cold
            .compute(&HostRepositorySourceFileObservationKey::new(
                route.clone(),
                PathBuf::from(path),
            ))
            .await
            .unwrap();
        let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
            panic!("expected complete observed external source");
        };
        expected = super::union_host_observations(&expected, source.observations()).unwrap();
    }
    assert_same_epoch_arcs(carrier.observations(), &expected);
    tracker.take();
    let legacy_value = cold
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    assert_eq!(external_terminal(&legacy_value).manifest.reachable.len(), 4);
    assert!(tracker.take().iter().all(|entry| {
        !entry.key.starts_with("observed-external-bzl-module:")
            && !entry
                .key
                .starts_with("observed-host-repository-source-file:")
    }));
    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let warm_route = external_route(&mut warm).await;
    warm_tracker.take();
    let warm_value = warm
        .compute(&observed_external_bzl_key(warm_route, "", "root.bzl"))
        .await
        .unwrap();
    assert!(ExternalBzlModuleObservationKey::equality(
        &value,
        &warm_value
    ));
    assert_same_epoch_arcs(
        carrier.observations(),
        observed_external(&warm_value).observations(),
    );
    assert!(warm_tracker.take().iter().all(|entry| {
        !entry.key.starts_with("observed-external-bzl-module:")
            || (entry.kind == ActivationKind::Reused && entry.batch.is_none())
    }));
    let a_fingerprint = module.manifest.fingerprint;
    let edited_root =
        b"load(\":left.bzl\", \"LEFT\")\nload(\":right.bzl\", \"RIGHT\")\nRESULT = LEFT + RIGHT + 1\n";
    for (variant, source, restored) in [
        (141, Some(edited_root.as_slice()), false),
        (142, None, false),
        (143, Some(edited_root.as_slice()), false),
        (144, Some(files[0].1), true),
    ] {
        let mut next_files = files[1..].to_vec();
        if let Some(source) = source {
            next_files.insert(0, ("root.bzl", source));
        }
        let mut next_epoch = EpochBuilder::external_sources(&next_files, variant);
        if source.is_none() {
            next_epoch.missing("/workspace/dep/root.bzl");
        }
        let mut next = transaction(&dice, next_epoch.build(), false, None).await;
        let route = external_route(&mut next).await;
        let next = next
            .compute(&observed_external_bzl_key(route, "", "root.bzl"))
            .await
            .unwrap();
        match observed_external(&next).result().as_ref() {
            Ok(module) => assert_eq!(module.manifest.fingerprint == a_fingerprint, restored),
            Err(error) => {
                assert!(source.is_none() && matches!(error, ExternalBzlModuleError::Absent { .. }))
            }
        }
    }
}
#[tokio::test]
async fn observed_external_bzl_terminals_keep_decisive_prefixes_and_stop_children() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut absent_epoch = EpochBuilder::external_sources(&[], 150);
    absent_epoch.missing("/workspace/dep/missing.bzl");
    let mut absent = transaction(&dice, absent_epoch.build(), false, None).await;
    let route = external_route(&mut absent).await;
    let absent_value = absent
        .compute(&observed_external_bzl_key(route.clone(), "", "missing.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        observed_external_error(&absent_value),
        ExternalBzlModuleError::Absent { .. }
    ));
    assert!(ExternalBzlModuleObservationKey::validity(&absent_value));
    let source = absent
        .compute(&HostRepositorySourceFileObservationKey::new(
            route,
            PathBuf::from("missing.bzl"),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
        panic!("expected missing source carrier");
    };
    assert_same_epoch_arcs(
        observed_external(&absent_value).observations(),
        source.observations(),
    );
    let tracker = Arc::new(EventTracker::default());
    let mut need = transaction(
        &dice,
        EpochBuilder::external_sources(&[], 152).build(),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let route = external_route(&mut need).await;
    tracker.take();
    let need_value = need
        .compute(&observed_external_bzl_key(route, "", "need.bzl"))
        .await
        .unwrap();
    assert!(matches!(need_value, LoadingPreparationOutcome::Need(_)));
    assert!(!ExternalBzlModuleObservationKey::validity(&need_value));
    assert!(!ExternalBzlModuleObservationKey::equality(
        &need_value,
        &need_value,
    ));
    let activations = tracker.take();
    assert!(
        activations
            .iter()
            .filter(|entry| entry.key.starts_with("observed-external-bzl-module:"))
            .all(|entry| entry.key.ends_with("need.bzl") && entry.batch.is_none())
    );
    let outer = PathObservationEpoch::from_shared([(
        EpochBuilder::demand("/outer", PathObservationOperation::Lstat),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Missing,
        )),
    )])
    .unwrap_err()
    .into();
    assert!(matches!(
        super::external_bzl_observed_child::<()>(LoadingPreparationOutcome::Complete(Err(outer))),
        std::ops::ControlFlow::Break(LoadingPreparationOutcome::Complete(Err(_)))
    ));
    let demand = EpochBuilder::demand("/same", PathObservationOperation::Lstat);
    let result = |variant| {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::RegularFile,
            variant,
            variant,
            variant,
            variant,
            0o644,
        )))
    };
    let first = PathObservationEpoch::new([(demand.dupe(), result(1))]).unwrap();
    let duplicate = PathObservationEpoch::new([(demand.dupe(), result(1))]).unwrap();
    let conflict = PathObservationEpoch::new([(demand.dupe(), result(2))]).unwrap();
    let first_arc = first.get(&demand).unwrap().dupe();
    let merged = super::union_host_observations(&first, &duplicate).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first_arc));
    assert!(super::union_host_observations(&merged, &conflict).is_err());
    let absent_prefix = super::finish_external_bzl_source(
        Ok(slug_bzlmod_v2::HostRepositorySourceFileValue::Absent),
        CanonicalLabel::parse("@@dep+//:missing.bzl").unwrap(),
        first.dupe(),
    )
    .unwrap_err();
    let LoadingPreparationOutcome::Complete(Ok((result, retained))) = absent_prefix else {
        panic!("cycle source Absent did not remain semantic");
    };
    assert!(matches!(
        result.as_ref(),
        Err(ExternalBzlModuleError::Absent { .. })
    ));
    assert_same_epoch_arcs(&retained, &first);
}
#[tokio::test]
async fn observed_external_bzl_child_positions_stop_at_need_or_semantic() {
    const PARENT: &[u8] =
        b"load(\":a.bzl\", \"A\")\nload(\":b.bzl\", \"B\")\nload(\":c.bzl\", \"C\")\n";
    const CHILDREN: [(&str, &[u8]); 3] = [
        ("a.bzl", b"A = 1\n"),
        ("b.bzl", b"B = 1\n"),
        ("c.bzl", b"C = 1\n"),
    ];
    for position in 0..CHILDREN.len() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(EventTracker::default());
        let mut files = vec![("parent.bzl", PARENT)];
        files.extend(
            CHILDREN
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != position)
                .map(|(_, file)| *file),
        );
        let mut tx = transaction(
            &dice,
            EpochBuilder::external_sources(&files, 170 + position as i64).build(),
            true,
            Some(tracker.dupe()),
        )
        .await;
        let route = external_route(&mut tx).await;
        tracker.take();
        let value = tx
            .compute(&observed_external_bzl_key(route, "", "parent.bzl"))
            .await
            .unwrap();
        assert!(matches!(value, LoadingPreparationOutcome::Need(_)));
        let activations = tracker.take();
        assert!(
            CHILDREN[position + 1..]
                .iter()
                .all(|(name, _)| { activations.iter().all(|entry| !entry.key.contains(name)) })
        );
        let mut semantic_files = vec![("parent.bzl", PARENT)];
        semantic_files.extend(CHILDREN.iter().copied());
        let mut epoch = EpochBuilder::external_sources(&semantic_files, 180 + position as i64);
        epoch.missing(&format!("/workspace/dep/{}", CHILDREN[position].0));
        let mut tx = transaction(&dice, epoch.build(), false, None).await;
        let route = external_route(&mut tx).await;
        let value = tx
            .compute(&observed_external_bzl_key(route.clone(), "", "parent.bzl"))
            .await
            .unwrap();
        assert!(matches!(
            observed_external_error(&value),
            ExternalBzlModuleError::Child { error, .. }
                if matches!(error.as_ref(), ExternalBzlModuleError::Absent { .. })
        ));
        let mut expected = PathObservationEpoch::empty();
        for path in
            std::iter::once("parent.bzl").chain(CHILDREN[..=position].iter().map(|(name, _)| *name))
        {
            let source = tx
                .compute(&HostRepositorySourceFileObservationKey::new(
                    route.clone(),
                    PathBuf::from(path),
                ))
                .await
                .unwrap();
            let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
                panic!("expected positional source carrier");
            };
            expected = super::union_host_observations(&expected, source.observations()).unwrap();
        }
        assert_same_epoch_arcs(observed_external(&value).observations(), &expected);
    }
}
#[tokio::test]
async fn observed_external_bzl_poll_drop_publishes_no_parent_and_recovers() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(&[("root.bzl", b"VALUE = 1\n")], 190).build();
    let mut cancelled = transaction(&dice, epoch.dupe(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cancelled).await;
    tracker.take();
    let key = observed_external_bzl_key(route, "", "root.bzl");
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(
        tracker
            .take()
            .iter()
            .all(|entry| { entry.key != key.to_string() || entry.batch.is_none() })
    );
    drop(cancelled);
    let mut successor = transaction(&dice, epoch, true, Some(tracker.dupe())).await;
    let route = external_route(&mut successor).await;
    let recovered = successor
        .compute(&observed_external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    assert!(observed_external(&recovered).result().is_ok());
    let recovered_batches = tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key == key.to_string() && entry.kind == ActivationKind::Evaluated)
        .collect::<Vec<_>>();
    assert_eq!(recovered_batches.len(), 1);
    assert!(event_texts(recovered_batches[0].batch.as_ref().unwrap()).is_empty());
}
#[test]
fn external_bzl_module_normalizes_exact_same_package_labels_before_source() {
    let root = PackagePath::parse("").unwrap();
    assert_eq!(
        resolve_external_load_label(&root, ":defs.bzl").unwrap(),
        resolve_external_load_label(&root, "//:defs.bzl").unwrap()
    );
    let package = PackagePath::parse("pkg").unwrap();
    assert_eq!(
        resolve_external_load_label(&package, ":defs.bzl").unwrap(),
        resolve_external_load_label(&package, "//pkg:defs.bzl").unwrap()
    );
    for rejected in [
        "@dep//pkg:defs.bzl",
        "@@dep+//pkg:defs.bzl",
        "@//pkg:defs.bzl",
        "@@//pkg:defs.bzl",
        "//other:defs.bzl",
        ":sub/defs.bzl",
        "//pkg:sub/defs.bzl",
        ":../defs.bzl",
        ":defs.star",
        "defs.bzl",
    ] {
        assert!(
            resolve_external_load_label(&package, rejected).is_err(),
            "{rejected:?} entered external key identity"
        );
    }
    let latin1 = RepositoryBzlLabel::new(
        package,
        slug_bzlmod_v2::RootPackageBzlTarget::parse("\u{00ff}.bzl").unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    assert_eq!(
        latin1.repository_relative_path().as_os_str().as_bytes(),
        b"pkg/\xff.bzl"
    );
}
#[tokio::test]
async fn external_bzl_module_full_route_keys_are_unequal_while_canonical_labels_match() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut direct = transaction(
        &dice,
        EpochBuilder::external_sources(&[("defs.bzl", b"VALUE = 1\n")], 39).build(),
        false,
        None,
    )
    .await;
    let direct_route = external_route_named(&mut direct, "dep").await;
    let direct_key = external_bzl_key(direct_route, "", "defs.bzl");
    let mut alias_epoch = EpochBuilder::external_sources(&[("defs.bzl", b"VALUE = 1\n")], 391);
    alias_epoch.file(
        "/workspace/MODULE.bazel",
        "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\", repo_name = \"alias\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
        391,
    );
    let mut alias = transaction(&dice, alias_epoch.build(), false, None).await;
    let alias_route = external_route_named(&mut alias, "alias").await;
    let alias_key = external_bzl_key(alias_route, "", "defs.bzl");
    assert_ne!(direct_key, alias_key);
    assert_eq!(direct_key.canonical_label(), alias_key.canonical_label());
    assert_eq!(
        direct_key.canonical_label().to_string(),
        "@@dep+//:defs.bzl"
    );
}

#[tokio::test]
async fn external_bzl_module_evaluates_recursive_bazel_keyword_only_structs() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":support.bzl\", \"RESULT\")\nCHECKED = RESULT.std and not RESULT.host_tools\nEXPORTED = RESULT\n",
        ),
        (
            "support.bzl",
            b"def _support(*, std = False, host_tools = False):\n    return struct(std = std, host_tools = host_tools)\nRESULT = _support(std = True)\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 392).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    let module = &external_terminal(&outcome).module;
    assert_eq!(module.get("CHECKED").unwrap().unpack_bool(), Some(true));
    let exported_value = module.get("EXPORTED").unwrap();
    let exported = StructRef::from_value(exported_value.value()).unwrap();
    let fields = exported
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.unpack_bool()))
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            ("std".to_owned(), Some(true)),
            ("host_tools".to_owned(), Some(false))
        ]
    );
}

#[tokio::test]
async fn external_bzl_module_accepts_bazel_provider_doc_and_freezes_exports() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":support.bzl\", \"DocumentedInfo\", \"NoneInfo\")\nDOCUMENTED = DocumentedInfo\nNONE = NoneInfo\n",
        ),
        (
            "support.bzl",
            b"DocumentedInfo = provider(\n    doc = \"A documented \" + \"provider.\",\n    fields = {\"value\": \"String value.\"},\n)\nNoneInfo = provider(doc = None, fields = {})\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 393).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    let module = &external_terminal(&outcome).module;
    for (variable, exported_name) in [("DOCUMENTED", "DocumentedInfo"), ("NONE", "NoneInfo")] {
        let exported = module.get(variable).unwrap();
        let callable = FrozenUserProviderCallable::from_value(exported.value()).unwrap();
        assert_eq!(callable.id().source_label(), "@@dep+//:support.bzl");
        assert_eq!(callable.id().exported_name(), exported_name);
    }
}

#[tokio::test]
async fn external_bzl_module_rejects_non_string_provider_doc() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[("root.bzl", b"BadInfo = provider(doc = 1, fields = {})\n")],
            394,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    let ExternalBzlModuleError::Evaluation { message, .. } = external_error(&outcome) else {
        panic!("expected evaluation failure");
    };
    assert!(
        message.contains("provider doc must be a string or None"),
        "{message}"
    );
}

#[tokio::test]
async fn external_bzl_module_retains_canonical_manifest_lifetime_and_local_events() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":left.bzl\", \"LEFT\")\nload(\"//:left.bzl\", \"LEFT_ALIAS\")\nload(\"//:right.bzl\", \"RIGHT\")\nprint(\"ROOT_BZL\")\nRESULT = LEFT + LEFT_ALIAS + RIGHT\n",
        ),
        (
            "left.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"LEFT_BZL\")\nLEFT = H\nLEFT_ALIAS = H\n",
        ),
        (
            "right.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"RIGHT_BZL\")\nRIGHT = H\n",
        ),
        ("helper.bzl", b"print(\"HELPER_BZL\")\nH = 1\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(files, 40).build();
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = external_bzl_key(route, "", "root.bzl");
    let cold_value = cold.compute(&key).await.unwrap();
    let module = external_terminal(&cold_value);
    assert_eq!(module.path, PathBuf::from("/workspace/dep/root.bzl"));
    assert_eq!(module.loads, [":left.bzl", "//:left.bzl", "//:right.bzl"]);
    assert_eq!(
        module
            .manifest
            .direct_children
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:left.bzl", "@@dep+//:right.bzl"]
    );
    assert_eq!(
        module
            .manifest
            .reachable
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        [
            "@@dep+//:root.bzl",
            "@@dep+//:left.bzl",
            "@@dep+//:helper.bzl",
            "@@dep+//:right.bzl",
        ]
    );
    assert_eq!(module.retained_bzl_modules.len(), 3);
    assert!(ExternalBzlModuleEvalKey::validity(&cold_value));
    assert!(ExternalBzlModuleEvalKey::equality(&cold_value, &cold_value));

    let cold_batches = tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key.starts_with("external-bzl-module:"))
        .collect::<Vec<_>>();
    assert_eq!(cold_batches.len(), 6);
    assert_eq!(
        cold_batches
            .iter()
            .filter(|entry| entry.kind == ActivationKind::Evaluated)
            .count(),
        4
    );
    assert_eq!(
        cold_batches
            .iter()
            .filter(|entry| entry.kind == ActivationKind::Reused)
            .count(),
        2
    );
    for entry in &cold_batches {
        if entry.kind == ActivationKind::Evaluated {
            assert_eq!(event_texts(entry.batch.as_ref().unwrap()).len(), 1);
        }
    }

    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let warm_route = external_route(&mut warm).await;
    let warm_key = external_bzl_key(warm_route, "", "root.bzl");
    let warm_value = warm.compute(&warm_key).await.unwrap();
    assert!(ExternalBzlModuleEvalKey::equality(&cold_value, &warm_value));
    let warm_batches = warm_tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key.starts_with("external-bzl-module:"))
        .collect::<Vec<_>>();
    assert_eq!(warm_batches.len(), 1);
    assert!(
        warm_batches
            .iter()
            .all(|entry| entry.kind == ActivationKind::Reused)
    );
    assert!(warm_batches.iter().all(|entry| entry.batch.is_none()));

    let empty_files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":left.bzl\", \"LEFT\")\nRESULT = LEFT\n",
        ),
        ("left.bzl", b"load(\":helper.bzl\", \"H\")\nLEFT = H\n"),
        ("helper.bzl", b"H = 1\n"),
    ];
    let empty_tracker = Arc::new(EventTracker::default());
    let mut empty = transaction(
        &dice,
        EpochBuilder::external_sources(empty_files, 41).build(),
        true,
        Some(empty_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut empty).await;
    external_terminal(
        &empty
            .compute(&external_bzl_key(route, "", "root.bzl"))
            .await
            .unwrap(),
    );
    let empty_root = empty_tracker
        .take()
        .into_iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:root.bzl")
        .unwrap();
    assert_eq!(
        event_texts(empty_root.batch.as_ref().unwrap()),
        Vec::<&str>::new()
    );

    let prefix_tracker = Arc::new(EventTracker::default());
    let mut prefix = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "root.bzl",
                b"print(\"PREFIX\")\nfail(\"terminal\")\nprint(\"AFTER\")\n",
            )],
            42,
        )
        .build(),
        true,
        Some(prefix_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut prefix).await;
    let failed = prefix
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&failed),
        ExternalBzlModuleError::Evaluation { .. }
    ));
    let prefix_root = prefix_tracker
        .take()
        .into_iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:root.bzl")
        .unwrap();
    assert_eq!(event_texts(prefix_root.batch.as_ref().unwrap()), ["PREFIX"]);
}

#[tokio::test]
async fn external_bzl_module_prevalidates_loads_and_preserves_typed_terminal_equality() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut invalid_epoch = EpochBuilder::external_sources(
        &[(
            "root.bzl",
            b"load(\":unobserved.bzl\", \"X\")\nload(\"@other//:bad.bzl\", \"Y\")\n",
        )],
        50,
    );
    // There is deliberately no observation for unobserved.bzl. A Need here
    // would prove that validation happened after child source lookup.
    invalid_epoch.missing("/workspace/dep/BUILD.bazel");
    let mut invalid = transaction(&dice, invalid_epoch.build(), false, None).await;
    let route = external_route(&mut invalid).await;
    let key = external_bzl_key(route, "", "root.bzl");
    let invalid_value = invalid.compute(&key).await.unwrap();
    assert!(matches!(
        external_error(&invalid_value),
        ExternalBzlModuleError::LoadLabel { .. }
    ));
    assert!(ExternalBzlModuleEvalKey::equality(
        &invalid_value,
        &invalid_value
    ));

    let mut missing_epoch = EpochBuilder::external_sources(&[], 51);
    missing_epoch.missing("/workspace/dep/missing.bzl");
    let mut missing = transaction(&dice, missing_epoch.build(), false, None).await;
    let route = external_route(&mut missing).await;
    let missing_key = external_bzl_key(route, "", "missing.bzl");
    let missing_value = missing.compute(&missing_key).await.unwrap();
    assert!(matches!(
        external_error(&missing_value),
        ExternalBzlModuleError::Absent { .. }
    ));

    let mut parse = transaction(
        &dice,
        EpochBuilder::external_sources(&[("bad.bzl", b"VALUE =\n")], 52).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut parse).await;
    let parse_value = parse
        .compute(&external_bzl_key(route, "", "bad.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&parse_value),
        ExternalBzlModuleError::Parse { .. }
    ));

    let mut encoding = transaction(
        &dice,
        EpochBuilder::external_sources(&[("encoding.bzl", &[0xff])], 521).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut encoding).await;
    let encoding_value = encoding
        .compute(&external_bzl_key(route, "", "encoding.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&encoding_value),
        ExternalBzlModuleError::Encoding { .. }
    ));

    let mut child_epoch = EpochBuilder::external_sources(
        &[("parent.bzl", b"load(\":missing_child.bzl\", \"VALUE\")\n")],
        522,
    );
    child_epoch.missing("/workspace/dep/missing_child.bzl");
    let mut child = transaction(&dice, child_epoch.build(), false, None).await;
    let route = external_route(&mut child).await;
    let child_value = child
        .compute(&external_bzl_key(route, "", "parent.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&child_value),
        ExternalBzlModuleError::Child { .. }
    ));

    let mut evaluation = transaction(
        &dice,
        EpochBuilder::external_sources(&[("fail.bzl", b"fail(\"boom\")\n")], 53).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut evaluation).await;
    let eval_key = external_bzl_key(route, "", "fail.bzl");
    let eval_value = evaluation.compute(&eval_key).await.unwrap();
    assert!(matches!(
        external_error(&eval_value),
        ExternalBzlModuleError::Evaluation { .. }
    ));

    // Every value exposed by loading_globals implements Freeze. Retain a
    // typed equality discriminator for the future error path without adding
    // a non-freezable evaluator value solely for this dormant owner.
    let freeze_a = ExternalBzlModuleError::Freeze {
        label: eval_key.canonical_label(),
        message: Arc::from("freeze discriminator"),
    };
    assert_eq!(freeze_a, freeze_a.clone());

    let mut need = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let need_value = need.compute(&eval_key).await.unwrap();
    assert!(matches!(need_value, LoadingPreparationOutcome::Need(_)));
    assert!(!ExternalBzlModuleEvalKey::validity(&need_value));
    assert!(!ExternalBzlModuleEvalKey::equality(
        &need_value,
        &need_value
    ));
}

#[tokio::test]
async fn external_bzl_module_cycle_releases_and_recovers_with_fresh_detector() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let self_epoch = EpochBuilder::external_sources(
        &[("self.bzl", b"load(\":self.bzl\", \"VALUE\")\nVALUE = 1\n")],
        59,
    )
    .build();
    let mut self_cycle = transaction(&dice, self_epoch, false, None).await;
    let route = external_route(&mut self_cycle).await;
    let self_key = external_bzl_key(route, "", "self.bzl");
    let self_value = tokio::time::timeout(Duration::from_secs(5), self_cycle.compute(&self_key))
        .await
        .expect("external self-cycle detector must release recursive DICE wait")
        .unwrap();
    let self_detected = external_error(&self_value).cycle().unwrap();
    assert!(self_detected.path.is_empty());
    assert_eq!(
        self_detected
            .keys
            .iter()
            .map(ExternalBzlCycleIdentity::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:self.bzl"]
    );

    let cycle_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"load(\":one.bzl\", \"ONE\")\nTWO = ONE\n"),
        ],
        60,
    )
    .build();
    let mut cycle = transaction(&dice, cycle_epoch, false, None).await;
    let route = external_route(&mut cycle).await;
    let key = external_bzl_key(route, "", "entry.bzl");
    let cycle_value = tokio::time::timeout(Duration::from_secs(5), cycle.compute(&key))
        .await
        .expect("external cycle detector must release recursive DICE wait")
        .unwrap();
    let detected = external_error(&cycle_value).cycle().unwrap();
    assert_eq!(
        detected
            .path
            .iter()
            .map(ExternalBzlCycleIdentity::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:entry.bzl"]
    );
    assert_eq!(
        detected
            .keys
            .iter()
            .map(ExternalBzlCycleIdentity::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:one.bzl", "@@dep+//:two.bzl"]
    );

    let fixed_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"TWO = 2\n"),
        ],
        61,
    )
    .build();
    // transaction() installs a fresh request-scoped detector while retaining
    // the same DICE engine and all non-cycle dependencies.
    let mut fixed = transaction(&dice, fixed_epoch, false, None).await;
    let route = external_route(&mut fixed).await;
    let fixed_value = fixed
        .compute(&external_bzl_key(route, "", "entry.bzl"))
        .await
        .unwrap();
    assert_eq!(external_terminal(&fixed_value).manifest.reachable.len(), 3);
}

#[tokio::test]
async fn observed_external_bzl_cycle_retains_all_sources_and_recovers() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let self_epoch = EpochBuilder::external_sources(
        &[("self.bzl", b"load(\":self.bzl\", \"VALUE\")\nVALUE = 1\n")],
        159,
    )
    .build();
    let mut self_cycle = transaction(&dice, self_epoch, false, None).await;
    let self_route = external_route(&mut self_cycle).await;
    let self_value = tokio::time::timeout(
        Duration::from_secs(5),
        self_cycle.compute(&observed_external_bzl_key(
            self_route.clone(),
            "",
            "self.bzl",
        )),
    )
    .await
    .expect("observed external self-cycle must release")
    .unwrap();
    let self_detected = observed_external_error(&self_value).cycle().unwrap();
    assert!(self_detected.path.is_empty());
    assert_eq!(
        self_detected.keys[0].canonical_label().to_string(),
        "@@dep+//:self.bzl"
    );
    let self_source = self_cycle
        .compute(&HostRepositorySourceFileObservationKey::new(
            self_route,
            PathBuf::from("self.bzl"),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(self_source)) = self_source else {
        panic!("expected self-cycle source carrier");
    };
    assert_same_epoch_arcs(
        observed_external(&self_value).observations(),
        self_source.observations(),
    );
    let cycle_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"load(\":one.bzl\", \"ONE\")\nTWO = ONE\n"),
        ],
        160,
    )
    .build();
    let tracker = Arc::new(EventTracker::default());
    let mut cycle = transaction(&dice, cycle_epoch, true, Some(tracker.dupe())).await;
    let route = external_route(&mut cycle).await;
    tracker.take();
    let key = observed_external_bzl_key(route.clone(), "", "entry.bzl");
    let value = tokio::time::timeout(Duration::from_secs(5), cycle.compute(&key))
        .await
        .expect("observed external cycle detector must release recursive DICE wait")
        .unwrap();
    let detected = observed_external_error(&value).cycle().unwrap();
    assert_eq!(
        detected
            .path
            .iter()
            .map(ExternalBzlCycleIdentity::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:entry.bzl"]
    );
    assert_eq!(
        detected
            .keys
            .iter()
            .map(ExternalBzlCycleIdentity::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:one.bzl", "@@dep+//:two.bzl"]
    );
    let mut expected = PathObservationEpoch::empty();
    for path in ["entry.bzl", "one.bzl", "two.bzl"] {
        let source = cycle
            .compute(&HostRepositorySourceFileObservationKey::new(
                route.clone(),
                PathBuf::from(path),
            ))
            .await
            .unwrap();
        let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
            panic!("expected cycle source carrier");
        };
        expected = super::union_host_observations(&expected, source.observations()).unwrap();
    }
    assert_same_epoch_arcs(observed_external(&value).observations(), &expected);
    let cycle_batches = tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key.starts_with("observed-external-bzl-module:"))
        .collect::<Vec<_>>();
    assert!(
        cycle_batches
            .iter()
            .all(|entry| { matches!(entry.batch, Some(ref batch) if batch.events().is_empty()) })
    );

    let fixed_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"TWO = 2\n"),
        ],
        161,
    )
    .build();
    let mut fixed = transaction(&dice, fixed_epoch, false, None).await;
    let route = external_route(&mut fixed).await;
    let fixed_value = fixed
        .compute(&observed_external_bzl_key(route, "", "entry.bzl"))
        .await
        .unwrap();
    assert_eq!(
        observed_external(&fixed_value)
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .manifest
            .reachable
            .len(),
        3
    );
}

#[tokio::test]
async fn repository_package_load_activates_external_macro_manifest_lifetime_and_local_events() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"make_filegroup\")\nprint(\"BUILD_TOP\")\nmake_filegroup(name = \"macro_files\")\n",
        ),
        (
            "defs.bzl",
            b"print(\"DEFS_TOP\")\ndef make_filegroup(name):\n    print(\"MACRO_BODY\")\n    native.filegroup(name = name)\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::external_sources(files, 70);
    epoch.file("/workspace/dep/REPO.bazel", b"print(\"REPO_TOP\")\n", 70);
    let epoch = epoch.build();
    let tracker = Arc::new(EventTracker::default());
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = RepositoryPackageLoadKey::new(route, PackagePath::parse("").unwrap());
    let cold_value = cold.compute(&key).await.unwrap();
    let loaded = repository_package_terminal(&cold_value);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["macro_files"]
    );
    assert_eq!(
        loaded
            .direct_load_roots
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:defs.bzl"]
    );
    assert_eq!(
        loaded
            .reachable_loads
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:defs.bzl"]
    );
    assert_eq!(loaded.retained_bzl_module_count(), 1);
    assert_ne!(loaded.load_fingerprint, [0; 32]);
    assert!(RepositoryPackageLoadKey::validity(&cold_value));
    assert!(RepositoryPackageLoadKey::equality(&cold_value, &cold_value));
    let batches = tracker.take();
    let bzl = batches
        .iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:defs.bzl")
        .unwrap();
    assert_eq!(bzl.kind, ActivationKind::Evaluated);
    assert_eq!(event_texts(bzl.batch.as_ref().unwrap()), ["DEFS_TOP"]);
    let package = batches
        .iter()
        .find(|entry| entry.key.starts_with("repository-package-load:"))
        .unwrap();
    assert_eq!(package.kind, ActivationKind::Evaluated);
    assert_eq!(
        event_texts(package.batch.as_ref().unwrap()),
        ["BUILD_TOP", "MACRO_BODY"]
    );
    let selected_source = batches
        .iter()
        .find(|entry| entry.key.starts_with("repository-package-source:"))
        .unwrap();
    assert_eq!(selected_source.kind, ActivationKind::Evaluated);
    assert!(selected_source.batch.is_none());
    let route_policy = batches
        .iter()
        .find(|entry| entry.key.starts_with("host-route-repo-file:"))
        .unwrap();
    assert_eq!(route_policy.kind, ActivationKind::Evaluated);
    assert_eq!(
        event_texts(route_policy.batch.as_ref().unwrap()),
        ["REPO_TOP"]
    );
    let uncaptured_dice = Dice::builder().build(DetectCycles::Enabled);
    let uncaptured_tracker = Arc::new(EventTracker::default());
    let mut uncaptured = transaction(
        &uncaptured_dice,
        epoch.clone(),
        false,
        Some(uncaptured_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut uncaptured).await;
    repository_package_terminal(
        &uncaptured
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap(),
    );
    let uncaptured_batches = uncaptured_tracker.take();
    assert!(uncaptured_batches.iter().all(|entry| entry.batch.is_none()));
    assert!(
        uncaptured_batches
            .iter()
            .any(|entry| entry.key.starts_with("host-route-repo-file:"))
    );
    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let route = external_route(&mut warm).await;
    let warm_value = warm
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(RepositoryPackageLoadKey::equality(&cold_value, &warm_value));
    let warm_batches = warm_tracker.take();
    assert!(
        warm_batches
            .iter()
            .all(|entry| { entry.kind == ActivationKind::Reused && entry.batch.is_none() })
    );
}
#[tokio::test]
async fn observed_repository_package_load_retains_source_child_arcs_and_local_batch() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"VALUE\")\nprint(\"BUILD_OBSERVED\")\nfilegroup(name = \"files\")\n",
        ),
        ("defs.bzl", b"print(\"BZL_OBSERVED\")\nVALUE = 1\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let mut epoch = EpochBuilder::external_sources(files, 145);
    epoch.file("/workspace/dep/REPO.bazel", b"", 145);
    let mut tx = transaction(&dice, epoch.build(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut tx).await;
    tracker.take();
    let package = PackagePath::parse("").unwrap();
    let observed_key = RepositoryPackageLoadObservationKey::new(route.clone(), package.clone());
    let legacy_key = RepositoryPackageLoadKey::new(route.clone(), package.clone());
    let mut observed_tx = tx.dupe();
    let (value, legacy) = tokio::join!(observed_tx.compute(&observed_key), tx.compute(&legacy_key));
    let value = value.unwrap();
    repository_package_terminal(&legacy.unwrap());
    let carrier = observed_repository_package(&value);
    assert_eq!(
        carrier
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["files"]
    );
    assert!(RepositoryPackageLoadObservationKey::validity(&value));
    let source = tx
        .compute(
            &RepositoryPackageSourceObservationKey::new(
                route.clone(),
                PackageIdentifier::new(route.canonical_repo().clone(), package),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
        panic!("expected observed package source");
    };
    let child = tx
        .compute(&observed_external_bzl_key(route.clone(), "", "defs.bzl"))
        .await
        .unwrap();
    let expected = super::union_host_observations(
        source.observations(),
        observed_external(&child).observations(),
    )
    .unwrap();
    assert_same_epoch_arcs(carrier.observations(), &expected);
    let batches = tracker.take();
    assert_eq!(
        batches
            .iter()
            .filter(|entry| entry.kind == ActivationKind::Evaluated)
            .filter(|entry| entry.key.starts_with("observed-"))
            .filter_map(|entry| entry.batch.as_ref())
            .map(event_texts)
            .filter(|events| !events.is_empty())
            .collect::<Vec<_>>(),
        [vec!["BZL_OBSERVED"], vec!["BUILD_OBSERVED"]]
    );
    let dependencies = tracker.take_package_dependencies();
    assert_eq!(dependencies.len(), 2);
    let families = dependencies
        .iter()
        .map(|row| {
            let observed = row[0].starts_with("observed-");
            assert!(
                row.iter()
                    .all(|key| key.starts_with("observed-") == observed)
            );
            observed
        })
        .collect::<Vec<_>>();
    assert!(families.contains(&false) && families.contains(&true));
}
#[tokio::test]
async fn observed_repository_package_load_source_and_child_terminals_keep_prefixes() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let package = PackagePath::parse("").unwrap();
    let tracker = Arc::new(EventTracker::default());
    let mut need = transaction(
        &dice,
        EpochBuilder::external_sources(&[], 146).build(),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let route = external_route(&mut need).await;
    let value = need
        .compute(&RepositoryPackageLoadObservationKey::new(
            route,
            package.clone(),
        ))
        .await
        .unwrap();
    assert!(matches!(value, LoadingPreparationOutcome::Need(_)));
    assert!(!RepositoryPackageLoadObservationKey::validity(&value));
    assert!(!RepositoryPackageLoadObservationKey::equality(
        &value, &value
    ));
    assert!(tracker.take().iter().all(|entry| {
        !entry.key.starts_with("observed-external-bzl-module:")
            && (!entry.key.starts_with("observed-repository-package-load:")
                || entry.batch.is_none())
    }));
    let mut missing_source_epoch = EpochBuilder::external_sources(&[], 147);
    missing_source_epoch.missing("/workspace/dep/BUILD.bazel");
    missing_source_epoch.missing("/workspace/dep/BUILD");
    let mut missing_source = transaction(
        &dice,
        missing_source_epoch.build(),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let route = external_route(&mut missing_source).await;
    let value = missing_source
        .compute(&RepositoryPackageLoadObservationKey::new(
            route.clone(),
            package.clone(),
        ))
        .await
        .unwrap();
    observed_repository_package_error(&value);
    let source = missing_source
        .compute(
            &RepositoryPackageSourceObservationKey::new(
                route.clone(),
                PackageIdentifier::new(route.canonical_repo().clone(), package.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
        panic!("expected semantic package source");
    };
    assert_same_epoch_arcs(
        observed_repository_package(&value).observations(),
        source.observations(),
    );
    assert!(tracker.take().iter().any(|entry| {
        entry.key.starts_with("observed-repository-package-load:")
            && entry
                .batch
                .as_ref()
                .is_some_and(|batch| batch.events().is_empty())
    }));
    const BUILD: &[u8] = b"load(\":first.bzl\", \"FIRST\")\nload(\":middle.bzl\", \"MIDDLE\")\nload(\":last.bzl\", \"LAST\")\nfilegroup(name = \"files\")\n";
    const CHILD: &[u8] = b"FIRST = 1\nMIDDLE = 2\nLAST = 3\n";
    let children = ["first.bzl", "middle.bzl", "last.bzl"];
    for (case, semantic) in (0..3).flat_map(|position| [(position, false), (position, true)]) {
        let mut files = vec![("BUILD.bazel", BUILD)];
        files.extend(
            children
                .iter()
                .enumerate()
                .filter(|(position, _)| *position != case)
                .map(|(_, name)| (*name, CHILD)),
        );
        let mut epoch =
            EpochBuilder::external_sources(&files, 150 + case as i64 * 2 + semantic as i64);
        if semantic {
            epoch.missing(&format!("/workspace/dep/{}", children[case]));
        }
        let mut tx = transaction(&dice, epoch.build(), true, Some(tracker.dupe())).await;
        let route = external_route(&mut tx).await;
        let value = tx
            .compute(&RepositoryPackageLoadObservationKey::new(
                route.clone(),
                package.clone(),
            ))
            .await
            .unwrap();
        if semantic {
            assert!(matches!(
                observed_repository_package_error(&value).inner,
                RepositoryPackageLoadErrorInner::Bzl { .. }
            ));
            assert!(RepositoryPackageLoadObservationKey::validity(&value));
            assert!(RepositoryPackageLoadObservationKey::equality(
                &value, &value
            ));
        } else {
            assert!(matches!(value, LoadingPreparationOutcome::Need(_)));
        }
        let batches = tracker.take();
        for (position, name) in children.iter().enumerate() {
            assert_eq!(
                batches.iter().any(|entry| {
                    entry.key == format!("observed-external-bzl-module:@@dep+//:{name}")
                }),
                position <= case
            );
        }
        assert!(
            batches
                .iter()
                .filter(|entry| {
                    entry.key.starts_with("observed-repository-package-load:")
                        && entry
                            .batch
                            .as_ref()
                            .is_some_and(|batch| batch.events().is_empty())
                })
                .count()
                == semantic as usize
        );
        if semantic {
            let source = tx
                .compute(
                    &RepositoryPackageSourceObservationKey::new(
                        route.clone(),
                        PackageIdentifier::new(route.canonical_repo().clone(), package.clone()),
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
                panic!("expected observed source");
            };
            let mut expected = source.observations().dupe();
            for name in &children[..=case] {
                let child = tx
                    .compute(&observed_external_bzl_key(route.clone(), "", name))
                    .await
                    .unwrap();
                expected = super::union_host_observations(
                    &expected,
                    observed_external(&child).observations(),
                )
                .unwrap();
            }
            assert_same_epoch_arcs(
                observed_repository_package(&value).observations(),
                &expected,
            );
        }
    }
}
#[tokio::test]
async fn observed_repository_package_load_keeps_terminal_prefixes_and_error_batches() {
    let cases: &[(&str, &[u8])] = &[
        ("encoding", b"\xff"),
        ("parse", b"["),
        ("load-label", b"load(\"@other//:bad.bzl\", \"X\")\n"),
        ("evaluation", b"print(\"BEFORE\")\nfail(\"boom\")\n"),
        (
            "glob",
            b"filegroup(name = \"x\", srcs = glob([\"*.txt\"]))\n",
        ),
        (
            "postvalidation",
            b"load(\":defs.bzl\", \"make_alias\")\nmake_alias(name = \"blocked\")\n",
        ),
    ];
    for (variant, (name, build)) in cases.iter().enumerate() {
        let mut files = vec![("BUILD.bazel", *build)];
        if *name == "postvalidation" {
            files.push((
                "defs.bzl",
                b"def make_alias(name):\n    native.alias(name = name, actual = \":missing\")\n",
            ));
        }
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(EventTracker::default());
        let mut tx = transaction(
            &dice,
            EpochBuilder::external_sources(&files, 170 + variant as i64).build(),
            true,
            Some(tracker.dupe()),
        )
        .await;
        let route = external_route(&mut tx).await;
        tracker.take();
        let package = PackagePath::parse("").unwrap();
        let value = tx
            .compute(&RepositoryPackageLoadObservationKey::new(
                route.clone(),
                package.clone(),
            ))
            .await
            .unwrap();
        let error = observed_repository_package_error(&value);
        assert!(match (*name, &error.inner) {
            ("encoding", RepositoryPackageLoadErrorInner::Encoding { .. })
            | ("parse", RepositoryPackageLoadErrorInner::Parse { .. })
            | ("load-label", RepositoryPackageLoadErrorInner::LoadLabel { .. })
            | ("evaluation", RepositoryPackageLoadErrorInner::Attempt(_))
            | ("glob", RepositoryPackageLoadErrorInner::GlobUnsupported { .. })
            | ("postvalidation", RepositoryPackageLoadErrorInner::LoadedTargetKind { .. }) => true,
            _ => false,
        });
        let source = tx
            .compute(
                &RepositoryPackageSourceObservationKey::new(
                    route.clone(),
                    PackageIdentifier::new(route.canonical_repo().clone(), package),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
            panic!("expected source carrier");
        };
        let expected = if *name == "postvalidation" {
            let child = tx
                .compute(&observed_external_bzl_key(route, "", "defs.bzl"))
                .await
                .unwrap();
            super::union_host_observations(
                source.observations(),
                observed_external(&child).observations(),
            )
            .unwrap()
        } else {
            source.observations().dupe()
        };
        assert_same_epoch_arcs(
            observed_repository_package(&value).observations(),
            &expected,
        );
        let package_batch = tracker
            .take()
            .into_iter()
            .find(|entry| entry.key.starts_with("observed-repository-package-load:"))
            .unwrap();
        assert_eq!(
            event_texts(package_batch.batch.as_ref().unwrap()),
            if *name == "evaluation" {
                vec!["BEFORE"]
            } else {
                vec![]
            }
        );
    }
}
#[tokio::test]
async fn observed_repository_package_load_replays_a_b_a_and_recovers_after_cancel() {
    const BUILD_A: &[u8] =
        b"load(\":defs.bzl\", \"NAME\")\nprint(\"PACKAGE_A\")\nfilegroup(name = NAME)\n";
    const BUILD_B: &[u8] =
        b"load(\":defs.bzl\", \"NAME\")\nprint(\"PACKAGE_B\")\nfilegroup(name = NAME)\n";
    const A: &[u8] = b"NAME = \"a\"\n";
    const B: &[u8] = b"NAME = \"b\"\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch_a =
        EpochBuilder::external_sources(&[("BUILD.bazel", BUILD_A), ("defs.bzl", A)], 160).build();
    let mut cold = transaction(&dice, epoch_a.dupe(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    tracker.take();
    let key = RepositoryPackageLoadObservationKey::new(route, PackagePath::parse("").unwrap());
    let cold_value = cold.compute(&key).await.unwrap();
    let cold_result = observed_repository_package(&cold_value).result();
    let cold_loaded = cold_result.as_ref().as_ref().unwrap();
    assert_eq!(cold_loaded.targets[0].name, "a");
    assert_eq!(
        tracker
            .take()
            .iter()
            .filter(|entry| entry.key == key.to_string())
            .map(|entry| event_texts(entry.batch.as_ref().unwrap()))
            .collect::<Vec<_>>(),
        [vec!["PACKAGE_A"]]
    );
    let mut warm = transaction(&dice, epoch_a.dupe(), true, Some(tracker.dupe())).await;
    let warm_value = warm.compute(&key).await.unwrap();
    assert!(RepositoryPackageLoadObservationKey::equality(
        &cold_value,
        &warm_value
    ));
    assert!(tracker.take().iter().all(|entry| {
        !entry.key.starts_with("observed-repository-package-load:") || entry.batch.is_none()
    }));
    for (variant, build, defs, expected) in [
        (161, Some(BUILD_B), Some(B), Some("b")),
        (162, Some(BUILD_B), None, None),
        (163, Some(BUILD_B), Some(B), Some("b")),
        (164, None, None, None),
        (165, Some(BUILD_B), Some(B), Some("b")),
        (160, Some(BUILD_A), Some(A), Some("a")),
    ] {
        let files = build
            .map(|source| ("BUILD.bazel", source))
            .into_iter()
            .chain(defs.map(|source| ("defs.bzl", source)))
            .collect::<Vec<_>>();
        let mut epoch = EpochBuilder::external_sources(&files, variant);
        if build.is_none() {
            epoch.missing("/workspace/dep/BUILD.bazel");
            epoch.missing("/workspace/dep/BUILD");
        } else if defs.is_none() {
            epoch.missing("/workspace/dep/defs.bzl");
        }
        let mut tx = transaction(&dice, epoch.build(), false, None).await;
        let route = external_route(&mut tx).await;
        let value = tx
            .compute(&RepositoryPackageLoadObservationKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap();
        match expected {
            Some(name) => assert_eq!(
                observed_repository_package(&value)
                    .result()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .targets[0]
                    .name,
                name
            ),
            None => assert!(observed_repository_package(&value).result().is_err()),
        }
        if variant == 160 {
            assert!(RepositoryPackageLoadObservationKey::equality(
                &cold_value,
                &value
            ));
        }
    }
    assert_eq!(
        cold_loaded.direct_load_roots[0].label.to_string(),
        "@@dep+//:defs.bzl"
    );
    assert_eq!(cold_loaded.retained_bzl_module_count(), 1);
    let cancel_dice = Dice::builder().build(DetectCycles::Enabled);
    let mut cancelled = transaction(&cancel_dice, epoch_a.dupe(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cancelled).await;
    tracker.take();
    let key = RepositoryPackageLoadObservationKey::new(route, PackagePath::parse("").unwrap());
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(
        tracker
            .take()
            .iter()
            .all(|entry| entry.key != key.to_string() || entry.batch.is_none())
    );
    drop(cancelled);
    let mut recovered = transaction(&cancel_dice, epoch_a, true, Some(tracker.dupe())).await;
    let route = external_route(&mut recovered).await;
    recovered
        .compute(&RepositoryPackageLoadObservationKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(
        tracker
            .take()
            .iter()
            .filter(|entry| {
                entry.key.starts_with("observed-repository-package-load:")
                    && entry.kind == ActivationKind::Evaluated
            })
            .count(),
        1
    );
}
#[tokio::test]
async fn repository_package_load_preserves_selected_source_error_display_and_chain() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::external_sources(&[], 79);
    epoch.missing("/workspace/dep/BUILD.bazel");
    epoch.missing("/workspace/dep/BUILD");
    let mut transaction = transaction(&dice, epoch.build(), false, None).await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    let error = repository_package_typed_error(&outcome);
    let expected = "no such package '@@dep+//': BUILD file not found in directory '' of external repository @@dep+. Add a BUILD file to a directory to mark it as a package.";
    assert_eq!(error.to_string(), expected);
    assert_eq!(
        std::error::Error::source(error).unwrap().to_string(),
        expected
    );
    assert!(RepositoryPackageLoadKey::validity(&outcome));
}

#[tokio::test]
async fn repository_package_load_accepts_only_dependency_free_public_starlark_rule_shape() {
    let valid: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"probe\")\nprint(\"RULE_BUILD\")\nprobe(name = \"probe\", empty = [], visibility = [\"//visibility:public\"])\n",
        ),
        (
            "defs.bzl",
            b"print(\"RULE_BZL\")\ndef _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"empty\": attr.label_list()})\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(valid, 80).build();
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = RepositoryPackageLoadKey::new(route, PackagePath::parse("").unwrap());
    let cold_value = cold.compute(&key).await.unwrap();
    let loaded = repository_package_terminal(&cold_value);
    assert_eq!(loaded.targets.len(), 1);
    assert!(matches!(
        loaded.targets[0].kind,
        crate::PackageTargetKind::StarlarkRule(_)
    ));
    let capability = loaded.targets[0].rule_capability().unwrap();
    assert_eq!(capability.rule_class, "probe");
    assert!(!capability.executable);
    assert!(capability.test_kind.is_none());
    assert_eq!(loaded.retained_bzl_module_count(), 1);
    let batches = tracker.take();
    assert_eq!(
        event_texts(
            batches
                .iter()
                .find(|entry| entry.key == "external-bzl-module:@@dep+//:defs.bzl")
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["RULE_BZL"]
    );
    assert_eq!(
        event_texts(
            batches
                .iter()
                .find(|entry| entry.key.starts_with("repository-package-load:"))
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["RULE_BUILD"]
    );
    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let route = external_route(&mut warm).await;
    repository_package_terminal(
        &warm
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap(),
    );
    assert!(
        warm_tracker
            .take()
            .iter()
            .all(|entry| entry.kind == ActivationKind::Reused && entry.batch.is_none())
    );

    let cases = [
        (
            "probe(name = \"bad\")",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", visibility = [\"//visibility:private\"])",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", visibility = [\"//pkg:__pkg__\"])",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", dep = \":dep.txt\", visibility = [\"//visibility:public\"])",
            "ordinary dependencies are deferred",
        ),
        (
            "probe(name = \"bad\", out = \"out.txt\", visibility = [\"//visibility:public\"])",
            "attribute `out` contains a reachable label",
        ),
        (
            "exports_files([\"extra.txt\"])\nprobe(name = \"bad\", visibility = [\"//visibility:public\"])",
            "package contains 2 targets",
        ),
    ];
    for (index, (invocation, expected)) in cases.into_iter().enumerate() {
        let build = format!("load(\":defs.bzl\", \"probe\")\n{invocation}\n");
        let defs = b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"dep\": attr.label(), \"out\": attr.output()})\n";
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(
                &[("BUILD.bazel", build.as_bytes()), ("defs.bzl", defs)],
                81 + index as i64,
            )
            .build(),
            false,
            None,
        )
        .await;
        let route = external_route(&mut transaction).await;
        let outcome = transaction
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap();
        assert!(
            repository_package_error(&outcome).contains(expected),
            "{expected}"
        );
    }
    for (source, symbol, expected) in [
        (
            b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe_test = rule(implementation = _impl, test = True)\n".as_slice(),
            "probe_test",
            "test rules are deferred",
        ),
        (
            b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, executable = True)\n".as_slice(),
            "probe",
            "executable rules are deferred",
        ),
    ] {
        let build = format!(
            "load(\":defs.bzl\", \"{symbol}\")\n{symbol}(name = \"bad\", visibility = [\"//visibility:public\"])\n"
        );
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(
                &[("BUILD.bazel", build.as_bytes()), ("defs.bzl", source)],
                90,
            )
            .build(),
            false,
            None,
        )
        .await;
        let route = external_route(&mut transaction).await;
        let outcome = transaction
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap();
        let error = repository_package_error(&outcome);
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[tokio::test]
async fn repository_package_load_prevalidates_all_loads_and_gates_loaded_target_kinds() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let mut invalid = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "BUILD.bazel",
                b"load(\":unobserved.bzl\", \"FIRST\")\nload(\"@other//:bad.bzl\", \"SECOND\")\n",
            )],
            71,
        )
        .build(),
        false,
        Some(tracker.dupe()),
    )
    .await;
    let route = external_route(&mut invalid).await;
    let invalid_key = RepositoryPackageLoadKey::new(route.clone(), PackagePath::parse("").unwrap());
    let invalid_value = invalid.compute(&invalid_key).await.unwrap();
    assert!(
        repository_package_error(&invalid_value)
            .contains("repository-qualified external load is deferred: @other//:bad.bzl")
    );
    assert!(
        tracker
            .take()
            .iter()
            .all(|entry| !entry.key.starts_with("external-bzl-module:"))
    );
    assert!(RepositoryPackageLoadKey::validity(&invalid_value));
    assert!(RepositoryPackageLoadKey::equality(
        &invalid_value,
        &invalid_value
    ));

    let sequential_tracker = Arc::new(EventTracker::default());
    let mut sequential_need = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[
                (
                    "BUILD.bazel",
                    b"load(\":first.bzl\", \"FIRST\")\nload(\":second.bzl\", \"SECOND\")\n",
                ),
                ("second.bzl", b"SECOND = 2\n"),
            ],
            711,
        )
        .build(),
        false,
        Some(sequential_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut sequential_need).await;
    let sequential_need = sequential_need
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        sequential_need,
        LoadingPreparationOutcome::Need(_)
    ));
    assert!(
        sequential_tracker
            .take()
            .iter()
            .all(|entry| entry.key != "external-bzl-module:@@dep+//:second.bzl")
    );

    let mut need = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let need_value = need.compute(&invalid_key).await.unwrap();
    assert!(matches!(need_value, LoadingPreparationOutcome::Need(_)));
    assert!(!RepositoryPackageLoadKey::validity(&need_value));
    assert!(!RepositoryPackageLoadKey::equality(
        &need_value,
        &need_value
    ));

    let mut rejected = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[
                (
                    "BUILD.bazel",
                    b"load(\":defs.bzl\", \"make_alias\")\nmake_alias(name = \"blocked\")\n",
                ),
                (
                    "defs.bzl",
                    b"def make_alias(name):\n    native.alias(name = name, actual = \":missing\")\n",
                ),
            ],
            72,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut rejected).await;
    let rejected = rejected
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(
        repository_package_error(&rejected)
            .contains("produced unsupported target `blocked` of kind alias")
    );

    let mut native = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[
                (
                    "BUILD.bazel",
                    b"load(\":defs.bzl\", \"IGNORED\")\ntoolchain_type(name = \"type\")\n",
                ),
                ("defs.bzl", b"IGNORED = True\n"),
            ],
            721,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut native).await;
    let native = native
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    let native = repository_package_terminal(&native);
    assert_eq!(
        native
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["type"]
    );
    assert!(native.native_attributes("type").is_some());

    let mut unloaded = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "BUILD.bazel",
                b"alias(name = \"accepted\", actual = \":missing\")\n",
            )],
            73,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut unloaded).await;
    let unloaded = unloaded
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(
        repository_package_terminal(&unloaded).targets[0].name,
        "accepted"
    );
}

#[tokio::test]
async fn repository_package_load_renders_missing_and_cycle_and_recovers_on_same_dice() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut missing_epoch = EpochBuilder::external_sources(
        &[(
            "BUILD.bazel",
            b"load(\":missing.bzl\", \"make\")\nmake(name = \"x\")\n",
        )],
        74,
    );
    missing_epoch.missing("/workspace/dep/missing.bzl");
    let mut missing = transaction(&dice, missing_epoch.build(), false, None).await;
    let route = external_route(&mut missing).await;
    let missing = missing
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(
        repository_package_error(&missing),
        "cannot load '@@dep+//:missing.bzl': no such file"
    );

    let cycle_files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"make\")\nmake(name = \"x\")\n",
        ),
        (
            "defs.bzl",
            b"load(\":helper.bzl\", \"HELPER\")\ndef make(name):\n    native.filegroup(name = name)\n",
        ),
        (
            "helper.bzl",
            b"load(\":defs.bzl\", \"make\")\nHELPER = make\n",
        ),
    ];
    let mut cycle = transaction(
        &dice,
        EpochBuilder::external_sources(cycle_files, 75).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut cycle).await;
    let cycle = tokio::time::timeout(
        Duration::from_secs(5),
        cycle.compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        )),
    )
    .await
    .expect("external BUILD cycle detector must release the recursive DICE wait")
    .unwrap();
    let rendered = repository_package_error(&cycle);
    assert!(rendered.starts_with("cycle detected in extension files:"));
    assert!(rendered.contains("@@dep+///BUILD.bazel"));
    assert!(rendered.contains(".-> @@dep+//:defs.bzl"));
    assert!(rendered.contains("|   @@dep+//:helper.bzl"));
    assert!(rendered.contains("`-- @@dep+//:defs.bzl"));

    let fixed_files: &[(&str, &[u8])] = &[
        cycle_files[0],
        cycle_files[1],
        ("helper.bzl", b"HELPER = 1\n"),
    ];
    let mut fixed = transaction(
        &dice,
        EpochBuilder::external_sources(fixed_files, 76).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut fixed).await;
    let fixed = fixed
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(repository_package_terminal(&fixed).targets[0].name, "x");
}
