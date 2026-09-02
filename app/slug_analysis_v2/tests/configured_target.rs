/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use slug_analysis_v2::AnalysisDiagnostic;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionExecutionState as State;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredActionPlatformConstraint;
use slug_analysis_v2::ConfiguredActionToolchainContext;
use slug_analysis_v2::ConfiguredAttributeDependency;
use slug_analysis_v2::ConfiguredEdge;
use slug_analysis_v2::ConfiguredEdgeKind;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::ConfiguredToolchainContextRow;
use slug_analysis_v2::ConfiguredToolchainSelection;
use slug_analysis_v2::DiagnosticSeverity;
use slug_analysis_v2::PlatformSemanticFact;
use slug_analysis_v2::ToolchainTopology;
use slug_analysis_v2::key::StarlarkOption;
use slug_analysis_v2::key::StarlarkOptionScope;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ParamFile;
use slug_build_api_v2::ParamFileFormat;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::TestRuleKind;
use slug_workspace_v2::NormalizedAbsolutePath;

fn no_runfiles_packages() -> RunfilesPackageDepset {
    RunfilesPackageDepset::empty()
}

#[allow(dead_code)]
enum PreviousConfiguredEdgeKind {
    TransitionedAttribute {
        attribute: CompactString,
        index: u32,
        output: CanonicalLabel,
    },
    Source,
}

#[test]
fn configured_edge_layout_growth_is_measured() {
    assert_eq!(
        (
            size_of::<PreviousConfiguredEdgeKind>(),
            size_of::<ConfiguredEdgeKind>(),
            size_of::<ConfiguredAttributeDependency>(),
        ),
        (128, 72, 40),
    );
    assert!(size_of::<ConfiguredEdgeKind>() < size_of::<PreviousConfiguredEdgeKind>());
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct ContextInputKey;

impl fmt::Display for ContextInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test-toolchain-context-input")
    }
}

#[async_trait]
impl Key for ContextInputKey {
    type Value = Arc<ConfiguredActionToolchainContext>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        panic!("ContextInputKey is injected")
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Allocative)]
struct ContextParentKey {
    #[allocative(skip)]
    evaluations: Arc<AtomicUsize>,
}

impl PartialEq for ContextParentKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.evaluations, &other.evaluations)
    }
}

impl Eq for ContextParentKey {}

impl Hash for ContextParentKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.evaluations).hash(state);
    }
}

impl fmt::Display for ContextParentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test-toolchain-context-parent")
    }
}

#[async_trait]
impl Key for ContextParentKey {
    type Value = Arc<ConfiguredActionToolchainContext>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        ctx.compute(&ContextInputKey).await.unwrap()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct PublicationInputKey;

impl fmt::Display for PublicationInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test-action-provider-publication-input")
    }
}

#[async_trait]
impl Key for PublicationInputKey {
    type Value = Arc<ConfiguredNodeResult>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        panic!("PublicationInputKey is injected")
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Allocative)]
struct PublicationParentKey {
    #[allocative(skip)]
    evaluations: Arc<AtomicUsize>,
}

impl PartialEq for PublicationParentKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.evaluations, &other.evaluations)
    }
}

impl Eq for PublicationParentKey {}

impl Hash for PublicationParentKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.evaluations).hash(state);
    }
}

impl fmt::Display for PublicationParentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test-action-provider-publication-parent")
    }
}

#[async_trait]
impl Key for PublicationParentKey {
    type Value = Arc<ConfiguredNodeResult>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        ctx.compute(&PublicationInputKey).await.unwrap()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

fn target_config() -> ConfigurationKey {
    ConfigurationKey::target("targetabc").unwrap()
}

fn structural_configurations() -> [ConfigurationKey; 3] {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    [
        SlugConfiguration::default_target(&host).unwrap(),
        SlugConfiguration::default_exec(&host).unwrap(),
        SlugConfiguration::default_host_like(&host).unwrap(),
    ]
    .map(ConfigurationKey::from_slug)
}

fn canonical(value: &str) -> CanonicalLabel {
    CanonicalLabel::parse(value).unwrap()
}

fn mapped_label(mapping_name: &str, repo_version: &str) -> CanonicalLabel {
    let apparent = ApparentLabel::parse("@dep//pkg:target").unwrap();
    let mut mapping = RepositoryMapping::new(RepositoryMappingId::new(mapping_name).unwrap());
    mapping.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new(repo_version).unwrap(),
    );
    apparent.resolve(&mapping)
}

fn toolchain_context(
    owner: &ConfiguredTargetKey,
    platform: &ConfiguredTargetKey,
    marker: &str,
) -> Arc<ConfiguredActionToolchainContext> {
    let implementation = ConfiguredTargetKey::new(
        canonical("@@//:implementation"),
        platform.configuration().clone(),
    );
    let selection = ConfiguredToolchainSelection::new(
        canonical("@@//:toolchain"),
        implementation.clone(),
        implementation,
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string(marker))],
        ),
    );
    Arc::new(
        ConfiguredActionToolchainContext::new(
            platform.clone(),
            vec![ConfiguredToolchainContextRow::new(
                ConfiguredTargetKey::new(canonical("@@//:type"), owner.configuration().clone()),
                ConfiguredTargetKey::new(canonical("@@//:type"), owner.configuration().clone()),
                true,
                Some(selection),
            )],
        )
        .unwrap(),
    )
}

