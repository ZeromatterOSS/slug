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
use sha2::Digest;
use sha2::Sha256;
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
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::FrozenListRef;
use starlark::values::structs::StructRef;
use starlark_map::small_map::SmallMap;

use super::BzlLoadManifest;
use super::BzlModuleIdentity;
use super::ExternalBzlCycleIdentity;
use super::ExternalBzlModuleError;
use super::ExternalBzlModuleEvalKey;
use super::ExternalBzlModuleObservationKey;
use super::ForceRootPackageObservationOuter;
use super::HostPackageLoadMode;
use super::LocalBzlLoader;
use super::ObservedRootPackageLoad;
use super::RepositoryBzlLabel;
use super::RepositoryPackageLoadError;
use super::RepositoryPackageLoadErrorInner;
use super::RepositoryPackageLoadKey;
use super::RepositoryPackageLoadObservationKey;
use super::RootPackageDirectLoad;
use super::RootPackageLoadObservationKey;
use super::build_file_loading_globals;
use super::loading_globals;
use super::merge_root_package_observations;
use super::resolve_external_load_label;
use super::resolve_host_load_label;
use super::resolve_root_package_direct_load;
use crate::AllowSingleFile;
use crate::AttributeKind;
use crate::CoercedAttributeValue;
use crate::LoadingPreparationOutcome;
use crate::RootPackageLoadKey;
use crate::TestRuleKind;
use crate::attrs::AllowedAttributeValues;
use crate::cycle_detector::bzl_load_cycle_detector;
use crate::package::BuildSettingKind;
use crate::package::FrozenAspectDefinition;
use crate::package::FrozenRuleDefinition;
use crate::provider::BzlEvaluationContext;
use crate::provider::FrozenUserProviderCallable;
use crate::provider::OutputGroupInfo;
use crate::provider::RunEnvironmentInfo;
use crate::provider::StarlarkUserProvider;
use crate::provider::loading_provider_id;
use crate::starlark_label::StarlarkLabel;

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
                Some(b"module(name='dep', version='1', repo_name='rules_rust')\n")
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
        "module(name='dep', version='1', repo_name='rules_rust')\n",
        901,
    );
    epoch.materialized_missing(instance, "/registry-dep/REPO.bazel");
    epoch.materialized_missing(instance, "/registry-dep/.bazelignore");
    epoch.materialized_file(
        instance,
        "/registry-dep/defs.bzl",
        "load(':nested.bzl', 'NESTED_NAME', 'current_rust_analyzer_toolchain', 'lint_rule', 'rust_analyzer_detect_sysroot')\nprint('SELECTED_BZL')\nSELECTED_NAME=NESTED_NAME\nCURRENT_RULE=current_rust_analyzer_toolchain\nDETECT_RULE=rust_analyzer_detect_sysroot\nLINT_RULE=lint_rule\n",
        901,
    );
    epoch.materialized_file(
        instance,
        "/registry-dep/nested.bzl",
        "print('SELECTED_NESTED')\ndef _current_rust_analyzer_toolchain_impl(ctx): fail('current implementation must stay lazy')\ncurrent_rust_analyzer_toolchain = rule(doc = 'current', implementation = _current_rust_analyzer_toolchain_impl, toolchains = [str(Label('@rules_rust//rust/rust_analyzer:toolchain_type'))])\ndef _rust_analyzer_detect_sysroot_impl(ctx): fail('detect implementation must stay lazy')\nrust_analyzer_detect_sysroot = rule(doc = 'detect', implementation = _rust_analyzer_detect_sysroot_impl, toolchains = ['@rules_rust//rust:toolchain_type', '@rules_rust//rust/rust_analyzer:toolchain_type'])\ndef _lint_impl(ctx): fail('lint implementation must stay lazy')\nlint_rule = rule(implementation = _lint_impl, attrs = {'runner': attr.label(default = Label('//rust/private/lint_test_runner'), cfg = 'exec', executable = True)})\nNESTED_NAME='selected_target'\n",
        901,
    );
    (epoch.build(), instance)
}

async fn assert_selected_rust_analyzer_rules(
    transaction: &mut DiceTransaction,
    route: &RootRepositoryRoute,
) {
    assert!(
        route
            .bzl_repository_mapping()
            .iter()
            .any(|(apparent, canonical)| apparent.as_str() == "rules_rust"
                && canonical.as_str() == "dep+")
    );
    let loaded = transaction
        .compute(&observed_external_bzl_key(route.clone(), "", "defs.bzl"))
        .await
        .unwrap();
    let module = &observed_external(&loaded)
        .result()
        .as_ref()
        .as_ref()
        .unwrap()
        .module;
    let current = module
        .get("CURRENT_RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_eq!(
        current.required_toolchains(),
        [CanonicalLabel::parse("@@dep+//rust/rust_analyzer:toolchain_type").unwrap()]
    );
    let detect = module
        .get("DETECT_RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_eq!(
        detect.required_toolchains(),
        [
            CanonicalLabel::parse("@@dep+//rust:toolchain_type").unwrap(),
            CanonicalLabel::parse("@@dep+//rust/rust_analyzer:toolchain_type").unwrap(),
        ]
    );
    let lint = module
        .get("LINT_RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let runner = lint
        .schema
        .iter()
        .find(|schema| schema.name == "runner")
        .unwrap();
    assert!(runner.executable && runner.exec_configuration);
    assert!(matches!(
        runner.default.as_ref(),
        Some(CoercedAttributeValue::Label(label))
            if label.to_string() == "@@dep+//rust/private/lint_test_runner:lint_test_runner"
    ));
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
    assert_selected_rust_analyzer_rules(&mut transaction, route).await;
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

async fn load_repository_package_fixture(
    files: &[(&str, &[u8])],
    variant: i64,
) -> RepositoryPackageOutcome {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, variant).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut transaction).await;
    transaction
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap()
}

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
async fn external_bzl_module_accepts_bazel_rule_doc_and_freezes_exports() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":support.bzl\", \"documented_rule\", \"none_rule\")\nDOCUMENTED = documented_rule\nNONE = none_rule\n",
        ),
        (
            "support.bzl",
            b"def _impl(ctx): return []\ndocumented_rule = rule(implementation = _impl, doc = \"A documented rule.\", build_setting = config.string(flag = True))\nnone_rule = rule(implementation = _impl, doc = None)\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 395).build(),
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
    for variable in ["DOCUMENTED", "NONE"] {
        assert_eq!(module.get(variable).unwrap().value().get_type(), "rule");
    }
}

#[tokio::test]
async fn external_bzl_module_rejects_non_string_rule_doc() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "root.bzl",
                b"def _impl(ctx): return []\nbad_rule = rule(implementation = _impl, doc = 1)\n",
            )],
            396,
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
        message.contains("rule doc must be a string or None"),
        "{message}"
    );
}

#[tokio::test]
async fn external_bzl_module_freezes_typed_bazel_config_definitions() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":support.bzl\", \"bool_flag\", \"bool_setting\", \"bool_false\", \"list_rule\", \"repeatable_rule\", \"list_setting\", \"list_false\")\nBOOL_FLAG = bool_flag\nBOOL_SETTING = bool_setting\nBOOL_FALSE = bool_false\n",
        ),
        (
            "support.bzl",
            b"def _impl(ctx): fail('build setting implementations must stay lazy')\nbool_flag = rule(implementation = _impl, build_setting = config.bool(flag = True))\nbool_setting = rule(implementation = _impl, build_setting = config.bool())\nbool_false = rule(implementation = _impl, build_setting = config.bool(flag = False))\nlist_rule = rule(implementation = _impl, build_setting = config.string_list(flag = True))\nrepeatable_rule = rule(implementation = _impl, build_setting = config.string_list(flag = True, repeatable = True))\nlist_setting = rule(implementation = _impl, build_setting = config.string_list())\nlist_false = rule(implementation = _impl, build_setting = config.string_list(flag = False, repeatable = False))\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 397).build(),
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
    let bool_kind = |name| {
        module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap()
            .build_setting_kind
    };
    assert_eq!(
        bool_kind("BOOL_FLAG"),
        Some(BuildSettingKind::Boolean { flag: true })
    );
    assert_eq!(
        bool_kind("BOOL_SETTING"),
        Some(BuildSettingKind::Boolean { flag: false })
    );
    assert_eq!(bool_kind("BOOL_SETTING"), bool_kind("BOOL_FALSE"));
    assert_ne!(bool_kind("BOOL_FLAG"), bool_kind("BOOL_SETTING"));
    for export in ["BOOL_FLAG", "BOOL_SETTING", "BOOL_FALSE"] {
        let bool_rule = module
            .get(export)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let default = bool_rule
            .schema
            .iter()
            .find(|schema| schema.name == "build_setting_default")
            .unwrap();
        assert_eq!(default.kind, AttributeKind::Boolean);
        assert!(default.mandatory);
        assert!(!default.configurable);
        let help = bool_rule
            .schema
            .iter()
            .find(|schema| schema.name == "help")
            .unwrap();
        assert!(!help.mandatory);
        assert!(!help.configurable);
    }
    let list_kind = |name| {
        module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap()
            .build_setting_kind
    };
    assert_eq!(
        list_kind("list_rule"),
        Some(BuildSettingKind::StringList {
            flag: true,
            repeatable: false,
        })
    );
    assert_eq!(
        list_kind("repeatable_rule"),
        Some(BuildSettingKind::StringList {
            flag: true,
            repeatable: true,
        })
    );
    assert_eq!(
        list_kind("list_setting"),
        Some(BuildSettingKind::StringList {
            flag: false,
            repeatable: false,
        })
    );
    assert_eq!(list_kind("list_setting"), list_kind("list_false"));
    for export in ["list_rule", "repeatable_rule", "list_setting", "list_false"] {
        let list_rule = module
            .get(export)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        assert_eq!(list_rule.capability().rule_class, export);
        let default = list_rule
            .schema
            .iter()
            .find(|schema| schema.name == "build_setting_default")
            .unwrap();
        assert!(
            default.kind == AttributeKind::StringList && default.mandatory && !default.configurable
        );
        let help = list_rule
            .schema
            .iter()
            .find(|schema| schema.name == "help")
            .unwrap();
        assert!(!help.mandatory && !help.configurable);
    }
}

#[tokio::test]
async fn external_bzl_module_freezes_config_string_definitions() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(':support.bzl', 'string_flag', 'string_setting', 'string_false', 'string_multiple', 'setting_multiple')\nFLAG = string_flag\nSETTING = string_setting\nFALSE = string_false\nMULTIPLE = string_multiple\nSETTING_MULTIPLE = setting_multiple\n",
        ),
        (
            "support.bzl",
            b"def _impl(ctx): fail('string implementation must stay lazy')\nstring_flag=rule(implementation=_impl, build_setting=config.string(flag=True))\nstring_setting=rule(implementation=_impl, build_setting=config.string())\nstring_false=rule(implementation=_impl, build_setting=config.string(flag=False, allow_multiple=False))\nstring_multiple=rule(implementation=_impl, build_setting=config.string(flag=True, allow_multiple=True))\nsetting_multiple=rule(implementation=_impl, build_setting=config.string(allow_multiple=True))\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 3971).build(),
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
    for (export, rule_class, flag, allow_multiple) in [
        ("FLAG", "string_flag", true, false),
        ("SETTING", "string_setting", false, false),
        ("FALSE", "string_false", false, false),
        ("MULTIPLE", "string_multiple", true, true),
        ("SETTING_MULTIPLE", "setting_multiple", false, true),
    ] {
        let rule = module
            .get(export)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        assert_eq!(rule.capability().rule_class, rule_class);
        assert_eq!(
            rule.build_setting_kind,
            Some(BuildSettingKind::String {
                flag,
                allow_multiple,
            })
        );
        let default = rule
            .schema
            .iter()
            .find(|schema| schema.name == "build_setting_default")
            .unwrap();
        assert!(
            default.kind == AttributeKind::String && default.mandatory && !default.configurable
        );
        let help = rule
            .schema
            .iter()
            .find(|schema| schema.name == "help")
            .unwrap();
        assert!(!help.mandatory && !help.configurable);
    }
    let kind = |name| {
        module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap()
            .build_setting_kind
    };
    assert_eq!(kind("SETTING"), kind("FALSE"));
}

#[tokio::test]
async fn external_bzl_module_freezes_config_int_definitions() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(':support.bzl', 'int_flag', 'int_setting', 'int_setting_explicit_false')\nFLAG=int_flag\nSETTING=int_setting\nEXPLICIT_FALSE=int_setting_explicit_false\n",
        ),
        (
            "support.bzl",
            b"def _impl(ctx): fail('integer implementation must stay lazy')\nint_flag=rule(implementation=_impl, build_setting=config.int(flag=True))\nint_setting=rule(implementation=_impl, build_setting=config.int())\nint_setting_explicit_false=rule(implementation=_impl, build_setting=config.int(flag=False))\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 3971).build(),
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
    for (alias, rule_class, flag) in [
        ("FLAG", "int_flag", true),
        ("SETTING", "int_setting", false),
        ("EXPLICIT_FALSE", "int_setting_explicit_false", false),
    ] {
        let rule = module
            .get(alias)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        assert_eq!(rule.capability().rule_class, rule_class);
        assert_eq!(
            rule.build_setting_kind,
            Some(BuildSettingKind::Integer { flag })
        );
        let default = rule
            .schema
            .iter()
            .find(|schema| schema.name == "build_setting_default")
            .unwrap();
        assert_eq!(default.kind, AttributeKind::Integer);
        assert!(default.mandatory && !default.configurable);
        let help = rule
            .schema
            .iter()
            .find(|schema| schema.name == "help")
            .unwrap();
        assert_eq!(help.kind, AttributeKind::String);
        assert!(!help.mandatory && !help.configurable);
    }
}

#[tokio::test]
async fn external_bzl_module_freezes_and_imports_fixed_aspect_definition() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":support.bzl\", \"rust_analyzer_aspect\", \"UNEXPORTED\")\nIMPORTED = rust_analyzer_aspect\nNESTED = UNEXPORTED\n",
        ),
        (
            "support.bzl",
            b"def _impl(target, ctx): return []\nrust_analyzer_aspect = aspect(attr_aspects = [\"srcs\", \"deps\", \"proc_macro_deps\", \"crate\", \"actual\", \"proto\"], implementation = _impl, toolchains = [\"//rust:toolchain_type\"], doc = \"Rust analyzer\")\nUNEXPORTED = [aspect(implementation = _impl)]\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 400).build(),
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
    let imported = module.get("IMPORTED").unwrap();
    let aspect = imported.downcast::<FrozenAspectDefinition>().unwrap();
    assert_eq!(
        aspect.attr_aspects.join(","),
        "srcs,deps,proc_macro_deps,crate,actual,proto"
    );
    assert!(aspect.attributes.is_empty());
    assert!(aspect.required_aspect.is_none());
    assert!(aspect.advertised_providers.is_empty());
    assert_mandatory_aspect_toolchain(&aspect, "@@dep+//rust:toolchain_type");
    assert_eq!(
        aspect.defining_label,
        CanonicalLabel::parse("@@dep+//:support.bzl").unwrap()
    );
    assert_eq!(
        aspect.exported_name.as_deref().unwrap(),
        "rust_analyzer_aspect"
    );
    let nested = FrozenListRef::from_value(module.get("NESTED").unwrap().value()).unwrap();
    let nested = nested[0].downcast_ref::<FrozenAspectDefinition>().unwrap();
    assert!(nested.exported_name.is_none());
    assert!(nested.required_toolchains.is_empty());
}

fn assert_mandatory_aspect_toolchain(aspect: &FrozenAspectDefinition, label: &str) {
    let [requirement] = aspect.required_toolchains.as_ref() else {
        panic!("expected one aspect toolchain requirement");
    };
    assert_eq!(requirement.label().to_string(), label);
    assert!(requirement.mandatory());
}

