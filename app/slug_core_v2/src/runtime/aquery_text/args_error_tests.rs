//! Query rendering preserves consumer-time callback failures as owned diagnostics.
use std::collections::BTreeMap;
use std::sync::Arc;

use compact_str::CompactString;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::*;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;

use super::*;

#[derive(Debug)]
struct CallbackOriginals;
impl PublishedOriginalBinding for CallbackOriginals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        1
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}

#[derive(Debug)]
struct FailAtExpansion;
impl RuntimeMapper for FailAtExpansion {
    fn map_batch(
        &self,
        _: &[AnalysisValue],
        _: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        Err(ArgsExpansionError::Callback(
            "deliberate args callback failure".into(),
        ))
    }
}

fn callback_owner() -> ConfiguredNodeResult {
    let inputs = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    let configuration = SlugConfiguration::default_target(&inputs).unwrap();
    let owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:unrelated").unwrap(),
        ConfigurationKey::from_slug(configuration.clone()),
    );
    let platform = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:exec").unwrap(),
        ConfigurationKey::from_slug(configuration.to_exec().unwrap()),
    );
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredExecGroup::Default,
        platform,
        PlatformSemanticFact {
            exec_properties: Arc::from([]),
            missing_toolchain_error: None,
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        Vec::new(),
        None,
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    let mut graph = CallableGraphBuilder::new();
    let root = graph.reserve().unwrap();
    graph
        .set_node(
            root,
            CallableNode::Intrinsic(CallableIntrinsic {
                api: NativeCallableApiId::new("test.query_mapper", 1).unwrap(),
                parameters: Arc::from([]),
                return_annotation: None,
                receiver: None,
            }),
        )
        .unwrap();
    graph
        .set_original_slot(root, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let mapper = RetainedArgsMapper::from_published(
        graph
            .seal(Arc::new(CallbackOriginals))
            .unwrap()
            .value(root)
            .unwrap(),
        Arc::new(FailAtExpansion),
    )
    .unwrap();
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Mapped {
                inputs: RetainedArgsInputs::Sequence(vec![AnalysisValue::string("input")].into()),
                mapper,
                registration_location: Some("//:unrelated.bzl:17".into()),
            },
            RetainedVectorOptions {
                arg_name: None,
                format_each: None,
                before_each: None,
                join_with: None,
                format_joined: None,
                omit_if_empty: true,
                uniquify: false,
                expand_directories: true,
                terminate_with: None,
            },
        ))],
        RetainedParamFileFormat::Multiline,
    );
    let spawn = SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        )),
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
            RetainedSpawnArgsSnapshot::new(recipe, None),
        )]),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new(
            DerivedArtifactRoot::bin(configuration).unwrap(),
            "out",
            ActionOutputKind::File,
        )],
        None,
        RetainedActionEnvironment::default(),
        CanonicalStringMap::default(),
        "Generic",
        None::<&str>,
    );
    let owner = ConfiguredNodeResult::new_rule(
        owner,
        ProviderCollection::from_values(Vec::new(), false).unwrap(),
        None,
        RunfilesPackageDepset::new(DepsetOrder::Default, Vec::new(), Vec::new()).unwrap(),
    )
    .with_action_specs(vec![ActionSpec::spawn(spawn)], vec![Arc::new(context)])
    .unwrap();
    owner
}

#[test]
fn aquery_spawn_preserves_owned_args_callback_diagnostic() {
    let owner = callback_owner();
    let view = ConfiguredSpawnActionView::from_configured_action(&owner.actions()[0]).unwrap();
    let error = append_spawn(&mut String::new(), view, &NoopArgsDirectoryContext).unwrap_err();
    assert!(
        error.contains("//:unrelated.bzl:17") && error.contains("deliberate args callback failure"),
        "{error}"
    );
}

#[derive(Debug)]
struct CancelledQuery;
impl ArgsDirectoryContext for CancelledQuery {
    fn is_cancelled(&self) -> bool {
        true
    }
    fn list(&self, _: &AnalysisArtifact) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        panic!("cancelled query must not discover directories")
    }
}

#[test]
fn aquery_spawn_uses_current_consumer_cancellation_before_callback() {
    let owner = callback_owner();
    let view = ConfiguredSpawnActionView::from_configured_action(&owner.actions()[0]).unwrap();
    let error = append_spawn(&mut String::new(), view, &CancelledQuery).unwrap_err();
    assert!(error.contains("Args expansion cancelled"), "{error}");
    assert!(
        !error.contains("deliberate args callback failure"),
        "{error}"
    );
    // A subsequent unrelated consumer sees the published callback unchanged.
    let view = ConfiguredSpawnActionView::from_configured_action(&owner.actions()[0]).unwrap();
    let error = append_spawn(&mut String::new(), view, &NoopArgsDirectoryContext).unwrap_err();
    assert!(
        error.contains("deliberate args callback failure"),
        "{error}"
    );
}