fn toolchain_marker(context: &ConfiguredActionToolchainContext) -> &str {
    let value = context.rows()[0]
        .selected()
        .unwrap()
        .info()
        .field("marker")
        .unwrap();
    let AnalysisValueKind::String(marker) = value.kind() else {
        panic!("test toolchain marker must be a string")
    };
    marker
}

fn aliased_payload_context(shared: bool) -> Arc<ConfiguredActionToolchainContext> {
    let target = structural_configurations()[0].clone();
    let exec = structural_configurations()[1].clone();
    let platform = ConfiguredTargetKey::new(canonical("@@//:platform"), exec.clone());
    let leaf = || {
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::string("same")],
            vec![],
        )
        .unwrap()
    };
    let first = leaf();
    let second = if shared { first.clone() } else { leaf() };
    let rows = [first, second]
        .into_iter()
        .enumerate()
        .map(|(index, depset)| {
            let implementation = ConfiguredTargetKey::new(
                canonical(&format!("@@//:implementation_{index}")),
                exec.clone(),
            );
            let nested = ProviderCollection::new(vec![
                ProviderValue::DefaultInfo(DefaultInfo::empty()),
                ProviderValue::Occurrence(ProviderOccurrence::new(
                    ProviderIdentity::user(ProviderId::new("//:nested.bzl", "Nested").unwrap()),
                    [("payload", AnalysisValue::depset(depset))],
                )),
            ])
            .unwrap();
            let configured_payload = AnalysisValue::configured_target(ConfiguredTargetValue::new(
                AnalysisConfiguredTargetKey::new(
                    canonical(&format!("@@//:dependency_{index}")),
                    b"same-config".as_slice(),
                ),
                nested,
            ));
            ConfiguredToolchainContextRow::new(
                ConfiguredTargetKey::new(canonical(&format!("@@//:type_{index}")), target.clone()),
                ConfiguredTargetKey::new(canonical(&format!("@@//:type_{index}")), target.clone()),
                true,
                Some(ConfiguredToolchainSelection::new(
                    canonical(&format!("@@//:declaration_{index}")),
                    implementation.clone(),
                    implementation,
                    ProviderOccurrence::new(
                        ProviderIdentity::builtin("ToolchainInfo"),
                        [("payload", configured_payload)],
                    ),
                )),
            )
        })
        .collect::<Vec<_>>();
    Arc::new(ConfiguredActionToolchainContext::new(platform, rows).unwrap())
}

#[tokio::test]
async fn parent_dice_cutoff_tracks_cross_row_toolchain_payload_aliases() {
    let a = aliased_payload_context(true);
    let b = aliased_payload_context(false);
    let restored_a = aliased_payload_context(true);
    for (left, right) in a.rows().iter().zip(b.rows()) {
        assert_eq!(
            left.selected().unwrap().info(),
            right.selected().unwrap().info()
        );
    }
    assert_ne!(a, b);
    assert_eq!(a, restored_a);

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let evaluations = Arc::new(AtomicUsize::new(0));
    let parent = ContextParentKey {
        evaluations: evaluations.clone(),
    };
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(ContextInputKey, a.clone())])
        .unwrap();
    let mut transaction = updater.commit().await;
    let first = transaction.compute(&parent).await.unwrap();
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);
    assert_eq!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);

    let mut updater = dice.updater();
    updater
        .changed_to(vec![(ContextInputKey, b.clone())])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(transaction.compute(&parent).await.unwrap(), b);
    assert_eq!(evaluations.load(Ordering::SeqCst), 2);

    let mut updater = dice.updater();
    updater
        .changed_to(vec![(ContextInputKey, restored_a)])
        .unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 3);
}