fn assert_frozen_rustfmt_aspect(aspect: &FrozenAspectDefinition) {
    assert_eq!(aspect.attributes.len(), 2);
    let config = &aspect.attributes[0];
    assert_eq!(config.name, "_config");
    assert_eq!(config.kind, AttributeKind::Label);
    assert!(matches!(
        config.allow_single_file,
        Some(AllowSingleFile::True)
    ));
    assert!(
        matches!(config.default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@dep+//rust/settings:rustfmt.toml")
    );
    assert!(!config.executable && !config.exec_configuration);
    let process_wrapper = &aspect.attributes[1];
    assert_eq!(process_wrapper.name, "_process_wrapper");
    assert_eq!(process_wrapper.kind, AttributeKind::Label);
    assert!(process_wrapper.allow_single_file.is_none());
    assert!(
        matches!(process_wrapper.default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@dep+//util/process_wrapper:process_wrapper")
    );
    assert!(process_wrapper.executable && process_wrapper.exec_configuration);
    assert_eq!(aspect.required_providers.len(), 2);
    assert_eq!(
        aspect.required_providers[0][0].to_string(),
        "@@dep+//rust/private:providers.bzl%CrateInfo"
    );
    assert_eq!(
        aspect.required_providers[1][0].to_string(),
        "@@dep+//rust/private:providers.bzl%TestCrateInfo"
    );
    assert!(aspect.advertised_providers.is_empty());
    assert_eq!(aspect.required_fragments.as_ref(), ["cpp"]);
    assert_eq!(
        aspect.defining_label,
        CanonicalLabel::parse("@@dep+//rust/private:rustfmt.bzl").unwrap()
    );
    assert_eq!(aspect.exported_name.as_deref(), Some("rustfmt_aspect"));
    assert_mandatory_aspect_toolchain(aspect, "@@dep+//rust/rustfmt:toolchain_type");
    let required = aspect
        .required_aspect
        .unwrap()
        .downcast_ref::<FrozenAspectDefinition>()
        .unwrap();
    assert_eq!(
        required.defining_label,
        CanonicalLabel::parse("@@dep+//rust/private:rustfmt.bzl").unwrap()
    );
    assert_eq!(
        required.exported_name.as_deref(),
        Some("rustfmt_srcs_aspect")
    );
    assert_eq!(
        required.required_providers[0][0].to_string(),
        "@@dep+//rust/private:providers.bzl%CrateInfo"
    );
}

const LINT_TEST_SOURCE: &str = r###""""Shared helpers for `rust_clippy_test` and `rustfmt_test`.

Both rules follow the same shape: a thin wrapper aspect that walks
`deps`/`proc_macro_deps`/`crate` and collects the output-group markers
produced by the underlying real aspect (`rust_clippy_aspect` /
`rustfmt_aspect`), plus a rule impl that symlinks a shared runner binary
and hands it the collected marker rlocationpaths via `RUST_LINT_TEST_MARKERS`.

The pieces exposed here — `rlocationpath`, `platform_transition`,
`LINT_TEST_COMMON_ATTRS`, `lint_test_aspect_impl`, `lint_test_rule_impl` —
let each rule file supply only what actually differs (the provider type
and the output-group names it collects).
"""

def rlocationpath(file, workspace_name):
    """Compute the runfile rlocationpath for a `File`.

    Args:
        file (File): The file to compute the rlocationpath for.
        workspace_name (str): The name of the current workspace.

    Returns:
        str: The rlocationpath the runner should look up for `file`.
    """
    if file.short_path.startswith("../"):
        return file.short_path[len("../"):]
    return "{}/{}".format(workspace_name, file.short_path)

def _platform_transition_impl(_settings, attr):
    if not attr.platform:
        return {}
    platform = str(attr.platform)
    if not platform.startswith("@"):
        platform = "@" + platform
    return {"//command_line_option:platforms": platform}

platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

# Attrs every lint-test rule needs alongside its own `targets`. Callers
# merge this dict into their `attrs = {...}`.
LINT_TEST_COMMON_ATTRS = {
    "platform": attr.label(
        doc = "Optional platform to transition `targets` to before running the aspect. When set, `--platforms` is switched to this label for the duration of this rule's aspect actions.",
    ),
    "transitive": attr.bool(
        doc = "If True, lint `targets` and every crate reachable via `deps`, `proc_macro_deps`, and `crate`. If False, lint only the exact targets listed.",
        default = False,
    ),
    "_allowlist_function_transition": attr.label(
        default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
    ),
    "_runner": attr.label(
        doc = "The shared runner (prints/inspects collected marker paths).",
        cfg = "exec",
        executable = True,
        default = Label("//rust/private/lint_test_runner"),
    ),
}

def lint_test_aspect_impl(target, ctx, info_provider, output_group_names):
    """Thin collector: walk deps and roll up the markers the underlying aspect produced.

    Args:
        target (Target): The target the aspect is running on.
        ctx (ctx): The aspect's context object.
        info_provider (provider): The provider type to read from deps and return
            (e.g. `RustClippyTestInfo` or `RustfmtTestInfo`).
        output_group_names (list): A `list` of `str` naming the `OutputGroupInfo`
            fields to collect from the current target (e.g.
            `["clippy_checks", "clippy_output"]` or `["rustfmt_checks"]`).

    Returns:
        list: A single-element list containing an `info_provider` with `direct`
            (`depset[File]`) for `target` and `checks` (`depset[File]`) that
            folds in every dep's `checks`.
    """
    direct_depsets = []
    if OutputGroupInfo in target:
        og = target[OutputGroupInfo]
        for name in output_group_names:
            if hasattr(og, name):
                direct_depsets.append(getattr(og, name))
    direct = depset(transitive = direct_depsets)

    transitive = [direct]
    for attr_name in ("deps", "proc_macro_deps"):
        for dep in getattr(ctx.rule.attr, attr_name, []):
            if info_provider in dep:
                transitive.append(dep[info_provider].checks)
    crate_dep = getattr(ctx.rule.attr, "crate", None)
    if crate_dep and info_provider in crate_dep:
        transitive.append(crate_dep[info_provider].checks)

    return [info_provider(
        direct = direct,
        checks = depset(transitive = transitive),
    )]

def lint_test_rule_impl(ctx, info_provider, output_group_names):
    """Symlink the shared runner and hand it the collected marker rlocationpaths.

    Args:
        ctx (ctx): The rule's context object.
        info_provider (provider): The provider type produced by the rule's
            aspect, carrying `direct` and `checks` depsets.
        output_group_names (list): A `list` of `str` naming the
            `OutputGroupInfo` fields to expose the collected markers under
            (e.g. `["clippy_checks", "clippy_output"]` or
            `["rustfmt_checks"]`). Each name maps to the same `checks` depset.

    Returns:
        list: `[DefaultInfo, RunEnvironmentInfo, OutputGroupInfo]` for the
            test target.
    """
    is_windows = ctx.executable._runner.extension == ".exe"
    runner = ctx.actions.declare_file("{}{}".format(
        ctx.label.name,
        ".exe" if is_windows else "",
    ))
    ctx.actions.symlink(
        output = runner,
        target_file = ctx.executable._runner,
        is_executable = True,
    )

    check_depsets = []
    for target in ctx.attr.targets:
        if info_provider not in target:
            continue
        info = target[info_provider]
        check_depsets.append(info.checks if ctx.attr.transitive else info.direct)
    checks = depset(transitive = check_depsets)

    runfiles = ctx.runfiles(transitive_files = checks).merge(
        ctx.attr._runner[DefaultInfo].default_runfiles,
    )

    workspace_name = ctx.workspace_name
    markers_env = ctx.configuration.host_path_separator.join([
        rlocationpath(f, workspace_name)
        for f in checks.to_list()
    ])

    return [
        DefaultInfo(
            files = depset([runner]),
            runfiles = runfiles,
            executable = runner,
        ),
        RunEnvironmentInfo(environment = {
            "RUST_BACKTRACE": "1",
            "RUST_LINT_TEST_MARKERS": markers_env,
        }),
        OutputGroupInfo(**{name: checks for name in output_group_names}),
    ]
"###;

const CLIPPY_ASPECT_SOURCE: &str = r#"
ClippyInfo = provider(doc = "clippy", fields = {"output": "output"})
CrateInfo = provider()
TestCrateInfo = provider()
rust_common = struct(crate_info = CrateInfo, test_crate_info = TestCrateInfo)
def _clippy_aspect_impl(target, ctx):
    fail("clippy implementation must stay lazy")
rust_clippy_aspect = aspect(
    implementation = _clippy_aspect_impl,
    attrs = {
        "_capture_output": attr.label(doc = "capture", default = Label("//rust/settings:capture_clippy_output")),
        "_clippy_error_format": attr.label(doc = "clippy format", default = "//rust/settings:clippy_error_format"),
        "_clippy_flag": attr.label(doc = "flag", default = Label("//rust/settings:clippy_flag")), "_clippy_flags": attr.label(doc = "flags", default = Label("//rust/settings:clippy_flags")),
        "_clippy_output_diagnostics": attr.label(doc = "diagnostics", default = "//rust/settings:clippy_output_diagnostics"),
        "_config": attr.label(doc = "config", allow_single_file = True, default = Label("//rust/settings:clippy.toml")),
        "_error_format": attr.label(doc = "rustc format", default = "//rust/settings:error_format"), "_extra_rustc_flag": attr.label(default = Label("//rust/settings:extra_rustc_flag")),
        "_incompatible_change_clippy_error_format": attr.label(doc = "incompatible", default = "//rust/settings:incompatible_change_clippy_error_format"),
        "_per_crate_rustc_flag": attr.label(default = Label("//rust/settings:per_crate_rustc_flag")),
        "_process_wrapper": attr.label(doc = "wrapper", default = Label("//util/process_wrapper"), executable = True, cfg = "exec"),
    },
    provides = [ClippyInfo],
    required_providers = [[rust_common.crate_info], [rust_common.test_crate_info]],
    fragments = ["cpp"],
    toolchains = TOOLCHAINS,
    doc = "Executes the clippy checker on specified targets.",
)
def _rust_clippy_rule_impl(ctx):
    clippy_ready_targets = [dep for dep in ctx.attr.deps if "clippy_checks" in dir(dep[OutputGroupInfo])]
    files = depset([], transitive = [dep[OutputGroupInfo].clippy_checks for dep in clippy_ready_targets])
    return [DefaultInfo(files = files)]

rust_clippy = rule(
    implementation = _rust_clippy_rule_impl,
    attrs = {
        "deps": attr.label_list(
            doc = "Rust targets to run clippy on.",
            providers = [
                [rust_common.crate_info],
                [rust_common.test_crate_info],
            ],
            aspects = [rust_clippy_aspect],
        ),
    },
    doc = """\
Executes the clippy checker on a specific target.

Similar to `rust_clippy_aspect`, but allows specifying a list of dependencies \
within the build system.

For example, given the following example targets:

```python
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

rust_library(
    name = "hello_lib",
    srcs = ["src/lib.rs"],
)

rust_test(
    name = "greeting_test",
    srcs = ["tests/greeting.rs"],
    deps = [":hello_lib"],
)
```

Rust clippy can be set as a build target with the following:

```python
load("@rules_rust//rust:defs.bzl", "rust_clippy")

rust_clippy(
    name = "hello_library_clippy",
    testonly = True,
    deps = [
        ":hello_lib",
        ":greeting_test",
    ],
)
```
""",
)
"#;

const CLIPPY_TEST_TAIL_SOURCE: &str = r###"RustClippyTestInfo = provider(
    doc = "Clippy check outputs collected by `rust_clippy_test` from the underlying `rust_clippy_aspect`.",
    fields = {
        "checks": "depset[File]: Clippy markers for the visited target plus every crate reached via `deps`, `proc_macro_deps`, and `crate`.",
        "direct": "depset[File]: Clippy markers for the visited target only.",
    },
)

# clippy contributes to two output groups: `clippy_checks` (`.clippy.ok`
# marker in the default config, `.clippy.out` when `capture_clippy_output`
# is on) and `clippy_output` (`.clippy.diagnostics` JSON when
# `clippy_output_diagnostics` is on). Capture modes make clippy exit 0 even
# on real issues, so the runner inspects file contents to decide pass/fail.
_CLIPPY_OUTPUT_GROUPS = ["clippy_checks", "clippy_output"]

def _rust_clippy_test_aspect_impl(target, ctx):
    return lint_test_aspect_impl(target, ctx, RustClippyTestInfo, _CLIPPY_OUTPUT_GROUPS)

def _rust_clippy_test_impl(ctx):
    return lint_test_rule_impl(ctx, RustClippyTestInfo, _CLIPPY_OUTPUT_GROUPS)

_rust_clippy_test_aspect = aspect(
    implementation = _rust_clippy_test_aspect_impl,
    attr_aspects = ["deps", "proc_macro_deps", "crate"],
    requires = [rust_clippy_aspect],
    provides = [RustClippyTestInfo],
    doc = "Walks `deps`/`proc_macro_deps`/`crate` and rolls up the markers produced by `rust_clippy_aspect` into a transitive `RustClippyTestInfo`.",
)

rust_clippy_test = rule(
    implementation = _rust_clippy_test_impl,
    attrs = dict(LINT_TEST_COMMON_ATTRS, **{
        "targets": attr.label_list(
            doc = "Rust targets to run clippy on.",
            providers = [
                [rust_common.crate_info],
                [rust_common.test_crate_info],
            ],
            aspects = [_rust_clippy_test_aspect],
            cfg = platform_transition,
        ),
    }),
    test = True,
    doc = """\
A test rule that runs `clippy` over a set of Rust targets.

By default (`transitive = False`), only the exact targets listed are checked. Set
`transitive = True` to walk `deps`, `proc_macro_deps`, and `crate` so that listing a
top-level target checks its whole crate graph.

The clippy actions run during the build phase, so a clippy failure fails `bazel test` before
the test executable is invoked. The rule also exposes the collected markers under the
`clippy_checks` output group, so `bazel build //x:my_clippy_test --output_groups=clippy_checks`
drives the clippy actions without running the test.

When `capture_clippy_output` or `clippy_output_diagnostics` is set globally clippy exits 0
even on real issues; in that case the runner inspects the captured stderr / JSON diagnostics
and reports the verdict.

An optional `platform` attribute transitions `targets` to the given platform before running
clippy.

Example:

```python
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_clippy_test", "rust_library")

rust_library(
    name = "lib",
    srcs = ["src/lib.rs"],
    edition = "2021",
)

rust_binary(
    name = "app",
    srcs = ["src/main.rs"],
    edition = "2021",
    deps = [":lib"],
)

rust_clippy_test(
    name = "clippy_app_only_test",
    targets = [":app"],
)

rust_clippy_test(
    name = "clippy_tree_test",
    targets = [":app"],
    transitive = True,
)
```

Targets tagged `no_clippy`, `no_lint`, `nolint`, or `noclippy` are skipped.
""",
)

def _capture_clippy_output_impl(ctx):
    """Implementation of the `capture_clippy_output` rule

    Args:
        ctx (ctx): The rule's context object

    Returns:
        list: A list containing the CaptureClippyOutputInfo provider
    """
    return [CaptureClippyOutputInfo(capture_output = ctx.build_setting_value)]

capture_clippy_output = rule(
    doc = "Control whether to print clippy output or store it to a file, using the configured error_format.",
    implementation = _capture_clippy_output_impl,
    build_setting = config.bool(flag = True),
)

def _clippy_output_diagnostics_impl(ctx):
    """Implementation of the `clippy_output_diagnostics` rule

    Args:
        ctx (ctx): The rule's context object

    Returns:
        list: A list containing the CaptureClippyOutputInfo provider
    """
    return [ClippyOutputDiagnosticsInfo(output_diagnostics = ctx.build_setting_value)]

clippy_output_diagnostics = rule(
    doc = (
        "Setting this flag from the command line with `--@rules_rust//rust/settings:clippy_output_diagnostics` " +
        "makes rules_rust save lippy json output (suitable for consumption by rust-analyzer) in a file, " +
        "available from the `clippy_output` output group. This is the clippy equivalent of " +
        "`@rules_rust//settings:rustc_output_diagnostics`."
    ),
    implementation = _clippy_output_diagnostics_impl,
    build_setting = config.bool(flag = True),
)
"###;

const LINTS_SOURCE: &str = r###""""Rules for defining lints to apply to various Rust targets"""

load("//rust/private:providers.bzl", "LintsInfo")

def _rust_lint_config(ctx):
    """Implementation of the `rust_lint_config` rule.

    Args:
        ctx (ctx): The rule's context object.

    Returns:
        list: The LintsInfo provider.
    """

    allowed_levels = ["allow", "warn", "deny", "forbid", "force-warn"]

    rustc_flags = []
    for lint, level in ctx.attr.rustc.items():
        if level not in allowed_levels:
            fail("Invalid rustc lint level '{0}'".format(level))
        rustc_flags.append("--{LEVEL}={LINT}".format(LEVEL = level, LINT = lint))
    for name, values in ctx.attr.rustc_check_cfg.items():
        if len(values) != 0:
            values_list = ", ".join(["\"{0}\"".format(v) for v in values])
            values_arg = ", values({0})".format(values_list)
        else:
            values_arg = ""
        rustc_flags.append("--check-cfg=cfg({NAME}{VALUES})".format(NAME = name, VALUES = values_arg))

    clippy_flags = []
    for lint, level in ctx.attr.clippy.items():
        if level not in allowed_levels:
            fail("Invalid clippy lint level '{0}'".format(level))
        clippy_flags.append("--{LEVEL}=clippy::{LINT}".format(LEVEL = level, LINT = lint))

    rustdoc_flags = []
    for lint, level in ctx.attr.rustdoc.items():
        if level not in allowed_levels:
            fail("Invalid rustdoc lint level '{0}'".format(level))
        rustdoc_flags.append("--{LEVEL}=rustdoc::{LINT}".format(LEVEL = level, LINT = lint))

    return LintsInfo(
        rustc_lint_flags = rustc_flags,
        rustc_lint_files = [],
        clippy_lint_flags = clippy_flags,
        clippy_lint_files = [],
        rustdoc_lint_flags = rustdoc_flags,
        rustdoc_lint_files = [],
    )

# buildifier: disable=unsorted-dict-items
rust_lint_config = rule(
    implementation = _rust_lint_config,
    attrs = {
        "rustc": attr.string_dict(
            doc = "Set of 'rustc' lints to 'allow', 'expect', 'warn', 'force-warn', 'deny', or 'forbid'.",
        ),
        "rustc_check_cfg": attr.string_list_dict(
            doc = "Set of 'cfg' names and list of values to expect.",
        ),
        "clippy": attr.string_dict(
            doc = "Set of 'clippy' lints to 'allow', 'expect', 'warn', 'force-warn', 'deny', or 'forbid'.",
        ),
        "rustdoc": attr.string_dict(
            doc = "Set of 'rustdoc' lints to 'allow', 'expect', 'warn', 'force-warn', 'deny', or 'forbid'.",
        ),
    },
    doc = """\
Defines a group of lints that can be applied when building Rust targets.

For example, you can define a single group of lints:

```python
load("@rules_rust//rust:defs.bzl", "rust_lint_config")

rust_lint_config(
    name = "workspace_lints",
    rustc = {
        "unknown_lints": "allow",
        "unexpected_cfgs": "warn",
    },
    rustc_check_cfg = {
        "bazel": [],
        "fuzzing": [],
        "mz_featutres": ["laser", "rocket"],
    },
    clippy = {
        "box_default": "allow",
        "todo": "warn",
        "unused_async": "warn",
    },
    rustdoc = {
        "unportable_markdown": "allow",
    },
)
```
""",
)
"###;

const FIND_CC_TOOLCHAIN_SOURCE: &str = r###"# pylint: disable=g-bad-file-header
# Copyright 2016 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""
Helpers for CC Toolchains.

Rules that require a CC toolchain should call `use_cc_toolchain` and `find_cc_toolchain`
to depend on and find a cc toolchain.

* When https://github.com/bazelbuild/bazel/issues/7260 is **not** flipped, current
  C++ toolchain is selected using the legacy mechanism (`--crosstool_top`,
  `--cpu`, `--compiler`). For that to work the rule needs to declare an
  `_cc_toolchain` attribute, e.g.

    foo = rule(
        implementation = _foo_impl,
        attrs = {
            "_cc_toolchain": attr.label(
                default = Label(
                    "@rules_cc//cc:current_cc_toolchain",
                ),
            ),
        },
    )

* When https://github.com/bazelbuild/bazel/issues/7260 **is** flipped, current
  C++ toolchain is selected using the toolchain resolution mechanism
  (`--platforms`). For that to work the rule needs to declare a dependency on
  C++ toolchain type:

    load(":find_cc_toolchain/bzl", "use_cc_toolchain")

    foo = rule(
        implementation = _foo_impl,
        toolchains = use_cc_toolchain(),
    )

We advise to depend on both `_cc_toolchain` attr and on the toolchain type for
the duration of the migration. After
https://github.com/bazelbuild/bazel/issues/7260 is flipped (and support for old
Bazel version is not needed), it's enough to only keep the toolchain type.
"""

load("//cc/common:cc_common.bzl", "cc_common")

CC_TOOLCHAIN_TYPE = Label("@bazel_tools//tools/cpp:toolchain_type")

CC_TOOLCHAIN_ATTRS = {
    # Needed for Bazel 6.x and 7.x compatibility.
    "_cc_toolchain": attr.label(default = Label("@rules_cc//cc:current_cc_toolchain")),
}

def find_cc_toolchain(ctx, *, mandatory = True):
    """
    Returns the current `CcToolchainInfo`.

    Args:
      ctx: The rule context for which to find a toolchain.
      mandatory: (bool) If this is set to False, this function will return None
        rather than fail if no toolchain is found.

    Returns:
      A CcToolchainInfo or None if the c++ toolchain is declared as
      optional, mandatory is False and no toolchain has been found.
    """

    # Check the incompatible flag for toolchain resolution.
    if hasattr(cc_common, "is_cc_toolchain_resolution_enabled_do_not_use") and cc_common.is_cc_toolchain_resolution_enabled_do_not_use(ctx = ctx):
        if not CC_TOOLCHAIN_TYPE in ctx.toolchains:
            fail("In order to use find_cc_toolchain, your rule has to depend on C++ toolchain. See find_cc_toolchain.bzl docs for details.")
        toolchain_info = ctx.toolchains[CC_TOOLCHAIN_TYPE]
        if toolchain_info == None:
            if not mandatory:
                return None

            # No cpp toolchain was found, so report an error.
            fail("Unable to find a CC toolchain using toolchain resolution. Target: %s, Platform: %s, Exec platform: %s" %
                 (ctx.label, ctx.fragments.platform.platform, ctx.fragments.platform.host_platform))
        if hasattr(toolchain_info, "cc_provider_in_toolchain") and hasattr(toolchain_info, "cc"):
            return toolchain_info.cc
        return toolchain_info

    # Fall back to the legacy implicit attribute lookup.
    if hasattr(ctx.attr, "_cc_toolchain"):
        return ctx.attr._cc_toolchain[cc_common.CcToolchainInfo]

    # We didn't find anything.
    if not mandatory:
        return None
    fail("In order to use find_cc_toolchain, your rule has to depend on C++ toolchain. See find_cc_toolchain.bzl docs for details.")

def find_cpp_toolchain(ctx):
    """Deprecated, use `find_cc_toolchain` instead.

    Args:
      ctx: See `find_cc_toolchain`.

    Returns:
      A CcToolchainInfo.
    """
    return find_cc_toolchain(ctx)

def use_cc_toolchain(mandatory = True):
    """
    Helper to depend on the cc toolchain.

    Usage:
    ```
    my_rule = rule(
        toolchains = [other toolchain types] + use_cc_toolchain(),
    )
    ```

    Args:
      mandatory: Whether or not it should be an error if the toolchain cannot be resolved.

    Returns:
      A list that can be used as the value for `rule.toolchains`.
    """
    return [config_common.toolchain_type(CC_TOOLCHAIN_TYPE, mandatory = mandatory)]
"###;

const PATHS_SOURCE: &str = r###"# Copyright 2017 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Skylib module containing file path manipulation functions.

NOTE: The functions in this module currently only support paths with Unix-style
path separators (forward slash, "/"); they do not handle Windows-style paths
with backslash separators or drive letters.
"""

def _basename(p):
    """Returns the basename (i.e., the file portion) of a path.

    Note that if `p` ends with a slash, this function returns an empty string.
    This matches the behavior of Python's `os.path.basename`, but differs from
    the Unix `basename` command (which would return the path segment preceding
    the final slash).

    Args:
      p: The path whose basename should be returned.

    Returns:
      The basename of the path, which includes the extension.
    """
    return p.rpartition("/")[-1]

def _dirname(p):
    """Returns the dirname of a path.

    The dirname is the portion of `p` up to but not including the file portion
    (i.e., the basename). Any slashes immediately preceding the basename are not
    included, unless omitting them would make the dirname empty.

    Args:
      p: The path whose dirname should be returned.

    Returns:
      The dirname of the path.
    """
    prefix, sep, _ = p.rpartition("/")
    if not prefix:
        return sep
    else:
        # If there are multiple consecutive slashes, strip them all out as Python's
        # os.path.dirname does.
        return prefix.rstrip("/")

def _is_absolute(path):
    """Returns `True` if `path` is an absolute path.

    Args:
      path: A path (which is a string).

    Returns:
      `True` if `path` is an absolute path.
    """
    return path.startswith("/") or (len(path) > 2 and path[1] == ":")

def _join(path, *others):
    """Joins one or more path components intelligently.

    This function mimics the behavior of Python's `os.path.join` function on POSIX
    platform. It returns the concatenation of `path` and any members of `others`,
    inserting directory separators before each component except the first. The
    separator is not inserted if the path up until that point is either empty or
    already ends in a separator.

    If any component is an absolute path, all previous components are discarded.

    Args:
      path: A path segment.
      *others: Additional path segments.

    Returns:
      A string containing the joined paths.
    """
    result = path

    for p in others:
        if _is_absolute(p):
            result = p
        elif not result or result.endswith("/"):
            result += p
        else:
            result += "/" + p

    return result

def _normalize(path):
    """Normalizes a path, eliminating double slashes and other redundant segments.

    This function mimics the behavior of Python's `os.path.normpath` function on
    POSIX platforms; specifically:

    - If the entire path is empty, "." is returned.
    - All "." segments are removed, unless the path consists solely of a single
      "." segment.
    - Trailing slashes are removed, unless the path consists solely of slashes.
    - ".." segments are removed as long as there are corresponding segments
      earlier in the path to remove; otherwise, they are retained as leading ".."
      segments.
    - Single and double leading slashes are preserved, but three or more leading
      slashes are collapsed into a single leading slash.
    - Multiple adjacent internal slashes are collapsed into a single slash.

    Args:
      path: A path.

    Returns:
      The normalized path.
    """
    if not path:
        return "."

    if path.startswith("//") and not path.startswith("///"):
        initial_slashes = 2
    elif path.startswith("/"):
        initial_slashes = 1
    else:
        initial_slashes = 0
    is_relative = (initial_slashes == 0)

    components = path.split("/")
    new_components = []

    for component in components:
        if component in ("", "."):
            continue
        if component == "..":
            if new_components and new_components[-1] != "..":
                # Only pop the last segment if it isn't another "..".
                new_components.pop()
            elif is_relative:
                # Preserve leading ".." segments for relative paths.
                new_components.append(component)
        else:
            new_components.append(component)

    path = "/".join(new_components)
    if not is_relative:
        path = ("/" * initial_slashes) + path

    return path or "."

_BASE = 0
_SEPARATOR = 1
_DOT = 2
_DOTDOT = 3

def _is_normalized(str, look_for_same_level_references = True):
    """Returns true if the passed path doesn't contain uplevel references "..".

    Also checks for single-dot references "." if look_for_same_level_references
    is `True.`

    Args:
      str: The path string to check.
      look_for_same_level_references: If True checks if path doesn't contain
        uplevel references ".." or single-dot references ".".

    Returns:
      True if the path is normalized, False otherwise.
    """
    state = _SEPARATOR
    for c in str.elems():
        is_separator = False
        if c == "/":
            is_separator = True

        if state == _BASE:
            if is_separator:
                state = _SEPARATOR
            else:
                state = _BASE
        elif state == _SEPARATOR:
            if is_separator:
                state = _SEPARATOR
            elif c == ".":
                state = _DOT
            else:
                state = _BASE
        elif state == _DOT:
            if is_separator:
                if look_for_same_level_references:
                    # "." segment found.
                    return False
                state = _SEPARATOR
            elif c == ".":
                state = _DOTDOT
            else:
                state = _BASE
        elif state == _DOTDOT:
            if is_separator:
                return False
            else:
                state = _BASE

    if state == _DOT:
        if look_for_same_level_references:
            # "." segment found.
            return False
    elif state == _DOTDOT:
        return False
    return True

def _relativize(path, start):
    """Returns the portion of `path` that is relative to `start`.

    Because we do not have access to the underlying file system, this
    implementation differs slightly from Python's `os.path.relpath` in that it
    will fail if `path` is not beneath `start` (rather than use parent segments to
    walk up to the common file system root).

    Relativizing paths that start with parent directory references only works if
    the path both start with the same initial parent references.

    Args:
      path: The path to relativize.
      start: The ancestor path against which to relativize.

    Returns:
      The portion of `path` that is relative to `start`.
    """
    segments = _normalize(path).split("/")
    start_segments = _normalize(start).split("/")
    if start_segments == ["."]:
        start_segments = []
    start_length = len(start_segments)

    if (path.startswith("/") != start.startswith("/") or
        len(segments) < start_length):
        fail("Path '%s' is not beneath '%s'" % (path, start))

    for ancestor_segment, segment in zip(start_segments, segments):
        if ancestor_segment != segment:
            fail("Path '%s' is not beneath '%s'" % (path, start))

    length = len(segments) - start_length
    result_segments = segments[-length:]
    return "/".join(result_segments)

def _replace_extension(p, new_extension):
    """Replaces the extension of the file at the end of a path.

    If the path has no extension, the new extension is added to it.

    Args:
      p: The path whose extension should be replaced.
      new_extension: The new extension for the file. The new extension should
          begin with a dot if you want the new filename to have one.

    Returns:
      The path with the extension replaced (or added, if it did not have one).
    """
    return _split_extension(p)[0] + new_extension

def _split_extension(p):
    """Splits the path `p` into a tuple containing the root and extension.

    Leading periods on the basename are ignored, so
    `path.split_extension(".bashrc")` returns `(".bashrc", "")`.

    Args:
      p: The path whose root and extension should be split.

    Returns:
      A tuple `(root, ext)` such that the root is the path without the file
      extension, and `ext` is the file extension (which, if non-empty, contains
      the leading dot). The returned tuple always satisfies the relationship
      `root + ext == p`.
    """
    b = _basename(p)
    last_dot_in_basename = b.rfind(".")

    # If there is no dot or the only dot in the basename is at the front, then
    # there is no extension.
    if last_dot_in_basename <= 0:
        return (p, "")

    dot_distance_from_end = len(b) - last_dot_in_basename
    return (p[:-dot_distance_from_end], p[-dot_distance_from_end:])

def _starts_with(path_a, path_b):
    """Returns True if and only if path_b is an ancestor of path_a.

    Does not handle OS dependent case-insensitivity."""
    if not path_b:
        # all paths start with the empty string
        return True
    norm_a = _normalize(path_a)
    norm_b = _normalize(path_b)
    if len(norm_b) > len(norm_a):
        return False
    if not norm_a.startswith(norm_b):
        return False
    return len(norm_a) == len(norm_b) or norm_a[len(norm_b)] == "/"

paths = struct(
    basename = _basename,
    dirname = _dirname,
    is_absolute = _is_absolute,
    join = _join,
    normalize = _normalize,
    is_normalized = _is_normalized,
    relativize = _relativize,
    replace_extension = _replace_extension,
    split_extension = _split_extension,
    starts_with = _starts_with,
)
"###;

const CLIPPY_TOOLCHAINS: &str = "[str(Label('//rust:toolchain_type')), config_common.toolchain_type('@bazel_tools//tools/cpp:toolchain_type', mandatory = False)]";

fn clippy_owner() -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_rust+//rust/private:clippy.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_rust/rust/private/clippy.bzl"),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("bazel_tools").unwrap(),
            CanonicalRepoName::new("bazel_tools+").unwrap(),
        )]),
    }
}

