//! Native renderer/ownership contracts; actual Starlark evaluation has Analysis tests.
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use compact_str::CompactString;
use slug_build_api_v2::*;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_identity_v2::CanonicalLabel;

mod root_support;

#[derive(Debug)]
enum Behavior {
    Echo,
    Fail,
    Oversized,
}

#[derive(Debug)]
struct MockRuntime {
    behavior: Behavior,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for MockRuntime {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl RuntimeMapper for MockRuntime {
    fn map_batch(
        &self,
        values: &[AnalysisValue],
        _directory: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.behavior {
            Behavior::Fail => Err(ArgsExpansionError::Callback("deliberate failure".into())),
            Behavior::Oversized => Ok(vec!["x".into(); 65_537]),
            Behavior::Echo => values
                .iter()
                .map(|value| match value.kind() {
                    AnalysisValueKind::String(text) => Ok(text.into()),
                    AnalysisValueKind::Artifact(artifact) => Ok(artifact.path().as_ref().into()),
                    _ => Err(ArgsExpansionError::InvalidReturn(
                        "mock expects strings or Files".into(),
                    )),
                })
                .collect(),
        }
    }
}

fn mapper(
    receiver: AnalysisValue,
    behavior: Behavior,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
) -> RetainedArgsMapper {
    let mut graph = CallableGraphBuilder::new();
    let native = graph.reserve().unwrap();
    let atom = graph.native_atom(receiver).unwrap();
    graph.set_node(native, CallableNode::Native(atom)).unwrap();
    graph
        .set_original_slot(native, PublishedRootSlot::new(native.index()).unwrap())
        .unwrap();
    let root = graph.reserve().unwrap();
    graph
        .set_node(
            root,
            CallableNode::Intrinsic(CallableIntrinsic {
                api: NativeCallableApiId::new("test.echo", 1).unwrap(),
                parameters: Arc::from([]),
                return_annotation: None,
                receiver: Some(PublishedEdge::Local(native)),
            }),
        )
        .unwrap();
    graph
        .set_original_slot(root, PublishedRootSlot::new(root.index()).unwrap())
        .unwrap();
    RetainedArgsMapper::from_published(
        graph
            .seal(Arc::new(TestOriginals))
            .unwrap()
            .value(root)
            .unwrap(),
        Arc::new(MockRuntime {
            behavior,
            calls,
            drops,
        }),
    )
    .unwrap()
}

fn options() -> RetainedVectorOptions {
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
    }
}

fn source(inputs: RetainedArgsInputs, mapper: RetainedArgsMapper) -> RetainedVectorSource {
    RetainedVectorSource::Mapped {
        inputs,
        mapper,
        registration_location: Some("//:unrelated.bzl:7".into()),
    }
}

fn recipe(inputs: RetainedArgsInputs, mapper: RetainedArgsMapper) -> RetainedArgsRecipe {
    RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            source(inputs, mapper),
            options(),
        ))],
        RetainedParamFileFormat::Multiline,
    )
}