fn default_action_context(
    owner: &ConfiguredTargetKey,
    platform_label: &str,
) -> (Arc<ConfiguredActionOwnerContext>, ToolchainTopology) {
    let platform = ConfiguredTargetKey::new(
        canonical(platform_label),
        structural_configurations()[1].clone(),
    );
    let toolchain = toolchain_context(owner, &platform, "marker");
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredExecGroup::Default,
        platform.clone(),
        PlatformSemanticFact {
            exec_properties: Arc::from([]),
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        Vec::new(),
        Some(toolchain.clone()),
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    (
        Arc::new(context),
        ToolchainTopology::new(vec![platform], Some(toolchain)).unwrap(),
    )
}

fn publication_depset(marker: &str, nested: bool) -> AnalysisDepset {
    let artifact = AnalysisValue::artifact(AnalysisArtifact::Source(canonical(&format!(
        "@@//inputs:{marker}.txt"
    ))));
    if nested {
        let child = AnalysisDepset::new(DepsetOrder::Default, vec![artifact], Vec::new()).unwrap();
        AnalysisDepset::new(DepsetOrder::Default, Vec::new(), vec![child]).unwrap()
    } else {
        AnalysisDepset::new(DepsetOrder::Default, vec![artifact], Vec::new()).unwrap()
    }
}

fn publication_result(marker: &str, nested: bool) -> Arc<ConfiguredNodeResult> {
    let files = publication_depset(marker, nested);
    let providers = ProviderCollection::new(vec![ProviderValue::DefaultInfo(
        DefaultInfo::from_files(files.clone()).unwrap(),
    )])
    .unwrap();
    let owner = ConfiguredTargetKey::new(
        canonical("@@//:publication"),
        structural_configurations()[0].clone(),
    );
    let (context, _) = default_action_context(&owner, "@@//:publication_platform");
    let action = ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tools/runner").unwrap(),
        )),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(files).unwrap(),
        )]),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new("publication.out", ActionOutputKind::File)],
        None,
        RetainedActionEnvironment::default().for_action(false, Vec::<(String, String)>::new()),
        CanonicalStringMap::default(),
        "PublicationProof",
        None::<&str>,
    ));
    Arc::new(
        ConfiguredNodeResult::new_rule(owner, providers, None, no_runfiles_packages())
            .with_action_specs(vec![action], vec![context])
            .unwrap(),
    )
}

#[tokio::test]
async fn action_and_default_files_publication_equality_cut_off_parent_dice() {
    let a1 = publication_result("a", false);
    let a2 = publication_result("a", false);
    let b = publication_result("b", true);
    let a3 = publication_result("a", false);
    let a4 = publication_result("a", false);
    assert_eq!(a1, a2);
    assert_ne!(a1, b);
    assert_eq!(a1, a3);
    assert_eq!(a1, a4);

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let evaluations = Arc::new(AtomicUsize::new(0));
    let parent = PublicationParentKey {
        evaluations: evaluations.clone(),
    };

    let mut updater = dice.updater();
    updater.changed_to(vec![(PublicationInputKey, a1)]).unwrap();
    let mut transaction = updater.commit().await;
    let first = transaction.compute(&parent).await.unwrap();
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);

    let mut updater = dice.updater();
    updater.changed_to(vec![(PublicationInputKey, a2)]).unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 1);

    let mut updater = dice.updater();
    updater.changed_to(vec![(PublicationInputKey, b)]).unwrap();
    let mut transaction = updater.commit().await;
    assert_ne!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 2);

    let mut updater = dice.updater();
    updater.changed_to(vec![(PublicationInputKey, a3)]).unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 3);

    let mut updater = dice.updater();
    updater.changed_to(vec![(PublicationInputKey, a4)]).unwrap();
    let mut transaction = updater.commit().await;
    assert_eq!(transaction.compute(&parent).await.unwrap(), first);
    assert_eq!(evaluations.load(Ordering::SeqCst), 3);
}

fn action_context(
    owner: &ConfiguredTargetKey,
    group: ConfiguredExecGroup,
    platform: ConfiguredTargetKey,
    platform_properties: &[(&str, &str)],
    target_properties: &[(&str, &str)],
    group_properties: &[(&str, &str)],
    marker: &str,
    constraints: Vec<ConfiguredActionPlatformConstraint>,
) -> Result<Arc<ConfiguredActionOwnerContext>, String> {
    if platform.configuration().kind() != slug_analysis_v2::ConfigurationKind::Exec
        || platform.configuration().slug_configuration().is_none()
    {
        return Err("configured action platform requires structural exec configuration".to_owned());
    }
    let properties = |entries: &[(&str, &str)]| {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>()
    };
    let toolchain = toolchain_context(owner, &platform, marker);
    ConfiguredActionOwnerContext::new(
        owner.clone(),
        group,
        platform,
        PlatformSemanticFact {
            exec_properties: platform_properties
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .collect::<Vec<_>>()
                .into(),
        },
        &properties(target_properties),
        &properties(group_properties),
        constraints,
        Some(toolchain),
        ConfiguredActionAspectProvenance::Absent,
    )
    .map(Arc::new)
}

fn file_write_result(
    configuration: ConfigurationKey,
    platform_label: &str,
    content: &str,
    output_path: &str,
) -> ConfiguredNodeResult {
    let owner = ConfiguredTargetKey::new(canonical("@@//:probe"), configuration.clone());
    let (context, topology) = default_action_context(&owner, platform_label);
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    ConfiguredNodeResult::new_rule(owner, providers, None, no_runfiles_packages())
        .with_action_specs(
            vec![ActionSpec::new(
                ActionKind::Write {
                    content: content.to_owned(),
                    is_executable: false,
                },
                "FileWrite",
                vec![ActionOutput::new(output_path, ActionOutputKind::File)],
            )],
            vec![context],
        )
        .unwrap()
        .with_toolchain_topology(topology)
}