fn eval_bzl_with_loaded_children(
    source: &str,
    owner: BzlModuleIdentity,
    children: &[(&str, BzlModuleIdentity, FrozenModule)],
) -> anyhow::Result<FrozenModule> {
    let child_identities = children
        .iter()
        .map(|(_, identity, _)| identity.clone())
        .collect::<Vec<_>>();
    let mut reachable = vec![owner.clone()];
    reachable.extend(child_identities.iter().cloned());
    let filename = owner.workspace_path.to_string_lossy().into_owned();
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: owner,
        direct_children: child_identities.into(),
        reachable: reachable.into(),
        fingerprint: [0; 32],
    });
    let ast = AstModule::parse(&filename, source.to_owned(), &Dialect::Bazel)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let module = Module::new();
    let loader = LocalBzlLoader {
        modules: children
            .iter()
            .map(|(load, _, module)| (*load, module.dupe()))
            .collect(),
    };
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator.set_loader(&loader);
    evaluator
        .eval_module(ast, &loading_globals())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    drop(evaluator);
    Ok(module.freeze()?)
}

#[test]
fn exact_rules_cc_find_toolchain_child_freezes_eager_constants_and_functions() {
    assert_eq!(
        format!("{:x}", Sha256::digest(FIND_CC_TOOLCHAIN_SOURCE.as_bytes())),
        "3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655"
    );
    let identity = |label: &str, path: &str| BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: PathBuf::from(path),
        repository_mapping: Arc::from([
            (
                ApparentRepoName::new("bazel_tools").unwrap(),
                CanonicalRepoName::new("bazel_tools+").unwrap(),
            ),
            (
                ApparentRepoName::new("rules_cc").unwrap(),
                CanonicalRepoName::new("rules_cc+").unwrap(),
            ),
        ]),
    };
    let cc_common_owner = identity(
        "@@rules_cc+//cc/common:cc_common.bzl",
        "/rules_cc/cc/common/cc_common.bzl",
    );
    let cc_common =
        eval_bzl_with_identity("cc_common = struct()\n", cc_common_owner.clone()).unwrap();
    let owner = identity(
        "@@rules_cc+//cc:find_cc_toolchain.bzl",
        "/rules_cc/cc/find_cc_toolchain.bzl",
    );
    let module = eval_bzl_with_loaded_children(
        FIND_CC_TOOLCHAIN_SOURCE,
        owner.clone(),
        &[("//cc/common:cc_common.bzl", cc_common_owner, cc_common)],
    )
    .unwrap();

    for (name, expected_type) in [
        ("CC_TOOLCHAIN_ATTRS", "dict"),
        ("CC_TOOLCHAIN_TYPE", "Label"),
        ("find_cc_toolchain", "function"),
        ("find_cpp_toolchain", "function"),
        ("use_cc_toolchain", "function"),
    ] {
        assert_eq!(module.get(name).unwrap().value().get_type(), expected_type);
    }
    let toolchain_type = module
        .get("CC_TOOLCHAIN_TYPE")
        .unwrap()
        .downcast::<StarlarkLabel>()
        .unwrap();
    assert_eq!(
        toolchain_type.canonical().to_string(),
        "@@bazel_tools+//tools/cpp:toolchain_type"
    );
    let attrs_value = module.get("CC_TOOLCHAIN_ATTRS").unwrap();
    let attrs = DictRef::from_value(attrs_value.value()).unwrap();
    let entries = attrs.iter().collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0.unpack_str(), Some("_cc_toolchain"));
    assert_eq!(entries[0].1.get_type(), "attribute");

    let consumer_owner = identity(
        "@@rules_cc+//cc:find_cc_toolchain_proof.bzl",
        "/rules_cc/cc/find_cc_toolchain_proof.bzl",
    );
    let consumer = eval_bzl_with_loaded_children(
        "load(\"//cc:find_cc_toolchain.bzl\", \"CC_TOOLCHAIN_ATTRS\")\ndef _impl(ctx): fail(\"implementation must stay lazy\")\nproof_rule = rule(implementation = _impl, attrs = CC_TOOLCHAIN_ATTRS)\n",
        consumer_owner,
        &[("//cc:find_cc_toolchain.bzl", owner, module.dupe())],
    )
    .unwrap();
    let rule = consumer
        .get("proof_rule")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let attribute = rule
        .schema
        .iter()
        .find(|attribute| attribute.name == "_cc_toolchain")
        .unwrap();
    assert_eq!(attribute.kind, AttributeKind::Label);
    assert!(matches!(
        attribute.default.as_ref(),
        Some(CoercedAttributeValue::Label(label))
            if label.to_string() == "@@rules_cc+//cc:current_cc_toolchain"
    ));
}

#[test]
fn exact_bazel_skylib_paths_child_freezes_exported_function_bindings() {
    assert_eq!(
        format!("{:x}", Sha256::digest(PATHS_SOURCE.as_bytes())),
        "96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83"
    );
    let module = eval_bzl_with_identity(
        PATHS_SOURCE,
        BzlModuleIdentity {
            label: CanonicalLabel::parse("@@bazel_skylib+//lib:paths.bzl").unwrap(),
            workspace_path: PathBuf::from("/bazel_skylib/lib/paths.bzl"),
            repository_mapping: Arc::from([]),
        },
    )
    .unwrap();
    let paths_value = module.get("paths").unwrap();
    let paths = StructRef::from_value(paths_value.value()).unwrap();
    let mut fields = paths
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.get_type()))
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        fields,
        [
            ("basename".to_owned(), "function"),
            ("dirname".to_owned(), "function"),
            ("is_absolute".to_owned(), "function"),
            ("is_normalized".to_owned(), "function"),
            ("join".to_owned(), "function"),
            ("normalize".to_owned(), "function"),
            ("relativize".to_owned(), "function"),
            ("replace_extension".to_owned(), "function"),
            ("split_extension".to_owned(), "function"),
            ("starts_with".to_owned(), "function"),
        ]
    );
}

#[test]
fn exact_lints_child_freezes_with_provider_identity_and_ordered_schema() {
    assert_eq!(
        format!("{:x}", Sha256::digest(LINTS_SOURCE.as_bytes())),
        "0c6dcf615bb9f43d57c4056253f89a9f1bed0b16b9e17d8eed64da85d1b05677"
    );
    let owner = |name: &str| BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@rules_rust+//rust/private:{name}.bzl")).unwrap(),
        workspace_path: PathBuf::from(format!("/rules_rust/rust/private/{name}.bzl")),
        repository_mapping: Arc::from([]),
    };
    let providers_owner = owner("providers");
    let providers = eval_bzl_with_identity(
        r#"LintsInfo = provider(
    doc = "LintsInfo holds the 'allow', 'warn', etc. config for rustc, clippy, and rustdoc lints.",
    fields = {
        "clippy_lint_files": "List[File]: files with rustc args for clippy targets.",
        "clippy_lint_flags": "List[String]: rustc flags to specify when building clippy targets.",
        "rustc_lint_files": "List[File]: list of files with rustc flags to specify when building rust_* targets.",
        "rustc_lint_flags": "List[String]: rustc flags to specify when building rust_* targets.",
        "rustdoc_lint_files": "List[File]: files with rustc args for rustdoc target.",
        "rustdoc_lint_flags": "List[String]: rustc flags to specify when building rust_doc targets.",
    },
)
"#,
        providers_owner.clone(),
    )
    .unwrap();
    let module = eval_bzl_with_loaded_children(
        LINTS_SOURCE,
        owner("lints"),
        &[(
            "//rust/private:providers.bzl",
            providers_owner,
            providers.dupe(),
        )],
    )
    .unwrap();
    assert!(
        module
            .get("LintsInfo")
            .unwrap()
            .value()
            .ptr_eq(providers.get("LintsInfo").unwrap().value())
    );
    let rule = module
        .get("rust_lint_config")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_eq!(rule.capability().rule_class, "rust_lint_config");
    let declared = &rule.schema[rule.schema.len() - 4..];
    assert_eq!(
        declared
            .iter()
            .map(|attribute| attribute.name.as_str())
            .collect::<Vec<_>>(),
        ["rustc", "rustc_check_cfg", "clippy", "rustdoc"]
    );
    assert_eq!(
        declared
            .iter()
            .map(|attribute| attribute.kind)
            .collect::<Vec<_>>(),
        [
            AttributeKind::StringDict,
            AttributeKind::StringListDict,
            AttributeKind::StringDict,
            AttributeKind::StringDict,
        ]
    );
    assert!(declared.iter().all(|attribute| {
        !attribute.mandatory
            && attribute.configurable
            && attribute.default.is_none()
            && !attribute.executable
            && !attribute.exec_configuration
    }));
}