fn depset(name: &str) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap(),
        ))],
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn deferred_callback_rendering_orders_formats_unique_join_and_retains_runtime_guard() {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let mapper = mapper(
        AnalysisValue::none(),
        Behavior::Echo,
        calls.clone(),
        drops.clone(),
    );
    let inputs = RetainedArgsInputs::Sequence(
        vec![
            AnalysisValue::string("b"),
            AnalysisValue::string("a"),
            AnalysisValue::string("b"),
        ]
        .into(),
    );
    let mut all = options();
    all.arg_name = Some("--paths".into());
    all.before_each = Some("--next".into());
    all.format_each = Some("value=%s".into());
    all.uniquify = true;
    all.terminate_with = Some("--end".into());
    let mut joined = options();
    joined.join_with = Some("|".into());
    joined.format_joined = Some("joined=%s".into());
    let recipe = RetainedArgsRecipe::new(
        vec![
            RetainedArgCall::AddAll(RetainedVectorArg::new(
                source(inputs.clone(), mapper.clone()),
                all,
            )),
            RetainedArgCall::AddJoined(RetainedVectorArg::new(
                source(inputs, mapper.clone()),
                joined,
            )),
        ],
        RetainedParamFileFormat::Multiline,
    );
    drop(mapper);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "callback must stay deferred"
    );
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "recipe must own the actual callback guard"
    );
    assert_eq!(
        recipe.render().unwrap(),
        [
            "--paths",
            "--next",
            "value=b",
            "--next",
            "value=a",
            "--end",
            "joined=b|a|b"
        ]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(recipe);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn deferred_callback_recipe_and_spawn_share_native_alias_identity() {
    let make = |captured: AnalysisDepset, input: AnalysisDepset| {
        let mapper = mapper(
            AnalysisValue::depset(captured),
            Behavior::Echo,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        );
        recipe(RetainedArgsInputs::Depset(input), mapper)
    };
    let shared = depset("input");
    let aliased = make(shared.clone(), shared);
    let split = make(depset("input"), depset("input"));
    assert_eq!(aliased.render().unwrap(), split.render().unwrap());
    assert_ne!(
        aliased, split,
        "capture and input aliases are one semantic graph"
    );
    let shared = depset("input");
    assert_eq!(aliased, make(shared.clone(), shared));
    let spawn = |args: RetainedArgsRecipe, input: AnalysisDepset| {
        SpawnSpec::new(
            RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
                NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
            )),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
                RetainedSpawnArgsSnapshot::new(args, None),
            )]),
            ArtifactInputs::new(vec![ArtifactInputSource::Depset(
                RetainedArtifactInputs::new(input).unwrap(),
            )]),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new(
                root_support::root(),
                "out",
                ActionOutputKind::File,
            )],
            None,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Generic",
            None::<&str>,
        )
    };
    let shared = depset("input");
    let aliased = spawn(make(shared.clone(), shared.clone()), shared);
    let split = spawn(make(depset("input"), depset("input")), depset("input"));
    assert_eq!(aliased.render_argv().unwrap(), split.render_argv().unwrap());
    assert_ne!(aliased, split);
}

#[test]
fn deferred_callback_failures_limits_and_execution_directory_capability_propagate() {
    for (behavior, message) in [
        (Behavior::Fail, "deliberate failure"),
        (Behavior::Oversized, "callback output count"),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let mapper = mapper(
            AnalysisValue::none(),
            behavior,
            calls.clone(),
            Arc::new(AtomicUsize::new(0)),
        );
        let recipe = recipe(
            RetainedArgsInputs::Sequence(vec![AnalysisValue::string("input")].into()),
            mapper,
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let error = recipe.render().unwrap_err().to_string();
        assert!(
            error.contains("//:unrelated.bzl:7") && error.contains(message),
            "{error}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(recipe.render_write_content().is_err());
    }
    let tree = AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:owner").unwrap(),
            b"configuration".as_slice(),
        ),
        output: ActionOutput::new(root_support::root(), "tree", ActionOutputKind::Directory),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let mapper = mapper(
        AnalysisValue::none(),
        Behavior::Echo,
        calls.clone(),
        Arc::new(AtomicUsize::new(0)),
    );
    let recipe = recipe(
        RetainedArgsInputs::Sequence(vec![AnalysisValue::artifact(tree.clone())].into()),
        mapper,
    );
    assert_eq!(
        recipe.render().unwrap(),
        [tree.path().into_owned()],
        "query receives the declared directory itself"
    );
    let error = recipe
        .render_with_context(&UnsupportedTreeArgsDirectoryContext)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unsupported"), "{error}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "execution must refuse unavailable tree metadata before invoking callback"
    );
}

fn quota_spawn(segments: Vec<RetainedCommandLineSegment>) -> SpawnSpec {
    SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        )),
        RetainedCommandLine::new(segments),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new(
            root_support::root(),
            "out",
            ActionOutputKind::File,
        )],
        None,
        RetainedActionEnvironment::default(),
        CanonicalStringMap::default(),
        "Generic",
        None::<CompactString>,
    )
}

#[test]
fn native_expansion_preflights_repeated_formats_prefixes_and_delimiters() {
    let calls = Arc::new(AtomicUsize::new(0));
    let inputs = RetainedArgsInputs::Sequence(vec![AnalysisValue::string("x"); 65_536].into());
    let mapper = mapper(
        AnalysisValue::none(),
        Behavior::Echo,
        calls.clone(),
        Arc::new(AtomicUsize::new(0)),
    );
    for variant in 0..3 {
        let mut options = options();
        let repeated: CompactString = "p".repeat(1_100).into();
        match variant {
            0 => options.format_each = Some(format!("{repeated}%s").into()),
            1 => options.before_each = Some(repeated),
            _ => options.join_with = Some(repeated),
        }
        let vector = RetainedVectorArg::new(source(inputs.clone(), mapper.clone()), options);
        let call = if variant == 2 {
            RetainedArgCall::AddJoined(vector)
        } else {
            RetainedArgCall::AddAll(vector)
        };
        let recipe = RetainedArgsRecipe::new(vec![call], RetainedParamFileFormat::Multiline);
        assert!(
            matches!(recipe.render(), Err(ArgsExpansionError::Limit(ref reason))
            if reason.as_str() == "native expanded output bytes")
        );
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "forecast must not invoke callbacks again"
    );

    let mut joined = options();
    joined.join_with = Some("".into());
    joined.format_joined = Some("%s%s%s%s".into());
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddJoined(RetainedVectorArg::new(
            RetainedVectorSource::Sequence(
                vec![RetainedScalarValue::String(
                    "x".repeat(17 * 1024 * 1024).into(),
                )]
                .into(),
            ),
            joined,
        ))],
        RetainedParamFileFormat::Multiline,
    );
    assert!(matches!(recipe.render(), Err(ArgsExpansionError::Limit(_))));
}