fn only_file_write(result: &ConfiguredNodeResult) -> slug_analysis_v2::ConfiguredActionView<'_> {
    result
        .configured_file_write_actions()
        .unwrap()
        .next()
        .unwrap()
}

#[test]
fn configured_target_key_serializes_label_mapping_and_configuration() {
    let first = ConfiguredTargetKey::new(mapped_label("first", "dep~1.0.0"), target_config());
    let second = ConfiguredTargetKey::new(mapped_label("second", "dep~1.0.0"), target_config());
    let exec = ConfiguredTargetKey::new(
        mapped_label("first", "dep~1.0.0"),
        ConfigurationKey::exec("execabc").unwrap(),
    );

    assert_ne!(first, second);
    assert_ne!(first.stable_serialize(), second.stable_serialize());
    assert_eq!(
        first.stable_serialize(),
        "@@dep~1.0.0//pkg:target@mapping:first [target:targetabc]"
    );
    assert_eq!(
        exec.stable_serialize(),
        "@@dep~1.0.0//pkg:target@mapping:first [exec:execabc]"
    );
}

#[test]
fn configured_node_key_distinguishes_configured_null_and_configuration_kinds() {
    let label = canonical("@@//pkg:target");
    let [target_configuration, exec_configuration, host_configuration] =
        structural_configurations();
    let target = ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
        label.clone(),
        target_configuration,
    ));
    let exec =
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(label.clone(), exec_configuration));
    let host_like =
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(label.clone(), host_configuration));
    let null = ConfiguredNodeKey::null(label);

    assert_ne!(target, exec);
    assert_ne!(target, host_like);
    assert_ne!(exec, host_like);
    assert_ne!(target, null);
    assert!(null.configured_target().is_none());
    assert_eq!(null.label().to_string(), "@@//pkg:target");
}

#[test]
fn configured_analysis_key_rejects_legacy_configuration_identity() {
    let target = ConfiguredTargetKey::new(canonical("@@//pkg:target"), target_config());
    let error =
        ConfiguredNodeAnalysisKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), target)
            .unwrap_err();
    assert_eq!(
        error.to_string(),
        "production configured-node analysis requires a structural Slug configuration"
    );
}