#[test]
#[rustfmt::skip]
fn clippy_test_tail_freezes_with_recursive_producer_identities() {
    let child_owner = |name: &str| {
        let mut owner = clippy_owner();
        owner.label = CanonicalLabel::parse(&format!("@@rules_rust+//rust/private:{name}.bzl")).unwrap();
        owner.workspace_path = PathBuf::from(format!("/rules_rust/rust/private/{name}.bzl"));
        owner
    };
    let providers_owner = child_owner("providers");
    let providers = eval_bzl_with_identity(
        r#"CrateInfo = provider(doc = "crate", fields = {})
TestCrateInfo = provider(doc = "test crate", fields = {})
CaptureClippyOutputInfo = provider(doc = "Value of capture", fields = {"capture_output": "value"})
ClippyInfo = provider(doc = "Provides information on a clippy run.", fields = {"output": "File with the clippy output."})
ClippyOutputDiagnosticsInfo = provider(doc = "Value of diagnostics", fields = {"output_diagnostics": "value"})
"#,
        providers_owner.clone(),
    ).unwrap();
    let common_owner = child_owner("common");
    let common = eval_bzl_with_loaded_children(
        "load(':providers.bzl', 'CrateInfo', 'TestCrateInfo')\nrust_common = struct(crate_info = CrateInfo, test_crate_info = TestCrateInfo)\n",
        common_owner.clone(),
        &[(":providers.bzl", providers_owner.clone(), providers.dupe())],
    ).unwrap();
    let lint_owner = child_owner("lint_test");
    let lint = eval_bzl_with_identity(&format!("{LINT_TEST_SOURCE}\nTRANSITION_IMPL=_platform_transition_impl\n"), lint_owner.clone()).unwrap();
    let prefix = CLIPPY_ASPECT_SOURCE.replace("TOOLCHAINS", CLIPPY_TOOLCHAINS);
    let prefix = &prefix[prefix.find("def _clippy_aspect_impl").unwrap()..];
    let mut source = r#"load("//rust/private:common.bzl", "rust_common")
load("//rust/private:lint_test.bzl", "LINT_TEST_COMMON_ATTRS", "lint_test_aspect_impl", "lint_test_rule_impl", "platform_transition")
load("//rust/private:providers.bzl", "CaptureClippyOutputInfo", "ClippyInfo", "ClippyOutputDiagnosticsInfo")
"#.to_owned();
    source.push_str(prefix);
    source.push_str(CLIPPY_TEST_TAIL_SOURCE);
    source.push_str("\nIMPORTED_LINT=[LINT_TEST_COMMON_ATTRS, lint_test_aspect_impl, lint_test_rule_impl, platform_transition]\nIMPORTED_PROVIDERS=[CaptureClippyOutputInfo, ClippyInfo, ClippyOutputDiagnosticsInfo]\nIMPORTED_COMMON=rust_common\nOUTPUT_GROUPS=_CLIPPY_OUTPUT_GROUPS\nTEST_ASPECT=_rust_clippy_test_aspect\n");
    let module = eval_bzl_with_loaded_children(
        &source, clippy_owner(), &[
            ("//rust/private:common.bzl", common_owner, common.dupe()),
            ("//rust/private:lint_test.bzl", lint_owner, lint.dupe()),
            ("//rust/private:providers.bzl", providers_owner, providers.dupe()),
        ],
    ).unwrap();
    let imported = |name| FrozenListRef::from_value(module.get(name).unwrap().value()).unwrap();
    for (value, name) in imported("IMPORTED_LINT").iter().zip(["LINT_TEST_COMMON_ATTRS", "lint_test_aspect_impl", "lint_test_rule_impl", "platform_transition"]) {
        assert!(value.to_value().ptr_eq(lint.get(name).unwrap().value()));
    }
    for (value, name) in imported("IMPORTED_PROVIDERS").iter().zip(["CaptureClippyOutputInfo", "ClippyInfo", "ClippyOutputDiagnosticsInfo"]) {
        assert!(value.to_value().ptr_eq(providers.get(name).unwrap().value()));
    }
    assert!(module.get("IMPORTED_COMMON").unwrap().value().ptr_eq(common.get("rust_common").unwrap().value()));
    let groups = imported("OUTPUT_GROUPS");
    assert_eq!(groups.iter().map(|v| v.to_value().unpack_str().unwrap()).collect::<Vec<_>>(), ["clippy_checks", "clippy_output"]);
    let aspect_value = module.get("TEST_ASPECT").unwrap();
    let aspect = aspect_value.clone().downcast::<FrozenAspectDefinition>().unwrap();
    assert_eq!(aspect.attr_aspects.as_ref(), ["deps", "proc_macro_deps", "crate"]);
    assert!(aspect.required_aspect.unwrap().to_value().ptr_eq(module.get("rust_clippy_aspect").unwrap().value()));
    assert_eq!(aspect.advertised_providers[0].to_string(), "@@rules_rust+//rust/private:clippy.bzl%RustClippyTestInfo");
    let rule = module.get("rust_clippy_test").unwrap().downcast::<FrozenRuleDefinition>().unwrap();
    assert_eq!(rule.capability().test_kind, Some(TestRuleKind::Test));
    let declared = &rule.schema[rule.schema.len() - 5..];
    assert_eq!(declared.iter().map(|attribute| attribute.name.as_str()).collect::<Vec<_>>(), ["platform", "transitive", "_allowlist_function_transition", "_runner", "targets"]);
    assert_eq!(declared[..4].iter().map(|attribute| attribute.kind).collect::<Vec<_>>(), [AttributeKind::Label, AttributeKind::Boolean, AttributeKind::Label, AttributeKind::Label]);
    assert!(declared[0].default.is_none() && matches!(declared[1].default, Some(CoercedAttributeValue::Boolean(false))) && matches!(declared[2].default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@bazel_tools+//tools/allowlists/function_transition_allowlist:function_transition_allowlist") && declared[3].executable && declared[3].exec_configuration && matches!(declared[3].default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@rules_rust+//rust/private/lint_test_runner:lint_test_runner"));
    let targets = &declared[4];
    assert_eq!(targets.required_providers[0][0].to_string(), "@@rules_rust+//rust/private:providers.bzl%CrateInfo");
    assert_eq!(targets.required_providers[1][0].to_string(), "@@rules_rust+//rust/private:providers.bzl%TestCrateInfo");
    assert!(targets.attached_aspect.unwrap().to_value().ptr_eq(aspect_value.value()));
    let transition = targets.transition.as_ref().unwrap();
    assert!(transition.implementation().to_value().ptr_eq(lint.get("TRANSITION_IMPL").unwrap().value()));
    assert_eq!(transition.output(), "//command_line_option:platforms");
    for name in ["capture_clippy_output", "clippy_output_diagnostics"] {
        let setting = module.get(name).unwrap().downcast::<FrozenRuleDefinition>().unwrap();
        assert_eq!(setting.build_setting_kind, Some(BuildSettingKind::Boolean { flag: true }));
    }
    for rich in [
        "P=provider()\nX=attr.label(providers=[P])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl)\nX=attr.label_list(aspects=[A])",
        "def impl(settings, attr): return {}\nT=transition(implementation=impl, inputs=[], outputs=['//:setting'])\nX=attr.label(cfg=T)",
    ] {
        let rich_owner = child_owner("rich");
        let rich = eval_bzl_with_identity(rich, rich_owner.clone()).unwrap();
        assert!(eval_bzl_with_loaded_children("load(':rich.bzl','X')\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'x':X})", clippy_owner(), &[(":rich.bzl", rich_owner, rich)]).is_err());
    }
}

#[test]
fn external_bzl_module_freezes_exact_lint_test_child_without_invocation() {
    let mut child_owner = clippy_owner();
    child_owner.label = CanonicalLabel::parse("@@rules_rust+//rust/private:lint_test.bzl").unwrap();
    child_owner.workspace_path = PathBuf::from("/rules_rust/rust/private/lint_test.bzl");
    let child = eval_bzl_with_identity(LINT_TEST_SOURCE, child_owner.clone()).unwrap();
    let parent_owner = clippy_owner();
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: parent_owner.clone(),
        direct_children: Arc::from([child_owner.clone()]),
        reachable: Arc::from([parent_owner.clone(), child_owner]),
        fingerprint: [0; 32],
    });
    let source = r#"load(
    "//rust/private:lint_test.bzl",
    "LINT_TEST_COMMON_ATTRS",
    "lint_test_aspect_impl",
    "lint_test_rule_impl",
    "platform_transition",
)
IMPORTED = [LINT_TEST_COMMON_ATTRS, lint_test_aspect_impl, lint_test_rule_impl, platform_transition]
RUN = RunEnvironmentInfo
"#;
    let ast = AstModule::parse("clippy.bzl", source.to_owned(), &Dialect::Bazel).unwrap();
    let module = Module::new();
    let loader = LocalBzlLoader {
        modules: vec![("//rust/private:lint_test.bzl", child.dupe())],
    };
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator.set_loader(&loader);
    evaluator.eval_module(ast, &loading_globals()).unwrap();
    drop(evaluator);
    let module = module.freeze().unwrap();
    let imported = FrozenListRef::from_value(module.get("IMPORTED").unwrap().value()).unwrap();
    for (imported, name) in imported.iter().zip([
        "LINT_TEST_COMMON_ATTRS",
        "lint_test_aspect_impl",
        "lint_test_rule_impl",
        "platform_transition",
    ]) {
        assert!(imported.to_value().ptr_eq(child.get(name).unwrap().value()));
    }
    let run = module.get("RUN").unwrap();
    assert_eq!(run.to_string(), "<function RunEnvironmentInfo>");
    assert!(run.clone().downcast::<RunEnvironmentInfo>().is_ok());
    assert!(run.clone().downcast::<OutputGroupInfo>().is_err());
    assert!(run.downcast::<FrozenUserProviderCallable>().is_err());
    assert!(eval_global("X = RunEnvironmentInfo", &build_file_loading_globals()).is_err());
    let error = eval_bzl_with_identity("X = RunEnvironmentInfo(environment = {})", clippy_owner())
        .unwrap_err()
        .to_string();
    assert!(error.contains("RunEnvironmentInfo construction is unsupported during loading"));
}

#[test]
fn output_group_info_is_a_bzl_only_fail_closed_native_declaration() {
    let module = eval_bzl_with_identity(
        "NATIVE = OutputGroupInfo\nUSER = provider()",
        clippy_owner(),
    )
    .unwrap();
    let native = module.get("NATIVE").unwrap();
    assert_eq!(native.to_string(), "<function OutputGroupInfo>");
    assert!(native.downcast::<OutputGroupInfo>().is_ok());
    assert!(
        module
            .get("USER")
            .unwrap()
            .downcast::<FrozenUserProviderCallable>()
            .is_ok()
    );
    assert!(eval_global("X = OutputGroupInfo", &build_file_loading_globals()).is_err());
    let error = eval_bzl_with_identity("X = OutputGroupInfo()", clippy_owner())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("OutputGroupInfo construction is unsupported during loading"),
        "{error}"
    );
}

#[test]
fn clippy_aspect_freezes_complete_source_declaration() {
    let source = CLIPPY_ASPECT_SOURCE.replace("TOOLCHAINS", CLIPPY_TOOLCHAINS);
    let module = eval_bzl_with_identity(&source, clippy_owner()).unwrap();
    let aspect_value = module.get("rust_clippy_aspect").unwrap();
    let aspect = aspect_value
        .clone()
        .downcast::<FrozenAspectDefinition>()
        .unwrap();
    let expected = [
        ("_capture_output", "rust/settings:capture_clippy_output"),
        ("_clippy_error_format", "rust/settings:clippy_error_format"),
        ("_clippy_flag", "rust/settings:clippy_flag"),
        ("_clippy_flags", "rust/settings:clippy_flags"),
        (
            "_clippy_output_diagnostics",
            "rust/settings:clippy_output_diagnostics",
        ),
        ("_config", "rust/settings:clippy.toml"),
        ("_error_format", "rust/settings:error_format"),
        ("_extra_rustc_flag", "rust/settings:extra_rustc_flag"),
        (
            "_incompatible_change_clippy_error_format",
            "rust/settings:incompatible_change_clippy_error_format",
        ),
        (
            "_per_crate_rustc_flag",
            "rust/settings:per_crate_rustc_flag",
        ),
        ("_process_wrapper", "util/process_wrapper:process_wrapper"),
    ];
    assert_eq!(aspect.attributes.len(), expected.len());
    for (attribute, (name, target)) in aspect.attributes.iter().zip(expected) {
        assert_eq!(attribute.name, name);
        assert_eq!(attribute.kind, AttributeKind::Label);
        assert!(!attribute.mandatory && attribute.configurable && !attribute.allow_files);
        assert!(matches!(
            attribute.allowed_values,
            AllowedAttributeValues::None
        ));
        assert!(attribute.required_providers.is_empty());
        assert!(attribute.attached_aspect.is_none() && attribute.transition.is_none());
        assert!(
            matches!(attribute.default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == format!("@@rules_rust+//{target}"))
        );
        assert_eq!(
            attribute.allow_single_file,
            (name == "_config").then_some(AllowSingleFile::True)
        );
        assert_eq!(attribute.executable, name == "_process_wrapper");
        assert_eq!(attribute.exec_configuration, name == "_process_wrapper");
    }
    let [rust, cpp] = aspect.required_toolchains.as_ref() else {
        panic!("expected the source's two aspect toolchain requirements");
    };
    assert_eq!(
        rust.label().to_string(),
        "@@rules_rust+//rust:toolchain_type"
    );
    assert!(rust.mandatory());
    assert_eq!(
        cpp.label().to_string(),
        "@@bazel_tools+//tools/cpp:toolchain_type"
    );
    assert!(!cpp.mandatory());
    assert_eq!(aspect.required_providers.len(), 2);
    assert_eq!(aspect.advertised_providers.len(), 1);
    assert_eq!(aspect.required_fragments.as_ref(), ["cpp"]);

    let rule = module
        .get("rust_clippy")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_eq!(rule.capability().rule_class, "rust_clippy");
    assert!(!rule.capability().executable);
    assert_eq!(rule.capability().test_kind, None);
    assert!(rule.required_toolchains().is_empty());
    assert_eq!(rule.schema.len(), 23);
    let deps = rule.schema.last().unwrap();
    assert_eq!(deps.name, "deps");
    assert_eq!(deps.kind, AttributeKind::LabelList);
    assert!(!deps.mandatory && deps.configurable && !deps.allow_files);
    assert!(deps.default.is_none() && deps.allow_single_file.is_none());
    assert!(!deps.executable && !deps.exec_configuration && deps.transition.is_none());
    assert_eq!(deps.required_providers.len(), 2);
    assert_eq!(
        deps.required_providers[0][0].to_string(),
        "@@rules_rust+//rust/private:clippy.bzl%CrateInfo"
    );
    assert_eq!(
        deps.required_providers[1][0].to_string(),
        "@@rules_rust+//rust/private:clippy.bzl%TestCrateInfo"
    );
    let attached_value = deps.attached_aspect.unwrap();
    assert!(attached_value.to_value().ptr_eq(aspect_value.value()));
    let attached = attached_value
        .downcast_ref::<FrozenAspectDefinition>()
        .unwrap();
    assert_eq!(attached.defining_label, aspect.defining_label);
    assert_eq!(attached.exported_name, aspect.exported_name);
    assert_eq!(attached.attributes.len(), aspect.attributes.len());
    assert_eq!(attached.required_toolchains, aspect.required_toolchains);
    assert_eq!(attached.required_providers, aspect.required_providers);
    assert_eq!(attached.advertised_providers, aspect.advertised_providers);
    assert_eq!(attached.required_fragments, aspect.required_fragments);
}

#[test]
fn clippy_aspect_rejects_source_mutations() {
    let admitted = CLIPPY_ASPECT_SOURCE.replace("TOOLCHAINS", CLIPPY_TOOLCHAINS);
    for (from, to) in [
        (
            "\"_capture_output\": attr.label",
            "\"capture_output\": attr.label",
        ),
        (
            "default = Label(\"//rust/settings:capture_clippy_output\")",
            "configurable = False, default = Label(\"//rust/settings:capture_clippy_output\")",
        ),
        (
            "default = \"//rust/settings:clippy_error_format\"",
            "default = \"//rust/settings:wrong\"",
        ),
        (
            "attr.label(doc = \"flag\", default = Label(\"//rust/settings:clippy_flag\"))",
            "attr.label(doc = \"flag\")",
        ),
        ("attr.label(doc = \"flags\"", "attr.string(doc = \"flags\""),
        (
            "doc = \"diagnostics\", default",
            "doc = \"diagnostics\", allow_files = True, default",
        ),
        (
            "doc = \"rustc format\", default",
            "doc = \"rustc format\", cfg = \"exec\", default",
        ),
        (
            "doc = \"wrapper\", default",
            "doc = \"wrapper\", providers = [1], default",
        ),
        (
            "doc = \"wrapper\", default",
            "doc = \"wrapper\", aspects = [1], default",
        ),
        (
            "cfg = \"exec\"",
            "cfg = transition(implementation = _clippy_aspect_impl, inputs = [], outputs = [\"//:setting\"] )",
        ),
        (
            "executable = True, cfg = \"exec\"",
            "executable = False, cfg = \"exec\"",
        ),
    ] {
        let source = admitted.replacen(from, to, 1);
        assert_ne!(source, admitted, "mutation anchor must remain live: {from}");
        assert!(
            eval_bzl_with_identity(&source, clippy_owner()).is_err(),
            "{from}"
        );
    }

    for source in [
        admitted.replacen("        \"_capture_output\": attr.label(doc = \"capture\", default = Label(\"//rust/settings:capture_clippy_output\")),\n", "", 1),
        admitted.replacen("        \"_process_wrapper\":", "        \"_extra\": attr.label(default = Label(\"//:extra\")),\n        \"_process_wrapper\":", 1),
        admitted.replace("\"_capture_output\"", "\"_temporary\"").replace("\"_clippy_error_format\"", "\"_capture_output\"").replace("\"_temporary\"", "\"_clippy_error_format\""),
    ] {
        assert!(eval_bzl_with_identity(&source, clippy_owner()).is_err());
    }
}

#[test]
fn aspect_toolchain_requirements_retain_mapping_order_and_mandatory_identity() {
    let source = r#"
def impl(target, ctx): return []
MIXED = aspect(implementation = impl, toolchains = [":local", Label("@bazel_tools//tools:label"), config_common.toolchain_type("//tools:optional", mandatory = False)])
TRUE = aspect(implementation = impl, toolchains = [config_common.toolchain_type("//tools:same", mandatory = True)])
FALSE = aspect(implementation = impl, toolchains = [config_common.toolchain_type("//tools:same", mandatory = False)])
"#;
    let module = eval_bzl_with_identity(source, clippy_owner()).unwrap();
    let mixed = module
        .get("MIXED")
        .unwrap()
        .downcast::<FrozenAspectDefinition>()
        .unwrap();
    let expected = [
        ("@@rules_rust+//rust/private:local", true),
        ("@@bazel_tools+//tools:label", true),
        ("@@rules_rust+//tools:optional", false),
    ];
    assert_eq!(mixed.required_toolchains.len(), expected.len());
    for (requirement, (label, mandatory)) in mixed.required_toolchains.iter().zip(expected) {
        assert_eq!(requirement.label().to_string(), label);
        assert_eq!(requirement.mandatory(), mandatory);
    }
    let get = |name| {
        module
            .get(name)
            .unwrap()
            .downcast::<FrozenAspectDefinition>()
            .unwrap()
    };
    assert_ne!(
        get("TRUE").required_toolchains,
        get("FALSE").required_toolchains
    );

    for source in [
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, toolchains=1)",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, toolchains=[None])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, toolchains=['//:same', Label('//:same')])",
    ] {
        assert!(
            eval_bzl_with_identity(source, clippy_owner()).is_err(),
            "{source}"
        );
    }
}

fn assert_frozen_rustfmt_test_aspect(aspect: &FrozenAspectDefinition) {
    assert_eq!(
        aspect.attr_aspects.as_ref(),
        ["deps", "proc_macro_deps", "crate"]
    );
    assert!(aspect.attributes.is_empty());
    assert!(aspect.required_providers.is_empty());
    assert_eq!(aspect.advertised_providers.len(), 1);
    assert_eq!(
        aspect.advertised_providers[0].to_string(),
        "@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo"
    );
    assert_eq!(
        aspect.defining_label,
        CanonicalLabel::parse("@@dep+//rust/private:rustfmt.bzl").unwrap()
    );
    assert_eq!(
        aspect.exported_name.as_deref(),
        Some("_rustfmt_test_aspect")
    );
    let required = aspect
        .required_aspect
        .unwrap()
        .downcast_ref::<FrozenAspectDefinition>()
        .unwrap();
    assert_frozen_rustfmt_aspect(required);
}

fn assert_frozen_rustfmt_test_rule(rule: &FrozenRuleDefinition) {
    assert_eq!(rule.capability().rule_class, "rustfmt_test");
    assert!(rule.capability().executable);
    assert_eq!(rule.capability().test_kind, Some(TestRuleKind::Test));
    let declared = &rule.schema[rule.schema.len() - 6..];
    assert_eq!(
        declared
            .iter()
            .map(|attribute| attribute.name.as_str())
            .collect::<Vec<_>>(),
        [
            "platform",
            "transitive",
            "_allowlist_function_transition",
            "_runner",
            "provider_tool",
            "targets"
        ]
    );
    assert!(declared[..4].iter().all(|attribute| {
        attribute.required_providers.is_empty() && attribute.attached_aspect.is_none()
    }));
    assert_eq!(
        declared[4].required_providers[0][0].to_string(),
        "@@dep+//rust/private:providers.bzl%CrateInfo"
    );
    let targets = &declared[5];
    assert_eq!(targets.kind, AttributeKind::LabelList);
    assert!(!targets.mandatory && !targets.executable && !targets.exec_configuration);
    assert!(targets.default.is_none() && targets.allow_single_file.is_none());
    assert_eq!(targets.required_providers.len(), 2);
    assert_eq!(
        targets.required_providers[0][0].to_string(),
        "@@dep+//rust/private:providers.bzl%CrateInfo"
    );
    assert_eq!(
        targets.required_providers[1][0].to_string(),
        "@@dep+//rust/private:providers.bzl%TestCrateInfo"
    );
    assert_eq!(
        targets.transition.as_ref().unwrap().output(),
        "//command_line_option:platforms"
    );
    assert_frozen_rustfmt_test_aspect(
        targets
            .attached_aspect
            .unwrap()
            .downcast_ref::<FrozenAspectDefinition>()
            .unwrap(),
    );
}