#[test]
fn native_expansion_budgets_grouping_escaping_and_complete_consumers() {
    let mut grouped = options();
    grouped.before_each = Some("--flag".into());
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Sequence(
                vec![RetainedScalarValue::String("x".into()); 40_000].into(),
            ),
            grouped,
        ))],
        RetainedParamFileFormat::FlagPerLine,
    );
    let rendered = recipe.render().unwrap();
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].starts_with("--flag=x --flag x"));

    for format in [RetainedParamFileFormat::Shell, RetainedParamFileFormat::Gcc] {
        let size = if format == RetainedParamFileFormat::Shell {
            17
        } else {
            33
        };
        let recipe = RetainedArgsRecipe::new(
            vec![RetainedArgCall::Scalar(RetainedScalarArg::new(
                None::<CompactString>,
                RetainedScalarValue::String("'".repeat(size * 1024 * 1024).into()),
                None::<CompactString>,
            ))],
            format,
        );
        assert!(matches!(
            recipe.render_write_content(),
            Err(ArgsExpansionError::Limit(_))
        ));
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mapper = mapper(
        AnalysisValue::none(),
        Behavior::Echo,
        calls.clone(),
        Arc::new(AtomicUsize::new(0)),
    );
    let mut formatted = options();
    formatted.format_each = Some(format!("{}%s", "p".repeat(1_200)).into());
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            source(
                RetainedArgsInputs::Sequence(vec![AnalysisValue::string("x"); 30_000].into()),
                mapper,
            ),
            formatted,
        ))],
        RetainedParamFileFormat::Multiline,
    );
    let snapshot =
        RetainedCommandLineSegment::ArgsSnapshot(RetainedSpawnArgsSnapshot::new(recipe, None));
    assert!(matches!(
        quota_spawn(vec![snapshot.clone(), snapshot]).render_argv(),
        Err(ArgsExpansionError::Limit(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // Each file fits separately; all three must fail the joint preflight.
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::Scalar(RetainedScalarArg::new(
            None::<CompactString>,
            RetainedScalarValue::String("'".repeat(6 * 1024 * 1024).into()),
            None::<CompactString>,
        ))],
        RetainedParamFileFormat::Shell,
    );
    let snapshot = RetainedCommandLineSegment::ArgsSnapshot(RetainedSpawnArgsSnapshot::new(
        recipe,
        Some(RetainedSpawnParamFilePolicy::new("@%s", true)),
    ));
    assert!(matches!(
        quota_spawn(vec![snapshot.clone(), snapshot.clone(), snapshot]).expand_forced_param_files(),
        Err(SpawnCommandLineError::ArgsExpansion(
            ArgsExpansionError::Limit(_)
        ))
    ));

    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::Scalar(RetainedScalarArg::new(
            None::<CompactString>,
            RetainedScalarValue::String("a'b".into()),
            None::<CompactString>,
        ))],
        RetainedParamFileFormat::Shell,
    );
    let expanded = quota_spawn(vec![RetainedCommandLineSegment::ArgsSnapshot(
        RetainedSpawnArgsSnapshot::new(
            recipe,
            Some(RetainedSpawnParamFilePolicy::new("@%s", true)),
        ),
    )])
    .expand_forced_param_files()
    .unwrap();
    assert_eq!(
        expanded.argv(),
        [
            "tool",
            &format!("@{}", root_support::exec_path("out-0.params"))
        ]
    );
    assert_eq!(expanded.param_files()[0].bytes(), b"'a'\\''b'\n");
}

#[derive(Debug)]
struct TestOriginals;
impl PublishedOriginalBinding for TestOriginals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        2
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