#[test]
fn configured_edges_preserve_transition_convergence_order_and_fixed_bits() {
    let target = ConfiguredTargetKey::new(canonical("@@//dep:lib"), target_config());
    let first = ConfiguredEdge::new(
        target.clone().into(),
        ConfiguredEdgeKind::Attribute {
            attribute: "left".into(),
            index: 0,
            hidden: false,
            dependency: ConfiguredAttributeDependency::Starlark {
                outputs: Arc::from([canonical("@@//settings:out")]),
                exec_group: None,
            },
        },
    );
    let second = ConfiguredEdge::new(
        target.clone().into(),
        ConfiguredEdgeKind::Attribute {
            attribute: "right".into(),
            index: 1,
            hidden: false,
            dependency: ConfiguredAttributeDependency::Starlark {
                outputs: Arc::from([canonical("@@//settings:out")]),
                exec_group: None,
            },
        },
    );
    assert_ne!(first, second);
    assert_eq!(first.configured_target(), second.configured_target());

    let kinds = vec![
        (
            ConfiguredEdgeKind::Attribute {
                attribute: "deps".into(),
                index: 0,
                hidden: false,
                dependency: ConfiguredAttributeDependency::Target,
            },
            false,
        ),
        (
            ConfiguredEdgeKind::Attribute {
                attribute: "deps".into(),
                index: 1,
                hidden: false,
                dependency: ConfiguredAttributeDependency::Starlark {
                    outputs: Arc::from([]),
                    exec_group: None,
                },
            },
            false,
        ),
        (ConfiguredEdgeKind::AliasActual { rule_class: None }, false),
        (ConfiguredEdgeKind::GeneratedBy, false),
        (ConfiguredEdgeKind::DeclaringVisibility, false),
        (ConfiguredEdgeKind::PackageGroupInclude { index: 0 }, true),
        (ConfiguredEdgeKind::ToolchainRequirement, true),
        (ConfiguredEdgeKind::SelectedToolchainImplementation, true),
        (
            ConfiguredEdgeKind::CandidateExecutionPlatform { index: 0 },
            true,
        ),
        (ConfiguredEdgeKind::HostPlatform, true),
        (ConfiguredEdgeKind::PlatformConstraint { index: 0 }, true),
        (ConfiguredEdgeKind::ConstraintSetting, true),
        (ConfiguredEdgeKind::FunctionTransitionAllowlist, true),
    ];
    for (kind, implicit) in kinds {
        let edge = ConfiguredEdge::new(target.clone().into(), kind);
        assert_eq!(edge.implicit(), implicit);
        assert!(!edge.tool());
    }

    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let ordered = ConfiguredNodeResult::new_rule(
        target.clone(),
        providers.clone(),
        None,
        no_runfiles_packages(),
    )
    .with_edges(vec![first.clone(), second.clone()]);
    let reordered = ConfiguredNodeResult::new_rule(target, providers, None, no_runfiles_packages())
        .with_edges(vec![second, first]);
    assert_ne!(ordered, reordered);
    assert_eq!(
        ordered.edges()[0].kind(),
        &ConfiguredEdgeKind::Attribute {
            attribute: "left".into(),
            index: 0,
            hidden: false,
            dependency: ConfiguredAttributeDependency::Starlark {
                outputs: Arc::from([canonical("@@//settings:out")]),
                exec_group: None,
            },
        }
    );

    let empty = ConfiguredAttributeDependency::Starlark {
        outputs: Arc::from([]),
        exec_group: None,
    };
    let one = ConfiguredAttributeDependency::Starlark {
        outputs: Arc::from([canonical("@@//settings:a")]),
        exec_group: None,
    };
    let multiple = ConfiguredAttributeDependency::Starlark {
        outputs: Arc::from([canonical("@@//settings:a"), canonical("@@//settings:b")]),
        exec_group: None,
    };
    let reordered = ConfiguredAttributeDependency::Starlark {
        outputs: Arc::from([canonical("@@//settings:b"), canonical("@@//settings:a")]),
        exec_group: None,
    };
    assert_ne!(empty, one);
    assert_ne!(one, multiple);
    assert_ne!(multiple, reordered);

    let hidden_target = ConfiguredEdge::new(
        ConfiguredNodeKey::null(canonical("@@//pkg:source.txt")),
        ConfiguredEdgeKind::Attribute {
            attribute: "$hidden".into(),
            index: 0,
            hidden: true,
            dependency: ConfiguredAttributeDependency::Target,
        },
    );
    assert!(hidden_target.implicit());
    assert!(!hidden_target.tool());
    let visible_exec_source = ConfiguredEdge::new(
        ConfiguredNodeKey::null(canonical("@@//pkg:source.txt")),
        ConfiguredEdgeKind::Attribute {
            attribute: "tool".into(),
            index: 0,
            hidden: false,
            dependency: ConfiguredAttributeDependency::Exec(ConfiguredExecGroup::Default),
        },
    );
    assert!(!visible_exec_source.implicit());
    assert!(visible_exec_source.tool());
    let composed = ConfiguredEdge::new(
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
            canonical("@@//dep:composed"),
            target_config(),
        )),
        ConfiguredEdgeKind::Attribute {
            attribute: "tool".into(),
            index: 1,
            hidden: false,
            dependency: ConfiguredAttributeDependency::Starlark {
                outputs: Arc::from([canonical("@@//settings:out")]),
                exec_group: Some(ConfiguredExecGroup::Named("named".into())),
            },
        },
    );
    assert!(composed.tool());
}

#[test]
fn configured_node_result_keeps_provider_collection_outputs_and_diagnostics() {
    let files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(AnalysisArtifact::Source(
            canonical("@@//pkg:out.txt"),
        ))],
        Vec::new(),
    )
    .unwrap();
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(files).unwrap()),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::unqualified("MyInfo").unwrap()),
            [("value", AnalysisValue::string("custom"))],
        )),
    ])
    .unwrap();

    let owner = ConfiguredTargetKey::new(
        canonical("@@//pkg:custom"),
        structural_configurations()[0].clone(),
    );
    let expected_owner = owner.clone();
    let (context, _) = default_action_context(&owner, "@@//:platform");
    let result = ConfiguredNodeResult::new_rule(owner, providers, None, no_runfiles_packages())
        .with_action_specs(
            vec![ActionSpec::new(
                ActionKind::Write {
                    content: "out".to_owned(),
                    is_executable: false,
                },
                "FileWrite",
                vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
            )],
            vec![context],
        )
        .unwrap()
        .with_declared_outputs(vec!["pkg/out.txt".to_owned()])
        .with_diagnostics(vec![AnalysisDiagnostic::new(
            DiagnosticSeverity::Warning,
            "placeholder analysis warning",
        )]);

    assert_eq!(result.configured_target_key(), Some(&expected_owner));
    assert_eq!(
        result.configured_target_key().unwrap().stable_serialize(),
        expected_owner.stable_serialize()
    );
    assert_eq!(
        result
            .configured_target_key()
            .unwrap()
            .configuration()
            .complete_identity_bytes(),
        expected_owner.configuration().complete_identity_bytes()
    );
    assert_eq!(result.actions()[0].mnemonic(), "FileWrite");
    assert_eq!(result.declared_outputs(), &["pkg/out.txt".to_owned()]);
    let default_files = result.providers().default_info().unwrap().file_artifacts();
    assert_eq!(default_files.len(), 1);
    assert_eq!(default_files[0].path().as_ref(), "pkg/out.txt");
    assert_eq!(
        result.diagnostics()[0].severity(),
        DiagnosticSeverity::Warning
    );
    assert_eq!(
        result.diagnostics()[0].message(),
        "placeholder analysis warning"
    );
}