#[tokio::test]
async fn external_bzl_module_freezes_rustfmt_test_rule_dependency_schema() {
    let files: &[(&str, &[u8])] = &[
        (
            "rust/private/root.bzl",
            b"load(':rustfmt.bzl', 'RUSTFMT_TEST_ASPECT', 'rustfmt_test')\nIMPORTED = RUSTFMT_TEST_ASPECT\nIMPORTED_RULE = rustfmt_test\n",
        ),
        ("rust/private/BUILD.bazel", b""),
        (
            "rust/private/providers.bzl",
            b"CrateInfo = provider(doc = 'crate', fields = {})\nTestCrateInfo = provider(doc = 'test crate', fields = {})\n",
        ),
        (
            "rust/private/common.bzl",
            b"load(':providers.bzl', 'CrateInfo', 'TestCrateInfo')\nrust_common = struct(crate_info = CrateInfo, test_crate_info = TestCrateInfo)\n",
        ),
        (
            "rust/private/rustfmt.bzl",
            br#"load(":common.bzl", "rust_common")
RustfmtTargetInfo = provider(
    doc = "A provider containing rustfmt formattable sources for a target.",
    fields = {"edition": "str", "srcs": "list[File]"},
)
def _rustfmt_srcs_aspect_impl(target, ctx):
    fail("aspect implementation must stay lazy")
rustfmt_srcs_aspect = aspect(
    implementation = _rustfmt_srcs_aspect_impl,
    doc = "This aspect collects formattable sources from a Rust target.",
    required_providers = [
        [rust_common.crate_info],
        [rust_common.test_crate_info],
    ],
    fragments = ["cpp"],
)
def _rustfmt_aspect_impl(target, ctx):
    fail("second aspect implementation must stay lazy")
rustfmt_aspect = aspect(
    implementation = _rustfmt_aspect_impl,
    doc = "This aspect gathers crate information and performs rustfmt checks.",
    attrs = {
        "_config": attr.label(
            doc = "The rustfmt.toml file used for formatting",
            allow_single_file = True,
            default = Label("//rust/settings:rustfmt.toml"),
        ),
        "_process_wrapper": attr.label(
            doc = "A process wrapper for running rustfmt on all platforms",
            cfg = "exec",
            executable = True,
            default = Label("//util/process_wrapper"),
        ),
    },
    required_providers = [
        [rust_common.crate_info],
        [rust_common.test_crate_info],
    ],
    requires = [rustfmt_srcs_aspect],
    fragments = ["cpp"],
    toolchains = [str(Label("//rust/rustfmt:toolchain_type"))],
)
RustfmtTestInfo = provider(
    doc = "Rustfmt check outputs collected from the underlying rustfmt aspect.",
    fields = {"checks": "depset[File]", "direct": "depset[File]"},
)
def _rustfmt_test_aspect_impl(target, ctx):
    fail("test aspect implementation must stay lazy")
_rustfmt_test_aspect = aspect(
    implementation = _rustfmt_test_aspect_impl,
    attr_aspects = ["deps", "proc_macro_deps", "crate"],
    requires = [rustfmt_aspect],
    provides = [RustfmtTestInfo],
    doc = "Rolls up markers produced by rustfmt_aspect.",
)
RUSTFMT_TEST_ASPECT = _rustfmt_test_aspect
def _platform_transition_impl(settings, attr):
    fail("transition implementation must stay lazy")
platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)
LINT_TEST_COMMON_ATTRS = {
    "platform": attr.label(doc = "platform"),
    "transitive": attr.bool(doc = "transitive", default = False),
    "_allowlist_function_transition": attr.label(default = Label("//tools/allowlists/function_transition_allowlist")),
    "_runner": attr.label(doc = "runner", cfg = "exec", executable = True, default = Label("//rust/private/lint_test_runner")),
    "provider_tool": attr.label(providers = [rust_common.crate_info]),
}
def _rustfmt_test_impl(ctx):
    fail("rule implementation must stay lazy")
rustfmt_test = rule(
    implementation = _rustfmt_test_impl,
    attrs = dict(LINT_TEST_COMMON_ATTRS, **{
        "targets": attr.label_list(
            doc = "Rust targets to run rustfmt on.",
            providers = [[rust_common.crate_info], [rust_common.test_crate_info]],
            aspects = [_rustfmt_test_aspect],
            cfg = platform_transition,
        ),
    }),
    test = True,
    doc = "Runs rustfmt checks.",
)
"#,
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::external_sources(files, 404);
    epoch.directory("/workspace/dep/rust", 404);
    epoch.directory("/workspace/dep/rust/private", 404);
    epoch.missing("/workspace/dep/rust/BUILD.bazel");
    epoch.missing("/workspace/dep/rust/BUILD");
    let mut transaction = transaction(&dice, epoch.build(), false, None).await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&external_bzl_key(route, "rust/private", "root.bzl"))
        .await
        .unwrap();
    let module = &external_terminal(&outcome).module;
    let aspect = module
        .get("IMPORTED")
        .unwrap()
        .downcast::<FrozenAspectDefinition>()
        .unwrap();
    assert_frozen_rustfmt_test_aspect(&aspect);
    let rule = module
        .get("IMPORTED_RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_frozen_rustfmt_test_rule(&rule);
}

#[test]
fn rustfmt_first_aspect_rejects_unadmitted_requirement_shapes() {
    eval_global(
        "def impl(target, ctx): return []\nA=aspect(implementation=impl)",
        &loading_globals(),
    )
    .unwrap();
    for source in [
        "P=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[P])",
        "P=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[P], P])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[]])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[1]])",
        "P=provider()\nQ=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[P, Q], [P]])",
        "P=provider()\nQ=provider()\nR=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[P], [Q], [R]])",
        "def make_provider(): return provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, required_providers=[[make_provider()]])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, fragments=[])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, fragments=['java'])",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
}

#[test]
fn rustfmt_second_aspect_rejects_unadmitted_declaration_shapes() {
    for source in [
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label()})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_process_wrapper': attr.label(), '_config': attr.label()})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label(), '_wrapper': attr.label()})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.string(), '_process_wrapper': attr.label()})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label(allow_single_file=True, default=Label('//rust/settings:rustfmt.toml')), '_process_wrapper': attr.label(cfg='exec', executable=True, default=Label('//util/process_wrapper')), '_extra': attr.label()})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label(allow_single_file=True, default=Label('//rust/settings:other.toml')), '_process_wrapper': attr.label(cfg='exec', executable=True, default=Label('//util/process_wrapper'))})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label(default=Label('//rust/settings:rustfmt.toml')), '_process_wrapper': attr.label(cfg='exec', executable=True, default=Label('//util/process_wrapper'))})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attrs={'_config': attr.label(allow_single_file=True, default=Label('//rust/settings:rustfmt.toml')), '_process_wrapper': attr.label(cfg='exec', default=Label('//util/process_wrapper'))})",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, requires=[])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, requires=[1])",
        "def impl(target, ctx): return []\nB=aspect(implementation=impl)\nC=aspect(implementation=impl)\nA=aspect(implementation=impl, requires=[B, C])",
        "def impl(target, ctx): return []\nNESTED=[aspect(implementation=impl)]\nA=aspect(implementation=impl, requires=[NESTED[0]])",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
}

#[test]
fn rustfmt_test_aspect_rejects_unadmitted_provides_shapes() {
    for source in [
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, provides=[])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, provides=1)",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, provides=[1])",
        "P=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, provides=[P, P])",
        "P=provider()\nQ=provider()\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, provides=[P, Q])",
        "NESTED=[provider()]\ndef impl(target, ctx): return []\nA=aspect(implementation=impl, provides=NESTED)",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
}

#[test]
fn rustfmt_test_rule_rejects_unadmitted_dependency_schemas() {
    eval_global(
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(doc=None)})",
        &loading_globals(),
    )
    .unwrap();
    for source in [
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(doc=1)})",
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[])})",
        "P=provider()\nQ=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[P, Q])})",
        "P=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[P], [P]])})",
        "P=provider()\nQ=provider()\nS=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[P], [Q], [S]])})",
        "P=provider()\nQ=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[P, Q], [P]])})",
        "P=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[P], [1]])})",
        "def make_provider(): return provider()\nQ=provider()\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[make_provider()], [Q]])})",
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(aspects=[])})",
        "def aspect_impl(target, ctx): return []\nA=aspect(implementation=aspect_impl)\nB=aspect(implementation=aspect_impl)\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(aspects=[A, B])})",
        "def aspect_impl(target, ctx): return []\nA=aspect(implementation=aspect_impl)\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(aspects=[A, A])})",
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(aspects=[1])})",
        "def aspect_impl(target, ctx): return []\nNESTED=[aspect(implementation=aspect_impl)]\ndef impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(aspects=NESTED)})",
        "def impl(ctx): return []\nR=rule(implementation=impl, attrs={'targets': attr.label_list(cfg=1)})",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
}

#[tokio::test]
async fn repository_package_rejects_provider_or_aspect_dependency_before_recording() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(':defs.bzl', 'probe')\nprobe(name = 'blocked')\n",
        ),
        (
            "defs.bzl",
            b"P=provider(fields={})\nQ=provider(fields={})\ndef aspect_impl(target, ctx): return []\nA=aspect(implementation=aspect_impl)\ndef transition_impl(settings, attr): return {}\nT=transition(implementation=transition_impl, inputs=[], outputs=['//:setting'])\ndef impl(ctx): return []\nprobe=rule(implementation=impl, attrs={'targets': attr.label_list(providers=[[P], [Q]], aspects=[A], cfg=T)})\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 405).build(),
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
    assert!(
        error.contains(
            "target invocation for provider-constrained or aspect-bearing attribute 'targets' is not supported"
        ),
        "{error}"
    );
}

#[tokio::test]
async fn repository_package_rejects_scalar_provider_constraint_before_recording() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(':defs.bzl','probe')\nprobe(name='blocked')\n",
        ),
        (
            "defs.bzl",
            b"P=provider()\ndef impl(ctx): return []\nprobe=rule(implementation=impl, attrs={'tool':attr.label(providers=[P])})\n",
        ),
    ];
    let outcome = load_repository_package_fixture(files, 426).await;
    let error = repository_package_error(&outcome);
    assert!(error.contains("provider-constrained or aspect-bearing attribute 'tool'"));
}

#[tokio::test]
async fn external_bzl_module_freezes_rust_analyzer_toolchain_rule_schema() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(':support.bzl', 'rust_analyzer_toolchain', 'EXEC_ONLY', 'CUSTOM_TRUE')\nRULE=rust_analyzer_toolchain\n",
        ),
        (
            "support.bzl",
            br#"def _impl(ctx): fail("implementation must not run while loading")
def _transition(settings, attr): return {}
rust_analyzer_toolchain = rule(implementation = _impl, doc = "toolchain", attrs = {
    "proc_macro_srv": attr.label(doc = "proc macro", cfg = "exec", executable = True, allow_single_file = True),
    "rust_analyzer": attr.label(doc = "analyzer", cfg = "exec", executable = True, allow_single_file = True),
    "rustc": attr.label(doc = "rustc", cfg = "exec", executable = True, allow_single_file = True, mandatory = True),
    "rustc_srcs": attr.label(doc = "sources", mandatory = True),
    "rustc_srcs_path": attr.string(doc = "path", default = "library"),
    "version": attr.string(doc = None, default = ""),
})
EXEC_ONLY = rule(implementation = _impl, attrs = {"x": attr.label(cfg = "exec")})
CUSTOM_TRUE = rule(implementation = _impl, attrs = {"x": attr.label(cfg = transition(implementation = _transition, inputs = [], outputs = ["//:setting"]), executable = True)})
"#,
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let epoch = EpochBuilder::external_sources(files, 402).build();
    let mut transaction = transaction(&dice, epoch, false, None).await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    let module = &external_terminal(&outcome).module;
    let rule = module
        .get("RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let attr_index = |rule: &FrozenRuleDefinition, name: &str| {
        rule.schema
            .iter()
            .position(|schema| schema.name == name)
            .unwrap()
    };
    for name in ["proc_macro_srv", "rust_analyzer", "rustc"] {
        let schema = &rule.schema[attr_index(&rule, name)];
        assert!(schema.executable && schema.exec_configuration);
        assert!(matches!(
            schema.allow_single_file,
            Some(AllowSingleFile::True)
        ));
    }
    assert!(rule.schema[attr_index(&rule, "rustc")].mandatory);
    assert!(rule.schema[attr_index(&rule, "rustc_srcs")].mandatory);
    assert!(
        matches!(rule.schema[attr_index(&rule, "rustc_srcs_path")].default.as_ref(), Some(CoercedAttributeValue::String(value)) if value == "library")
    );
    assert!(
        matches!(rule.schema[attr_index(&rule, "version")].default.as_ref(), Some(CoercedAttributeValue::String(value)) if value.is_empty())
    );
    for name in ["EXEC_ONLY", "CUSTOM_TRUE"] {
        let definition = module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let schema = &definition.schema[attr_index(&definition, "x")];
        if name == "EXEC_ONLY" {
            assert!(!schema.executable && schema.exec_configuration && schema.transition.is_none());
        } else {
            assert!(schema.executable && !schema.exec_configuration && schema.transition.is_some());
        }
    }
}

#[test]
fn bazel_label_list_boolean_allow_files_freezes_rust_stdlib_filegroup() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_rust+//rust/private:toolchain.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_rust/rust/private/toolchain.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
def _impl(ctx): fail("implementation must stay lazy")
OMITTED = rule(implementation = _impl, attrs = {"srcs": attr.label_list()})
EXPLICIT_NONE = rule(implementation = _impl, attrs = {"srcs": attr.label_list(allow_files = None)})
EXPLICIT_FALSE = rule(implementation = _impl, attrs = {"srcs": attr.label_list(allow_files = False)})
rust_stdlib_filegroup = rule(doc = "stdlib", implementation = _impl, attrs = {
    "srcs": attr.label_list(allow_files = True, doc = "stdlib files", mandatory = True),
})
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    for (name, allow_files) in [
        ("OMITTED", false),
        ("EXPLICIT_NONE", false),
        ("EXPLICIT_FALSE", false),
        ("rust_stdlib_filegroup", true),
    ] {
        let rule = module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let srcs = rule
            .schema
            .iter()
            .find(|schema| schema.name == "srcs")
            .unwrap();
        assert_eq!(srcs.allow_files, allow_files, "{name}");
        if allow_files {
            assert!(srcs.mandatory, "{name}");
        }
    }
    let rejects = |source| eval_bzl_with_identity(source, owner.clone()).is_err();
    assert!(rejects("X = attr.label_list(allow_files = ['.rlib'])"));
    assert!(rejects("X = attr.label_list(allow_files = 1)"));
}

#[tokio::test]
async fn rust_stdlib_filegroup_projects_file_allowance_into_target_schema() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(':defs.bzl', 'rust_stdlib_filegroup')\nrust_stdlib_filegroup(name = 'stdlib', srcs = [], visibility = ['//visibility:public'])\n",
        ),
        (
            "defs.bzl",
            b"def _impl(ctx): fail('must stay lazy')\nrust_stdlib_filegroup=rule(implementation=_impl, attrs={'srcs':attr.label_list(allow_files=True, mandatory=True)})\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 407).build(),
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
    let package = repository_package_terminal(&outcome);
    let crate::package::PackageTargetKind::StarlarkRule(rule) = &package.targets[0].kind else {
        panic!("stdlib did not retain its Starlark rule implementation")
    };
    let srcs = rule
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "srcs")
        .unwrap();
    assert!(srcs.mandatory() && srcs.allow_files());
    assert!(srcs.allow_single_file().is_none());
}

#[tokio::test]
async fn scalar_label_file_allowance_projects_without_single_artifact_identity() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(':defs.bzl','r')\nr(name='probe',visibility=['//visibility:public'])\n",
        ),
        (
            "defs.bzl",
            b"def _impl(ctx): fail('must stay lazy')\nr=rule(implementation=_impl, attrs={'tool':attr.label(allow_files=True)})\n",
        ),
    ];
    let outcome = load_repository_package_fixture(files, 425).await;
    let package = repository_package_terminal(&outcome);
    let crate::package::PackageTargetKind::StarlarkRule(rule) = &package.targets[0].kind else {
        panic!("probe did not retain its Starlark rule")
    };
    let tool = rule
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "tool")
        .unwrap();
    assert!(tool.allow_files());
    assert!(tool.allow_single_file().is_none());
}

#[test]
fn bazel_data_attribute_docs_advance_rust_toolchain_to_values_constraint() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_rust+//rust/private:toolchain.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_rust/rust/private/toolchain.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
def _impl(ctx): fail("implementation must stay lazy")
def _attrs(doc):
    return {
        "count": attr.int(doc = doc, default = -1),
        "flags": attr.string_list(doc = doc, default = ["--cfg", "probe"]),
        "debug_info": attr.string_dict(doc = doc, default = {"dbg": "2", "opt": "0"}),
        "crate_flags": attr.string_list_dict(doc = doc, default = {"bin": ["--emit=link"]}),
    }
OMITTED = rule(implementation = _impl, attrs = {
    "count": attr.int(default = -1),
    "flags": attr.string_list(default = ["--cfg", "probe"]),
    "debug_info": attr.string_dict(default = {"dbg": "2", "opt": "0"}),
    "crate_flags": attr.string_list_dict(default = {"bin": ["--emit=link"]}),
})
EXPLICIT_NONE = rule(implementation = _impl, attrs = _attrs(None))
FIRST = rule(implementation = _impl, attrs = _attrs("first documentation"))
SECOND = rule(implementation = _impl, attrs = _attrs("different documentation"))
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    let snapshot = |name| {
        let rule = module
            .get(name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        ["count", "flags", "debug_info", "crate_flags"].map(|name| {
            let schema = rule
                .schema
                .iter()
                .find(|schema| schema.name == name)
                .unwrap();
            (
                schema.kind,
                schema.mandatory,
                schema.configurable,
                schema.default.clone(),
            )
        })
    };
    let omitted = snapshot("OMITTED");
    for name in ["EXPLICIT_NONE", "FIRST", "SECOND"] {
        assert_eq!(snapshot(name), omitted, "{name}");
    }
    for invalid in ["1", "[]", "{}", "provider(fields = {})"] {
        let source = format!("X = attr.string_dict(doc = {invalid})");
        assert!(
            eval_bzl_with_identity(&source, owner.clone()).is_err(),
            "{invalid}"
        );
    }
    assert!(eval_bzl_with_identity("X = config_common.FeatureFlagInfo", owner).is_err());
}

#[test]
fn bazel_integer_allowed_values_freeze_rust_toolchain_prefix() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_rust+//rust/private:toolchain.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_rust/rust/private/toolchain.bzl"),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("bazel_tools").unwrap(),
            CanonicalRepoName::new("bazel_tools").unwrap(),
        )]),
    };
    let source = r#"