#[test]
fn configured_node_result_capability_is_borrowed_and_participates_in_equality() {
    let key = ConfiguredTargetKey::new(canonical("@@//pkg:custom"), target_config());
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let executable = RuleCapability {
        rule_class: "custom_rule".into(),
        executable: true,
        test_kind: None,
    };
    let test = RuleCapability {
        rule_class: "custom_test".into(),
        executable: true,
        test_kind: Some(TestRuleKind::Test),
    };

    let absent = ConfiguredNodeResult::new_rule(
        key.clone(),
        providers.clone(),
        None,
        no_runfiles_packages(),
    );
    let executable_result = ConfiguredNodeResult::new_rule(
        key.clone(),
        providers.clone(),
        Some(executable.clone()),
        no_runfiles_packages(),
    );
    let renamed = ConfiguredNodeResult::new_rule(
        key.clone(),
        providers.clone(),
        Some(RuleCapability {
            rule_class: "other_rule".into(),
            ..executable.clone()
        }),
        no_runfiles_packages(),
    );
    let test_result =
        ConfiguredNodeResult::new_rule(key, providers, Some(test), no_runfiles_packages());

    assert_eq!(executable_result.rule_capability(), Some(&executable));
    assert_ne!(absent, executable_result);
    assert_ne!(executable_result, renamed);
    assert_ne!(executable_result, test_result);
}

#[test]
fn toolchain_topology_is_ordered_role_checked_and_structurally_equal() {
    let candidate = ConfiguredTargetKey::new(
        canonical("@@//:platform"),
        structural_configurations()[1].clone(),
    );
    let owner = ConfiguredTargetKey::new(
        canonical("@@//:owner"),
        structural_configurations()[0].clone(),
    );
    let context = toolchain_context(&owner, &candidate, "topology");
    let topology = ToolchainTopology::new(vec![candidate.clone()], Some(context)).unwrap();
    assert_eq!(
        topology.toolchain().unwrap().execution_platform(),
        &candidate
    );
    assert!(
        ToolchainTopology::new(
            vec![ConfiguredTargetKey::new(
                canonical("@@//:wrong_role"),
                target_config(),
            )],
            None,
        )
        .is_err()
    );

    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let key = ConfiguredTargetKey::new(canonical("@@//:root"), target_config());
    let plain = ConfiguredNodeResult::new_rule(
        key.clone(),
        providers.clone(),
        None,
        no_runfiles_packages(),
    );
    let retained = ConfiguredNodeResult::new_rule(key, providers, None, no_runfiles_packages())
        .with_toolchain_topology(topology.clone());
    assert_ne!(plain, retained);
    assert_eq!(retained.toolchain_topology(), Some(&topology));
}

#[test]
fn configured_file_write_view_tracks_and_restores_structural_identity() {
    let c0 = structural_configurations()[0].clone();
    let c1 = c0.with_starlark_option(StarlarkOption::string(
        canonical("@@//:setting"),
        "c1",
        StarlarkOptionScope::Default,
    ));
    let baseline = file_write_result(c0.clone(), "@@//:p0", "content-A", "path-A.txt");
    let changed_configuration = file_write_result(c1, "@@//:p0", "content-A", "path-A.txt");
    let changed_platform = file_write_result(c0.clone(), "@@//:p1", "content-A", "path-A.txt");
    let changed_content = file_write_result(c0.clone(), "@@//:p0", "content-B", "path-A.txt");
    let changed_path = file_write_result(c0.clone(), "@@//:p0", "content-A", "path-B.txt");
    let restored = file_write_result(c0, "@@//:p0", "content-A", "path-A.txt");

    let baseline = only_file_write(&baseline);
    assert_eq!(baseline.exec_group(), &ConfiguredExecGroup::Default);
    assert_eq!(baseline.owner().label(), &canonical("@@//:probe"));
    assert_eq!(baseline.execution_platform().label(), &canonical("@@//:p0"));
    assert_eq!(baseline.output().path(), "path-A.txt");
    assert!(matches!(
        baseline.spec().kind(),
        ActionKind::Write { content, is_executable: false } if content == "content-A"
    ));
    assert_ne!(baseline, only_file_write(&changed_configuration));
    assert_ne!(baseline, only_file_write(&changed_platform));
    assert_ne!(baseline, only_file_write(&changed_content));
    assert_ne!(baseline, only_file_write(&changed_path));
    assert_eq!(baseline, only_file_write(&restored));
}