def _impl(ctx): fail("implementation must stay lazy")
RustLtoInfo = provider()
BuildSettingInfo = provider()
OMITTED = rule(implementation = _impl, attrs = {"x": attr.int()})
EMPTY_LIST = rule(implementation = _impl, attrs = {"x": attr.int(values = [])})
EMPTY_TUPLE = rule(implementation = _impl, attrs = {"x": attr.int(values = ())})
NORMALIZED = rule(implementation = _impl, attrs = {"x": attr.int(values = [1, -1, 1, 0])})
SAME = rule(implementation = _impl, attrs = {"x": attr.int(values = (-1, 0, 1))})
DISTINCT = rule(implementation = _impl, attrs = {"x": attr.int(values = [-1, 1])})
BAD_DEFAULT = rule(implementation = _impl, attrs = {"x": attr.int(default = 2, values = [0, 1])})
STRING_OMITTED = rule(implementation = _impl, attrs = {"x": attr.string()})
STRING_EMPTY_LIST = rule(implementation = _impl, attrs = {"x": attr.string(values = [])})
STRING_EMPTY_TUPLE = rule(implementation = _impl, attrs = {"x": attr.string(values = ())})
STRING_NORMALIZED = rule(implementation = _impl, attrs = {"x": attr.string(values = ["rust", "cc", "rust"])})
STRING_SAME = rule(implementation = _impl, attrs = {"x": attr.string(values = ("cc", "rust"))})
STRING_DISTINCT = rule(implementation = _impl, attrs = {"x": attr.string(values = ["cc"])})
LABEL_OMITTED = rule(implementation = _impl, attrs = {"x": attr.label()})
LABEL_NONE = rule(implementation = _impl, attrs = {"x": attr.label(allow_files = None)})
LABEL_FALSE = rule(implementation = _impl, attrs = {"x": attr.label(allow_files = False)})
LABEL_TRUE = rule(implementation = _impl, attrs = {"x": attr.label(allow_files = True)})
LABEL_SINGLE = rule(implementation = _impl, attrs = {"x": attr.label(allow_files = None, allow_single_file = True)})
LABEL_PROVIDERS_EMPTY = rule(implementation = _impl, attrs = {"x": attr.label(providers = [])})
LABEL_PROVIDERS_ONE = rule(implementation = _impl, attrs = {"x": attr.label(providers = [RustLtoInfo])})
rust_toolchain = rule(implementation = _impl, attrs = {
    "experimental_use_allocator_libraries_with_mangled_symbols": attr.int(doc = "allocator", values = [-1, 0, 1], default = -1), "experimental_use_cc_common_link": attr.label(default = Label("//rust/settings:cc_common_link"), doc = "cc link"), "extra_exec_rustc_flags": attr.string_list(doc = "exec flags"), "extra_rustc_flags_for_crate_types": attr.string_list_dict(doc = "crate flags"), "global_allocator_library": attr.label(default = Label("//rust/private/cc:global_allocator_library"), doc = "allocator library"), "iso_date": attr.string(doc = "date", default = ""), "linker": attr.label(doc = "linker", cfg = "exec", allow_single_file = True), "linker_preference": attr.string(doc = "preferred", values = ["cc", "rust"]), "linker_type": attr.string(doc = "type", values = ["direct", "indirect"]), "llvm_cov": attr.label(doc = "llvm-cov", cfg = "exec", allow_single_file = True), "llvm_lib": attr.label(doc = "libLLVM", allow_files = True, cfg = "exec"), "llvm_profdata": attr.label(doc = "profdata", allow_single_file = True, cfg = "exec"), "llvm_tools": attr.label(doc = "tools", allow_files = True), "lto": attr.label(providers = [RustLtoInfo], default = Label("//rust/settings:lto"), doc = "lto"), "opt_level": attr.string_dict(doc = "levels", default = {"dbg":"0","fastbuild":"0","opt":"3"}), "per_crate_rustc_flags": attr.string_list(doc = "flags"), "require_explicit_unstable_features": attr.label(default = Label("//rust/settings:require_explicit_unstable_features"), doc = "unstable"), "rust_doc": attr.label(doc = "rustdoc", allow_single_file = True, cfg = "exec", mandatory = True), "rust_objcopy": attr.label(doc = "objcopy", allow_single_file = True, cfg = "exec"), "rust_std": attr.label(doc = "std", mandatory = True), "rustc": attr.label(doc = "rustc", allow_single_file = True, cfg = "exec", mandatory = True), "rustc_lib": attr.label(doc = "rustc libs", cfg = "exec"), "rustfmt": attr.label(doc = "rustfmt", allow_single_file = True, cfg = "exec"), "staticlib_ext": attr.string(doc = "static", mandatory = True), "stdlib_linkflags": attr.string_list(doc = "linkflags", mandatory = True), "strip_level": attr.string_dict(doc = "strip", default = {"dbg":"none","fastbuild":"none","opt":"debuginfo"}), "target_json": attr.string(doc = "json"), "target_triple": attr.string(doc = "triple"), "version": attr.string(doc = "version", default = ""), "_codegen_units": attr.label(default = Label("//rust/settings:codegen_units")), "_experimental_compile_rustdoc_tests": attr.label(default = Label("//rust/settings:experimental_compile_rustdoc_tests")), "_experimental_use_allocator_libraries_with_mangled_symbols_setting": attr.label(default = Label("//rust/settings:experimental_use_allocator_libraries_with_mangled_symbols"), providers = [BuildSettingInfo], doc = "allocator setting"), "_experimental_use_coverage_metadata_files": attr.label(default = Label("//rust/settings:experimental_use_coverage_metadata_files")), "_experimental_use_global_allocator": attr.label(default = Label("//rust/settings:experimental_use_global_allocator"), doc = "allocator"), "_incompatible_do_not_include_data_in_compile_data": attr.label(default = Label("//rust/settings:incompatible_do_not_include_data_in_compile_data"), doc = "data"), "_incompatible_do_not_include_transitive_data_in_compile_inputs": attr.label(default = Label("//rust/settings:incompatible_do_not_include_transitive_data_in_compile_inputs"), doc = "transitive"), "_linker_preference": attr.label(default = Label("//rust/settings:toolchain_linker_preference")), "_no_std": attr.label(default = Label("//rust/settings:no_std")), "_pipelined_compilation": attr.label(default = Label("//rust/settings:pipelined_compilation")), "_rename_first_party_crates": attr.label(default = Label("//rust/settings:rename_first_party_crates")), "_third_party_dir": attr.label(default = Label("//rust/settings:third_party_dir")), "_toolchain_generated_sysroot": attr.label(default = Label("//rust/settings:toolchain_generated_sysroot"), doc = "sysroot"),
}, toolchains = [config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type", mandatory = False)])
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    let snapshot = |rule_name: &str, attribute_name: &str| {
        let rule = module
            .get(rule_name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let schema = rule
            .schema
            .iter()
            .find(|schema| schema.name == attribute_name)
            .unwrap();
        (schema.default.clone(), schema.allowed_values.clone())
    };
    for name in ["OMITTED", "EMPTY_LIST", "EMPTY_TUPLE"] {
        assert_eq!(
            snapshot(name, "x").1,
            AllowedAttributeValues::None,
            "{name}"
        );
    }
    let normalized = snapshot("NORMALIZED", "x");
    assert_eq!(
        normalized.1,
        AllowedAttributeValues::Integer(Arc::from([-1, 0, 1]))
    );
    assert_eq!(snapshot("SAME", "x"), normalized);
    assert_eq!(
        snapshot("DISTINCT", "x").1,
        AllowedAttributeValues::Integer(Arc::from([-1, 1]))
    );
    let bad_default = snapshot("BAD_DEFAULT", "x");
    assert!(matches!(
        bad_default.0,
        Some(CoercedAttributeValue::Integer(2))
    ));
    assert_eq!(
        bad_default.1,
        AllowedAttributeValues::Integer(Arc::from([0, 1]))
    );
    let allocator = snapshot(
        "rust_toolchain",
        "experimental_use_allocator_libraries_with_mangled_symbols",
    );
    assert_eq!(
        allocator.1,
        AllowedAttributeValues::Integer(Arc::from([-1, 0, 1]))
    );
    for name in ["STRING_OMITTED", "STRING_EMPTY_LIST", "STRING_EMPTY_TUPLE"] {
        assert_eq!(
            snapshot(name, "x").1,
            AllowedAttributeValues::None,
            "{name}"
        );
    }
    let strings = snapshot("STRING_NORMALIZED", "x");
    assert_eq!(
        strings.1,
        AllowedAttributeValues::String(Arc::from(["cc".into(), "rust".into()]))
    );
    assert_eq!(snapshot("STRING_SAME", "x"), strings);
    assert_ne!(snapshot("STRING_DISTINCT", "x").1, strings.1);
    assert_eq!(snapshot("rust_toolchain", "linker_preference").1, strings.1);
    assert_eq!(
        snapshot("rust_toolchain", "linker_type").1,
        AllowedAttributeValues::String(Arc::from(["direct".into(), "indirect".into()]))
    );
    let label_snapshot = |rule_name: &str, attribute_name: &str| {
        let rule = module
            .get(rule_name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let schema = rule
            .schema
            .iter()
            .find(|schema| schema.name == attribute_name)
            .unwrap();
        (schema.allow_files, schema.allow_single_file.clone())
    };
    for name in ["LABEL_OMITTED", "LABEL_NONE", "LABEL_FALSE"] {
        assert_eq!(label_snapshot(name, "x"), (false, None), "{name}");
    }
    assert_eq!(label_snapshot("LABEL_TRUE", "x"), (true, None));
    assert_eq!(
        label_snapshot("LABEL_SINGLE", "x"),
        (false, Some(AllowSingleFile::True))
    );
    for name in ["llvm_lib", "llvm_tools"] {
        assert_eq!(label_snapshot("rust_toolchain", name), (true, None));
    }
    assert_eq!(
        label_snapshot("rust_toolchain", "llvm_profdata"),
        (false, Some(AllowSingleFile::True))
    );
    let provider_snapshot = |rule_name: &str, attribute_name: &str| {
        let rule = module
            .get(rule_name)
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        rule.schema
            .iter()
            .find(|schema| schema.name == attribute_name)
            .unwrap()
            .required_providers
            .clone()
    };
    assert!(provider_snapshot("LABEL_OMITTED", "x").is_empty());
    assert!(provider_snapshot("LABEL_PROVIDERS_EMPTY", "x").is_empty());
    let lto = provider_snapshot("LABEL_PROVIDERS_ONE", "x");
    assert_eq!(lto.len(), 1);
    assert_eq!(
        lto[0][0].to_string(),
        "@@rules_rust+//rust/private:toolchain.bzl%RustLtoInfo"
    );
    assert_eq!(provider_snapshot("rust_toolchain", "lto"), lto);
    assert_eq!(
        provider_snapshot(
            "rust_toolchain",
            "_experimental_use_allocator_libraries_with_mangled_symbols_setting"
        )[0][0]
            .to_string(),
        "@@rules_rust+//rust/private:toolchain.bzl%BuildSettingInfo"
    );
    let rust_toolchain = module
        .get("rust_toolchain")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    assert_eq!(rust_toolchain.required_toolchains().len(), 1);
    assert_eq!(
        rust_toolchain.required_toolchains()[0].label().to_string(),
        "@@bazel_tools//tools/cpp:toolchain_type"
    );
    assert!(!rust_toolchain.required_toolchains()[0].mandatory());
    for invalid in ["None", "1", "['1']", "[True]", "{}", "[2147483648]"] {
        let source = format!("X = attr.int(values = {invalid})");
        assert!(
            eval_bzl_with_identity(&source, owner.clone()).is_err(),
            "{invalid}"
        );
    }
    for invalid in ["None", "'cc'", "['cc', 1]", "[True]", "{}"] {
        let source = format!("X = attr.string(values = {invalid})");
        assert!(
            eval_bzl_with_identity(&source, owner.clone()).is_err(),
            "{invalid}"
        );
    }
    for invalid in ["1", "'file'", "['.so']", "{}"] {
        let source = format!("X = attr.label(allow_files = {invalid})");
        assert!(
            eval_bzl_with_identity(&source, owner.clone()).is_err(),
            "{invalid}"
        );
    }
    for conflict in [
        "X = attr.label(allow_files = True, allow_single_file = True)",
        "X = attr.label(allow_files = False, allow_single_file = False)",
    ] {
        assert!(eval_bzl_with_identity(conflict, owner.clone()).is_err());
    }
    for invalid in [
        "None",
        "1",
        "['provider']",
        "[P, Q]",
        "[[P]]",
        "[provider()]",
    ] {
        let source = format!("P=provider()\nQ=provider()\nX=attr.label(providers={invalid})");
        assert!(
            eval_bzl_with_identity(&source, owner.clone()).is_err(),
            "{invalid}"
        );
    }
    assert!(eval_bzl_with_identity("X = config_common.FeatureFlagInfo", owner.clone()).is_err());
    assert!(
        eval_bzl_with_identity(
            "def impl(ctx): pass\nX = repository_rule(impl, attrs = {'x': attr.int(values = [1])})",
            owner.clone()
        )
        .is_err()
    );
    assert!(
        eval_bzl_with_identity(
            "X = tag_class(attrs = {'x': attr.int(values = [1])})",
            owner.clone()
        )
        .is_err()
    );
    assert!(eval_bzl_with_identity("def impl(ctx): pass\nX = repository_rule(impl, attrs = {'x': attr.string(values = ['cc'])})", owner.clone()).is_err());
    assert!(
        eval_bzl_with_identity(
            "X = tag_class(attrs = {'x': attr.string(values = ['cc'])})",
            owner.clone()
        )
        .is_err()
    );
    assert!(eval_bzl_with_identity("def impl(ctx): pass\nX = repository_rule(impl, attrs = {'x': attr.label(allow_files = True)})", owner.clone()).is_err());
    assert!(
        eval_bzl_with_identity(
            "X = tag_class(attrs = {'x': attr.label(allow_files = True)})",
            owner.clone()
        )
        .is_err()
    );
    assert!(eval_bzl_with_identity("P=provider()\ndef impl(ctx): pass\nX=repository_rule(impl, attrs={'x':attr.label(providers=[P])})", owner.clone()).is_err());
    assert!(
        eval_bzl_with_identity(
            "P=provider()\nX=tag_class(attrs={'x':attr.label(providers=[P])})",
            owner
        )
        .is_err()
    );
}

#[tokio::test]
async fn integer_allowed_values_enforce_explicit_and_select_candidates() {
    let defs = b"def _impl(ctx): fail('must stay lazy')\nr=rule(implementation=_impl, attrs={'x':attr.int(default=2, values=[1,-1,0,1])})\n";
    let good: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(':defs.bzl','r')\nr(name='default',visibility=['//visibility:public'])\n",
        ),
        ("defs.bzl", defs),
    ];
    let outcome = load_repository_package_fixture(good, 411).await;
    let package = repository_package_terminal(&outcome);
    let default = package
        .targets
        .iter()
        .find(|target| target.name == "default")
        .unwrap();
    let crate::package::PackageTargetKind::StarlarkRule(rule) = &default.kind else {
        panic!("default target did not retain its Starlark rule")
    };
    let x_schema = rule
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "x")
        .unwrap();
    let x_value = rule
        .values()
        .iter()
        .find(|value| value.declaration_name == "x")
        .unwrap();
    assert_eq!(
        x_schema.allowed_values(),
        &AllowedAttributeValues::Integer(Arc::from([-1, 0, 1]))
    );
    assert!(matches!(
        x_value.value.as_ref(),
        CoercedAttributeValue::Integer(2)
    ));

    for (variant, build) in [
        (415, b"load(':defs.bzl','r')\nr(name='allowed',x=-1,visibility=['//visibility:public'])\n".as_slice()),
        (416, b"load(':defs.bzl','r')\nr(name='selected',x=select({'//conditions:default':0}),visibility=['//visibility:public'])\n".as_slice()),
    ] {
        let files = [("BUILD.bazel", build), ("defs.bzl", defs.as_slice())];
        let outcome = load_repository_package_fixture(&files, variant).await;
        let _ = repository_package_terminal(&outcome);
    }

    for (variant, build) in [
        (412, b"load(':defs.bzl','r')\nr(name='bad',x=2,visibility=['//visibility:public'])\n".as_slice()),
        (413, b"load(':defs.bzl','r')\nr(name='bad',x=select({'//conditions:default':2}),visibility=['//visibility:public'])\n".as_slice()),
        (414, b"load(':defs.bzl','r')\nr(name='bad',x=select({'//:flag':2,'//conditions:default':0}),visibility=['//visibility:public'])\n".as_slice()),
    ] {
        let files = [("BUILD.bazel", build), ("defs.bzl", defs.as_slice())];
        let outcome = load_repository_package_fixture(&files, variant).await;
        assert!(repository_package_error(&outcome).contains("2 is not allowed"));
    }
}

#[tokio::test]
async fn string_allowed_values_enforce_configurable_candidates() {
    let defs = b"def _impl(ctx): fail('must stay lazy')\nr=rule(implementation=_impl, attrs={'x':attr.string(default='bad', values=['one','two','prefix-a','prefix-b'])})\n";
    let omitted = [
        (
            "BUILD.bazel",
            b"load(':defs.bzl','r')\nr(name='default',visibility=['//visibility:public'])\n"
                .as_slice(),
        ),
        ("defs.bzl", defs.as_slice()),
    ];
    let outcome = load_repository_package_fixture(&omitted, 417).await;
    let package = repository_package_terminal(&outcome);
    let crate::package::PackageTargetKind::StarlarkRule(rule) = &package.targets[0].kind else {
        panic!("default target did not retain its Starlark rule")
    };
    let schema = rule
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "x")
        .unwrap();
    let value = rule
        .values()
        .iter()
        .find(|value| value.declaration_name == "x")
        .unwrap();
    assert_eq!(
        schema.allowed_values(),
        &AllowedAttributeValues::String(Arc::from([
            "one".into(),
            "prefix-a".into(),
            "prefix-b".into(),
            "two".into()
        ]))
    );
    assert!(matches!(
        value.value.as_ref(),
        CoercedAttributeValue::String(value) if value == "bad"
    ));

    for (variant, build) in [
        (
            418,
            "r(name='direct',x='one',visibility=['//visibility:public'])",
        ),
        (
            419,
            "r(name='selected',x=select({'//conditions:default':'two'}),visibility=['//visibility:public'])",
        ),
        (
            420,
            "r(name='joined',x='prefix-'+select({'//conditions:default':'a'}),visibility=['//visibility:public'])",
        ),
    ] {
        let build = format!("load(':defs.bzl','r')\n{build}\n");
        let files = [
            ("BUILD.bazel", build.as_bytes()),
            ("defs.bzl", defs.as_slice()),
        ];
        let outcome = load_repository_package_fixture(&files, variant).await;
        let _ = repository_package_terminal(&outcome);
    }

    for (variant, build, invalid) in [
        (
            421,
            "r(name='bad',x='bad',visibility=['//visibility:public'])",
            "bad",
        ),
        (
            422,
            "r(name='bad',x=select({'//:a':'bad','//conditions:default':'one'}),visibility=['//visibility:public'])",
            "bad",
        ),
        (
            423,
            "r(name='bad',x=select({'//:a':'one','//conditions:default':'bad'}),visibility=['//visibility:public'])",
            "bad",
        ),
        (
            424,
            "r(name='bad',x='prefix-'+select({'//conditions:default':'bad'}),visibility=['//visibility:public'])",
            "prefix-bad",
        ),
    ] {
        let build = format!("load(':defs.bzl','r')\n{build}\n");
        let files = [
            ("BUILD.bazel", build.as_bytes()),
            ("defs.bzl", defs.as_slice()),
        ];
        let outcome = load_repository_package_fixture(&files, variant).await;
        assert!(repository_package_error(&outcome).contains(&format!("{invalid} is not allowed")));
    }
}

fn eval_bzl_with_identity(
    source: &str,
    owner: BzlModuleIdentity,
) -> starlark::Result<FrozenModule> {
    let path = owner.workspace_path.to_str().unwrap();
    let ast = AstModule::parse(path, source.to_owned(), &Dialect::Bazel)?;
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: owner.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([owner]),
        fingerprint: [0; 32],
    });
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator.eval_module(ast, &loading_globals())?;
    drop(evaluator);
    Ok(module.freeze()?)
}

#[test]
fn bazel_cc_common_private_bridge_is_bzl_only_owner_checked_and_opaque() {
    let owner = |label: &str| BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: PathBuf::from("/registry-rules-cc/cc/private/cc_internal.bzl"),
        repository_mapping: Arc::from([]),
    };
    let module = eval_bzl_with_identity(
        "INTERNAL=cc_common.internal_DO_NOT_USE()\nHAS_BRIDGE=hasattr(cc_common, 'internal_DO_NOT_USE')\nHAS_INTERNAL_METHOD=hasattr(INTERNAL, 'check_private_api')\n",
        owner("@@rules_cc+//cc/private:cc_internal.bzl"),
    )
    .unwrap();
    assert_eq!(
        module.get("INTERNAL").unwrap().value().get_type(),
        "cc_internal"
    );
    assert_eq!(module.get("HAS_BRIDGE").unwrap().unpack_bool(), Some(true));
    assert_eq!(
        module.get("HAS_INTERNAL_METHOD").unwrap().unpack_bool(),
        Some(false)
    );

    for source in [
        "X=cc_common.internal_DO_NOT_USE(1)",
        "X=cc_common.internal_DO_NOT_USE(value=True)",
    ] {
        assert!(
            eval_bzl_with_identity(source, owner("@@rules_cc+//cc/private:cc_internal.bzl"))
                .is_err(),
            "{source}"
        );
    }
    for (label, diagnostic_label) in [
        (
            "@@//cc/private:cc_internal.bzl",
            "//cc/private:cc_internal.bzl",
        ),
        (
            "@@dep+//cc/private:cc_internal.bzl",
            "@@dep+//cc/private:cc_internal.bzl",
        ),
    ] {
        let error = eval_bzl_with_identity("X=cc_common.internal_DO_NOT_USE()", owner(label))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("file '{diagnostic_label}' cannot use private API")),
            "{error}"
        );
    }

    eval_global(
        "PRESENT=hasattr(cc_common, 'internal_DO_NOT_USE')",
        &loading_globals(),
    )
    .unwrap();
    assert!(
        eval_global("X=cc_common", &build_file_loading_globals())
            .unwrap_err()
            .contains("cc_common")
    );
}

#[test]
fn bazel_cc_common_compiler_sentinel_freezes_exported_wrapper() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private:cc_common.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/private/cc_common.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
native_cc_common = cc_common
EXPORTED_CC_COMMON = struct(
    do_not_use_tools_cpp_compiler_present = native_cc_common.do_not_use_tools_cpp_compiler_present,
)
DIRECT_NONE = native_cc_common.do_not_use_tools_cpp_compiler_present == None
CAPTURED_NONE = EXPORTED_CC_COMMON.do_not_use_tools_cpp_compiler_present == None
PRESENT = hasattr(native_cc_common, "do_not_use_tools_cpp_compiler_present")
UNKNOWN_ABSENT = not hasattr(native_cc_common, "unselected_native_field")
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    for name in ["DIRECT_NONE", "CAPTURED_NONE", "PRESENT", "UNKNOWN_ABSENT"] {
        assert_eq!(
            module.get(name).unwrap().unpack_bool(),
            Some(true),
            "{name}"
        );
    }
    assert!(
        eval_bzl_with_identity(
            "X = cc_common.do_not_use_tools_cpp_compiler_present()",
            owner,
        )
        .is_err()
    );
}

#[test]
fn bazel_empty_header_info_freezes_rules_cc_compilation_context_row() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private:cc_info.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/private/cc_info.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
cc_internal = cc_common.internal_DO_NOT_USE()
FIRST = cc_internal.create_header_info()
ALIAS = FIRST
SECOND = cc_internal.create_header_info()
HEADER_TYPE = type(FIRST)
ALIASES_EQUAL = FIRST == ALIAS
CALLS_DISTINCT = FIRST != SECOND
MODULES_EMPTY = FIRST.header_module == None and FIRST.pic_header_module == None and FIRST.separate_module == None and FIRST.separate_pic_module == None
LISTS_EMPTY = FIRST.modular_public_headers == [] and FIRST.modular_private_headers == [] and FIRST.textual_headers == [] and FIRST.separate_module_headers == []
HAS_ALL_FIELDS = hasattr(FIRST, "header_module") and hasattr(FIRST, "pic_header_module") and hasattr(FIRST, "modular_public_headers") and hasattr(FIRST, "modular_private_headers") and hasattr(FIRST, "textual_headers") and hasattr(FIRST, "separate_module_headers") and hasattr(FIRST, "separate_module") and hasattr(FIRST, "separate_pic_module")
CcCompilationContextInfo = provider(
    "CcCompilationContext",
    fields = {"_header_info": "Internal"},
)
EMPTY_COMPILATION_CONTEXT = CcCompilationContextInfo(_header_info = FIRST)
NESTED_TYPE = type(EMPTY_COMPILATION_CONTEXT._header_info)
NESTED_HEADERS = EMPTY_COMPILATION_CONTEXT._header_info.modular_public_headers
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    assert_eq!(
        module.get("HEADER_TYPE").unwrap().unpack_str(),
        Some("HeaderInfo")
    );
    assert_eq!(
        module.get("NESTED_TYPE").unwrap().unpack_str(),
        Some("HeaderInfo")
    );
    for name in [
        "ALIASES_EQUAL",
        "CALLS_DISTINCT",
        "MODULES_EMPTY",
        "LISTS_EMPTY",
        "HAS_ALL_FIELDS",
    ] {
        assert_eq!(
            module.get(name).unwrap().unpack_bool(),
            Some(true),
            "{name}"
        );
    }
    assert_eq!(
        FrozenListRef::from_value(module.get("NESTED_HEADERS").unwrap().value())
            .unwrap()
            .len(),
        0
    );
    let first = module.get("FIRST").unwrap();
    let alias = module.get("ALIAS").unwrap();
    let second = module.get("SECOND").unwrap();
    assert!(first.value().equals(alias.value()).unwrap());
    assert!(!first.value().equals(second.value()).unwrap());

    for failure in [
        "cc_internal = cc_common.internal_DO_NOT_USE()\nX = cc_internal.create_header_info(1)",
        "cc_internal = cc_common.internal_DO_NOT_USE()\nX = cc_internal.create_header_info(unknown = None)",
        "cc_internal = cc_common.internal_DO_NOT_USE()\nX = cc_internal.create_header_info(header_module = None)",
        "cc_internal = cc_common.internal_DO_NOT_USE()\ncc_internal.create_header_info().modular_public_headers.append('mutable')",
        "cc_internal = cc_common.internal_DO_NOT_USE()\nX = {cc_internal.create_header_info(): True}",
    ] {
        assert!(
            eval_bzl_with_identity(failure, owner.clone()).is_err(),
            "{failure}"
        );
    }
}

#[test]
fn bazel_empty_list_freeze_loads_empty_cc_compilation_outputs() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private/compile:cc_compilation_outputs.bzl")
            .unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/private/compile/cc_compilation_outputs.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
cc_internal = cc_common.internal_DO_NOT_USE()
LtoCompilationContextInfo = provider(fields = {"lto_bitcode_inputs": "bitcode map"})
EMPTY_LTO_COMPILATION_CONTEXT = LtoCompilationContextInfo(lto_bitcode_inputs = {})
CcCompilationOutputsInfo = provider(fields = {
    "objects": "objects", "pic_objects": "pic objects", "temps": "temps",
    "_header_tokens": "tokens", "_module_files": "modules",
    "_lto_compilation_context": "lto", "_gcno_files": "gcno",
    "_pic_gcno_files": "pic gcno", "_dwo_files": "dwo",
    "_pic_dwo_files": "pic dwo", "cpp_module_files": "cpp modules",
    "pic_cpp_module_files": "pic cpp modules",
    "cpp_modules_info_file": "cpp info", "pic_cpp_modules_info_file": "pic cpp info",
})
def wrap_with_check_private_api(symbol):
    def callback():
        return symbol
    return callback
def create_compilation_outputs_internal(objects = [], pic_objects = [], temps = [], header_tokens = [], module_files = [], gcno_files = [], pic_gcno_files = [], dwo_files = [], pic_dwo_files = [], cpp_module_files = [], pic_cpp_module_files = []):
    return CcCompilationOutputsInfo(
        objects = cc_internal.freeze(objects), pic_objects = cc_internal.freeze(pic_objects),
        temps = wrap_with_check_private_api(depset(temps)),
        _header_tokens = cc_internal.freeze(header_tokens), _module_files = cc_internal.freeze(module_files),
        _lto_compilation_context = EMPTY_LTO_COMPILATION_CONTEXT,
        _gcno_files = cc_internal.freeze(gcno_files), _pic_gcno_files = cc_internal.freeze(pic_gcno_files),
        _dwo_files = cc_internal.freeze(dwo_files), _pic_dwo_files = cc_internal.freeze(pic_dwo_files),
        cpp_module_files = cc_internal.freeze(cpp_module_files), pic_cpp_module_files = cc_internal.freeze(pic_cpp_module_files),
        cpp_modules_info_file = None, pic_cpp_modules_info_file = None,
    )
EMPTY = create_compilation_outputs_internal()
LISTS_EMPTY = EMPTY.objects == [] and EMPTY.pic_objects == [] and EMPTY._header_tokens == [] and EMPTY._module_files == [] and EMPTY._gcno_files == [] and EMPTY._pic_gcno_files == [] and EMPTY._dwo_files == [] and EMPTY._pic_dwo_files == [] and EMPTY.cpp_module_files == [] and EMPTY.pic_cpp_module_files == []
SHAPE_OK = type(EMPTY.objects) == "list" and type(EMPTY.temps) == "function" and EMPTY._lto_compilation_context.lto_bitcode_inputs == {} and EMPTY.cpp_modules_info_file == None and EMPTY.pic_cpp_modules_info_file == None
POSITIONAL = cc_internal.freeze([])
NAMED = cc_internal.freeze(value = [])
FROM_FROZEN = cc_internal.freeze(cc_internal.create_header_info().modular_public_headers)
EXTRA_EMPTY = POSITIONAL == [] and NAMED == [] and FROM_FROZEN == []
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    for name in ["LISTS_EMPTY", "SHAPE_OK", "EXTRA_EMPTY"] {
        assert_eq!(
            module.get(name).unwrap().unpack_bool(),
            Some(true),
            "{name}"
        );
    }

    for failure in [
        "X = cc_common.internal_DO_NOT_USE().freeze([1])",
        "X = cc_common.internal_DO_NOT_USE().freeze(())",
        "X = cc_common.internal_DO_NOT_USE().freeze({})",
        "X = cc_common.internal_DO_NOT_USE().freeze(1)",
        "X = cc_common.internal_DO_NOT_USE().freeze()",
        "X = cc_common.internal_DO_NOT_USE().freeze([], [])",
        "cc_common.internal_DO_NOT_USE().freeze([]).append(1)",
    ] {
        assert!(
            eval_bzl_with_identity(failure, owner.clone()).is_err(),
            "{failure}"
        );
    }
}

#[test]
fn bazel_initialized_provider_loads_rules_cc_artifact_categories_and_stays_separate() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/common:cc_helper_internal.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/common/cc_helper_internal.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
events = []
def _artifact_category_info_init(name, default_prefix, *extensions):
    events.append(name)
    return {
        "allowed_extensions": extensions,
        "default_extension": extensions[0],
        "default_prefix": default_prefix,
        "name": name,
    }
ArtifactCategoryInfo, _new_aci = provider(
    """A category of artifacts that are candidate input/output to an action.""",
    fields = ["name", "default_prefix", "default_extension", "allowed_extensions"],
    init = _artifact_category_info_init,
)
STATIC = ArtifactCategoryInfo("STATIC_LIBRARY", "lib", ".a", ".lib")
OMITTED = _new_aci(name = "OMITTED")
CATEGORIES = [STATIC, OMITTED]
NAMES = struct(**{category.name: category.name for category in CATEGORIES})
STATIC_NAME = STATIC.name
STATIC_DEFAULT = STATIC.default_extension
STATIC_SECOND = STATIC.allowed_extensions[1]
HAS_OMITTED_PREFIX = hasattr(OMITTED, "default_prefix")
def _failing_init(name):
    fail("initializer must be bypassed")
FailingInfo, _new_failing = provider("Failing", fields = ["name"], init = _failing_init)
BYPASSED = _new_failing(name = "raw")
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    assert_eq!(
        module.get("STATIC_NAME").unwrap().unpack_str(),
        Some("STATIC_LIBRARY")
    );
    assert_eq!(
        module.get("STATIC_DEFAULT").unwrap().unpack_str(),
        Some(".a")
    );
    assert_eq!(
        module.get("STATIC_SECOND").unwrap().unpack_str(),
        Some(".lib")
    );
    assert_eq!(
        module.get("HAS_OMITTED_PREFIX").unwrap().unpack_bool(),
        Some(false)
    );
    let names_value = module.get("NAMES").unwrap();
    let names = StructRef::from_value(names_value.value()).unwrap();
    let names = names
        .iter()
        .map(|(name, value)| (name.as_str(), value.unpack_str().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [("STATIC_LIBRARY", "STATIC_LIBRARY"), ("OMITTED", "OMITTED")]
    );
    let events = FrozenListRef::from_value(module.get("events").unwrap().value()).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].to_value().unpack_str(), Some("STATIC_LIBRARY"));

    let normal = module.get("STATIC").unwrap();
    let raw = module.get("OMITTED").unwrap();
    let normal = normal.value();
    let raw = raw.value();
    assert_eq!(loading_provider_id(normal), loading_provider_id(raw));
    assert!(StarlarkUserProvider::from_value(normal).is_none());
    assert!(StarlarkUserProvider::from_value(raw).is_none());

    let failures = [
        "def init(): return {}\nX = provider('doc', fields = None, init = init)",
        "X = provider('doc', fields = ['x'], init = 1)",
        "def init(): return 'bad'\nInfo, raw = provider('doc', fields = ['x'], init = init)\nX = Info()",
        "def init(): return {1: 'bad'}\nInfo, raw = provider('doc', fields = ['x'], init = init)\nX = Info()",
        "def init(): return {'other': 'bad'}\nInfo, raw = provider('doc', fields = ['x'], init = init)\nX = Info()",
        "def init(): return {'x': 'ok'}\nInfo, raw = provider('doc', fields = ['x'], init = init)\nX = raw('bad')",
    ];
    for failure in failures {
        assert!(
            eval_bzl_with_identity(failure, owner.clone()).is_err(),
            "{failure}"
        );
    }
}

#[test]
fn bazel_documented_initialized_providers_freeze_cc_info_and_launcher_declarations() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private:cc_info.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/private/cc_info.bzl"),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
events = []
def _create_cc_info(*, compilation_context = None, linking_context = None, debug_context = None, cc_native_library_info = None):
    events.append("cc")
    return dict(
        compilation_context = compilation_context,
        linking_context = linking_context,
        _debug_context = debug_context,
        _legacy_transitive_native_libraries = cc_native_library_info,
    )
CcInfo, _new_cc_info = provider(
    doc = "Provider for C++ compilation and linking information.",
    fields = {
        "compilation_context": "A CcCompilationContext.",
        "linking_context": "A CcLinkingContext.",
        "_debug_context": "A CcDebugInfoContext.",
        "_legacy_transitive_native_libraries": "A CcNativeLibraryInfo.",
    },
    init = _create_cc_info,
)
INFO = CcInfo(compilation_context = struct(marker = "context"))
INFO_MARKER = INFO.compilation_context.marker
def _cc_launcher_info_constructor(cc_info, compilation_outputs):
    events.append("launcher")
    return dict(cc_info = cc_info, compilation_outputs = compilation_outputs)
CcLauncherInfo, _new_launcher_info = provider(
    doc = "Provider for a C++ launcher.",
    fields = {
        "cc_info": "The CcInfo provider of the launcher.",
        "compilation_outputs": "The compilation outputs.",
    },
    init = _cc_launcher_info_constructor,
)
LAUNCHER = CcLauncherInfo(INFO, ["output"])
LAUNCHER_OUTPUT = LAUNCHER.compilation_outputs[0]
RAW = _new_launcher_info(compilation_outputs = ["raw"])
RAW_OUTPUT = RAW.compilation_outputs[0]
RAW_OMITS_CC_INFO = not hasattr(RAW, "cc_info")
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    assert_eq!(
        module.get("INFO_MARKER").unwrap().unpack_str(),
        Some("context")
    );
    assert_eq!(
        module.get("LAUNCHER_OUTPUT").unwrap().unpack_str(),
        Some("output")
    );
    assert_eq!(module.get("RAW_OUTPUT").unwrap().unpack_str(), Some("raw"));
    assert_eq!(
        module.get("RAW_OMITS_CC_INFO").unwrap().unpack_bool(),
        Some(true)
    );
    let events = FrozenListRef::from_value(module.get("events").unwrap().value()).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].to_value().unpack_str(), Some("cc"));
    assert_eq!(events[1].to_value().unpack_str(), Some("launcher"));
    let launcher = module.get("LAUNCHER").unwrap();
    let raw = module.get("RAW").unwrap();
    assert_eq!(
        loading_provider_id(launcher.value()),
        loading_provider_id(raw.value())
    );
    assert!(StarlarkUserProvider::from_value(launcher.value()).is_none());

    let failures = [
        "def init(): return {}\nInfo, raw = provider('doc', fields = {1: 'doc'}, init = init)",
        "def init(): return {}\nInfo, raw = provider('doc', fields = {'x': 1}, init = init)",
        "def init(): return {'other': 1}\nInfo, raw = provider('doc', fields = {'x': 'doc'}, init = init)\nX = Info()",
        "def init(): return {}\nInfo, raw = provider('doc', fields = {'x': 'doc'}, init = init)\nX = raw(other = 1)",
        "def init(): return {}\nInfo, raw = provider('doc', fields = {'x': 'doc'}, init = init)\nX = raw(1)",
    ];
    for failure in failures {
        assert!(
            eval_bzl_with_identity(failure, owner.clone()).is_err(),
            "{failure}"
        );
    }
}

#[test]
fn bazel_provider_schemas_freeze_rules_cc_extra_library_child_and_stay_loading_only() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse(
            "@@rules_cc+//cc/private/link:create_extra_link_time_library.bzl",
        )
        .unwrap(),
        workspace_path: PathBuf::from(
            "/rules_cc/cc/private/link/create_extra_link_time_library.bzl",
        ),
        repository_mapping: Arc::from([]),
    };
    let source = r#"
ExtraLinkTimeLibraryInfo = provider("ExtraLinkTimeLibraryInfo")
ExtraLibraryInfo = provider("The result of building extra link-time libraries.")
_KeyInfo = provider(
    "_KeyInfo",
    fields = ["build_library_func", "constant_fields", "depset_fields"],
)
ExtraLinkTimeLibrariesInfo = provider(
    "ExtraLinkTimeLibrariesInfo",
    fields = {"libraries": "A list of extra libraries."},
)
_EMPTY = ExtraLinkTimeLibrariesInfo(libraries = [])
EMPTY_LIBRARIES = _EMPTY.libraries
FREE = ExtraLinkTimeLibraryInfo(payload = struct(marker = "free"))
FREE_MARKER = FREE.payload.marker
ExplicitNoneInfo = provider(fields = None)
EXPLICIT_NONE = ExplicitNoneInfo(other = ["none"])
NONE_FIRST = EXPLICIT_NONE.other[0]
KEY = _KeyInfo(build_library_func = struct(name = "key"))
KEY_NAME = KEY.build_library_func.name
KEY_OMITS_CONSTANTS = not hasattr(KEY, "constant_fields")
EmptyInfo = provider(fields = [])
EMPTY_OK = EmptyInfo()
StringInfo = provider(fields = {"value": "A string."})
CONFIGURED = StringInfo(value = "configured")
LOADING = StringInfo(value = ["loading"])
LOADING_FIRST = LOADING.value[0]
OPTIONAL = ExtraLinkTimeLibrariesInfo()
OPTIONAL_OMITS_LIBRARIES = not hasattr(OPTIONAL, "libraries")
"#;
    let module = eval_bzl_with_identity(source, owner.clone()).unwrap();
    assert_eq!(
        module.get("FREE_MARKER").unwrap().unpack_str(),
        Some("free")
    );
    assert_eq!(module.get("NONE_FIRST").unwrap().unpack_str(), Some("none"));
    assert_eq!(module.get("KEY_NAME").unwrap().unpack_str(), Some("key"));
    assert_eq!(
        module.get("KEY_OMITS_CONSTANTS").unwrap().unpack_bool(),
        Some(true)
    );
    assert_eq!(
        module.get("LOADING_FIRST").unwrap().unpack_str(),
        Some("loading")
    );
    assert_eq!(
        module
            .get("OPTIONAL_OMITS_LIBRARIES")
            .unwrap()
            .unpack_bool(),
        Some(true)
    );
    let libraries = module.get("EMPTY_LIBRARIES").unwrap();
    let libraries = libraries.value();
    assert_eq!(FrozenListRef::from_value(libraries).unwrap().len(), 0);
    assert!(
        FrozenUserProviderCallable::from_value(
            module.get("ExtraLinkTimeLibraryInfo").unwrap().value()
        )
        .is_some()
    );

    let configured = module.get("CONFIGURED").unwrap();
    let configured = configured.value();
    let configured = StarlarkUserProvider::from_value(configured).unwrap();
    let loading = module.get("LOADING").unwrap();
    let loading = loading.value();
    assert_eq!(loading_provider_id(loading), Some(configured.id().dupe()));
    assert!(StarlarkUserProvider::from_value(loading).is_none());
    assert!(StarlarkUserProvider::from_value(module.get("KEY").unwrap().value()).is_none());

    let failures = [
        "Info = provider(fields = ['same', 'same'])",
        "Info = provider(fields = ['name', 1])",
        "Info = provider(fields = ('name',))",
        "Info = provider(fields = [])\nX = Info(name = 1)",
        "Info = provider(fields = ['name'])\nX = Info(other = 1)",
        "Info = provider(fields = ['name'])\nX = Info(1)",
        "Info = provider()\nX = Info(1)",
    ];
    for failure in failures {
        assert!(
            eval_bzl_with_identity(failure, owner.clone()).is_err(),
            "{failure}"
        );
    }
}