#[test]
fn configured_actions_share_group_contexts_merge_properties_and_reject_mismatches() {
    let owner = ConfiguredTargetKey::new(
        canonical("@@//:probe"),
        structural_configurations()[0].clone(),
    );
    let exec = structural_configurations()[1].clone();
    let platform = |label: &str| ConfiguredTargetKey::new(canonical(label), exec.clone());
    let constraint = ConfiguredActionPlatformConstraint::new(
        ConfiguredTargetKey::new(canonical("@@//:linux"), exec.clone()),
        ConfiguredTargetKey::new(canonical("@@//:os"), exec.clone()),
    );
    let default = action_context(
        &owner,
        ConfiguredExecGroup::Default,
        platform("@@//:p0"),
        &[("a", "platform"), ("z", "platform")],
        &[("a", "target"), ("b", "target")],
        &[("a", "default"), ("c", "default")],
        "marker-A",
        vec![constraint.clone()],
    )
    .unwrap();
    let named = action_context(
        &owner,
        ConfiguredExecGroup::Named("named".into()),
        platform("@@//:p1"),
        &[("a", "platform")],
        &[("a", "target")],
        &[("a", "named")],
        "marker-B",
        vec![constraint],
    )
    .unwrap();
    let spec = |path: &str| {
        ActionSpec::new(
            ActionKind::Write {
                content: path.to_owned(),
                is_executable: false,
            },
            "FileWrite",
            vec![ActionOutput::new(path, ActionOutputKind::File)],
        )
    };
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let result = ConfiguredNodeResult::new_rule(
        owner.clone(),
        providers.clone(),
        None,
        no_runfiles_packages(),
    )
    .with_action_specs(
        vec![spec("a"), spec("b"), spec("c").with_exec_group("named")],
        vec![default.clone(), named.clone()],
    )
    .unwrap();
    assert_eq!(
        result
            .actions()
            .iter()
            .map(|row| row.outputs()[0].path())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert!(Arc::ptr_eq(
        result.actions()[0].context(),
        result.actions()[1].context()
    ));
    assert!(Arc::ptr_eq(result.actions()[0].context(), &default));
    assert!(Arc::ptr_eq(result.actions()[2].context(), &named));
    assert_eq!(
        default.platform_fact().unwrap().exec_properties.as_ref(),
        &[
            ("a".into(), "default".into()),
            ("b".into(), "target".into()),
            ("c".into(), "default".into()),
            ("z".into(), "platform".into()),
        ]
    );
    assert_eq!(toolchain_marker(default.toolchain().unwrap()), "marker-A");
    assert_eq!(toolchain_marker(named.toolchain().unwrap()), "marker-B");
    assert_eq!(default.execution_state(), State::SelectedToolchain);
    assert_ne!(default, named);

    let platform_only = Arc::new(
        ConfiguredActionOwnerContext::new(
            owner.clone(),
            ConfiguredExecGroup::Default,
            platform("@@//:p0"),
            PlatformSemanticFact {
                exec_properties: Arc::from([]),
            },
            &BTreeMap::new(),
            &BTreeMap::new(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap(),
    );
    assert_eq!(platform_only.execution_state(), State::SelectedPlatformOnly);
    assert!(platform_only.toolchain().is_none());
    let unresolved_toolchain = Arc::new(
        ConfiguredActionToolchainContext::new(
            platform("@@//:p0"),
            vec![ConfiguredToolchainContextRow::new(
                ConfiguredTargetKey::new(canonical("@@//:optional"), owner.configuration().clone()),
                ConfiguredTargetKey::new(canonical("@@//:optional"), owner.configuration().clone()),
                false,
                None,
            )],
        )
        .unwrap(),
    );
    let all_optional = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredExecGroup::Default,
        platform("@@//:p0"),
        PlatformSemanticFact {
            exec_properties: Arc::from([]),
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        Vec::new(),
        Some(unresolved_toolchain),
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    assert_eq!(all_optional.execution_state(), State::SelectedPlatformOnly);
    assert!(all_optional.toolchain().is_some());

    let restored = action_context(
        &owner,
        ConfiguredExecGroup::Default,
        platform("@@//:p0"),
        &[("a", "platform"), ("z", "platform")],
        &[("a", "target"), ("b", "target")],
        &[("a", "default"), ("c", "default")],
        "marker-A",
        vec![ConfiguredActionPlatformConstraint::new(
            ConfiguredTargetKey::new(canonical("@@//:linux"), exec.clone()),
            ConfiguredTargetKey::new(canonical("@@//:os"), exec.clone()),
        )],
    )
    .unwrap();
    assert_eq!(default, restored);
    assert_ne!(
        default,
        action_context(
            &owner,
            ConfiguredExecGroup::Default,
            platform("@@//:p0"),
            &[("a", "platform"), ("z", "platform")],
            &[],
            &[("a", "edited")],
            "marker-A",
            Vec::new(),
        )
        .unwrap()
    );

    let wrong_owner =
        ConfiguredTargetKey::new(canonical("@@//:other"), owner.configuration().clone());
    let wrong_context = action_context(
        &wrong_owner,
        ConfiguredExecGroup::Default,
        platform("@@//:p0"),
        &[],
        &[],
        &[],
        "marker",
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        ConfiguredNodeResult::new_rule(
            owner.clone(),
            providers.clone(),
            None,
            no_runfiles_packages(),
        )
        .with_action_specs(vec![spec("out")], vec![wrong_context])
        .unwrap_err(),
        "configured action context has mismatched owner"
    );
    assert_eq!(
        ConfiguredNodeResult::new_rule(
            owner.clone(),
            providers.clone(),
            None,
            no_runfiles_packages(),
        )
        .with_action_specs(vec![spec("out")], vec![default.clone(), default.clone()])
        .unwrap_err(),
        "configured action contexts contain duplicate group"
    );
    assert!(
        action_context(
            &owner,
            ConfiguredExecGroup::Default,
            ConfiguredTargetKey::new(canonical("@@//:bad"), target_config()),
            &[],
            &[],
            &[],
            "marker",
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        action_context(
            &owner,
            ConfiguredExecGroup::Default,
            platform("@@//:p0"),
            &[("z", "last"), ("a", "first")],
            &[],
            &[],
            "marker",
            Vec::new(),
        )
        .is_err()
    );
    let bad_constraint = ConfiguredActionPlatformConstraint::new(
        ConfiguredTargetKey::new(canonical("@@//:linux"), target_config()),
        ConfiguredTargetKey::new(canonical("@@//:os"), target_config()),
    );
    assert!(
        action_context(
            &owner,
            ConfiguredExecGroup::Default,
            platform("@@//:p0"),
            &[],
            &[],
            &[],
            "marker",
            vec![bad_constraint],
        )
        .is_err()
    );
}

#[test]
fn configured_file_write_view_uses_retained_context_and_rejects_shapes() {
    let c0 = structural_configurations()[0].clone();
    let baseline = file_write_result(c0, "@@//:p0", "content-A", "path-A.txt");
    let action = ActionSpec::clone(&baseline.actions()[0]);
    let context = baseline.actions()[0].context().clone();
    let empty = ConfiguredNodeResult::new_rule(
        baseline.configured_target_key().unwrap().clone(),
        baseline.providers().clone(),
        None,
        no_runfiles_packages(),
    );
    assert_eq!(empty.configured_file_write_actions().unwrap().len(), 0);
    let unresolved = Arc::new(
        ConfiguredActionOwnerContext::unresolved_default(
            baseline.configured_target_key().unwrap().clone(),
        )
        .unwrap(),
    );
    assert_eq!(unresolved.execution_state(), State::UnresolvedDefault);
    let unresolved = empty
        .clone()
        .with_action_specs(vec![action.clone()], vec![unresolved])
        .unwrap();
    assert!(unresolved.configured_file_write_actions().is_err());
    assert_eq!(
        empty
            .clone()
            .with_action_specs(vec![action.clone()], Vec::new())
            .unwrap_err(),
        "configured action has no matching exec-group context"
    );

    let unrelated = ConfiguredTargetKey::new(
        canonical("@@//:unrelated"),
        structural_configurations()[1].clone(),
    );
    let retained = baseline
        .clone()
        .with_toolchain_topology(ToolchainTopology::new(vec![unrelated], None).unwrap());
    assert_eq!(
        only_file_write(&retained).execution_platform().label(),
        &canonical("@@//:p0")
    );

    let unsupported_shapes = vec![
        ActionSpec::new(ActionKind::Run, "Spawn", action.outputs().to_vec()),
        ActionSpec::new(
            action.kind().clone(),
            "FileWrite",
            vec![ActionOutput::new("tree", ActionOutputKind::Directory)],
        ),
    ];
    for unsupported in unsupported_shapes {
        let result = baseline
            .clone()
            .with_action_specs(vec![unsupported], vec![context.clone()])
            .unwrap();
        assert!(result.configured_file_write_actions().is_err());
    }
    assert_eq!(
        baseline
            .clone()
            .with_action_specs(
                vec![action.clone().with_exec_group("named")],
                vec![context.clone()],
            )
            .unwrap_err(),
        "configured action has no matching exec-group context"
    );

    let mut field = BTreeMap::new();
    field.insert("key".to_owned(), "value".to_owned());
    let unsupported_execution_fields = vec![
        action.clone().with_argv(vec!["literal".to_owned()]),
        action
            .clone()
            .with_inputs(vec![ActionInput::new("input", None)]),
        action
            .clone()
            .with_tools(vec![ActionInput::new("tool", None)]),
        action.clone().with_param_files(vec![ParamFile::new(
            "params",
            vec!["arg".to_owned()],
            ParamFileFormat::Multiline,
        )]),
        action.clone().with_env(field.clone()),
        action.clone().with_execution_requirements(field.clone()),
        action.clone().with_exec_properties(field),
        action.with_progress_message("writing"),
    ];
    for unsupported in unsupported_execution_fields {
        let result = baseline
            .clone()
            .with_action_specs(vec![unsupported], vec![context.clone()])
            .unwrap();
        assert_eq!(
            result.configured_file_write_actions().err(),
            Some("configured FileWrite action has unsupported execution fields")
        );
    }
}