#[test]
fn label_attribute_defaults_keep_defining_module_identity() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@dep+//rust/private:lint_test.bzl").unwrap(),
        workspace_path: PathBuf::from("/registry-dep/rust/private/lint_test.bzl"),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("bazel_tools").unwrap(),
            CanonicalRepoName::new("bazel_tools").unwrap(),
        )]),
    };
    let source = "def _impl(ctx): fail('implementation must stay lazy')\ndef _platform_transition_impl(settings, attr): return {'//command_line_option:platforms': attr.platform}\nplatform_transition = transition(implementation = _platform_transition_impl, inputs = [], outputs = ['//command_line_option:platforms'])\nLINT_TEST_COMMON_ATTRS = {'platform': attr.label(doc = 'platform'), 'transitive': attr.bool(doc = 'transitive', default = False), '_allowlist_function_transition': attr.label(default = '@bazel_tools//tools/allowlists/function_transition_allowlist'), '_runner': attr.label(doc = 'runner', cfg = 'exec', executable = True, default = Label('//rust/private/lint_test_runner'))}\nLINT_TEST_RULE = rule(implementation = _impl, attrs = LINT_TEST_COMMON_ATTRS)\n";
    let module = eval_bzl_with_identity(source, owner).unwrap();
    let rule = module
        .get("LINT_TEST_RULE")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let attr = |name| {
        rule.schema
            .iter()
            .find(|schema| schema.name == name)
            .unwrap()
    };
    assert!(matches!(attr("platform").default, None));
    assert!(matches!(
        attr("transitive").default,
        Some(CoercedAttributeValue::Boolean(false))
    ));
    assert!(
        matches!(attr("_allowlist_function_transition").default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist")
    );
    let runner = attr("_runner");
    assert!(runner.executable && runner.exec_configuration && runner.transition.is_none());
    assert!(
        matches!(runner.default.as_ref(), Some(CoercedAttributeValue::Label(label)) if label.to_string() == "@@dep+//rust/private/lint_test_runner:lint_test_runner")
    );
}

#[test]
fn label_attribute_default_rejects_unadmitted_apparent_mapping() {
    let evaluate = |mapping| {
        let owner = BzlModuleIdentity {
            label: CanonicalLabel::parse("@@dep+//:defs.bzl").unwrap(),
            workspace_path: PathBuf::from("/registry-dep/defs.bzl"),
            repository_mapping: mapping,
        };
        eval_bzl_with_identity(
            "def impl(ctx): return None\nR=rule(implementation=impl, attrs={'x': attr.label(default='@alias//:x')})",
            owner,
        )
        .unwrap_err()
        .to_string()
    };
    assert!(evaluate(Arc::from([])).contains("not visible"));
    let alias = ApparentRepoName::new("alias").unwrap();
    let conflict = Arc::from([
        (alias.clone(), CanonicalRepoName::new("one+").unwrap()),
        (alias, CanonicalRepoName::new("two+").unwrap()),
    ]);
    assert!(evaluate(conflict).contains("ambiguous"));
}

#[test]
fn bazel_aspect_definition_validates_admitted_fixed_abi_and_build_absence() {
    eval_global(
        "def impl(target, ctx): return []\nNAMED=aspect(implementation=impl)\nPOSITIONAL=aspect(impl, attr_aspects=[])\nNESTED=[aspect(implementation=impl)]",
        &loading_globals(),
    )
    .unwrap();
    for source in [
        "A=aspect(implementation=print)",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, attr_aspects=['*', 'deps'])",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, doc=1)",
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, provides=[])",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
    let error = eval_global(
        "def impl(target, ctx): return []\nA=aspect(implementation=impl)",
        &build_file_loading_globals(),
    )
    .unwrap_err();
    assert!(error.contains("aspect"), "{error}");
    eval_global(
        "def impl(target, ctx): return []\nA=aspect(implementation=impl, toolchains=[str(Label('//rust:toolchain_type'))])",
        &loading_globals(),
    )
    .unwrap();
}

#[test]
fn recursive_bzl_label_uses_top_level_and_imported_function_owners() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@dep+//owner:support.bzl").unwrap(),
        workspace_path: PathBuf::from("/workspace/dep/owner/support.bzl"),
        repository_mapping: Arc::from([]),
    };
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: owner.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([owner.clone()]),
        fingerprint: [0; 32],
    });
    let ast = AstModule::parse(
        "/workspace/dep/owner/support.bzl",
        "LABEL_ALIAS = Label\ndef owned_label(): return str(Label(':owned'))\n".to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let support = Module::new();
    let mut evaluator = Evaluator::new(&support);
    evaluator.extra = Some(&context);
    evaluator.eval_module(ast, &loading_globals()).unwrap();
    drop(evaluator);
    let support = support.freeze().unwrap();

    let root = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@dep+//caller:root.bzl").unwrap(),
        workspace_path: PathBuf::from("/workspace/dep/caller/root.bzl"),
        repository_mapping: Arc::from([]),
    };
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: root.clone(),
        direct_children: Arc::from([owner.clone()]),
        reachable: Arc::from([root, owner]),
        fingerprint: [0; 32],
    });
    let ast = AstModule::parse(
        "/workspace/dep/caller/root.bzl",
        "load('//owner:support.bzl', 'LABEL_ALIAS', 'owned_label')\ndef _impl(target, ctx): return []\ndef wrapped():\n  result = owned_label()\n  return result\nTOP = str(Label(':top'))\nALIASED = str(LABEL_ALIAS(':alias'))\nIMPORTED = wrapped()\nIDEMPOTENT = Label(Label(':same')) == Label(':same')\nRUST_ANALYZER = aspect(implementation = _impl, attr_aspects = ['srcs', 'deps', 'proc_macro_deps', 'crate', 'actual', 'proto'], toolchains = [str(Label('//rust:toolchain_type'))], doc = 'Rust analyzer')\n".to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let module = Module::new();
    let loader = LocalBzlLoader {
        modules: vec![("//owner:support.bzl", support)],
    };
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator.set_loader(&loader);
    evaluator.eval_module(ast, &loading_globals()).unwrap();
    drop(evaluator);
    let module = module.freeze().unwrap();
    assert_eq!(
        module.get("TOP").unwrap().unpack_str(),
        Some("@@dep+//caller:top")
    );
    assert_eq!(
        module.get("ALIASED").unwrap().unpack_str(),
        Some("@@dep+//caller:alias")
    );
    assert_eq!(
        module.get("IMPORTED").unwrap().unpack_str(),
        Some("@@dep+//owner:owned")
    );
    assert_eq!(module.get("IDEMPOTENT").unwrap().unpack_bool(), Some(true));
    let aspect = module
        .get("RUST_ANALYZER")
        .unwrap()
        .downcast::<FrozenAspectDefinition>()
        .unwrap();
    assert_mandatory_aspect_toolchain(&aspect, "@@dep+//rust:toolchain_type");
}

#[test]
fn bazel_label_rejects_unadmitted_inputs_and_missing_function_provenance() {
    for source in [
        "X = Label('bare')",
        "X = Label('@repo//pkg:target')",
        "X = Label('@@repo//pkg:target')",
        "X = Label(1)",
        "def impl(ctx): return None\nR = rule(implementation = impl, toolchains = ['@repo//pkg:target'])",
    ] {
        assert!(eval_global(source, &loading_globals()).is_err(), "{source}");
    }
    let error = eval_global(
        "def make_label():\n  x = Label(':owned')\n  return x\nX = make_label()",
        &loading_globals(),
    )
    .unwrap_err();
    assert!(error.contains("recursive Bzl manifest"), "{error}");

    let conflicting = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@owner+//:defs.bzl").unwrap(),
        workspace_path: PathBuf::from("/workspace/owner/defs.bzl"),
        repository_mapping: Arc::from([
            (
                ApparentRepoName::new("alias").unwrap(),
                CanonicalRepoName::new("first+").unwrap(),
            ),
            (
                ApparentRepoName::new("alias").unwrap(),
                CanonicalRepoName::new("second+").unwrap(),
            ),
        ]),
    };
    let mut changed = conflicting.clone();
    changed.repository_mapping = Arc::from([(
        ApparentRepoName::new("alias").unwrap(),
        CanonicalRepoName::new("first+").unwrap(),
    )]);
    let hash = |identity: &BzlModuleIdentity| {
        let mut hasher = Sha256::new();
        super::fingerprint_identity(&mut hasher, identity);
        hasher.finalize()
    };
    assert_ne!(conflicting, changed);
    assert_ne!(hash(&conflicting), hash(&changed));
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: conflicting.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([conflicting]),
        fingerprint: [0; 32],
    });
    let ast = AstModule::parse(
        "/workspace/owner/defs.bzl",
        "X = Label('@alias//:target')".to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    let error = evaluator.eval_module(ast, &loading_globals()).unwrap_err();
    assert!(error.to_string().contains("ambiguous"), "{error}");

    let ast = AstModule::parse(
        "/workspace/owner/defs.bzl",
        "def impl(ctx): return None\nR = rule(implementation = impl, toolchains = ['@alias//:target'])".to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    let error = evaluator.eval_module(ast, &loading_globals()).unwrap_err();
    assert!(error.to_string().contains("ambiguous"), "{error}");
}

#[test]
fn bazel_config_common_toolchain_type_retains_typed_rule_requirements() {
    let owner = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@owner+//pkg:defs.bzl").unwrap(),
        workspace_path: PathBuf::from("/workspace/owner/pkg/defs.bzl"),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("mapped+").unwrap(),
        )]),
    };
    let module = eval_bzl_with_identity(
        r#"
def impl(ctx): return []
DEFAULT = config_common.toolchain_type(":default")
EXPLICIT = config_common.toolchain_type(Label("@alias//tools:explicit"), mandatory = True)
OPTIONAL = config_common.toolchain_type("//tools:optional", mandatory = False)
DEFAULT_MANDATORY = DEFAULT.mandatory
OPTIONAL_MANDATORY = OPTIONAL.mandatory
OPTIONAL_LABEL = str(OPTIONAL.toolchain_type)
R = rule(implementation = impl, toolchains = [":plain", Label("//tools:label"), EXPLICIT, OPTIONAL])
"#,
        owner.clone(),
    )
    .unwrap();
    assert_eq!(
        module.get("DEFAULT_MANDATORY").unwrap().unpack_bool(),
        Some(true)
    );
    assert_eq!(
        module.get("OPTIONAL_MANDATORY").unwrap().unpack_bool(),
        Some(false)
    );
    assert_eq!(
        module.get("OPTIONAL_LABEL").unwrap().unpack_str(),
        Some("@@owner+//tools:optional")
    );
    let rule = module
        .get("R")
        .unwrap()
        .downcast::<FrozenRuleDefinition>()
        .unwrap();
    let requirements = rule.required_toolchains();
    assert_eq!(requirements.len(), 4);
    assert_eq!(requirements[0].label().to_string(), "@@owner+//pkg:plain");
    assert_eq!(requirements[1].label().to_string(), "@@owner+//tools:label");
    assert_eq!(
        requirements[2].label().to_string(),
        "@@mapped+//tools:explicit"
    );
    assert_eq!(
        requirements[3].label().to_string(),
        "@@owner+//tools:optional"
    );
    assert!(requirements[..3].iter().all(|value| value.mandatory()));
    assert!(!requirements[3].mandatory());

    for source in [
        "X = config_common.toolchain_type(None)",
        "X = config_common.toolchain_type(1)",
        "X = config_common.toolchain_type([], mandatory = False)",
        "def impl(ctx): return []\nR = rule(implementation = impl, toolchains = [None])",
        "def impl(ctx): return []\nR = rule(implementation = impl, toolchains = [\"//:same\", Label(\"//:same\")])",
    ] {
        assert!(
            eval_bzl_with_identity(source, owner.clone()).is_err(),
            "{source}"
        );
    }
    assert!(eval_global("X = config_common", &build_file_loading_globals()).is_err());
}

#[tokio::test]
async fn repository_package_rejects_optional_toolchain_before_recording() {
    let files: &[(&str, &[u8])] = &[
        ("BUILD.bazel", b"load(':defs.bzl','probe')\nprobe(name='blocked')\n"),
        (
            "defs.bzl",
            b"def impl(ctx): return []\nprobe=rule(implementation=impl, toolchains=[config_common.toolchain_type('//tools:type', mandatory=False)])\n",
        ),
    ];
    let outcome = load_repository_package_fixture(files, 427).await;
    let error = repository_package_error(&outcome);
    assert!(
        error.contains("optional rule toolchain requirements"),
        "{error}"
    );
}

#[tokio::test]
async fn repository_package_rejects_reexported_label_builtin() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"LABEL_ALIAS\")\nBLOCKED = LABEL_ALIAS(\":target\")\n",
        ),
        ("defs.bzl", b"LABEL_ALIAS = Label\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 403).build(),
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
    assert!(repository_package_error(&outcome).contains("Label() may only be called in a .bzl"));
}

#[tokio::test]
async fn repository_package_rejects_bzl_aspect_factory_in_build_context() {
    let files: &[(&str, &[u8])] = &[
        ("BUILD.bazel", b"load(\":defs.bzl\", \"make_aspect\")\nBLOCKED = make_aspect()\n"),
        ("defs.bzl", b"def _impl(target, ctx): return []\ndef make_aspect(): return aspect(implementation = _impl)\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(
        &dice,
        EpochBuilder::external_sources(files, 401).build(),
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
    assert!(repository_package_error(&outcome).contains("aspect may only be called in a .bzl"));
}

fn eval_global(source: &str, globals: &Globals) -> Result<(), String> {
    let ast = AstModule::parse("BUILD.bazel", source.to_owned(), &Dialect::Bazel).unwrap();
    let module = Module::new();
    let context = BzlEvaluationContext::new("//:defs.bzl");
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    evaluator
        .eval_module(ast, globals)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[test]
fn bazel_config_typed_descriptors_are_bzl_only_and_require_supported_flags() {
    let bzl = loading_globals();
    eval_global(
        "IT=config.int(flag=True)\nIO=config.int()\nIF=config.int(flag=False)\nBT=config.bool(flag=True)\nBO=config.bool()\nBF=config.bool(flag=False)",
        &bzl,
    )
    .unwrap();
    for source in [
        "X=config.int(True)",
        "X=config.int(flag=None)",
        "X=config.int(flag=1)",
        "X=config.int(unknown=True)",
    ] {
        assert!(eval_global(source, &bzl).is_err(), "{source}");
    }
    for source in [
        "X=config.bool(True)",
        "X=config.bool(flag=None)",
        "X=config.bool(flag=1)",
        "X=config.bool(unknown=True)",
    ] {
        assert!(eval_global(source, &bzl).is_err(), "{source}");
    }
    eval_global(
        "ST=config.string(flag=True)\nSO=config.string()\nSF=config.string(flag=False, allow_multiple=False)\nSM=config.string(flag=True, allow_multiple=True)\nOM=config.string(allow_multiple=True)",
        &bzl,
    )
    .unwrap();
    for source in [
        "X=config.string(True)",
        "X=config.string(flag=None)",
        "X=config.string(flag=1)",
        "X=config.string(allow_multiple=None)",
        "X=config.string(allow_multiple=1)",
        "X=config.string(unknown=True)",
    ] {
        assert!(eval_global(source, &bzl).is_err(), "{source}");
    }
    for source in [
        "X=config.string_list(True)",
        "X=config.string_list(flag=None)",
        "X=config.string_list(flag=1)",
        "X=config.string_list(repeatable=None)",
        "X=config.string_list(repeatable=1)",
        "X=config.string_list(unknown=True)",
    ] {
        assert!(eval_global(source, &bzl).is_err(), "{source}");
    }
    eval_global(
        "L=config.string_list(flag=True)\nE=config.string_list(flag=True, repeatable=False)\nR=config.string_list(flag=True, repeatable=True)\nO=config.string_list()\nF=config.string_list(flag=False)\nFF=config.string_list(flag=False, repeatable=False)",
        &bzl,
    )
    .unwrap();
    for source in [
        "X=config.string_list(repeatable=True)",
        "X=config.string_list(flag=False, repeatable=True)",
    ] {
        let error = eval_global(source, &bzl).unwrap_err();
        assert!(
            error.contains("'repeatable' can only be set for a setting with 'flag = True'"),
            "{error}"
        );
    }
    let build = build_file_loading_globals();
    eval_global("S=config.string(flag=True)", &build).unwrap();
    for source in [
        "S=config.string()",
        "S=config.string(flag=True, allow_multiple=True)",
        "S=config.string(flag=True, unknown=True)",
    ] {
        assert!(eval_global(source, &build).is_err(), "{source}");
    }
    for (source, missing) in [
        ("S=config.string(flag=True)\nI=config.int()", "int"),
        (
            "S=config.string(flag=True)\nB=config.bool(flag=True)",
            "bool",
        ),
        (
            "S=config.string(flag=True)\nL=config.string_list(flag=True)",
            "string_list",
        ),
    ] {
        let error = eval_global(source, &build).unwrap_err();
        assert!(error.contains(missing), "{error}");
    }
}

#[tokio::test]
async fn repository_package_rejects_unsupported_config_string_rules_before_recording() {
    for (facts, descriptor) in [
        (3974, "config.string()"),
        (3975, "config.string(flag=True, allow_multiple=True)"),
        (3976, "config.string(allow_multiple=True)"),
    ] {
        let defs = format!(
            "def _impl(ctx): fail('string implementation must stay lazy')\nstring_rule=rule(implementation=_impl, build_setting={descriptor})\n"
        );
        let files: &[(&str, &[u8])] = &[
            (
                "BUILD.bazel",
                b"load(':defs.bzl', 'string_rule')\nstring_rule(name='blocked', build_setting_default='value')\n",
            ),
            ("defs.bzl", defs.as_bytes()),
        ];
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(files, facts).build(),
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
        assert!(repository_package_error(&outcome).contains(
            "non-flag or allow-multiple string build setting rule invocation is not supported"
        ));
    }
}

#[tokio::test]
async fn repository_package_rejects_config_int_rules_before_target_recording() {
    for (facts, descriptor) in [(3972, "config.int(flag=True)"), (3973, "config.int()")] {
        let defs = format!(
            "def _impl(ctx): fail('integer implementation must stay lazy')\nint_rule=rule(implementation=_impl, build_setting={descriptor})\n"
        );
        let files: &[(&str, &[u8])] = &[
            (
                "BUILD.bazel",
                b"load(':defs.bzl', 'int_rule')\nint_rule(name='blocked', build_setting_default=1)\n",
            ),
            ("defs.bzl", defs.as_bytes()),
        ];
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(files, facts).build(),
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
            repository_package_error(&outcome)
                .contains("integer build setting rule invocation is not supported")
        );
    }
}

#[tokio::test]
async fn repository_package_rejects_config_bool_rule_before_target_recording() {
    for (facts, descriptor) in [(398, "config.bool(flag=True)"), (399, "config.bool()")] {
        let defs = format!(
            "def _impl(ctx): fail('boolean implementation must stay lazy')\nbool_rule=rule(implementation=_impl, build_setting={descriptor})\n"
        );
        let files: &[(&str, &[u8])] = &[
            (
                "BUILD.bazel",
                b"load(':defs.bzl', 'bool_rule')\nbool_rule(name='blocked', build_setting_default=True)\n",
            ),
            ("defs.bzl", defs.as_bytes()),
        ];
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(files, facts).build(),
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
            repository_package_error(&outcome)
                .contains("boolean build setting rule invocation is not supported")
        );
    }
}

#[tokio::test]
async fn repository_package_rejects_config_string_list_rule_before_target_recording() {
    for (facts, descriptor) in [
        (400, "config.string_list(flag=True, repeatable=True)"),
        (401, "config.string_list()"),
    ] {
        let defs = format!(
            "def _impl(ctx): fail('list implementation must stay lazy')\nlist_rule=rule(implementation=_impl, build_setting={descriptor})\n"
        );
        let files: &[(&str, &[u8])] = &[
            (
                "BUILD.bazel",
                b"load(':defs.bzl', 'list_rule')\nlist_rule(name='blocked', build_setting_default=['value'])\n",
            ),
            ("defs.bzl", defs.as_bytes()),
        ];
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(files, facts).build(),
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
            repository_package_error(&outcome)
                .contains("string-list build setting rule invocation is not supported")
        );
    }
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
